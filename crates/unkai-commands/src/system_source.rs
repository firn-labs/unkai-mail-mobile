//! The **System** source: the device's own calendar, address book
//! and reminder lists, mapped onto the same cache the DAV sources
//! use.
//!
//! A support module, not a command module — nothing here is a
//! `#[tauri::command]`. `calendar.rs`, `contacts.rs` and `tasks.rs`
//! keep owning their commands and hand off to these functions when
//! the source turns out to be `DavSourceKind::System`, exactly as
//! they already branch on `is_local()`.
//!
//! # Snapshots, not deltas
//!
//! A DAV sync asks "what changed since token X?". EventKit and
//! Contacts have no such question — they answer "here is everything"
//! and nothing else. So each sync here reads the whole collection,
//! diffs it against the hrefs already cached, and hands
//! `apply_*_delta` both halves. That keeps the write atomic and
//! keeps every downstream reader (the month grid, search, the
//! reminder buckets) unchanged.
//!
//! # Why events are stored flattened
//!
//! EventKit answers a date-range query with occurrences, not with a
//! master plus its `RRULE`. We store exactly what it gives us: one
//! cache row per occurrence, `rrule` empty. The cost is rows — a
//! daily standup over the sync window is ~1 500 of them — and the
//! benefit is that a series the user edited or cancelled on one day
//! in the Calendar app shows up here exactly as it does there,
//! without this crate reimplementing Apple's recurrence engine.
//!
//! That is also why the window is bounded: see [`SYNC_PAST`] /
//! [`SYNC_FUTURE`].

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use unkai_caldav::TaskWriteOutcome;
use unkai_caldav::WriteOutcome as CaldavWriteOutcome;
use unkai_caldav::build_ics as caldav_build_ics;
use unkai_caldav::build_vtodo_ics as caldav_build_vtodo_ics;
use unkai_carddav::WriteOutcome as CarddavWriteOutcome;
use unkai_core::UnkaiError;
use unkai_core::models::{CalendarEvent, EventAttendee, EventReminder, Task, TaskList};
use unkai_store::Cache;
use unkai_store::cache::CalendarRow;
use unkai_system::{
    PermissionState, SystemEntity, SystemError, SystemEvent, SystemEventInput, SystemReminder,
    SystemReminderInput,
};

use crate::calendar::{SyncCalendarsReport, calendar_event_to_row};
use crate::contacts::{SyncContactsReport, raw_contact_to_row};

/// How far back the event sync reaches. A year covers "when did I
/// last see them?" without dragging a decade of standups into the
/// cache.
pub const SYNC_PAST: Duration = Duration::days(365);
/// How far ahead. Three years covers annual events (birthdays,
/// contract renewals) and stays inside EventKit's own four-year
/// query ceiling with the past year included.
pub const SYNC_FUTURE: Duration = Duration::days(365 * 3);

/// Prefix marking a cached calendar as an EventKit calendar. Sits
/// where a CalDAV collection URL would, so `calendar_id` keeps its
/// `{nc_id}::{path}` shape and every id-parsing call site is
/// untouched.
const CALENDAR_PREFIX: &str = "system-calendar:";
/// Same idea for a reminder list.
const REMINDER_LIST_PREFIX: &str = "system-reminders:";
/// Prefix on an event's href. What follows is the occurrence key
/// (`{eventIdentifier}#{startEpoch}`).
const EVENT_PREFIX: &str = "system-event:";
/// Prefix on a reminder's href.
const REMINDER_PREFIX: &str = "system-reminder:";
/// Prefix on a contact's href.
const CONTACT_PREFIX: &str = "system-contact:";

/// The single addressbook name the system address book is cached
/// under. iOS exposes *containers* (iCloud, an Exchange account, On
/// My iPhone) but the Contacts app itself presents one merged list,
/// and `CNContactStore`'s unified contacts are that merge — so
/// splitting them back apart here would show the user duplicates
/// their phone doesn't have.
const ADDRESSBOOK: &str = "system";

/* ── Permissions ─────────────────────────────────────────────────*/

/// Turn a permission problem into the sentence the user sees.
///
/// `unkai-system`'s errors are precise but framework-shaped; this is
/// where they become advice.
fn to_unkai_error(e: SystemError) -> UnkaiError {
    match e {
        SystemError::AccessDenied(entity) => UnkaiError::Other(format!(
            "Unkai has no access to the system {entity} database. \
             Grant it in the iOS Settings app under Privacy & Security."
        )),
        other => UnkaiError::Other(other.to_string()),
    }
}

/// Run a blocking `unkai-system` call off the async runtime's worker
/// threads.
///
/// Every entry point in that crate blocks — a permission prompt for
/// as long as the user takes to answer it, a full address-book read
/// for however many contacts there are. Holding a tokio worker for
/// that would stall unrelated IMAP work on the same runtime.
async fn on_blocking<T, F>(work: F) -> Result<T, UnkaiError>
where
    F: FnOnce() -> Result<T, SystemError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| UnkaiError::Other(format!("the system database task failed: {e}")))?
        .map_err(to_unkai_error)
}

/// Ask for one database's permission, prompting if it hasn't been
/// asked for yet.
pub async fn request_permission(entity: SystemEntity) -> Result<PermissionState, UnkaiError> {
    on_blocking(move || unkai_system::request_access(entity)).await
}

/// The current permission grade, without prompting.
pub fn permission_status(entity: SystemEntity) -> PermissionState {
    unkai_system::access_status(entity)
}

/* ── Calendars ───────────────────────────────────────────────────*/

/// Refresh every system calendar and its events in the sync window.
pub async fn sync_calendars(nc_id: &str, cache: &Cache) -> Result<SyncCalendarsReport, UnkaiError> {
    let mut report = SyncCalendarsReport {
        nc_account_id: nc_id.to_string(),
        calendars_synced: 0,
        upserted: 0,
        deleted: 0,
        errors: Vec::new(),
    };

    let calendars = on_blocking(unkai_system::list_calendars).await?;
    let rows: Vec<CalendarRow> = calendars
        .iter()
        .map(|c| CalendarRow {
            path: format!("{CALENDAR_PREFIX}{}", c.id),
            display_name: c.title.clone(),
            // EventKit's own colour, already normalised to the
            // `#rrggbb` shape CalDAV's `calendar-color` arrives in —
            // so a device calendar paints the same hue here as it
            // does in the Calendar app, and every screen reading the
            // column stays unchanged.
            color: c.color.clone(),
            ctag: None,
            hidden: false,
            muted: false,
            read_only: c.read_only,
        })
        .collect();
    cache.upsert_calendars(nc_id, &rows)?;

    let now = Utc::now();
    let events =
        on_blocking(move || unkai_system::list_events(now - SYNC_PAST, now + SYNC_FUTURE)).await?;

    // Group by calendar so each one's delta is one transaction — a
    // failure on the third calendar leaves the first two updated,
    // which is the same guarantee the CalDAV path gives.
    let mut by_calendar: HashMap<String, Vec<SystemEvent>> = HashMap::new();
    for event in events {
        by_calendar
            .entry(event.calendar_id.clone())
            .or_default()
            .push(event);
    }

    for calendar in &calendars {
        let calendar_id = format!("{nc_id}::{CALENDAR_PREFIX}{}", calendar.id);
        let fresh = by_calendar.remove(&calendar.id).unwrap_or_default();

        let upserts: Vec<_> = fresh
            .iter()
            // A withdrawn meeting is not something to paint on the
            // agenda. The app has no cancelled-event styling, and
            // inventing an English marker in the summary would be a
            // user-visible string the frontend can't translate — so
            // a cancelled occurrence is simply absent, exactly as a
            // deleted one is. `SystemEvent::cancelled` stays on the
            // struct for the day the reader grows a way to show it.
            .filter(|event| !event.cancelled)
            .map(|event| {
                let href = event_href(event);
                let domain = to_calendar_event(event);
                let ics = caldav_build_ics(&domain, event.organizer_email.as_deref(), None);
                calendar_event_to_row(&domain, &href, &etag_for(event.last_modified), &ics)
            })
            .collect();

        // Whatever the cache still holds for this calendar and the
        // device no longer reports has been deleted somewhere else —
        // in the Calendar app, or by the event simply ageing out of
        // the sync window.
        let live: HashSet<&str> = upserts.iter().map(|r| r.href.as_str()).collect();
        let deleted: Vec<String> = cache
            .list_calendar_event_hrefs(&calendar_id)?
            .into_iter()
            .filter(|href| !live.contains(href.as_str()))
            .collect();

        match cache.apply_event_delta(&calendar_id, &upserts, &deleted, None, None) {
            Ok(()) => {
                report.calendars_synced += 1;
                report.upserted += upserts.len() as u32;
                report.deleted += deleted.len() as u32;
            }
            Err(e) => {
                tracing::warn!("system calendar '{}' failed to cache: {e}", calendar.title);
                report.errors.push(format!("{}: {e}", calendar.title));
            }
        }
    }

    Ok(report)
}

/// The href a cached row carries for one occurrence.
fn event_href(event: &SystemEvent) -> String {
    format!("{EVENT_PREFIX}{}", event.occurrence_key())
}

/// Recover the occurrence key a write needs from a cached href.
fn occurrence_key_from_href(href: &str) -> Result<&str, UnkaiError> {
    href.strip_prefix(EVENT_PREFIX).ok_or_else(|| {
        UnkaiError::Other(format!("'{href}' is not a system-calendar event reference"))
    })
}

/// A revision marker standing in for a CalDAV etag.
///
/// EventKit has no etag; `lastModifiedDate` is the closest thing and
/// changes on every edit, which is all the cache uses an etag for.
/// An item with no modification date gets a fresh marker each sync —
/// slightly wasteful, always correct.
fn etag_for(last_modified: Option<DateTime<Utc>>) -> String {
    match last_modified {
        Some(t) => format!("sys-{}", t.timestamp()),
        None => format!("sys-{}", uuid::Uuid::new_v4()),
    }
}

/// EventKit occurrence → the app's `CalendarEvent`.
fn to_calendar_event(event: &SystemEvent) -> CalendarEvent {
    CalendarEvent {
        // The cache's uid must be unique per calendar, and every
        // occurrence of a series shares an `eventIdentifier` — so the
        // occurrence key, not the identifier, is the uid.
        id: event.occurrence_key(),
        summary: event.title.clone(),
        description: event.notes.clone(),
        start: event.start,
        end: event.end,
        location: event.location.clone(),
        // Deliberately flat — see the module docs.
        rrule: None,
        rdate: Vec::new(),
        exdate: Vec::new(),
        recurrence_id: None,
        url: event.url.clone(),
        transparency: None,
        attendees: event
            .attendees
            .iter()
            .map(|a| EventAttendee {
                email: a.email.clone(),
                common_name: a.name.clone(),
                status: Some(a.partstat.clone()),
                role: Some(a.role.clone()),
                force_send_reply: false,
            })
            .collect(),
        reminders: event
            .alarms_minutes_before
            .iter()
            .map(|m| EventReminder {
                trigger_minutes_before: *m as i32,
                action: Some("DISPLAY".to_string()),
            })
            .collect(),
        latitude: None,
        longitude: None,
    }
}

/// The app's `CalendarEvent` → what EventKit needs to write it.
fn to_event_input(event: &CalendarEvent) -> SystemEventInput {
    SystemEventInput {
        title: event.summary.clone(),
        notes: event.description.clone(),
        location: event.location.clone(),
        url: event.url.clone(),
        start: Some(event.start),
        end: Some(event.end),
        // The editor sends midnight-to-23:59:59 for an all-day event
        // (see `input_to_calendar_event`); recognising that shape is
        // how the flag survives the round trip.
        all_day: is_all_day(event),
        alarms_minutes_before: event
            .reminders
            .iter()
            .map(|r| r.trigger_minutes_before as i64)
            .collect(),
    }
}

fn is_all_day(event: &CalendarEvent) -> bool {
    use chrono::Timelike;
    event.start.hour() == 0
        && event.start.minute() == 0
        && event.end.hour() == 23
        && event.end.minute() == 59
}

/// Parse the iCalendar body a write path already built.
///
/// Taking `ics` rather than a `CalendarEvent` is what keeps this
/// module a drop-in for `unkai_caldav::create_event`: every caller —
/// the event editor, an accepted invite, an imported `.ics` — already
/// holds the serialised form at the point it would `PUT`, so the
/// system branch slots in without a single signature changing. The
/// re-parse costs microseconds against a framework call that costs
/// milliseconds.
fn event_from_ics(ics: &str) -> Result<CalendarEvent, UnkaiError> {
    unkai_caldav::parse_ics(ics)?
        .into_iter()
        .next()
        .ok_or_else(|| UnkaiError::Other("the event body held no VEVENT to write".to_string()))
}

/// Create an event in a system calendar. Mirrors
/// `unkai_caldav::create_event`'s contract so the calling command
/// doesn't branch beyond picking this function.
pub async fn create_event(
    calendar_path: &str,
    ics: &str,
) -> Result<CaldavWriteOutcome, UnkaiError> {
    let calendar_id = calendar_path
        .strip_prefix(CALENDAR_PREFIX)
        .ok_or_else(|| UnkaiError::Other(format!("'{calendar_path}' is not a system calendar")))?
        .to_string();
    let input = to_event_input(&event_from_ics(ics)?);

    let reference = on_blocking(move || unkai_system::create_event(&calendar_id, &input)).await?;
    Ok(CaldavWriteOutcome {
        href: format!("{EVENT_PREFIX}{}", reference.id),
        etag: etag_for(reference.last_modified),
    })
}

pub async fn update_event(href: &str, ics: &str) -> Result<CaldavWriteOutcome, UnkaiError> {
    let key = occurrence_key_from_href(href)?.to_string();
    let input = to_event_input(&event_from_ics(ics)?);

    let reference = on_blocking(move || unkai_system::update_event(&key, &input)).await?;
    Ok(CaldavWriteOutcome {
        href: format!("{EVENT_PREFIX}{}", reference.id),
        etag: etag_for(reference.last_modified),
    })
}

pub async fn delete_event(href: &str) -> Result<(), UnkaiError> {
    let key = occurrence_key_from_href(href)?.to_string();
    on_blocking(move || unkai_system::delete_event(&key)).await
}

/* ── Contacts ────────────────────────────────────────────────────*/

/// Replace the cached system address book with what the device holds.
pub async fn sync_contacts(nc_id: &str, cache: &Cache) -> Result<SyncContactsReport, UnkaiError> {
    let mut report = SyncContactsReport {
        nc_account_id: nc_id.to_string(),
        books_synced: 0,
        upserted: 0,
        deleted: 0,
        errors: Vec::new(),
    };

    let contacts = on_blocking(unkai_system::list_contacts).await?;

    // Reuse the CardDAV vCard parser rather than mapping Contacts
    // properties by hand: same parser, same edge cases, same bugs
    // fixed once (#413 follow-up).
    let upserts: Vec<_> = contacts
        .iter()
        .filter_map(|c| {
            let href = format!("{CONTACT_PREFIX}{}", c.id);
            match unkai_carddav::raw_contact_from_vcard(&href, &etag_for(None), &c.vcard) {
                Some(raw) => Some(raw_contact_to_row(&raw)),
                None => {
                    tracing::warn!("skipping an unparseable system vCard ({})", c.id);
                    None
                }
            }
        })
        .collect();

    let live: HashSet<&str> = upserts.iter().map(|r| r.href.as_str()).collect();
    let deleted: Vec<String> = cache
        .list_contact_hrefs(nc_id, ADDRESSBOOK)?
        .into_iter()
        .filter(|href| !live.contains(href.as_str()))
        .collect();

    cache.apply_contact_delta(
        nc_id,
        ADDRESSBOOK,
        Some("Device"),
        &upserts,
        &deleted,
        None,
        None,
    )?;

    report.books_synced = 1;
    report.upserted = upserts.len() as u32;
    report.deleted = deleted.len() as u32;
    Ok(report)
}

/// The addressbook name a system source's contacts live under —
/// the contacts command layer needs it when it writes a new card.
pub fn addressbook_name() -> &'static str {
    ADDRESSBOOK
}

pub async fn create_contact(vcard: &str) -> Result<CarddavWriteOutcome, UnkaiError> {
    let vcard = vcard.to_string();
    let id = on_blocking(move || unkai_system::create_contact(&vcard)).await?;
    Ok(CarddavWriteOutcome {
        href: format!("{CONTACT_PREFIX}{id}"),
        etag: etag_for(None),
    })
}

pub async fn update_contact(href: &str, vcard: &str) -> Result<CarddavWriteOutcome, UnkaiError> {
    let id = contact_id_from_href(href)?.to_string();
    let vcard = vcard.to_string();
    on_blocking(move || unkai_system::update_contact(&id, &vcard)).await?;
    Ok(CarddavWriteOutcome {
        href: href.to_string(),
        etag: etag_for(None),
    })
}

pub async fn delete_contact(href: &str) -> Result<(), UnkaiError> {
    let id = contact_id_from_href(href)?.to_string();
    on_blocking(move || unkai_system::delete_contact(&id)).await
}

/// The Contacts identifier inside a system contact href, if it is
/// one. Used after a create so the cached row is keyed by the
/// identifier the next sync will report, not by the placeholder UID
/// the vCard builder minted.
pub fn contact_uid_from_href(href: &str) -> Option<&str> {
    href.strip_prefix(CONTACT_PREFIX)
}

/// Same, for a reminder.
pub fn reminder_uid_from_href(href: &str) -> Option<&str> {
    href.strip_prefix(REMINDER_PREFIX)
}

fn contact_id_from_href(href: &str) -> Result<&str, UnkaiError> {
    href.strip_prefix(CONTACT_PREFIX)
        .ok_or_else(|| UnkaiError::Other(format!("'{href}' is not a system-contacts reference")))
}

/* ── Reminders (the Tasks tab) ───────────────────────────────────*/

/// Refresh the reminder lists themselves.
pub async fn sync_task_lists(nc_id: &str, cache: &Cache) -> Result<Vec<TaskList>, UnkaiError> {
    let lists = on_blocking(unkai_system::list_reminder_lists).await?;
    let rows: Vec<TaskList> = lists
        .iter()
        .map(|l| TaskList {
            id: format!("{nc_id}::{REMINDER_LIST_PREFIX}{}", l.id),
            nextcloud_account_id: nc_id.to_string(),
            path: format!("{REMINDER_LIST_PREFIX}{}", l.id),
            name: l.id.clone(),
            display_name: l.title.clone(),
            color: l.color.clone(),
            read_only: l.read_only,
            hidden: false,
            muted: false,
        })
        .collect();
    cache.apply_task_lists_delta(nc_id, &rows)?;
    Ok(cache
        .list_task_lists(nc_id)?
        .into_iter()
        .map(|c| c.list)
        .collect())
}

/// Refresh the reminders inside every list of this source.
pub async fn sync_tasks(nc_id: &str, cache: &Cache) -> Result<Vec<Task>, UnkaiError> {
    let reminders = on_blocking(unkai_system::list_reminders).await?;

    let mut by_list: HashMap<String, Vec<SystemReminder>> = HashMap::new();
    for reminder in reminders {
        by_list
            .entry(reminder.list_id.clone())
            .or_default()
            .push(reminder);
    }

    let cached_lists = cache.list_task_lists(nc_id)?;
    for cached in &cached_lists {
        let Some(list_key) = cached.list.path.strip_prefix(REMINDER_LIST_PREFIX) else {
            continue;
        };
        let fresh = by_list.remove(list_key).unwrap_or_default();
        let tasks: Vec<Task> = fresh.iter().map(|r| to_task(&cached.list.id, r)).collect();

        let live: HashSet<&str> = tasks.iter().map(|t| t.href.as_str()).collect();
        let deleted: Vec<String> = cache
            .list_tasks_for_account(nc_id)?
            .into_iter()
            .filter(|t| t.task_list_id == cached.list.id && !live.contains(t.href.as_str()))
            .map(|t| t.href)
            .collect();

        if let Err(e) = cache.apply_tasks_delta(&cached.list.id, &tasks, &deleted, None) {
            tracing::warn!(
                "system reminder list '{}' failed to cache: {e}",
                cached.list.display_name
            );
        }
    }

    cache.list_tasks_for_account(nc_id).map_err(Into::into)
}

fn to_task(list_id: &str, reminder: &SystemReminder) -> Task {
    let mut task = Task {
        uid: reminder.id.clone(),
        task_list_id: list_id.to_string(),
        href: format!("{REMINDER_PREFIX}{}", reminder.id),
        etag: etag_for(reminder.last_modified),
        summary: reminder.title.clone(),
        description: reminder.notes.clone(),
        status: if reminder.completed {
            "COMPLETED".to_string()
        } else {
            "NEEDS-ACTION".to_string()
        },
        priority: reminder.priority,
        due: reminder.due,
        completed: reminder.completed_at,
        created: reminder.created,
        last_modified: reminder.last_modified,
        url: reminder.url.clone(),
        categories: Vec::new(),
        ics_raw: String::new(),
    };
    // The cache keeps the VTODO body so a future parser can re-read
    // fields we don't surface yet; synthesising it keeps a system
    // task shaped exactly like a CalDAV one on disk.
    task.ics_raw = caldav_build_vtodo_ics(&task, None);
    task
}

fn to_reminder_input(task: &Task) -> SystemReminderInput {
    SystemReminderInput {
        title: task.summary.clone(),
        notes: task.description.clone(),
        url: task.url.clone(),
        due: task.due,
        completed: task.status == "COMPLETED",
        priority: task.priority,
    }
}

pub async fn create_task(list_path: &str, task: &Task) -> Result<TaskWriteOutcome, UnkaiError> {
    let list_id = list_path
        .strip_prefix(REMINDER_LIST_PREFIX)
        .ok_or_else(|| UnkaiError::Other(format!("'{list_path}' is not a system reminder list")))?
        .to_string();
    let input = to_reminder_input(task);

    let reference = on_blocking(move || unkai_system::create_reminder(&list_id, &input)).await?;
    Ok(TaskWriteOutcome {
        href: format!("{REMINDER_PREFIX}{}", reference.id),
        etag: etag_for(reference.last_modified),
    })
}

pub async fn update_task(href: &str, task: &Task) -> Result<TaskWriteOutcome, UnkaiError> {
    let id = reminder_id_from_href(href)?.to_string();
    let input = to_reminder_input(task);

    let reference = on_blocking(move || unkai_system::update_reminder(&id, &input)).await?;
    Ok(TaskWriteOutcome {
        href: href.to_string(),
        etag: etag_for(reference.last_modified),
    })
}

pub async fn delete_task(href: &str) -> Result<(), UnkaiError> {
    let id = reminder_id_from_href(href)?.to_string();
    on_blocking(move || unkai_system::delete_reminder(&id)).await
}

fn reminder_id_from_href(href: &str) -> Result<&str, UnkaiError> {
    href.strip_prefix(REMINDER_PREFIX)
        .ok_or_else(|| UnkaiError::Other(format!("'{href}' is not a system-reminders reference")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(start: &str, end: &str) -> CalendarEvent {
        CalendarEvent {
            id: "x".into(),
            summary: "Standup".into(),
            description: None,
            start: start.parse().unwrap(),
            end: end.parse().unwrap(),
            location: None,
            rrule: None,
            rdate: vec![],
            exdate: vec![],
            recurrence_id: None,
            url: None,
            transparency: None,
            attendees: vec![],
            reminders: vec![],
            latitude: None,
            longitude: None,
        }
    }

    #[test]
    fn midnight_to_end_of_day_reads_as_all_day() {
        assert!(is_all_day(&event(
            "2026-03-01T00:00:00Z",
            "2026-03-01T23:59:59Z"
        )));
    }

    #[test]
    fn a_timed_event_does_not() {
        assert!(!is_all_day(&event(
            "2026-03-01T09:00:00Z",
            "2026-03-01T10:00:00Z"
        )));
    }

    #[test]
    fn hrefs_round_trip_to_their_identifiers() {
        assert_eq!(
            occurrence_key_from_href("system-event:ABC#1700000000").unwrap(),
            "ABC#1700000000"
        );
        assert!(occurrence_key_from_href("https://cloud/x.ics").is_err());
        assert_eq!(
            contact_id_from_href("system-contact:ABC:ABPerson").unwrap(),
            "ABC:ABPerson"
        );
        assert!(reminder_id_from_href("system-contact:ABC").is_err());
    }

    #[test]
    fn an_etag_changes_with_the_modification_time() {
        let t1 = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let t2 = DateTime::from_timestamp(1_700_000_001, 0).unwrap();
        assert_ne!(etag_for(Some(t1)), etag_for(Some(t2)));
        assert_eq!(etag_for(Some(t1)), etag_for(Some(t1)));
    }
}
