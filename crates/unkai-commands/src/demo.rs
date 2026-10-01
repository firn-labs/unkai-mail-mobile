//! The demo account: a working app with no server behind it (test mode).
//!
//! One tap seeds the local cache with a mail account, folders, messages,
//! contacts, a calendar and a task list, so every screen in the app has
//! real-shaped data to render. It is what makes the app usable as a
//! design and QA surface: you can try the mail list, the reader, swipe
//! actions, search, the calendar grid and the contact sheet without an
//! account, without a network, and without the fixture server running on
//! someone's Mac.
//!
//! # What it deliberately is not
//!
//! Not a fake server. Nothing here speaks IMAP or SMTP; the data is
//! written straight into the cache the app already reads from. That is
//! the whole trick — every *read* path in the app is exercised exactly
//! as it would be against a real server, because reads only ever touch
//! the cache anyway (the app is offline-first by design).
//!
//! The consequence is that **write-to-server paths do not work**, and
//! they are not meant to: sending is refused with a clear message rather
//! than queued in the outbox to retry forever against a host that was
//! never real. `Account::demo` is the flag that lets those few paths say
//! so. Local edits — read/unread, flags, archive, delete, moving between
//! folders — all work, because they are cache writes.
//!
//! # Why the data looks like this
//!
//! The fixtures are chosen to cover the *states* the UI has to render,
//! not to look plausible in a screenshot: unread and read, flagged,
//! pinned, high priority, with and without attachments, HTML and plain
//! text, a two-message thread, a long newsletter that exercises the
//! rendering pipeline's width clamping, and a draft. A demo where every
//! message looks the same tests nothing.
//!
//! # Which language the data is in
//!
//! The sample data follows the UI: German for a German UI, English for
//! every other language ([`DemoLanguage::for_locale`]). It used to be
//! German only, so a user who had picked English — or any of the other
//! eight languages — tapped *Test mode* and met a German inbox, which
//! reads as a bug, not as sample data.
//!
//! Two languages, not ten: sample mails are prose, and prose in eight
//! more languages would be translation nobody on the team can check.
//! English is the most widely read fallback. Each fixture carries both
//! texts side by side ([`DemoLanguage::pick`]) instead of living in two
//! parallel lists, so the two versions cannot drift apart in the
//! *states* they cover — a test holds them to the same shape.

use chrono::{DateTime, Duration, Local, Timelike, Utc};
use unkai_core::UnkaiError;
use unkai_core::models::{
    Account, ContactEmail, ContactPhone, DavSourceKind, Email, EmailEnvelope, Folder,
    NextcloudAccount,
};
use unkai_store::Cache;
use unkai_store::cache::calendars::{CalendarEventRow, CalendarRow};
use unkai_store::cache::contacts::ContactRow;
use unkai_store::{account_store, nextcloud_store};

use crate::notify::UiNotifier;

/// Fixed ids so a second tap replaces the demo data instead of stacking
/// a second copy of it beside the first.
pub const DEMO_ACCOUNT_ID: &str = "demo-account";
pub const DEMO_NC_ID: &str = "demo-local-source";
const DEMO_EMAIL: &str = "you@example.com";
const DEMO_CALENDAR_PATH: &str = "local://demo/calendar/personal";
const DEMO_ADDRESSBOOK: &str = "local://demo/contacts/default";

/// What was created, so the UI can say so rather than guessing.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DemoSummary {
    pub account_id: String,
    pub messages: u32,
    pub contacts: u32,
    pub events: u32,
}

/// The language the sample data is written in. See the module docs for
/// why there are two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoLanguage {
    English,
    German,
}

impl DemoLanguage {
    /// German for a German UI (`de`, `de-AT`, `de-CH`, …), English for
    /// everything else — including no locale at all, since English is
    /// what the most people can read.
    pub fn for_locale(locale: Option<&str>) -> Self {
        match locale {
            Some(l) if l.trim().to_ascii_lowercase().starts_with("de") => Self::German,
            _ => Self::English,
        }
    }

    /// The text for this language, from an English/German pair.
    fn pick(self, english: &'static str, german: &'static str) -> &'static str {
        match self {
            Self::English => english,
            Self::German => german,
        }
    }
}

/// Seed everything. Idempotent: running it twice leaves one demo
/// account with freshly-regenerated data, not two — and re-seeding in
/// another language replaces the old language's data rather than
/// mixing the two.
pub async fn create_demo_account(
    cache: &Cache,
    ui: &dyn UiNotifier,
    lang: DemoLanguage,
) -> Result<DemoSummary, UnkaiError> {
    remove_demo_account(cache).await.ok();

    let account = demo_account(lang);
    account_store::add_account(cache, account)?;

    let folders = demo_folders(lang);
    cache.upsert_folders(DEMO_ACCOUNT_ID, &folders)?;

    let messages = seed_mail(cache, lang)?;

    // Groupware rides a `Local` DAV source — the kind that already
    // means "no remote at all, sync is a no-op, writes stay in the
    // cache". Exactly what a demo needs, and it means Calendar,
    // Contacts and Tasks need no demo-specific code: they loop over
    // Nextcloud accounts and this is simply one of them.
    nextcloud_store::upsert_account(cache, demo_source(lang))?;
    let contacts = seed_contacts(cache, lang)?;
    let events = seed_calendar(cache, lang)?;

    crate::mail::refresh_unread_badge(cache, ui);

    Ok(DemoSummary {
        account_id: DEMO_ACCOUNT_ID.to_string(),
        messages,
        contacts,
        events,
    })
}

/// Tear the demo down again, leaving real accounts alone.
///
/// This is what "Leave test mode" runs, so it has to take *everything*
/// the demo seeded. It used to delete only the two account rows, on the
/// assumption that the rest cascaded — it does not: messages, folders,
/// contacts, calendars and task lists are keyed by the account id but
/// not foreign-keyed to it. The orphans kept the unread badge at the
/// demo's count and the demo's people in the composer's suggestions
/// (see the tests below). The wipes are the same ones the real
/// account-removal commands use.
///
/// Idempotent: either half may already be gone (the demo mail account
/// can be removed on its own from Settings → Accounts), and the other
/// half must still be cleaned up. The row deletes are therefore guarded
/// by an existence check rather than by ignoring their errors, so a
/// real storage failure still surfaces.
pub async fn remove_demo_account(cache: &Cache) -> Result<(), UnkaiError> {
    remove_demo_data(cache)
}

/// The synchronous body of `remove_demo_account`, for callers that are
/// not async — `accounts::remove_account` routes the demo here, so
/// removing it from Settings → Accounts cleans up exactly as much as
/// "Leave test mode" does.
pub fn remove_demo_data(cache: &Cache) -> Result<(), UnkaiError> {
    cache.wipe_account(DEMO_ACCOUNT_ID)?;
    if demo_account_exists(cache) {
        account_store::remove_account(cache, DEMO_ACCOUNT_ID)?;
    }

    cache.wipe_nextcloud_contacts(DEMO_NC_ID)?;
    cache.wipe_nextcloud_calendars(DEMO_NC_ID)?;
    cache.wipe_task_lists_for_account(DEMO_NC_ID)?;
    cache.wipe_notes_for_account(DEMO_NC_ID)?;
    if nextcloud_store::load_accounts(cache)?
        .iter()
        .any(|a| a.id == DEMO_NC_ID)
    {
        nextcloud_store::remove_account(cache, DEMO_NC_ID)?;
    }
    Ok(())
}

/// True when this cache already holds the demo account.
pub fn demo_account_exists(cache: &Cache) -> bool {
    account_store::load_accounts(cache)
        .map(|list| list.iter().any(|a| a.id == DEMO_ACCOUNT_ID))
        .unwrap_or(false)
}

fn demo_account(lang: DemoLanguage) -> Account {
    Account {
        id: DEMO_ACCOUNT_ID.to_string(),
        display_name: lang.pick("Test account", "Testkonto").to_string(),
        email: DEMO_EMAIL.to_string(),
        // Hosts are recorded for display only — `demo` short-circuits
        // every path that would dial them. `.invalid` is reserved by
        // RFC 2606 precisely so it can never resolve, which makes an
        // accidental connection attempt fail instantly and loudly
        // instead of hanging on a stranger's server.
        imap_host: "demo.invalid".to_string(),
        imap_port: 993,
        smtp_host: "demo.invalid".to_string(),
        smtp_port: 465,
        use_jmap: false,
        jmap_url: None,
        signature: None,
        folder_icons: Vec::new(),
        folder_icon_overrides: Default::default(),
        hidden_folders: Vec::new(),
        trusted_certs: Vec::new(),
        emoji: None,
        sort_order: 0,
        person_name: Some("Alex Morgan".to_string()),
        pgp_key_fingerprint: None,
        smime_cert_fingerprint: None,
        demo: true,
    }
}

fn demo_source(lang: DemoLanguage) -> NextcloudAccount {
    NextcloudAccount {
        id: DEMO_NC_ID.to_string(),
        server_url: "local://demo".to_string(),
        username: DEMO_EMAIL.to_string(),
        display_name: Some(lang.pick("Sample data", "Testdaten").to_string()),
        capabilities: None,
        trusted_certs: Vec::new(),
        kind: DavSourceKind::Local,
        carddav_home: Some("local://demo/contacts/".to_string()),
        caldav_home: Some("local://demo/calendar/".to_string()),
    }
}

fn folder(name: &str, attrs: &[&str], unread: u32) -> Folder {
    Folder {
        name: name.to_string(),
        delimiter: Some("/".to_string()),
        attributes: attrs.iter().map(|a| a.to_string()).collect(),
        unread_count: Some(unread),
    }
}

/// The one user folder. Special-use folders keep their protocol names
/// (`Drafts`, `Sent`, …) in both languages — the app shows those by role,
/// in the UI's language — but a user folder is shown by its name.
fn projects_folder(lang: DemoLanguage) -> &'static str {
    lang.pick("Projects", "Projekte")
}

fn demo_folders(lang: DemoLanguage) -> Vec<Folder> {
    vec![
        folder("INBOX", &[], 4),
        folder("Drafts", &["Drafts"], 0),
        folder("Sent", &["Sent"], 0),
        folder("Archive", &["Archive"], 0),
        folder("Junk", &["Junk"], 0),
        folder("Trash", &["Trash"], 0),
        folder(projects_folder(lang), &[], 0),
    ]
}

/* ── Mail ─────────────────────────────────────────────────────────── */

/// One fixture message, in the shape the fixtures list wants to read.
struct Msg {
    uid: u32,
    folder: &'static str,
    from: &'static str,
    to: &'static str,
    subject: &'static str,
    /// Minutes before "now". Relative so the list always looks fresh
    /// rather than dated to whenever this file was written.
    minutes_ago: i64,
    read: bool,
    starred: bool,
    pinned: bool,
    priority: Option<&'static str>,
    thread: Option<&'static str>,
    attachments: &'static [(&'static str, &'static str)],
    text: &'static str,
    html: Option<&'static str>,
}

const fn msg(
    uid: u32,
    folder: &'static str,
    from: &'static str,
    subject: &'static str,
    minutes_ago: i64,
    text: &'static str,
) -> Msg {
    Msg {
        uid,
        folder,
        from,
        to: DEMO_EMAIL,
        subject,
        minutes_ago,
        read: true,
        starred: false,
        pinned: false,
        priority: None,
        thread: None,
        attachments: &[],
        text,
        html: None,
    }
}

fn fixtures(lang: DemoLanguage) -> Vec<Msg> {
    let l = |english, german| lang.pick(english, german);
    let quarterly = l(
        "Re: Q3 figures — numbers attached",
        "Re: Quartalszahlen — Zahlen im Anhang",
    );
    vec![
        Msg {
            read: false,
            starred: true,
            attachments: match lang {
                DemoLanguage::English => &[("Q3-figures.pdf", "application/pdf")],
                DemoLanguage::German => &[("Q3-Zahlen.pdf", "application/pdf")],
            },
            thread: Some("thread-quarterly"),
            html: Some(l(
                "<p>Hi Alex,</p><p>the Q3 figures are attached. Could you look \
                 them over by Friday — especially the margin in row 14?</p>\
                 <p>Best,<br>Jamie</p>",
                "<p>Hallo Alex,</p><p>die Zahlen für Q3 sind angehängt. \
                 Schau bitte bis Freitag drüber — besonders auf die \
                 Marge in Zeile 14.</p><p>Viele Grüße<br>Jamie</p>",
            )),
            ..msg(
                101,
                "INBOX",
                "Jamie Fischer <jamie@example.com>",
                quarterly,
                24,
                l(
                    "Hi Alex,\n\nthe Q3 figures are attached. Could you look them over \
                     by Friday — especially the margin in row 14?\n\nBest,\nJamie",
                    "Hallo Alex,\n\ndie Zahlen für Q3 sind angehängt. Schau bitte bis \
                     Freitag drüber — besonders auf die Marge in Zeile 14.\n\nViele Grüße\nJamie",
                ),
            )
        },
        Msg {
            read: false,
            priority: Some("high"),
            ..msg(
                100,
                "INBOX",
                l(
                    "Security <security@example.com>",
                    "Sicherheit <security@example.com>",
                ),
                l(
                    "New sign-in on an unrecognised device",
                    "Neue Anmeldung auf einem unbekannten Gerät",
                ),
                95,
                l(
                    "We noticed a sign-in from Hamburg. If this wasn't you, please \
                     change your password right away.",
                    "Wir haben eine Anmeldung aus Hamburg festgestellt. Warst du das \
                     nicht, ändere bitte sofort dein Passwort.",
                ),
            )
        },
        Msg {
            read: false,
            pinned: true,
            ..msg(
                99,
                "INBOX",
                "Robin Vale <robin@example.com>",
                l("Lunch on Thursday?", "Mittagessen am Donnerstag?"),
                240,
                l(
                    "Are you free around 12:30 on Thursday? The new place on the \
                     corner you wanted to try has opened.",
                    "Hast du Donnerstag gegen 12:30 Zeit? Es gibt das neue Lokal an \
                     der Ecke, das du dir anschauen wolltest.",
                ),
            )
        },
        Msg {
            read: false,
            thread: Some("thread-quarterly"),
            ..msg(
                98,
                "INBOX",
                "Kim Bauer <kim@example.com>",
                quarterly,
                310,
                l(
                    "I re-ran the margin and row 14 checks out. The difference comes \
                     from the reallocation in August.",
                    "Ich habe die Marge nachgerechnet, Zeile 14 stimmt. Die Abweichung \
                     kommt aus der Umbuchung im August.",
                ),
            )
        },
        Msg {
            html: Some(l(
                // Deliberately laid out for a 640px desktop pane: this is
                // what `makeResponsive` has to clamp, so the demo covers
                // the rendering pipeline and not just short plain text.
                "<table width=\"640\" cellpadding=\"0\" style=\"width:640px\"><tr><td \
                 style=\"font-family:Georgia,serif;color:#333\"><h1>What we built in \
                 August</h1><p>Six things we're excited about — from faster syncing \
                 to a new calendar.</p>\
                 <p><a href=\"https://example.com/changelog\">Read it all →</a></p>\
                 </td></tr></table>",
                "<table width=\"640\" cellpadding=\"0\" style=\"width:640px\"><tr><td \
                 style=\"font-family:Georgia,serif;color:#333\"><h1>Was wir im August \
                 gebaut haben</h1><p>Sechs Dinge, über die wir uns freuen — von \
                 schnelleren Synchronisierungen bis zu einem neuen Kalender.</p>\
                 <p><a href=\"https://example.com/changelog\">Alles lesen →</a></p>\
                 </td></tr></table>",
            )),
            ..msg(
                97,
                "INBOX",
                l(
                    "Product newsletter <news@example.com>",
                    "Produkt-Newsletter <news@example.com>",
                ),
                l(
                    "Six things we shipped in August",
                    "Sechs Dinge, die wir im August ausgeliefert haben",
                ),
                1500,
                l(
                    "Six things we're excited about — from faster syncing to a new \
                     calendar.",
                    "Sechs Dinge, über die wir uns freuen — von schnelleren \
                     Synchronisierungen bis zu einem neuen Kalender.",
                ),
            )
        },
        Msg {
            attachments: match lang {
                DemoLanguage::English => &[
                    ("floor-plan.png", "image/png"),
                    ("quote.pdf", "application/pdf"),
                ],
                DemoLanguage::German => &[
                    ("Grundriss.png", "image/png"),
                    ("Angebot.pdf", "application/pdf"),
                ],
            },
            ..msg(
                96,
                "INBOX",
                l(
                    "Nord Carpentry <info@example.com>",
                    "Tischlerei Nord <info@example.com>",
                ),
                l(
                    "Your quote for the built-in wardrobes",
                    "Ihr Angebot für die Einbauschränke",
                ),
                2900,
                l(
                    "Hello,\n\nplease find attached the quote we discussed and the \
                     floor plan with the measurements marked in.",
                    "Guten Tag,\n\nanbei finden Sie das besprochene Angebot sowie den \
                     Grundriss mit den eingezeichneten Maßen.",
                ),
            )
        },
        Msg {
            starred: true,
            ..msg(
                95,
                "INBOX",
                "CI <ci@example.com>",
                l(
                    "[unkai-mail] Build passed on main",
                    "[unkai-mail] Build auf main erfolgreich",
                ),
                4300,
                l(
                    "All 342 tests passed. Took 4m 12s.",
                    "Alle 342 Tests grün. Dauer: 4m 12s.",
                ),
            )
        },
        // ── Other folders, so those screens are not empty ──────────
        Msg {
            from: "Alex Morgan <you@example.com>",
            to: "Jamie Fischer <jamie@example.com>",
            ..msg(
                60,
                "Sent",
                "Alex Morgan <you@example.com>",
                quarterly,
                180,
                l(
                    "Thanks, I'll take a look first thing tomorrow.",
                    "Danke, ich schaue es mir morgen früh an.",
                ),
            )
        },
        Msg {
            from: "Alex Morgan <you@example.com>",
            to: "Robin Vale <robin@example.com>",
            ..msg(
                61,
                "Drafts",
                "Alex Morgan <you@example.com>",
                l("Thursday works", "Donnerstag passt"),
                45,
                // Unfinished on purpose: it is a draft.
                l(
                    "Thursday at 12:30 works for me. Shall I book a table",
                    "Donnerstag 12:30 passt mir gut. Soll ich einen Tisch",
                ),
            )
        },
        msg(
            62,
            "Archive",
            l(
                "Travel desk <booking@example.com>",
                "Reisebüro <booking@example.com>",
            ),
            l("Your booking confirmation", "Deine Buchungsbestätigung"),
            20000,
            l(
                "Your trip is confirmed. Departure 09:40, Terminal 2.",
                "Deine Reise ist bestätigt. Abflug 09:40, Terminal 2.",
            ),
        ),
        msg(
            63,
            "Junk",
            l(
                "Prize draw <noreply@example.com>",
                "Gewinnspiel <noreply@example.com>",
            ),
            l("You have won!!!", "Sie haben gewonnen!!!"),
            8000,
            l(
                "Click here to claim your prize.",
                "Klicken Sie hier, um Ihren Preis abzuholen.",
            ),
        ),
        msg(
            64,
            "Trash",
            l(
                "Mobile carrier <info@example.com>",
                "Netzbetreiber <info@example.com>",
            ),
            l("Your bill for August", "Ihre Rechnung für August"),
            30000,
            l(
                "Your bill is ready to view.",
                "Ihre Rechnung steht zum Abruf bereit.",
            ),
        ),
        msg(
            65,
            projects_folder(lang),
            "Nadia Roth <nadia@example.com>",
            l("Rewrite: where things stand", "Rewrite: Stand der Dinge"),
            5000,
            l(
                "Quick update: the migration is running, two endpoints are still missing.",
                "Kurzer Zwischenstand: die Migration läuft, zwei Endpunkte fehlen noch.",
            ),
        ),
    ]
}

fn seed_mail(cache: &Cache, lang: DemoLanguage) -> Result<u32, UnkaiError> {
    let now = Utc::now();
    let all = fixtures(lang);

    let envelopes: Vec<EmailEnvelope> = all
        .iter()
        .map(|m| {
            let date = now - Duration::minutes(m.minutes_ago);
            EmailEnvelope {
                uid: m.uid,
                folder: m.folder.to_string(),
                from: m.from.to_string(),
                to_addrs: vec![m.to.to_string()],
                subject: m.subject.to_string(),
                date,
                is_read: m.read,
                is_starred: m.starred,
                is_answered: false,
                replied_kind: None,
                account_id: DEMO_ACCOUNT_ID.to_string(),
                message_id: Some(format!("<demo-{}@example.com>", m.uid)),
                in_reply_to: None,
                references_ids: Vec::new(),
                thread_id: m.thread.map(str::to_string),
                thread_total_count: None,
                protection: None,
                is_pinned: m.pinned,
                priority: m.priority.map(str::to_string),
                priority_override: None,
                reminder_at: None,
                is_mdn_report: false,
            }
        })
        .collect();
    cache.upsert_envelopes_for_account(DEMO_ACCOUNT_ID, &envelopes)?;

    // Bodies are written too, so opening a message paints from cache
    // exactly as it would after a real fetch — no "loading" state that
    // never resolves because there is nothing to fetch from.
    for m in &all {
        let date = now - Duration::minutes(m.minutes_ago);
        let email = Email {
            id: format!("{}::{}::{}", DEMO_ACCOUNT_ID, m.folder, m.uid),
            account_id: DEMO_ACCOUNT_ID.to_string(),
            folder: m.folder.to_string(),
            from: m.from.to_string(),
            to: vec![m.to.to_string()],
            cc: Vec::new(),
            subject: m.subject.to_string(),
            body_text: Some(m.text.to_string()),
            body_html: m.html.map(str::to_string),
            date,
            is_read: m.read,
            is_starred: m.starred,
            has_attachments: !m.attachments.is_empty(),
            // Attachment *metadata* only. The bytes are not seeded: a
            // download would have to invent them, and a demo that hands
            // back a fabricated PDF is worse than one whose download
            // button honestly reports there is nothing behind it.
            attachments: m
                .attachments
                .iter()
                .enumerate()
                .map(|(i, (name, mime))| unkai_core::models::EmailAttachment {
                    filename: name.to_string(),
                    content_type: mime.to_string(),
                    // No byte count, because there are no bytes. The
                    // reader renders the chip without a size rather
                    // than claiming one.
                    size: None,
                    part_id: i as u32 + 1,
                    content_id: None,
                })
                .collect(),
            message_id: Some(format!("<demo-{}@example.com>", m.uid)),
            in_reply_to: None,
            references_ids: Vec::new(),
            protection: None,
            signature_status: None,
            signer_fingerprint: None,
            is_pinned: m.pinned,
            priority: m.priority.map(str::to_string),
            priority_override: None,
            reminder_at: None,
            mdn_requested_to: None,
            mdn_handled: None,
        };
        cache.upsert_message(&email)?;
    }

    Ok(all.len() as u32)
}

/* ── Contacts ─────────────────────────────────────────────────────── */

fn contact(
    n: u32,
    name: &str,
    email: &str,
    phone: &str,
    org: Option<&str>,
    title: Option<&str>,
) -> ContactRow {
    let uid = format!("demo-contact-{n}");
    // `vcard_raw` is not optional, and filling it with a placeholder
    // would be a trap: every write path in the app round-trips the
    // stored vCard, so a malformed one would only surface later, as a
    // corrupted contact after an edit. Real vCard 3.0 it is.
    let mut vcard = format!(
        "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{uid}\r\nFN:{name}\r\n\
         EMAIL;TYPE=HOME:{email}\r\nTEL;TYPE=CELL:{phone}\r\n"
    );
    if let Some(o) = org {
        vcard.push_str(&format!("ORG:{o}\r\n"));
    }
    if let Some(t) = title {
        vcard.push_str(&format!("TITLE:{t}\r\n"));
    }
    vcard.push_str("END:VCARD\r\n");

    ContactRow {
        href: format!("{DEMO_ADDRESSBOOK}/{uid}.vcf"),
        etag: format!("\"{uid}\""),
        vcard_uid: uid,
        display_name: name.to_string(),
        emails: vec![ContactEmail {
            kind: "home".to_string(),
            value: email.to_string(),
        }],
        phones: vec![ContactPhone {
            kind: "cell".to_string(),
            value: phone.to_string(),
        }],
        organization: org.map(str::to_string),
        photo_mime: None,
        photo_data: None,
        title: title.map(str::to_string),
        birthday: None,
        note: None,
        addresses: Vec::new(),
        urls: Vec::new(),
        vcard_raw: vcard,
        kind: "individual".to_string(),
        member_uids: Vec::new(),
        categories: Vec::new(),
    }
}

fn demo_contacts(lang: DemoLanguage) -> Vec<ContactRow> {
    let l = |english, german| lang.pick(english, german);
    // The senders in the mailbox are in here on purpose: the mail list
    // resolves a sender's card to show their photo and name, so a demo
    // whose contacts and mail don't overlap would leave that path
    // untested. The English numbers are in 555-0100…0199, the range
    // reserved for fiction, so none of them can ring a real phone.
    let nordwerk = l("Nordwerk Ltd", "Nordwerk GmbH");
    vec![
        contact(
            1,
            "Jamie Fischer",
            "jamie@example.com",
            l("+1 202 555 0101", "+49 151 2345678"),
            Some(nordwerk),
            Some(l("Head of Finance", "Leitung Finanzen")),
        ),
        contact(
            2,
            "Robin Vale",
            "robin@example.com",
            l("+1 202 555 0102", "+49 160 9876543"),
            None,
            None,
        ),
        contact(
            3,
            "Kim Bauer",
            "kim@example.com",
            l("+1 202 555 0103", "+49 170 5551234"),
            Some(nordwerk),
            Some(l("Financial Controller", "Controlling")),
        ),
        contact(
            4,
            "Nadia Roth",
            "nadia@example.com",
            l("+1 202 555 0104", "+49 152 4443322"),
            Some("Firn Labs"),
            Some(l("Engineering", "Entwicklung")),
        ),
        contact(
            5,
            "Sam Weber",
            "sam@example.com",
            l("+1 202 555 0105", "+49 176 1122334"),
            None,
            Some(l("Photography", "Fotografie")),
        ),
        contact(
            6,
            "Toni Lang",
            "toni@example.com",
            l("+1 202 555 0106", "+49 159 6677889"),
            Some(l("Nord Carpentry", "Tischlerei Nord")),
            None,
        ),
        contact(
            7,
            "Alex Morgan",
            "you@example.com",
            l("+1 202 555 0100", "+49 151 0000000"),
            None,
            None,
        ),
    ]
}

fn seed_contacts(cache: &Cache, lang: DemoLanguage) -> Result<u32, UnkaiError> {
    let rows = demo_contacts(lang);
    let count = rows.len() as u32;

    cache.apply_contact_delta(
        DEMO_NC_ID,
        DEMO_ADDRESSBOOK,
        Some(lang.pick("Contacts", "Kontakte")),
        &rows,
        &[],
        None,
        None,
    )?;
    Ok(count)
}

/* ── Calendar ─────────────────────────────────────────────────────── */

fn event(
    n: u32,
    summary: &str,
    start: DateTime<Utc>,
    minutes: i64,
    location: Option<&str>,
    url: Option<&str>,
) -> CalendarEventRow {
    let uid = format!("demo-event-{n}");
    let end = start + Duration::minutes(minutes);
    let stamp = |d: DateTime<Utc>| d.format("%Y%m%dT%H%M%SZ").to_string();

    // Same reasoning as the vCard above: `ics_raw` is what an edit
    // round-trips, so it has to be a real VEVENT rather than a filler
    // string that only breaks once someone taps Edit.
    let mut ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Unkai Mail//Demo//EN\r\n\
         BEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:{}\r\nDTSTART:{}\r\nDTEND:{}\r\n\
         SUMMARY:{summary}\r\n",
        stamp(Utc::now()),
        stamp(start),
        stamp(end),
    );
    if let Some(l) = location {
        ics.push_str(&format!("LOCATION:{l}\r\n"));
    }
    if let Some(u) = url {
        ics.push_str(&format!("URL:{u}\r\n"));
    }
    ics.push_str("END:VEVENT\r\nEND:VCALENDAR\r\n");

    CalendarEventRow {
        uid: uid.clone(),
        recurrence_id: None,
        href: format!("{DEMO_CALENDAR_PATH}/{uid}.ics"),
        etag: format!("\"{uid}\""),
        summary: summary.to_string(),
        description: None,
        start,
        end,
        location: location.map(str::to_string),
        rrule: None,
        rdate: Vec::new(),
        exdate: Vec::new(),
        url: url.map(str::to_string),
        transparency: None,
        attendees: Vec::new(),
        reminders: Vec::new(),
        latitude: None,
        longitude: None,
        ics_raw: ics,
    }
}

/// 09:00 today on the device's clock, so the agenda always has something
/// under "Today" whenever the demo is created.
///
/// It used to be 09:00 *UTC*, whose date is not the user's date for part
/// of every day — after local midnight in Europe, every evening in the
/// Americas — and the day's events then sat on yesterday or tomorrow.
/// `Local` is the same clock the search's date operators use.
fn today_at_nine() -> DateTime<Utc> {
    Local::now()
        .with_hour(9)
        .and_then(|d| d.with_minute(0))
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

fn seed_calendar(cache: &Cache, lang: DemoLanguage) -> Result<u32, UnkaiError> {
    let l = |english, german| lang.pick(english, german);
    cache.upsert_calendars(
        DEMO_NC_ID,
        &[CalendarRow {
            path: DEMO_CALENDAR_PATH.to_string(),
            display_name: l("Personal", "Persönlich").to_string(),
            color: Some("#3b82f6".to_string()),
            ctag: None,
            hidden: false,
            muted: false,
            read_only: false,
        }],
    )?;

    let calendar_id = cache
        .list_calendars(DEMO_NC_ID)?
        .into_iter()
        .next()
        .map(|c| c.id)
        .ok_or_else(|| UnkaiError::Other("demo calendar was not created".into()))?;

    let today9 = today_at_nine();

    let rows = vec![
        event(
            1,
            l("Team stand-up", "Team-Standup"),
            today9,
            15,
            Some(l("Office / Room 2", "Büro / Raum 2")),
            None,
        ),
        event(
            2,
            l("Quarterly review", "Quartalsreview"),
            today9 + Duration::hours(2),
            60,
            None,
            Some(l(
                "https://talk.example.com/call/quarterly",
                "https://talk.example.com/call/quartal",
            )),
        ),
        event(
            3,
            l("Lunch with Robin", "Mittagessen mit Robin"),
            today9 + Duration::hours(3) + Duration::minutes(30),
            60,
            Some(l("The place on the corner", "Lokal an der Ecke")),
            None,
        ),
        event(
            4,
            l("Dentist", "Zahnarzt"),
            today9 + Duration::days(1) + Duration::hours(7),
            45,
            Some(l("Dr Lang's practice", "Praxis Dr. Lang")),
            None,
        ),
        event(
            5,
            l("Sprint planning", "Sprint-Planung"),
            today9 + Duration::days(2),
            90,
            None,
            None,
        ),
        event(
            6,
            l("Sam's birthday", "Geburtstag Sam"),
            today9 + Duration::days(5) - Duration::hours(3),
            720,
            None,
            None,
        ),
        event(
            7,
            "Retro",
            today9 + Duration::days(9) + Duration::hours(6),
            60,
            None,
            None,
        ),
        event(
            8,
            l("Quote due", "Abgabe Angebot"),
            today9 - Duration::days(3),
            30,
            None,
            None,
        ),
    ];
    let count = rows.len() as u32;

    cache.apply_event_delta(&calendar_id, &rows, &[], None, None)?;
    Ok(count)
}

/* ── Local stand-ins for the server round-trips ───────────────────── */

/// Move a cached message between folders, the way the server would.
///
/// Archive, delete and move all end with "the message is somewhere
/// else now". Against a real server the IMAP `COPY`+`EXPUNGE` does
/// that and the cache follows; for a demo account there is no server,
/// so this *is* the move. Doing it properly — rather than just
/// dropping the row — is what lets the demo exercise the parts of the
/// UI that come after the gesture: the message really is in Archive
/// afterwards, and really is gone from the Inbox.
///
/// `dest = None` deletes outright, which is what "delete" means for a
/// message already sitting in Trash.
pub fn move_cached_message(
    cache: &Cache,
    account_id: &str,
    folder: &str,
    uid: u32,
    dest: Option<&str>,
) -> Result<(), UnkaiError> {
    // The store has no single-envelope read, and adding one for this
    // would be a public API change for a test-mode fixture. The folder page
    // is small and already in memory, so scanning it is cheaper than
    // that trade.
    let envelope = cache
        .get_envelopes(account_id, folder, 500)?
        .into_iter()
        .find(|e| e.uid == uid);

    if let (Some(dest), Some(mut envelope)) = (dest, envelope) {
        let body = cache.get_message(account_id, folder, uid).ok().flatten();
        envelope.folder = dest.to_string();
        cache.upsert_envelopes_for_account(account_id, std::slice::from_ref(&envelope))?;
        if let Some(mut email) = body {
            email.folder = dest.to_string();
            email.id = format!("{account_id}::{dest}::{uid}");
            let _ = cache.upsert_message(&email);
        }
    }

    cache.remove_envelope(account_id, folder, uid)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No-op notifier: the demo only uses it to push the unread badge.
    struct NullNotifier;
    impl UiNotifier for NullNotifier {
        fn new_mail(&self, _: &crate::notify::NewMailPayload) {}
        fn mail_flags_updated(&self, _: &crate::notify::MailFlagsUpdatedPayload) {}
        fn outbox_updated(&self, _: &crate::notify::OutboxUpdatedPayload) {}
        fn calendars_updated(&self, _: &crate::notify::CalendarsUpdatedPayload) {}
        fn event_reminder(&self, _: &crate::notify::EventReminderPayload) {}
        fn message_reminder(
            &self,
            _: &crate::notify::MessageReminderPayload,
        ) -> Result<(), UnkaiError> {
            Ok(())
        }
        fn unread_total_changed(&self, _: u32) {}
        fn unread_by_account_changed(&self, _: &std::collections::HashMap<String, u32>) {}
        fn custom_themes_changed(&self) {}
        fn profiles_changed(&self) {}
        fn apply_logo_style(&self, _: &str) -> Result<(), UnkaiError> {
            Ok(())
        }
    }

    fn cache() -> Cache {
        Cache::open_in_memory().expect("in-memory cache")
    }

    /// "Leave test mode" must leave nothing behind. It used to delete
    /// only the two account rows, so the seeded messages kept the unread
    /// badge at 4, and the seeded contacts kept turning up as address
    /// suggestions, for a user who had just asked for all of it to go.
    #[tokio::test]
    async fn removing_the_demo_leaves_no_demo_data_behind() {
        let cache = cache();
        let summary = create_demo_account(&cache, &NullNotifier, DemoLanguage::English)
            .await
            .expect("seed demo");
        assert!(summary.messages > 0 && summary.contacts > 0 && summary.events > 0);
        assert!(
            cache.total_unread_count().unwrap() > 0,
            "fixture should include unread mail"
        );

        remove_demo_account(&cache).await.expect("remove demo");

        assert!(!demo_account_exists(&cache));
        assert_eq!(
            cache.total_unread_count().unwrap(),
            0,
            "unread badge still counts demo mail"
        );
        assert!(
            cache.get_folders(DEMO_ACCOUNT_ID).unwrap().is_empty(),
            "demo folders remain"
        );
        for folder in ["INBOX", "Archive", "Sent", "Drafts", "Junk", "Trash"] {
            assert!(
                cache
                    .get_envelopes(DEMO_ACCOUNT_ID, folder, 500)
                    .unwrap()
                    .is_empty(),
                "demo messages remain in {folder}"
            );
        }
        assert!(
            cache.list_contacts(None, |_, _| {}).unwrap().is_empty(),
            "demo contacts remain"
        );
        assert!(
            cache.list_calendars(DEMO_NC_ID).unwrap().is_empty(),
            "demo calendars remain"
        );
        assert!(
            cache.list_task_lists(DEMO_NC_ID).unwrap().is_empty(),
            "demo task lists remain"
        );
    }

    /// Real accounts are none of test mode's business.
    #[tokio::test]
    async fn removing_the_demo_keeps_other_accounts() {
        let cache = cache();
        create_demo_account(&cache, &NullNotifier, DemoLanguage::English)
            .await
            .expect("seed demo");
        let mut real = demo_account(DemoLanguage::English);
        real.id = "real-account".to_string();
        real.demo = false;
        account_store::add_account(&cache, real).expect("add real account");

        remove_demo_account(&cache).await.expect("remove demo");

        let remaining = account_store::load_accounts(&cache).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "real-account");
    }

    /// Leaving test mode after the demo mail account was already
    /// removed by hand still clears the demo's contacts and calendar,
    /// and a second call is harmless.
    #[tokio::test]
    async fn removal_is_idempotent_and_finishes_a_half_removed_demo() {
        let cache = cache();
        create_demo_account(&cache, &NullNotifier, DemoLanguage::English)
            .await
            .expect("seed demo");
        account_store::remove_account(&cache, DEMO_ACCOUNT_ID).expect("remove mail half");

        remove_demo_account(&cache).await.expect("remove the rest");
        assert!(cache.list_contacts(None, |_, _| {}).unwrap().is_empty());
        assert!(cache.list_calendars(DEMO_NC_ID).unwrap().is_empty());
        assert!(
            !nextcloud_store::load_accounts(&cache)
                .unwrap()
                .iter()
                .any(|a| a.id == DEMO_NC_ID)
        );

        remove_demo_account(&cache)
            .await
            .expect("second removal is a no-op");
    }

    /// Removing the demo from Settings → Accounts (the generic
    /// `remove_account` command) is as thorough as "Leave test mode".
    #[tokio::test]
    async fn removing_the_demo_as_an_account_cleans_up_everything() {
        let cache = cache();
        create_demo_account(&cache, &NullNotifier, DemoLanguage::English)
            .await
            .expect("seed demo");
        let notify =
            crate::state::SettingsSyncNotify(std::sync::Arc::new(tokio::sync::Notify::new()));

        crate::accounts::remove_account(DEMO_ACCOUNT_ID.to_string(), &cache, &notify)
            .expect("remove through the accounts command");

        assert!(!demo_account_exists(&cache));
        assert_eq!(cache.total_unread_count().unwrap(), 0);
        assert!(cache.list_contacts(None, |_, _| {}).unwrap().is_empty());
        assert!(cache.list_calendars(DEMO_NC_ID).unwrap().is_empty());
    }

    /// Every seeded vCard reads back as the contact it describes. An
    /// edit round-trips `vcard_raw`, so a card whose text disagrees with
    /// its row turns into a different contact the first time someone
    /// taps Save.
    #[test]
    fn seeded_vcards_parse_back_to_the_same_contact() {
        let both = [DemoLanguage::English, DemoLanguage::German];
        for row in both.into_iter().flat_map(demo_contacts) {
            let card = unkai_carddav::vcard::parse_vcard(&row.vcard_raw).expect("parse");
            assert_eq!(card.display_name, row.display_name, "{}", row.vcard_raw);
            let emails: Vec<_> = card.emails.iter().map(|e| e.value.as_str()).collect();
            let expected: Vec<_> = row.emails.iter().map(|e| e.value.as_str()).collect();
            assert_eq!(emails, expected, "{}", row.vcard_raw);
        }
    }

    /// The UI's language picks the data's: German for any German
    /// locale, English for everything else, including no locale.
    #[test]
    fn the_language_follows_the_locale() {
        for de in ["de", "de-DE", "de-AT", "de_CH", "DE"] {
            assert_eq!(
                DemoLanguage::for_locale(Some(de)),
                DemoLanguage::German,
                "{de}"
            );
        }
        for other in ["en", "en-GB", "fr", "ja", "pt-BR", "", "dk"] {
            assert_eq!(
                DemoLanguage::for_locale(Some(other)),
                DemoLanguage::English,
                "{other}"
            );
        }
        assert_eq!(DemoLanguage::for_locale(None), DemoLanguage::English);
    }

    /// Both languages cover the same states — the point of the
    /// fixtures. Only the words may differ.
    #[test]
    fn both_languages_seed_the_same_states() {
        let en = fixtures(DemoLanguage::English);
        let de = fixtures(DemoLanguage::German);
        assert_eq!(en.len(), de.len());
        for (e, d) in en.iter().zip(&de) {
            assert_eq!(e.uid, d.uid);
            let same_folder = e.folder == d.folder
                || (e.folder == projects_folder(DemoLanguage::English)
                    && d.folder == projects_folder(DemoLanguage::German));
            assert!(same_folder, "uid {}: {} vs {}", e.uid, e.folder, d.folder);
            assert_eq!(
                (e.minutes_ago, e.read, e.starred, e.pinned),
                (d.minutes_ago, d.read, d.starred, d.pinned),
                "uid {}",
                e.uid
            );
            assert_eq!(
                (e.priority, e.thread),
                (d.priority, d.thread),
                "uid {}",
                e.uid
            );
            assert_eq!(e.attachments.len(), d.attachments.len(), "uid {}", e.uid);
            assert_eq!(e.html.is_some(), d.html.is_some(), "uid {}", e.uid);
            assert_eq!(
                e.from.split('<').nth(1),
                d.from.split('<').nth(1),
                "uid {}: sender addresses are the same in both languages (contact cards match by address)",
                e.uid
            );
        }
        let (ec, dc) = (
            demo_contacts(DemoLanguage::English),
            demo_contacts(DemoLanguage::German),
        );
        assert_eq!(ec.len(), dc.len());
        for (e, d) in ec.iter().zip(&dc) {
            assert_eq!(
                (&e.display_name, &e.emails[0].value),
                (&d.display_name, &d.emails[0].value)
            );
        }
    }

    /// A cheap net for a missed translation: no German letters in the
    /// English sample data.
    #[test]
    fn english_sample_data_has_no_german_left_in_it() {
        let lang = DemoLanguage::English;
        let mut texts: Vec<String> = Vec::new();
        for m in fixtures(lang) {
            texts.extend([m.from, m.to, m.subject, m.text, m.folder].map(String::from));
            texts.extend(m.html.map(String::from));
            texts.extend(m.attachments.iter().map(|(name, _)| name.to_string()));
        }
        for c in demo_contacts(lang) {
            texts.push(c.vcard_raw);
        }
        texts.push(demo_account(lang).display_name);
        texts.extend(demo_source(lang).display_name);
        for t in texts {
            assert!(
                !t.contains(['ä', 'ö', 'ü', 'Ä', 'Ö', 'Ü', 'ß']),
                "German in: {t}"
            );
        }
    }

    /// Re-seeding in the other language replaces the data instead of
    /// mixing the two — the user folder's name differs per language, so
    /// a leftover would show as a second, empty folder.
    #[tokio::test]
    async fn re_seeding_in_another_language_replaces_the_data() {
        let cache = cache();
        create_demo_account(&cache, &NullNotifier, DemoLanguage::English)
            .await
            .expect("seed in English");
        create_demo_account(&cache, &NullNotifier, DemoLanguage::German)
            .await
            .expect("seed in German");

        let folders: Vec<String> = cache
            .get_folders(DEMO_ACCOUNT_ID)
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert!(folders.contains(&"Projekte".to_string()), "{folders:?}");
        assert!(!folders.contains(&"Projects".to_string()), "{folders:?}");

        let subjects: Vec<String> = cache
            .get_envelopes(DEMO_ACCOUNT_ID, "INBOX", 500)
            .unwrap()
            .into_iter()
            .map(|e| e.subject)
            .collect();
        assert!(
            subjects.iter().any(|s| s == "Mittagessen am Donnerstag?"),
            "{subjects:?}"
        );
        assert!(
            !subjects.iter().any(|s| s == "Lunch on Thursday?"),
            "{subjects:?}"
        );
    }

    /// "Today" in the demo calendar is the user's today, not UTC's.
    #[test]
    fn todays_events_land_on_the_local_date() {
        let nine = today_at_nine().with_timezone(&Local);
        assert_eq!(nine.date_naive(), Local::now().date_naive());
        assert_eq!((nine.hour(), nine.minute()), (9, 0));
    }

    /// Loading the demo twice replaces it rather than doubling it.
    #[tokio::test]
    async fn re_creating_the_demo_does_not_duplicate_it() {
        let cache = cache();
        create_demo_account(&cache, &NullNotifier, DemoLanguage::English)
            .await
            .expect("seed demo");
        let unread = cache.total_unread_count().unwrap();
        let contacts = cache.list_contacts(None, |_, _| {}).unwrap().len();
        create_demo_account(&cache, &NullNotifier, DemoLanguage::English)
            .await
            .expect("seed demo again");
        assert_eq!(cache.total_unread_count().unwrap(), unread);
        assert_eq!(
            cache.list_contacts(None, |_, _| {}).unwrap().len(),
            contacts
        );
    }
}
