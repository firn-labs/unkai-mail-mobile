//! The device's own groupware databases, as plain Rust data.
//!
//! Unkai's calendar, contact and task views were built against
//! CalDAV / CardDAV: a *source* has collections, a collection has
//! items, and every item round-trips through iCalendar / vCard text.
//! The phone's own databases — EventKit for calendars and reminders,
//! Contacts for the address book — hold exactly the same information
//! behind a different API.
//!
//! This crate is the adapter between the two, and it deliberately
//! speaks the DAV side's vocabulary:
//!
//!   * a [`SystemCalendar`] looks like a discovered CalDAV collection,
//!   * a [`SystemContact`] carries **vCard text**, because the
//!     Contacts framework can serialise to and from vCard itself
//!     (`CNContactVCardSerialization`) and the existing parser in
//!     `unkai-carddav` is then reused verbatim,
//!   * writes take the same shape as a CalDAV `PUT` and answer with
//!     a href/etag-like [`SystemRef`].
//!
//! Nothing Objective-C escapes: every function takes and returns
//! owned Rust values, so callers need no `unsafe`, no autorelease
//! pool and no thread affinity of their own.
//!
//! # Permissions
//!
//! iOS gates all three databases behind a user prompt, and the prompt
//! only ever appears **once** per install. [`request_access`] is
//! therefore the first call any caller makes; [`access_status`] is
//! the cheap check afterwards. The matching `NS*UsageDescription`
//! keys have to be in the app's `Info.plist` or the process is
//! terminated by the OS on the first request — see
//! `src-tauri/Info.ios.plist`.
//!
//! # Threading
//!
//! Every function here is synchronous and may block for as long as
//! the system database takes (a permission prompt: until the user
//! answers). Call them from `tokio::task::spawn_blocking`, never
//! from an async task directly.

use chrono::{DateTime, Utc};

#[cfg(target_vendor = "apple")]
mod apple;
#[cfg(target_vendor = "apple")]
use apple as backend;

#[cfg(not(target_vendor = "apple"))]
mod unsupported;
#[cfg(not(target_vendor = "apple"))]
use unsupported as backend;

/// What went wrong talking to a system database.
#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    /// This platform has no system groupware bridge (everything that
    /// isn't iOS / macOS today). Callers turn this into "the System
    /// source isn't available here" rather than an error banner.
    #[error("the system {0} database is not available on this platform")]
    Unsupported(SystemEntity),
    /// The user hasn't granted access — or has actively denied it in
    /// Settings. Distinguished from a hard failure because the fix is
    /// a trip to the Settings app, not a retry.
    #[error("access to the system {0} database was not granted")]
    AccessDenied(SystemEntity),
    /// The item the caller addressed is gone — deleted in the Apple
    /// app between our last sync and this write.
    #[error("{0} not found in the system database")]
    NotFound(String),
    /// Anything the framework itself reported.
    #[error("{0}")]
    Backend(String),
}

/// Which system database a call is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemEntity {
    /// EventKit's `EKEntityTypeEvent` — the Calendar app's data.
    Calendars,
    /// EventKit's `EKEntityTypeReminder` — the Reminders app's data.
    /// A *separate* permission from calendars, even though both live
    /// behind `EKEventStore`.
    Reminders,
    /// The Contacts framework — the Contacts app's data.
    Contacts,
}

impl std::fmt::Display for SystemEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SystemEntity::Calendars => "calendar",
            SystemEntity::Reminders => "reminders",
            SystemEntity::Contacts => "contacts",
        })
    }
}

/// How far the user has let us into one database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    /// Never asked. The next [`request_access`] shows the OS prompt.
    NotDetermined,
    /// Denied, or blocked by a device restriction (parental controls,
    /// MDM). Asking again does nothing — only the Settings app can
    /// change it, which is why the UI offers a deep link there.
    Denied,
    /// Full read/write.
    Granted,
    /// iOS 17's "add events but don't read them" grade. We treat it
    /// as insufficient: a calendar the app cannot read is a calendar
    /// it cannot show, and showing an empty one would be a lie.
    WriteOnly,
}

impl PermissionState {
    /// Whether we can actually read and write this database.
    pub fn is_usable(self) -> bool {
        matches!(self, PermissionState::Granted)
    }
}

/// One calendar (or reminder list) in the system database. Shaped
/// like a CalDAV collection discovered by PROPFIND, because that is
/// what the cache stores.
#[derive(Debug, Clone)]
pub struct SystemCalendar {
    /// EventKit's `calendarIdentifier`. Stable for the life of the
    /// calendar on the device, which makes it our cache key.
    pub id: String,
    /// The name the user sees in the Calendar app.
    pub title: String,
    /// True for subscribed / delegated calendars EventKit refuses
    /// writes on. Mirrors CalDAV's read-only privilege verdict, and
    /// the editor hides its save button off exactly the same flag.
    pub read_only: bool,
    /// The colour the Calendar (or Reminders) app paints this
    /// collection in, as `#rrggbb`. Same shape CalDAV's
    /// `calendar-color` property arrives in, so the cache column and
    /// every screen reading it need no special case.
    ///
    /// `None` when EventKit has no colour for the calendar — a
    /// freshly created one, or a non-Apple platform.
    pub color: Option<String>,
}

/// One event occurrence, flattened.
///
/// EventKit answers a date-range query with *occurrences*, not with
/// a master plus an RRULE — so that's what we store: one row per
/// occurrence, no recurrence rule. It costs cache rows for a long
/// daily series and buys exactness: detached and cancelled
/// occurrences land in the app looking precisely as they do in the
/// Calendar app, with no RRULE re-implementation in between.
#[derive(Debug, Clone)]
pub struct SystemEvent {
    /// EventKit's `eventIdentifier`. **Shared by every occurrence of
    /// a recurring event**, which is why it alone can't identify a
    /// row — see [`SystemEvent::occurrence_key`].
    pub event_id: String,
    /// `calendarIdentifier` of the calendar it lives in.
    pub calendar_id: String,
    pub title: String,
    pub notes: Option<String>,
    pub location: Option<String>,
    pub url: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub all_day: bool,
    /// `lastModifiedDate`, used as the etag equivalent so an edit
    /// made in the Calendar app invalidates our cached copy.
    pub last_modified: Option<DateTime<Utc>>,
    /// Minutes before the start each alarm fires. Absolute-date
    /// alarms are converted to a relative lead time; geofenced ones
    /// are dropped (iCalendar has no portable equivalent, and the
    /// app has no location-triggered reminder surface).
    pub alarms_minutes_before: Vec<i64>,
    pub attendees: Vec<SystemAttendee>,
    /// The organiser's email, when EventKit knows one.
    pub organizer_email: Option<String>,
    /// `EKEventStatus == Canceled`.
    pub cancelled: bool,
}

impl SystemEvent {
    /// The identity of one *occurrence*: the event id plus its start.
    ///
    /// Recurring occurrences share an `eventIdentifier`, so the start
    /// instant is what tells them apart — and it is also what
    /// [`update_event`] / [`delete_event`] need to find the right one
    /// again later.
    pub fn occurrence_key(&self) -> String {
        occurrence_key(&self.event_id, self.start)
    }
}

/// Build the occurrence key for an event id + start instant.
pub fn occurrence_key(event_id: &str, start: DateTime<Utc>) -> String {
    format!("{event_id}#{}", start.timestamp())
}

/// Split an occurrence key back into its two halves.
pub fn parse_occurrence_key(key: &str) -> Option<(String, DateTime<Utc>)> {
    let (id, epoch) = key.rsplit_once('#')?;
    let secs: i64 = epoch.parse().ok()?;
    Some((id.to_string(), DateTime::from_timestamp(secs, 0)?))
}

/// An attendee as EventKit models it.
#[derive(Debug, Clone)]
pub struct SystemAttendee {
    /// Parsed out of the participant's `mailto:` URL. EventKit has no
    /// plain email property — the URL *is* the address.
    pub email: String,
    pub name: Option<String>,
    /// iCalendar `PARTSTAT` token (`ACCEPTED`, `DECLINED`, …).
    pub partstat: String,
    /// iCalendar `ROLE` token (`REQ-PARTICIPANT`, `CHAIR`, …).
    pub role: String,
}

/// What a caller wants an event to look like after a write.
#[derive(Debug, Clone, Default)]
pub struct SystemEventInput {
    pub title: String,
    pub notes: Option<String>,
    pub location: Option<String>,
    pub url: Option<String>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub alarms_minutes_before: Vec<i64>,
}

/// What a write left behind — the system's answer to a CalDAV
/// `PUT`'s href + etag.
#[derive(Debug, Clone)]
pub struct SystemRef {
    /// The identifier the item now has. For a created event this is
    /// the freshly minted `eventIdentifier`.
    pub id: String,
    /// Its revision marker, as far as the system exposes one.
    pub last_modified: Option<DateTime<Utc>>,
}

/// One address-book entry, carried as vCard text.
///
/// Serialising through `CNContactVCardSerialization` rather than
/// mapping ~20 Contacts properties by hand means the system address
/// book reaches the app through the *same* parser every CardDAV
/// server does — one code path, one set of edge cases, one place
/// where a photo or a labelled phone number can go wrong.
#[derive(Debug, Clone)]
pub struct SystemContact {
    /// `CNContact.identifier` — our cache key.
    pub id: String,
    /// The vCard as the Contacts framework wrote it, with `UID:`
    /// forced to `id` so the app-side identity survives a round trip.
    pub vcard: String,
}

/// One reminder, shaped like the `Task` the app already renders.
#[derive(Debug, Clone)]
pub struct SystemReminder {
    /// `calendarItemIdentifier` — unlike events, reminders have no
    /// occurrence problem, so this identifies the row on its own.
    pub id: String,
    /// `calendarIdentifier` of the list it belongs to.
    pub list_id: String,
    pub title: String,
    pub notes: Option<String>,
    pub url: Option<String>,
    pub completed: bool,
    pub completed_at: Option<DateTime<Utc>>,
    pub due: Option<DateTime<Utc>>,
    pub created: Option<DateTime<Utc>>,
    pub last_modified: Option<DateTime<Utc>>,
    /// Reminders' 0–9 priority scale is iCalendar's, unchanged.
    pub priority: u8,
}

/// What a caller wants a reminder to look like after a write.
#[derive(Debug, Clone, Default)]
pub struct SystemReminderInput {
    pub title: String,
    pub notes: Option<String>,
    pub url: Option<String>,
    pub due: Option<DateTime<Utc>>,
    pub completed: bool,
    pub priority: u8,
}

/* ── Permissions ─────────────────────────────────────────────────
 *
 * `access_status` is a pure lookup and cheap enough to call on every
 * render; `request_access` may block for as long as the user takes
 * to answer the prompt. */

/// Current permission grade for one database, without prompting.
pub fn access_status(entity: SystemEntity) -> PermissionState {
    backend::access_status(entity)
}

/// Ask for access, showing the OS prompt if it hasn't been shown yet.
///
/// Blocks until the user answers. Returns the resulting state rather
/// than a bare bool so a caller can tell "denied just now" from
/// "denied long ago in Settings" and word its message accordingly.
pub fn request_access(entity: SystemEntity) -> Result<PermissionState, SystemError> {
    backend::request_access(entity)
}

/* ── Calendars ───────────────────────────────────────────────────*/

/// Every calendar the user has in the Calendar app.
pub fn list_calendars() -> Result<Vec<SystemCalendar>, SystemError> {
    backend::list_calendars()
}

/// Every occurrence between `start` and `end`, across all calendars.
///
/// EventKit caps a single query at four years; longer ranges are
/// silently truncated by the framework, so callers should stay well
/// inside that.
pub fn list_events(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<SystemEvent>, SystemError> {
    backend::list_events(start, end)
}

/// Add an event to `calendar_id`.
pub fn create_event(calendar_id: &str, input: &SystemEventInput) -> Result<SystemRef, SystemError> {
    backend::create_event(calendar_id, input)
}

/// Rewrite one occurrence, addressed the way [`SystemEvent::occurrence_key`]
/// spells it. Only that occurrence changes — editing a Tuesday does
/// not move the whole series, which is what `EKSpanThisEvent` means
/// and what a user tapping one day expects.
pub fn update_event(
    occurrence_key: &str,
    input: &SystemEventInput,
) -> Result<SystemRef, SystemError> {
    backend::update_event(occurrence_key, input)
}

/// Remove one occurrence.
pub fn delete_event(occurrence_key: &str) -> Result<(), SystemError> {
    backend::delete_event(occurrence_key)
}

/* ── Contacts ────────────────────────────────────────────────────*/

/// The whole system address book as vCards.
pub fn list_contacts() -> Result<Vec<SystemContact>, SystemError> {
    backend::list_contacts()
}

/// Add a contact from vCard text. Returns its new identifier.
pub fn create_contact(vcard: &str) -> Result<String, SystemError> {
    backend::create_contact(vcard)
}

/// Overwrite an existing contact's fields from vCard text.
pub fn update_contact(id: &str, vcard: &str) -> Result<(), SystemError> {
    backend::update_contact(id, vcard)
}

/// Delete a contact.
pub fn delete_contact(id: &str) -> Result<(), SystemError> {
    backend::delete_contact(id)
}

/* ── Reminders ───────────────────────────────────────────────────*/

/// Every reminder list in the Reminders app.
pub fn list_reminder_lists() -> Result<Vec<SystemCalendar>, SystemError> {
    backend::list_reminder_lists()
}

/// Every reminder, complete and incomplete, across all lists.
pub fn list_reminders() -> Result<Vec<SystemReminder>, SystemError> {
    backend::list_reminders()
}

/// Add a reminder to `list_id`.
pub fn create_reminder(
    list_id: &str,
    input: &SystemReminderInput,
) -> Result<SystemRef, SystemError> {
    backend::create_reminder(list_id, input)
}

/// Rewrite a reminder.
pub fn update_reminder(id: &str, input: &SystemReminderInput) -> Result<SystemRef, SystemError> {
    backend::update_reminder(id, input)
}

/// Delete a reminder.
pub fn delete_reminder(id: &str) -> Result<(), SystemError> {
    backend::delete_reminder(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occurrence_key_round_trips() {
        let start = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let key = occurrence_key("ABC-123", start);
        assert_eq!(key, "ABC-123#1700000000");
        let (id, parsed) = parse_occurrence_key(&key).unwrap();
        assert_eq!(id, "ABC-123");
        assert_eq!(parsed, start);
    }

    #[test]
    fn occurrence_key_survives_hashes_in_the_identifier() {
        // EventKit identifiers are opaque; nothing forbids a '#'.
        // Splitting from the right keeps the epoch unambiguous.
        let start = DateTime::from_timestamp(42, 0).unwrap();
        let (id, parsed) = parse_occurrence_key(&occurrence_key("we#ird", start)).unwrap();
        assert_eq!(id, "we#ird");
        assert_eq!(parsed, start);
    }

    #[test]
    fn malformed_occurrence_keys_are_rejected() {
        assert!(parse_occurrence_key("no-separator").is_none());
        assert!(parse_occurrence_key("id#not-a-number").is_none());
    }
}
