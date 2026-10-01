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

use chrono::{DateTime, Duration, Timelike, Utc};
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

/// Seed everything. Idempotent: running it twice leaves one demo
/// account with freshly-regenerated data, not two.
pub async fn create_demo_account(
    cache: &Cache,
    ui: &dyn UiNotifier,
) -> Result<DemoSummary, UnkaiError> {
    remove_demo_account(cache).await.ok();

    let account = demo_account();
    account_store::add_account(cache, account)?;

    let folders = demo_folders();
    cache.upsert_folders(DEMO_ACCOUNT_ID, &folders)?;

    let messages = seed_mail(cache)?;

    // Groupware rides a `Local` DAV source — the kind that already
    // means "no remote at all, sync is a no-op, writes stay in the
    // cache". Exactly what a demo needs, and it means Calendar,
    // Contacts and Tasks need no demo-specific code: they loop over
    // Nextcloud accounts and this is simply one of them.
    nextcloud_store::upsert_account(cache, demo_source())?;
    let contacts = seed_contacts(cache)?;
    let events = seed_calendar(cache)?;

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

fn demo_account() -> Account {
    Account {
        id: DEMO_ACCOUNT_ID.to_string(),
        display_name: "Testkonto".to_string(),
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

fn demo_source() -> NextcloudAccount {
    NextcloudAccount {
        id: DEMO_NC_ID.to_string(),
        server_url: "local://demo".to_string(),
        username: DEMO_EMAIL.to_string(),
        display_name: Some("Testdaten".to_string()),
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

fn demo_folders() -> Vec<Folder> {
    vec![
        folder("INBOX", &[], 4),
        folder("Drafts", &["Drafts"], 0),
        folder("Sent", &["Sent"], 0),
        folder("Archive", &["Archive"], 0),
        folder("Junk", &["Junk"], 0),
        folder("Trash", &["Trash"], 0),
        folder("Projekte", &[], 0),
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

fn fixtures() -> Vec<Msg> {
    vec![
        Msg {
            read: false,
            starred: true,
            attachments: &[("Q3-Zahlen.pdf", "application/pdf")],
            thread: Some("thread-quarterly"),
            html: Some(
                "<p>Hallo Alex,</p><p>die Zahlen für Q3 sind angehängt. \
                 Schau bitte bis Freitag drüber — besonders auf die \
                 Marge in Zeile 14.</p><p>Viele Grüße<br>Jamie</p>",
            ),
            ..msg(
                101,
                "INBOX",
                "Jamie Fischer <jamie@example.com>",
                "Re: Quartalszahlen — Zahlen im Anhang",
                24,
                "Hallo Alex,\n\ndie Zahlen für Q3 sind angehängt. Schau bitte bis \
                 Freitag drüber — besonders auf die Marge in Zeile 14.\n\nViele Grüße\nJamie",
            )
        },
        Msg {
            read: false,
            priority: Some("high"),
            ..msg(
                100,
                "INBOX",
                "Sicherheit <security@example.com>",
                "Neue Anmeldung auf einem unbekannten Gerät",
                95,
                "Wir haben eine Anmeldung aus Hamburg festgestellt. Warst du das \
                 nicht, ändere bitte sofort dein Passwort.",
            )
        },
        Msg {
            read: false,
            pinned: true,
            ..msg(
                99,
                "INBOX",
                "Robin Vale <robin@example.com>",
                "Mittagessen am Donnerstag?",
                240,
                "Hast du Donnerstag gegen 12:30 Zeit? Es gibt das neue Lokal an \
                 der Ecke, das du dir anschauen wolltest.",
            )
        },
        Msg {
            read: false,
            thread: Some("thread-quarterly"),
            ..msg(
                98,
                "INBOX",
                "Kim Bauer <kim@example.com>",
                "Re: Quartalszahlen — Zahlen im Anhang",
                310,
                "Ich habe die Marge nachgerechnet, Zeile 14 stimmt. Die Abweichung \
                 kommt aus der Umbuchung im August.",
            )
        },
        Msg {
            html: Some(
                // Deliberately laid out for a 640px desktop pane: this is
                // what `makeResponsive` has to clamp, so the demo covers
                // the rendering pipeline and not just short plain text.
                "<table width=\"640\" cellpadding=\"0\" style=\"width:640px\"><tr><td \
                 style=\"font-family:Georgia,serif;color:#333\"><h1>Was wir im August \
                 gebaut haben</h1><p>Sechs Dinge, über die wir uns freuen — von \
                 schnelleren Synchronisierungen bis zu einem neuen Kalender.</p>\
                 <p><a href=\"https://example.com/changelog\">Alles lesen →</a></p>\
                 </td></tr></table>",
            ),
            ..msg(
                97,
                "INBOX",
                "Produkt-Newsletter <news@example.com>",
                "Sechs Dinge, die wir im August ausgeliefert haben",
                1500,
                "Sechs Dinge, über die wir uns freuen — von schnelleren \
                 Synchronisierungen bis zu einem neuen Kalender.",
            )
        },
        Msg {
            attachments: &[
                ("Grundriss.png", "image/png"),
                ("Angebot.pdf", "application/pdf"),
            ],
            ..msg(
                96,
                "INBOX",
                "Tischlerei Nord <info@example.com>",
                "Ihr Angebot für die Einbauschränke",
                2900,
                "Guten Tag,\n\nanbei finden Sie das besprochene Angebot sowie den \
                 Grundriss mit den eingezeichneten Maßen.",
            )
        },
        Msg {
            starred: true,
            ..msg(
                95,
                "INBOX",
                "CI <ci@example.com>",
                "[unkai-mail] Build auf main erfolgreich",
                4300,
                "Alle 342 Tests grün. Dauer: 4m 12s.",
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
                "Re: Quartalszahlen — Zahlen im Anhang",
                180,
                "Danke, ich schaue es mir morgen früh an.",
            )
        },
        Msg {
            from: "Alex Morgan <you@example.com>",
            to: "Robin Vale <robin@example.com>",
            ..msg(
                61,
                "Drafts",
                "Alex Morgan <you@example.com>",
                "Donnerstag passt",
                45,
                "Donnerstag 12:30 passt mir gut. Soll ich einen Tisch",
            )
        },
        msg(
            62,
            "Archive",
            "Reisebüro <buchung@example.com>",
            "Deine Buchungsbestätigung",
            20000,
            "Deine Reise ist bestätigt. Abflug 09:40, Terminal 2.",
        ),
        msg(
            63,
            "Junk",
            "Gewinnspiel <noreply@example.com>",
            "Sie haben gewonnen!!!",
            8000,
            "Klicken Sie hier, um Ihren Preis abzuholen.",
        ),
        msg(
            64,
            "Trash",
            "Netzbetreiber <info@example.com>",
            "Ihre Rechnung für August",
            30000,
            "Ihre Rechnung steht zum Abruf bereit.",
        ),
        msg(
            65,
            "Projekte",
            "Nadia Roth <nadia@example.com>",
            "Rewrite: Stand der Dinge",
            5000,
            "Kurzer Zwischenstand: die Migration läuft, zwei Endpunkte fehlen noch.",
        ),
    ]
}

fn seed_mail(cache: &Cache) -> Result<u32, UnkaiError> {
    let now = Utc::now();
    let all = fixtures();

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
        "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:{uid}\r\nFN:{name}\r\n         EMAIL;TYPE=HOME:{email}\r\nTEL;TYPE=CELL:{phone}\r\n"
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

fn seed_contacts(cache: &Cache) -> Result<u32, UnkaiError> {
    // The senders in the mailbox are in here on purpose: the mail list
    // resolves a sender's card to show their photo and name, so a demo
    // whose contacts and mail don't overlap would leave that path
    // untested.
    let rows = vec![
        contact(
            1,
            "Jamie Fischer",
            "jamie@example.com",
            "+49 151 2345678",
            Some("Nordwerk GmbH"),
            Some("Leitung Finanzen"),
        ),
        contact(
            2,
            "Robin Vale",
            "robin@example.com",
            "+49 160 9876543",
            None,
            None,
        ),
        contact(
            3,
            "Kim Bauer",
            "kim@example.com",
            "+49 170 5551234",
            Some("Nordwerk GmbH"),
            Some("Controlling"),
        ),
        contact(
            4,
            "Nadia Roth",
            "nadia@example.com",
            "+49 152 4443322",
            Some("Firn Labs"),
            Some("Entwicklung"),
        ),
        contact(
            5,
            "Sam Weber",
            "sam@example.com",
            "+49 176 1122334",
            None,
            Some("Fotografie"),
        ),
        contact(
            6,
            "Toni Lang",
            "toni@example.com",
            "+49 159 6677889",
            Some("Tischlerei Nord"),
            None,
        ),
        contact(
            7,
            "Alex Morgan",
            "you@example.com",
            "+49 151 0000000",
            None,
            None,
        ),
    ];
    let count = rows.len() as u32;

    cache.apply_contact_delta(
        DEMO_NC_ID,
        DEMO_ADDRESSBOOK,
        Some("Kontakte"),
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

fn seed_calendar(cache: &Cache) -> Result<u32, UnkaiError> {
    cache.upsert_calendars(
        DEMO_NC_ID,
        &[CalendarRow {
            path: DEMO_CALENDAR_PATH.to_string(),
            display_name: "Persönlich".to_string(),
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

    // Anchored to today at a whole hour, so the agenda always has
    // something in "Heute" whenever the demo is created.
    let today9 = Utc::now()
        .with_hour(9)
        .and_then(|d| d.with_minute(0))
        .and_then(|d| d.with_second(0))
        .unwrap_or_else(Utc::now);

    let rows = vec![
        event(1, "Team-Standup", today9, 15, Some("Büro / Raum 2"), None),
        event(
            2,
            "Quartalsreview",
            today9 + Duration::hours(2),
            60,
            None,
            Some("https://talk.example.com/call/quartal"),
        ),
        event(
            3,
            "Mittagessen mit Robin",
            today9 + Duration::hours(3) + Duration::minutes(30),
            60,
            Some("Lokal an der Ecke"),
            None,
        ),
        event(
            4,
            "Zahnarzt",
            today9 + Duration::days(1) + Duration::hours(7),
            45,
            Some("Praxis Dr. Lang"),
            None,
        ),
        event(
            5,
            "Sprint-Planung",
            today9 + Duration::days(2),
            90,
            None,
            None,
        ),
        event(
            6,
            "Geburtstag Sam",
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
            "Abgabe Angebot",
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
        let summary = create_demo_account(&cache, &NullNotifier)
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
        create_demo_account(&cache, &NullNotifier)
            .await
            .expect("seed demo");
        let mut real = demo_account();
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
        create_demo_account(&cache, &NullNotifier)
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
        create_demo_account(&cache, &NullNotifier)
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

    /// Loading the demo twice replaces it rather than doubling it.
    #[tokio::test]
    async fn re_creating_the_demo_does_not_duplicate_it() {
        let cache = cache();
        create_demo_account(&cache, &NullNotifier)
            .await
            .expect("seed demo");
        let unread = cache.total_unread_count().unwrap();
        let contacts = cache.list_contacts(None, |_, _| {}).unwrap().len();
        create_demo_account(&cache, &NullNotifier)
            .await
            .expect("seed demo again");
        assert_eq!(cache.total_unread_count().unwrap(), unread);
        assert_eq!(
            cache.list_contacts(None, |_, _| {}).unwrap().len(),
            contacts
        );
    }
}
