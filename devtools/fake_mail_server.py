#!/usr/bin/env python3
"""A throwaway IMAP + SMTP server for developing Unkai Mail.

Run this and you can point the app at a working mail account without
owning one:

    python3 devtools/fake_mail_server.py
    # IMAP over TLS  → localhost:9993
    # SMTP STARTTLS  → localhost:9465
    # user / pass    → anything (every login is accepted)

It is *not* a mail server. It implements exactly the slice of
IMAP4rev1 that `unkai-imap` speaks — LOGIN, LIST, SELECT/EXAMINE,
STATUS, UID FETCH/STORE/SEARCH/COPY, APPEND, EXPUNGE, CREATE, DELETE,
RENAME, LOGOUT — over a self-signed TLS certificate it generates on
first run into `devtools/.certs/` (gitignored).

Two things make it useful rather than just a stub:

  * The mailbox is **mutable**. Flags, moves, deletes and appends the
    app performs are visible on the next fetch, so triage gestures can
    actually be verified end to end.
  * SMTP **delivers back into the IMAP store**: a message the app
    sends to the test user lands in INBOX, and every send is also
    copied to Sent. Composing and then pulling to refresh shows the
    mail you just wrote.

Because the certificate is self-signed, the app will refuse the
connection until you trust it — which is exactly the flow the setup
screen's "certificate not trusted" step exists for. That is the
intended path; it exercises the same code a self-hosted server hits.
"""

from __future__ import annotations

import asyncio
import email.utils
import os
from email.header import Header
import re
import ssl
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

CERT_DIR = Path(__file__).parent / ".certs"
CERT_FILE = CERT_DIR / "fake-mail.crt"
KEY_FILE = CERT_DIR / "fake-mail.key"

IMAP_PORT = 9993
SMTP_PORT = 9465

# `UNKAI_FIXTURE_TRACE=1` logs every IMAP and SMTP command, which is
# how you find out where a client conversation stops — and, just as
# usefully, how many round trips a screen actually costs.
TRACE = bool(os.environ.get("UNKAI_FIXTURE_TRACE"))

TEST_USER = "you@example.com"


# ── Certificate ──────────────────────────────────────────────────


def ensure_cert() -> None:
    """Generate a self-signed cert for localhost if we don't have one."""
    if CERT_FILE.exists() and KEY_FILE.exists():
        return
    CERT_DIR.mkdir(parents=True, exist_ok=True)
    print("generating a self-signed certificate for localhost…")
    subprocess.run(
        [
            "openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes",
            "-keyout", str(KEY_FILE), "-out", str(CERT_FILE),
            "-days", "365", "-subj", "/CN=localhost/O=Unkai dev fixture",
            "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1",
        ],
        check=True,
        capture_output=True,
    )


def tls_context() -> ssl.SSLContext:
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(CERT_FILE, KEY_FILE)
    return ctx


# ── The mailbox ──────────────────────────────────────────────────


@dataclass
class Message:
    uid: int
    raw: bytes
    flags: set[str] = field(default_factory=set)
    internal_date: float = field(default_factory=time.time)


@dataclass
class Folder:
    name: str
    attributes: list[str] = field(default_factory=list)
    messages: list[Message] = field(default_factory=list)
    uid_next: int = 1
    uid_validity: int = 1_000_001

    def add(self, raw: bytes, flags: set[str] | None = None, when: float | None = None) -> Message:
        msg = Message(
            uid=self.uid_next,
            raw=raw,
            flags=set(flags or set()),
            internal_date=when if when is not None else time.time(),
        )
        self.uid_next += 1
        self.messages.append(msg)
        return msg


class Store:
    """One user's mailbox tree, shared by the IMAP and SMTP servers."""

    def __init__(self) -> None:
        self.folders: dict[str, Folder] = {}
        for name, attrs in [
            ("INBOX", []),
            ("Drafts", ["\\Drafts"]),
            ("Sent", ["\\Sent"]),
            ("Archive", ["\\Archive"]),
            ("Junk", ["\\Junk"]),
            ("Trash", ["\\Trash"]),
            ("Projects", []),
            ("Projects/Rewrite", []),
        ]:
            self.folders[name] = Folder(name=name, attributes=attrs)
        seed(self.folders["INBOX"], self.folders["Sent"], self.folders["Drafts"])

    def get(self, name: str) -> Folder | None:
        # IMAP folder names are case-insensitive for INBOX only.
        if name.upper() == "INBOX":
            return self.folders.get("INBOX")
        return self.folders.get(name)


def build_message(
    *,
    frm: str,
    to: str,
    subject: str,
    text: str,
    html: str | None = None,
    attachment: tuple[str, str, bytes] | None = None,
    inline_image: bool = False,
    minutes_ago: int = 0,
    message_id: str | None = None,
) -> bytes:
    """Assemble a realistic RFC 5322 message.

    Written by hand rather than with `email.message` so the MIME
    structure is explicit — the reader's inline-image and attachment
    handling is exactly what we want to exercise.
    """
    def header_value(value: str) -> str:
        """RFC 2047-encode anything non-ASCII.

        Real senders do this, and it matters here: IMAP quoted
        strings are 7-bit, so an unencoded em-dash in a Subject makes
        the ENVELOPE unparseable for a strict client (which is
        exactly what `unkai-imap` is).
        """
        return value if value.isascii() else Header(value, "utf-8").encode()

    when = email.utils.formatdate(time.time() - minutes_ago * 60, localtime=True)
    mid = message_id or f"<{int(time.time() * 1000)}.{abs(hash(subject)) % 99999}@example.com>"
    headers = [
        f"From: {header_value(frm)}",
        f"To: {header_value(to)}",
        f"Subject: {header_value(subject)}",
        f"Date: {when}",
        f"Message-ID: {mid}",
        "MIME-Version: 1.0",
    ]

    # A 1×1 PNG, standing in for a logo in the inline-image message.
    png = bytes.fromhex(
        "89504e470d0a1a0a0000000d4948445200000001000000010806000000"
        "1f15c4890000000a49444154789c6360000002000100ffff0300000600"
        "0557bfabd40000000049454e44ae426082"
    )

    if attachment:
        name, mime, data = attachment
        import base64

        b64 = base64.b64encode(data).decode()
        wrapped = "\r\n".join(b64[i : i + 76] for i in range(0, len(b64), 76))
        boundary = "unkai-mixed-boundary"
        headers.append(f'Content-Type: multipart/mixed; boundary="{boundary}"')
        body = (
            f"--{boundary}\r\n"
            f"Content-Type: text/html; charset=utf-8\r\n\r\n"
            f"{html or f'<p>{text}</p>'}\r\n"
            f"--{boundary}\r\n"
            f'Content-Type: {mime}; name="{name}"\r\n'
            f"Content-Transfer-Encoding: base64\r\n"
            f'Content-Disposition: attachment; filename="{name}"\r\n\r\n'
            f"{wrapped}\r\n"
            f"--{boundary}--\r\n"
        )
    elif inline_image:
        import base64

        b64 = base64.b64encode(png).decode()
        boundary = "unkai-related-boundary"
        headers.append(f'Content-Type: multipart/related; boundary="{boundary}"')
        body = (
            f"--{boundary}\r\n"
            f"Content-Type: text/html; charset=utf-8\r\n\r\n"
            f'{html or "<p>See the logo below.</p>"}'
            f'<p><img src="cid:logo@example" alt="logo" width="64"></p>\r\n'
            f"--{boundary}\r\n"
            f"Content-Type: image/png\r\n"
            f"Content-Transfer-Encoding: base64\r\n"
            f"Content-ID: <logo@example>\r\n"
            f'Content-Disposition: inline; filename="logo.png"\r\n\r\n'
            f"{b64}\r\n"
            f"--{boundary}--\r\n"
        )
    elif html:
        boundary = "unkai-alt-boundary"
        headers.append(f'Content-Type: multipart/alternative; boundary="{boundary}"')
        body = (
            f"--{boundary}\r\n"
            f"Content-Type: text/plain; charset=utf-8\r\n\r\n{text}\r\n"
            f"--{boundary}\r\n"
            f"Content-Type: text/html; charset=utf-8\r\n\r\n{html}\r\n"
            f"--{boundary}--\r\n"
        )
    else:
        headers.append("Content-Type: text/plain; charset=utf-8")
        body = text + "\r\n"

    return ("\r\n".join(headers) + "\r\n\r\n" + body).encode()


def seed(inbox: Folder, sent: Folder, drafts: Folder) -> None:
    """Fill the mailbox with messages worth looking at."""
    inbox.add(
        build_message(
            frm="Jamie Fischer <jamie@partner.example>",
            to=TEST_USER,
            subject="Re: Quarterly review — numbers attached",
            text="Numbers are in. Revenue up 12%, churn finally moved the right way.",
            html=(
                "<p>Hi,</p><p>Numbers are in — revenue is up <b>12%</b> on the quarter "
                "and churn finally moved the right way. The spreadsheet has the full "
                "breakdown.</p><p>Can we go through it on Thursday?</p><p>— Jamie</p>"
                "<blockquote><p>On Monday you wrote:</p><p>Do you have the quarterly "
                "numbers yet? The board wants them by Friday.</p></blockquote>"
            ),
            attachment=("Q3-summary.csv", "text/csv", b"quarter,revenue,churn\nQ3,1240000,2.1\n"),
            minutes_ago=14,
        ),
        flags=set(),
    )
    inbox.add(
        build_message(
            frm="Nextcloud <noreply@cloud.example>",
            to=TEST_USER,
            subject="Your share link expires tomorrow",
            text="The link to Q3-summary.pdf expires in 24 hours.",
            html='<p>The link to <b>Q3-summary.pdf</b> expires in 24 hours. '
            '<a href="https://cloud.example.com/s/abc">Extend it</a>.</p>',
            inline_image=True,
            minutes_ago=95,
        ),
        flags=set(),
    )
    inbox.add(
        build_message(
            frm="Robin Vale <robin@example.org>",
            to=TEST_USER,
            subject="Lunch on Thursday?",
            text="Are you free around 12:30? There's a new place near the office.",
            minutes_ago=210,
        ),
        flags={"\\Seen"},
    )
    inbox.add(
        build_message(
            frm="CI <ci@example.com>",
            to=TEST_USER,
            subject="[unkai-mail] Build passed on main",
            text="All checks green. 3 jobs, 4m12s.",
            minutes_ago=380,
        ),
        flags={"\\Seen"},
    )
    inbox.add(
        build_message(
            frm="Newsletter <news@example.com>",
            to=TEST_USER,
            subject="Six things we shipped in August",
            text="Plain-text fallback for the August roundup.",
            html=(
                '<div style="width:640px;background:#ffffff;color:#222">'
                '<h1 style="color:#1a1a1a">August roundup</h1>'
                '<p style="color:#555">Six things we shipped, one we un-shipped, '
                "and what's next.</p>"
                '<p><img src="https://tracker.example.com/pixel.gif" width="1" height="1"></p>'
                '<p><a href="https://example.com/blog/august">Read the post</a></p></div>'
            ),
            minutes_ago=1500,
        ),
        flags={"\\Seen"},
    )
    sent.add(
        build_message(
            frm=TEST_USER,
            to="Robin Vale <robin@example.org>",
            subject="Re: Lunch on Thursday?",
            text="Thursday works. 12:30 at the new place?",
            minutes_ago=180,
        ),
        flags={"\\Seen"},
    )
    drafts.add(
        build_message(
            frm=TEST_USER,
            to="jamie@partner.example",
            subject="Board deck outline",
            text="Rough outline:\n1. numbers\n2. the rewrite\n3. next quarter",
            minutes_ago=60,
        ),
        flags={"\\Draft", "\\Seen"},
    )


# ── IMAP ─────────────────────────────────────────────────────────


class ImapSession:
    """One client connection. Stateless between commands except for
    the selected folder, which is what IMAP itself is."""

    def __init__(self, store: Store, reader: asyncio.StreamReader, writer: asyncio.StreamWriter):
        self.store = store
        self.reader = reader
        self.writer = writer
        self.selected: Folder | None = None
        self.read_only = False

    def send(self, line: str) -> None:
        self.writer.write(line.encode() + b"\r\n")

    async def run(self) -> None:
        self.send("* OK [CAPABILITY IMAP4rev1 UIDPLUS LITERAL+] Unkai dev fixture ready")
        await self.writer.drain()
        while True:
            raw = await self.reader.readline()
            if not raw:
                return
            line = raw.decode(errors="replace").rstrip("\r\n")
            if not line:
                continue
            if TRACE:
                # Truncated: an APPEND literal is the whole message.
                print(f"  imap ← {line[:120]}", flush=True)
            tag, _, rest = line.partition(" ")
            verb, _, args = rest.partition(" ")
            verb = verb.upper()
            try:
                done = await self.dispatch(tag, verb, args)
            except Exception as exc:  # a fixture should never take the app down
                self.send(f"{tag} NO {verb} failed: {exc}")
                done = False
            await self.writer.drain()
            if done:
                return

    async def dispatch(self, tag: str, verb: str, args: str) -> bool:
        if verb == "CAPABILITY":
            self.send("* CAPABILITY IMAP4rev1 UIDPLUS LITERAL+")
            self.send(f"{tag} OK CAPABILITY completed")
        elif verb == "LOGIN":
            self.send(f"{tag} OK LOGIN completed")
        elif verb == "LOGOUT":
            self.send("* BYE logging out")
            self.send(f"{tag} OK LOGOUT completed")
            return True
        elif verb == "NOOP":
            self.send(f"{tag} OK NOOP completed")
        elif verb == "LIST":
            for folder in self.store.folders.values():
                attrs = " ".join(folder.attributes + ["\\HasNoChildren"])
                self.send(f'* LIST ({attrs}) "/" "{folder.name}"')
            self.send(f"{tag} OK LIST completed")
        elif verb in ("SELECT", "EXAMINE"):
            name = unquote(args)
            folder = self.store.get(name)
            if folder is None:
                self.send(f"{tag} NO no such mailbox")
                return False
            self.selected = folder
            self.read_only = verb == "EXAMINE"
            unseen = sum(1 for m in folder.messages if "\\Seen" not in m.flags)
            self.send(f"* {len(folder.messages)} EXISTS")
            self.send("* 0 RECENT")
            self.send(r"* FLAGS (\Seen \Answered \Flagged \Deleted \Draft)")
            self.send(r"* OK [PERMANENTFLAGS (\Seen \Answered \Flagged \Deleted \Draft \*)] ok")
            self.send(f"* OK [UIDVALIDITY {folder.uid_validity}] uid validity")
            self.send(f"* OK [UIDNEXT {folder.uid_next}] predicted next uid")
            self.send(f"* OK [UNSEEN {unseen}] unseen")
            mode = "READ-ONLY" if self.read_only else "READ-WRITE"
            self.send(f"{tag} OK [{mode}] {verb} completed")
        elif verb == "STATUS":
            name, _, items = args.partition(" ")
            folder = self.store.get(unquote(name))
            if folder is None:
                self.send(f"{tag} NO no such mailbox")
                return False
            values = []
            wanted = items.strip("()").split()
            for item in wanted:
                key = item.upper()
                if key == "MESSAGES":
                    values += ["MESSAGES", str(len(folder.messages))]
                elif key == "UNSEEN":
                    unseen = sum(1 for m in folder.messages if "\\Seen" not in m.flags)
                    values += ["UNSEEN", str(unseen)]
                elif key == "UIDNEXT":
                    values += ["UIDNEXT", str(folder.uid_next)]
                elif key == "UIDVALIDITY":
                    values += ["UIDVALIDITY", str(folder.uid_validity)]
                elif key == "RECENT":
                    values += ["RECENT", "0"]
            self.send(f'* STATUS "{folder.name}" ({" ".join(values)})')
            self.send(f"{tag} OK STATUS completed")
        elif verb == "UID":
            sub, _, rest = args.partition(" ")
            await self.uid_command(tag, sub.upper(), rest)
        elif verb == "FETCH":
            # Sequence-number FETCH — how the client lists a mailbox
            # ("the newest N messages" is only expressible in sequence
            # numbers, since UIDs aren't contiguous).
            seq_set, _, items = args.partition(" ")
            await self.fetch(tag, seq_set, items, by_uid=False)
        elif verb == "APPEND":
            await self.append(tag, args)
        elif verb == "EXPUNGE":
            if self.selected:
                keep = [m for m in self.selected.messages if "\\Deleted" not in m.flags]
                for index, msg in enumerate(list(self.selected.messages), start=1):
                    if "\\Deleted" in msg.flags:
                        self.send(f"* {index} EXPUNGE")
                self.selected.messages = keep
            self.send(f"{tag} OK EXPUNGE completed")
        elif verb == "CREATE":
            name = unquote(args)
            self.store.folders.setdefault(name, Folder(name=name))
            self.send(f"{tag} OK CREATE completed")
        elif verb == "DELETE":
            self.store.folders.pop(unquote(args), None)
            self.send(f"{tag} OK DELETE completed")
        elif verb == "RENAME":
            old, _, new = args.partition(" ")
            folder = self.store.folders.pop(unquote(old), None)
            if folder:
                folder.name = unquote(new)
                self.store.folders[folder.name] = folder
            self.send(f"{tag} OK RENAME completed")
        elif verb == "CLOSE":
            self.selected = None
            self.send(f"{tag} OK CLOSE completed")
        else:
            self.send(f"{tag} BAD unsupported command {verb}")
        return False

    async def uid_command(self, tag: str, sub: str, args: str) -> None:
        folder = self.selected
        if folder is None:
            self.send(f"{tag} NO no mailbox selected")
            return

        if sub == "FETCH":
            uid_set, _, items = args.partition(" ")
            await self.fetch(tag, uid_set, items, by_uid=True)

        elif sub == "STORE":
            uid_set, _, rest = args.partition(" ")
            op, _, flag_part = rest.partition(" ")
            flags = {f for f in flag_part.strip("()").split() if f}
            for msg in folder.messages:
                if not uid_in_set(msg.uid, uid_set, folder):
                    continue
                if op.startswith("+"):
                    msg.flags |= flags
                elif op.startswith("-"):
                    msg.flags -= flags
                else:
                    msg.flags = set(flags)
            self.send(f"{tag} OK UID STORE completed")

        elif sub == "SEARCH":
            criteria = args.strip()
            hits = [m.uid for m in folder.messages if matches(m, criteria, folder)]
            self.send("* SEARCH " + " ".join(str(u) for u in hits))
            self.send(f"{tag} OK UID SEARCH completed")

        elif sub == "COPY":
            uid_set, _, dest_name = args.partition(" ")
            dest = self.store.get(unquote(dest_name))
            if dest is None:
                self.send(f"{tag} NO [TRYCREATE] no such mailbox")
                return
            for msg in folder.messages:
                if uid_in_set(msg.uid, uid_set, folder):
                    dest.add(msg.raw, set(msg.flags), msg.internal_date)
            self.send(f"{tag} OK UID COPY completed")

        else:
            self.send(f"{tag} BAD unsupported UID command {sub}")

    async def fetch(self, tag: str, id_set: str, items: str, *, by_uid: bool) -> None:
        """Answer a FETCH for whichever items were asked for.

        Only the item names `unkai-imap` actually sends are handled:
        UID, FLAGS, INTERNALDATE, RFC822.SIZE, ENVELOPE, BODY[] and
        BODY[HEADER.FIELDS (…)]. Anything else is ignored rather than
        guessed at — a wrong answer is worse than a missing one.
        """
        folder = self.selected
        if folder is None:
            self.send(f"{tag} NO no mailbox selected")
            return

        wanted = items.upper()
        fields_match = re.search(r"HEADER\.FIELDS \(([^)]*)\)", items, re.I)
        field_names = fields_match.group(1).split() if fields_match else []

        for index, msg in enumerate(folder.messages, start=1):
            key = msg.uid if by_uid else index
            if not uid_in_set(key, id_set, folder):
                continue

            parts = [f"UID {msg.uid}"]
            if "FLAGS" in wanted:
                parts.append(f'FLAGS ({" ".join(sorted(msg.flags))})')
            if "INTERNALDATE" in wanted:
                parts.append(
                    'INTERNALDATE "%s"'
                    % time.strftime("%d-%b-%Y %H:%M:%S +0000", time.gmtime(msg.internal_date))
                )
            if "RFC822.SIZE" in wanted:
                parts.append(f"RFC822.SIZE {len(msg.raw)}")
            if "ENVELOPE" in wanted:
                parts.append(envelope_for(msg))

            # At most one body section per fetch in what the client
            # sends, so a single literal at the end is enough.
            payload: bytes | None = None
            section = ""
            if field_names:
                payload = header_fields(msg, field_names)
                section = f'BODY[HEADER.FIELDS ({" ".join(field_names)})]'
            elif "BODY.PEEK[]" in wanted or "BODY[]" in wanted or "RFC822" in wanted:
                payload = msg.raw
                section = "BODY[]"

            if payload is None:
                self.send(f'* {index} FETCH ({" ".join(parts)})')
                continue

            head = f'* {index} FETCH ({" ".join(parts)} {section} {{{len(payload)}}}'
            self.writer.write(head.encode() + b"\r\n")
            self.writer.write(payload)
            self.writer.write(b")\r\n")

        self.send(f"{tag} OK FETCH completed")

    async def append(self, tag: str, args: str) -> None:
        # APPEND <mbox> [(flags)] [date] {size}
        match = re.match(r'^(?:"([^"]+)"|(\S+))\s*(.*)\{(\d+)\+?\}$', args)
        if not match:
            self.send(f"{tag} BAD malformed APPEND")
            return
        name = match.group(1) or match.group(2)
        middle = match.group(3)
        size = int(match.group(4))
        flags = set(re.findall(r"\\\w+", middle))

        folder = self.store.get(name) or self.store.folders.setdefault(name, Folder(name=name))
        self.send("+ Ready for literal data")
        await self.writer.drain()
        raw = await self.reader.readexactly(size)
        await self.reader.readline()  # trailing CRLF after the literal
        msg = folder.add(raw, flags)
        self.send(f"{tag} OK [APPENDUID {folder.uid_validity} {msg.uid}] APPEND completed")


def imap_string(value) -> str:
    """An IMAP quoted string, or NIL. Backslashes and quotes are
    escaped; anything with a CR/LF is flattened, because a literal
    here would need a length prefix the caller can't predict.

    Takes `Any` rather than `str` on purpose: `email`'s getters hand
    back a `Header` object for encoded headers, not a string."""
    if value is None:
        return "NIL"
    value = str(value)
    flat = value.replace("\r", " ").replace("\n", " ")
    if not flat.isascii():
        # IMAP quoted strings are 7-bit. Anything else has to be
        # encoded, or a strict client rejects the whole response.
        flat = Header(flat, "utf-8").encode()
    escaped = flat.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def envelope_addresses(header_value) -> str:
    """RFC 3501 address list: ((name adl mailbox host) …) or NIL."""
    if not header_value:
        return "NIL"
    header_value = str(header_value)
    parts = []
    for name, addr in email.utils.getaddresses([header_value]):
        if not addr:
            continue
        mailbox, _, host = addr.partition("@")
        parts.append(
            f"({imap_string(name or None)} NIL {imap_string(mailbox)} {imap_string(host or None)})"
        )
    return f'({"".join(parts)})' if parts else "NIL"


def envelope_for(msg: Message) -> str:
    """The ENVELOPE structure the client parses instead of re-reading
    the whole message just to list it."""
    import email as email_mod

    # Decode as UTF-8 rather than `message_from_bytes`, which reads
    # headers as ASCII-with-surrogates and turns our em-dashes into
    # replacement characters.
    parsed = email_mod.message_from_string(msg.raw.decode("utf-8", "replace"))
    get = lambda name: parsed.get(name)  # noqa: E731
    frm = envelope_addresses(get("From"))
    return (
        "ENVELOPE ("
        + " ".join(
            [
                imap_string(get("Date")),
                imap_string(get("Subject")),
                frm,                                        # from
                frm,                                        # sender
                envelope_addresses(get("Reply-To") or get("From")),
                envelope_addresses(get("To")),
                envelope_addresses(get("Cc")),
                envelope_addresses(get("Bcc")),
                imap_string(get("In-Reply-To")),
                imap_string(get("Message-ID")),
            ]
        )
        + ")"
    )


def header_fields(msg: Message, names: list[str]) -> bytes:
    """The `BODY[HEADER.FIELDS (…)]` payload: the requested headers,
    in the message's own order, terminated by a blank line."""
    head = msg.raw.split(b"\r\n\r\n", 1)[0]
    wanted = {n.lower() for n in names}
    out: list[bytes] = []
    current_wanted = False
    for line in head.split(b"\r\n"):
        if line[:1] in (b" ", b"\t"):          # folded continuation
            if current_wanted:
                out.append(line)
            continue
        name = line.split(b":", 1)[0].decode(errors="replace").lower()
        current_wanted = name in wanted
        if current_wanted:
            out.append(line)
    return b"\r\n".join(out) + b"\r\n\r\n"


def unquote(value: str) -> str:
    value = value.strip()
    if value.startswith('"') and value.endswith('"'):
        return value[1:-1]
    return value


def uid_in_set(uid: int, uid_set: str, folder: Folder) -> bool:
    for part in uid_set.split(","):
        part = part.strip()
        if ":" in part:
            low, _, high = part.partition(":")
            lo = 1 if low == "*" else int(low)
            hi = folder.uid_next if high == "*" else int(high)
            if min(lo, hi) <= uid <= max(lo, hi):
                return True
        elif part == "*":
            return True
        elif part.isdigit() and int(part) == uid:
            return True
    return False


def matches(msg: Message, criteria: str, folder: "Folder") -> bool:
    """Enough of IMAP SEARCH for what the app asks: ALL, UID ranges,
    HEADER <name> <value>, and the flag keywords."""
    crit = criteria.strip()
    upper = crit.upper()
    if upper in ("ALL", ""):
        return True
    if upper.startswith("UID "):
        # `UID <sequence-set>` as a *search key* (RFC 3501 §6.4.4) is a
        # filter in its own right — unlike the UID set that prefixes a
        # FETCH or STORE, there is no separate argument that already
        # narrowed things down.  This used to `return True`, which made
        # every bounded search answer like `ALL`; the sync path's
        # reconcile now sends one on every poll, so getting it wrong
        # here would hide a real client bug behind a permissive fixture.
        return uid_in_set(msg.uid, crit.split(" ", 1)[1], folder)
    if upper.startswith("HEADER "):
        _, _, rest = crit.partition(" ")
        name, _, value = rest.partition(" ")
        value = unquote(value).lower()
        for line in msg.raw.split(b"\r\n\r\n", 1)[0].decode(errors="replace").split("\r\n"):
            if line.lower().startswith(name.lower() + ":") and value in line.lower():
                return True
        return False
    if upper == "UNSEEN":
        return "\\Seen" not in msg.flags
    if upper == "SEEN":
        return "\\Seen" in msg.flags
    # Text searches: match anywhere in the message.
    needle = unquote(crit.split(" ", 1)[-1]).lower()
    return needle.encode() in msg.raw.lower()


# ── SMTP ─────────────────────────────────────────────────────────


async def upgrade_to_tls(reader: asyncio.StreamReader, writer: asyncio.StreamWriter, ctx):
    """STARTTLS: swap the plaintext transport for a TLS one.

    `StreamWriter.start_tls()` only exists on Python 3.11+, and the
    system Python here is 3.9, so this goes through the loop's
    `start_tls` and re-points the existing reader/writer at the new
    transport — the documented workaround for that gap.
    """
    loop = asyncio.get_running_loop()
    transport = writer.transport
    protocol = transport.get_protocol()
    new_transport = await loop.start_tls(transport, protocol, ctx, server_side=True)
    reader._transport = new_transport  # noqa: SLF001
    writer._transport = new_transport  # noqa: SLF001
    protocol._stream_writer = writer  # noqa: SLF001
    protocol._transport = new_transport  # noqa: SLF001


async def handle_smtp(
    store: Store, reader: asyncio.StreamReader, writer: asyncio.StreamWriter, ctx
):
    """Accept a message and deliver it into the IMAP store.

    Submission runs on a **STARTTLS** port rather than an implicit-TLS
    one, because that's what the client expects anywhere other than
    port 465 — `unkai-smtp` picks its TLS mode from the port number,
    and binding 465 would need root. The connection is still
    encrypted before any credential crosses it: `AUTH` is refused
    until STARTTLS has run.
    """

    def send(line: str) -> None:
        writer.write(line.encode() + b"\r\n")

    send("220 localhost Unkai dev fixture ESMTP")
    await writer.drain()

    secure = False
    recipients: list[str] = []
    while True:
        raw = await reader.readline()
        if not raw:
            return
        line = raw.decode(errors="replace").rstrip("\r\n")
        verb = line.split(" ")[0].upper()
        if TRACE:
            print(f"  smtp ← {line[:80]}", flush=True)

        if verb in ("EHLO", "HELO"):
            send("250-localhost greets you")
            if secure:
                send("250-AUTH PLAIN LOGIN")
            else:
                send("250-STARTTLS")
            send("250-8BITMIME")
            send("250 SMTPUTF8")
        elif verb == "STARTTLS":
            send("220 2.0.0 Ready to start TLS")
            await writer.drain()
            await upgrade_to_tls(reader, writer, ctx)
            secure = True
            if TRACE:
                print("  smtp ↑ TLS established", flush=True)
        elif verb == "AUTH":
            if not secure:
                send("530 5.7.0 Must issue a STARTTLS command first")
                await writer.drain()
                continue
            if "LOGIN" in line.upper():
                send("334 VXNlcm5hbWU6")
                await writer.drain()
                await reader.readline()
                send("334 UGFzc3dvcmQ6")
                await writer.drain()
                await reader.readline()
            send("235 2.7.0 Authentication successful")
        elif verb == "MAIL":
            recipients = []
            send("250 2.1.0 Sender ok")
        elif verb == "RCPT":
            match = re.search(r"<([^>]*)>", line)
            if match:
                recipients.append(match.group(1))
            send("250 2.1.5 Recipient ok")
        elif verb == "DATA":
            send("354 End data with <CRLF>.<CRLF>")
            await writer.drain()
            chunks: list[bytes] = []
            while True:
                data_line = await reader.readline()
                if not data_line or data_line in (b".\r\n", b".\n"):
                    break
                if data_line.startswith(b".."):
                    data_line = data_line[1:]
                chunks.append(data_line)
            raw_message = b"".join(chunks)

            # Deliver: to INBOX when addressed to the test user, and
            # always a copy into Sent so the app's Sent folder fills up
            # even though it also APPENDs there itself.
            for rcpt in recipients:
                if rcpt.lower() == TEST_USER.lower():
                    store.folders["INBOX"].add(raw_message)
                    print(f"  ↳ delivered to INBOX ({len(raw_message)} bytes)")
            subject = "(no subject)"
            for line_ in raw_message.split(b"\r\n"):
                if line_.lower().startswith(b"subject:"):
                    subject = line_.decode(errors="replace")[8:].strip()
                    break
            print(f"SMTP accepted: {subject!r} → {recipients}")
            send("250 2.0.0 Ok: queued as fixture")
        elif verb == "RSET":
            recipients = []
            send("250 2.0.0 Ok")
        elif verb == "QUIT":
            send("221 2.0.0 Bye")
            await writer.drain()
            writer.close()
            return
        elif verb == "NOOP":
            send("250 2.0.0 Ok")
        else:
            send("250 2.0.0 Ok")
        await writer.drain()


# ── Entry point ──────────────────────────────────────────────────


async def main() -> None:
    ensure_cert()
    store = Store()
    ctx = tls_context()

    async def imap_client(reader, writer):
        peer = writer.get_extra_info("peername")
        print(f"IMAP connection from {peer}")
        try:
            await ImapSession(store, reader, writer).run()
        except (ConnectionResetError, asyncio.IncompleteReadError):
            pass
        finally:
            writer.close()

    async def smtp_client(reader, writer):
        print("SMTP connection")
        try:
            await handle_smtp(store, reader, writer, ctx)
        except (ConnectionResetError, asyncio.IncompleteReadError):
            pass
        finally:
            writer.close()

    # Both loopback families on purpose: `localhost` resolves to ::1
    # first on macOS, and a v4-only listener means a client that tries
    # the first address gets connection-refused with nothing in this
    # log to explain it.
    loopback = ["127.0.0.1", "::1"]
    imap = await asyncio.start_server(imap_client, loopback, IMAP_PORT, ssl=ctx)
    # No `ssl=` here: submission starts in the clear and upgrades
    # through STARTTLS (see `handle_smtp`).
    smtp = await asyncio.start_server(smtp_client, loopback, SMTP_PORT)

    fingerprint = subprocess.run(
        ["openssl", "x509", "-in", str(CERT_FILE), "-noout", "-fingerprint", "-sha256"],
        capture_output=True,
        text=True,
    ).stdout.strip()

    print(f"IMAP  TLS  → localhost:{IMAP_PORT}")
    print(f"SMTP STARTTLS → localhost:{SMTP_PORT}")
    print(f"user       → {TEST_USER} (any password)")
    print(fingerprint)
    print("Ctrl-C to stop.\n")

    async with imap, smtp:
        await asyncio.gather(imap.serve_forever(), smtp.serve_forever())


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        sys.exit(0)
