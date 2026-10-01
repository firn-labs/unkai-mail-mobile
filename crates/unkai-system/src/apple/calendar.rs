//! EventKit's calendar half.

use chrono::{DateTime, Utc};
use objc2::rc::Retained;
use objc2_core_graphics::CGColor;
use objc2_event_kit::{
    EKAlarm, EKCalendar, EKEntityType, EKEvent, EKEventStatus, EKEventStore, EKParticipant,
    EKParticipantRole, EKParticipantStatus, EKSpan,
};
use objc2_foundation::{NSArray, NSDate, NSURL};
// CoreGraphics' own float width — `f64` on 64-bit, which every
// target this builds for is, but naming the alias keeps the
// conversion honest if that ever stops being true.
use objc2_core_foundation::CGFloat;

use super::{
    backend_error, from_nsdate, local_date_parts, local_instant, nsstring, opt_from_nsdate,
    opt_string, require_access, to_nsdate,
};
use crate::{
    SystemAttendee, SystemCalendar, SystemEntity, SystemError, SystemEvent, SystemEventInput,
    SystemRef, occurrence_key, parse_occurrence_key,
};

pub fn list_calendars() -> Result<Vec<SystemCalendar>, SystemError> {
    require_access(SystemEntity::Calendars)?;
    let store = unsafe { EKEventStore::new() };
    let calendars = unsafe { store.calendarsForEntityType(EKEntityType::Event) };
    Ok(calendars
        .to_vec()
        .iter()
        .map(|c| to_system_calendar(c))
        .collect())
}

pub(super) fn to_system_calendar(calendar: &EKCalendar) -> SystemCalendar {
    SystemCalendar {
        id: unsafe { calendar.calendarIdentifier() }.to_string(),
        title: unsafe { calendar.title() }.to_string(),
        // `allowsContentModifications` is EventKit's own verdict and
        // covers both cases CalDAV needs two probes for: a subscribed
        // (read-only by nature) calendar and one shared with
        // view-only rights.
        read_only: !unsafe { calendar.allowsContentModifications() },
        color: calendar_hex(calendar),
    }
}

/// `EKCalendar.CGColor` as `#rrggbb`.
///
/// EventKit hands out a `CGColorRef` rather than a hex string, so we
/// read its components directly. A `CGColor` carries however many
/// components its colour space has, which for the calendars iOS
/// actually creates is either RGBA (4) or grayscale + alpha (2) — a
/// grey calendar would otherwise come out as pure red if we blindly
/// took the first three. Anything else is left as `None`: the app
/// falls back to its own palette, which is a better answer than a
/// wrong colour.
///
/// Alpha is dropped on purpose. The value lands in the same cache
/// column CalDAV's `calendar-color` fills, and every screen reading
/// it expects an opaque `#rrggbb`.
fn calendar_hex(calendar: &EKCalendar) -> Option<String> {
    let color = unsafe { calendar.CGColor() }?;
    let count = CGColor::number_of_components(Some(&color));
    // SAFETY: `components` points into the colour for as long as we
    // hold `color`, and `count` is the framework's own length for it.
    let components = CGColor::components(Some(&color));
    if components.is_null() {
        return None;
    }
    let at = |i: usize| unsafe { *components.add(i) };
    let (r, g, b) = match count {
        // Grayscale + alpha: one channel painted on all three.
        2 => (at(0), at(0), at(0)),
        3 | 4 => (at(0), at(1), at(2)),
        _ => return None,
    };
    Some(format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b)))
}

/// A 0.0…1.0 CoreGraphics component as an 0…255 channel, clamped —
/// wide-gamut colour spaces can hand back values slightly outside
/// the unit range.
fn byte(component: CGFloat) -> u8 {
    (component.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub fn list_events(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<SystemEvent>, SystemError> {
    require_access(SystemEntity::Calendars)?;
    let store = unsafe { EKEventStore::new() };

    // `nil` calendars means "every calendar", which is what we want:
    // per-calendar visibility is the app's own decision, made later
    // against the cached rows, so hiding one here would also hide it
    // from a user who re-enables it without a re-sync.
    let predicate = unsafe {
        store.predicateForEventsWithStartDate_endDate_calendars(
            &to_nsdate(start),
            &to_nsdate(end),
            None,
        )
    };
    let events = unsafe { store.eventsMatchingPredicate(&predicate) };

    Ok(events
        .to_vec()
        .iter()
        .filter_map(|e| to_system_event(e))
        .collect())
}

/// Convert one `EKEvent` occurrence.
///
/// Returns `None` for the two cases we can't store: an event whose
/// calendar has gone away underneath us, and one EventKit hasn't
/// given an identifier yet (an unsaved draft — it happens when the
/// Calendar app is mid-edit).
fn to_system_event(event: &EKEvent) -> Option<SystemEvent> {
    let event_id = unsafe { event.eventIdentifier() }?.to_string();
    let calendar = unsafe { event.calendar() }?;
    let all_day = unsafe { event.isAllDay() };
    let (start, end) = if all_day {
        // EventKit anchors an all-day event to local midnight; the app
        // anchors one to UTC midnight. Re-anchor by *date*, not by
        // instant — the two differ by a whole day west of Greenwich.
        (
            utc_day_start(&*unsafe { event.startDate() })?,
            utc_day_end(&*unsafe { event.endDate() })?,
        )
    } else {
        (
            from_nsdate(&*unsafe { event.startDate() })?,
            from_nsdate(&*unsafe { event.endDate() })?,
        )
    };

    Some(SystemEvent {
        event_id,
        calendar_id: unsafe { calendar.calendarIdentifier() }.to_string(),
        title: unsafe { event.title() }.to_string(),
        notes: opt_string(unsafe { event.notes() }),
        location: opt_string(unsafe { event.location() }),
        url: unsafe { event.URL() }
            .and_then(|u| u.absoluteString())
            .map(|s| s.to_string()),
        start,
        end,
        all_day,
        last_modified: opt_from_nsdate(unsafe { event.lastModifiedDate() }),
        alarms_minutes_before: alarm_lead_times(event, start),
        attendees: attendees(event),
        organizer_email: unsafe { event.organizer() }.and_then(|o| participant_email(&o)),
        cancelled: unsafe { event.status() } == EKEventStatus::Canceled,
    })
}

/// The local date of `date`, as UTC midnight — the app's all-day start.
fn utc_day_start(date: &NSDate) -> Option<DateTime<Utc>> {
    let (year, month, day) = local_date_parts(date);
    chrono::NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32)?
        .and_hms_opt(0, 0, 0)
        .map(|naive| naive.and_utc())
}

/// The local date of `date`, as UTC 23:59:59 — the app's all-day end.
fn utc_day_end(date: &NSDate) -> Option<DateTime<Utc>> {
    let (year, month, day) = local_date_parts(date);
    chrono::NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32)?
        .and_hms_opt(23, 59, 59)
        .map(|naive| naive.and_utc())
}

/// The inverse: the app's UTC-anchored all-day instant as the local
/// wall-clock instant EventKit expects.
fn local_all_day(value: DateTime<Utc>, time: (isize, isize, isize)) -> Option<Retained<NSDate>> {
    use chrono::Datelike;
    local_instant(
        (
            value.year() as isize,
            value.month() as isize,
            value.day() as isize,
        ),
        time,
    )
}

/// Alarms as "minutes before the start", which is the only form the
/// app's event editor offers.
///
/// EventKit models an alarm either relatively (seconds before, as a
/// negative offset) or absolutely (a wall-clock instant). We fold the
/// absolute kind into the relative one against this occurrence's
/// start; geofenced alarms have no lead time at all and are dropped.
fn alarm_lead_times(event: &EKEvent, start: DateTime<Utc>) -> Vec<i64> {
    let Some(alarms) = (unsafe { event.alarms() }) else {
        return Vec::new();
    };
    alarms
        .to_vec()
        .iter()
        .filter_map(|alarm: &Retained<EKAlarm>| {
            if let Some(absolute) = opt_from_nsdate(unsafe { alarm.absoluteDate() }) {
                let lead = (start - absolute).num_minutes();
                return (lead >= 0).then_some(lead);
            }
            let offset = unsafe { alarm.relativeOffset() };
            // A positive offset means "after the start" — a nudge for
            // a meeting already underway. iCalendar can express it,
            // the app's editor can't, so it isn't carried over.
            (offset < 0.0).then(|| (-offset / 60.0).round() as i64)
        })
        .collect()
}

fn attendees(event: &EKEvent) -> Vec<SystemAttendee> {
    let Some(list) = (unsafe { event.attendees() }) else {
        return Vec::new();
    };
    list.to_vec()
        .iter()
        .filter_map(|p| {
            let email = participant_email(p)?;
            Some(SystemAttendee {
                email,
                name: opt_string(unsafe { p.name() }),
                partstat: partstat_token(unsafe { p.participantStatus() }).to_string(),
                role: role_token(unsafe { p.participantRole() }).to_string(),
            })
        })
        .collect()
}

/// EventKit exposes no email property on a participant — the address
/// lives in its `mailto:` URL, and that is the documented way to get
/// at it.
fn participant_email(participant: &EKParticipant) -> Option<String> {
    let url = unsafe { participant.URL() };
    let text = url.absoluteString()?.to_string();
    let address = text.strip_prefix("mailto:").unwrap_or(&text).trim();
    (!address.is_empty() && address.contains('@')).then(|| address.to_string())
}

fn partstat_token(status: EKParticipantStatus) -> &'static str {
    match status {
        EKParticipantStatus::Accepted => "ACCEPTED",
        EKParticipantStatus::Declined => "DECLINED",
        EKParticipantStatus::Tentative => "TENTATIVE",
        EKParticipantStatus::Delegated => "DELEGATED",
        EKParticipantStatus::Completed => "COMPLETED",
        EKParticipantStatus::InProcess => "IN-PROCESS",
        _ => "NEEDS-ACTION",
    }
}

fn role_token(role: EKParticipantRole) -> &'static str {
    match role {
        EKParticipantRole::Optional => "OPT-PARTICIPANT",
        EKParticipantRole::Chair => "CHAIR",
        EKParticipantRole::NonParticipant => "NON-PARTICIPANT",
        _ => "REQ-PARTICIPANT",
    }
}

/* ── Writes ──────────────────────────────────────────────────────*/

pub fn create_event(calendar_id: &str, input: &SystemEventInput) -> Result<SystemRef, SystemError> {
    require_access(SystemEntity::Calendars)?;
    let store = unsafe { EKEventStore::new() };
    let calendar = unsafe { store.calendarWithIdentifier(&nsstring(calendar_id)) }
        .ok_or_else(|| SystemError::NotFound(format!("calendar '{calendar_id}'")))?;
    if !unsafe { calendar.allowsContentModifications() } {
        return Err(SystemError::Backend(format!(
            "the system calendar '{}' is read-only",
            unsafe { calendar.title() }
        )));
    }

    let event = unsafe { EKEvent::eventWithEventStore(&store) };
    unsafe { event.setCalendar(Some(&calendar)) };
    apply_input(&event, input)?;

    unsafe { store.saveEvent_span_error(&event, EKSpan::ThisEvent) }.map_err(backend_error)?;

    let id = unsafe { event.eventIdentifier() }
        .ok_or_else(|| SystemError::Backend("the saved event has no identifier".into()))?
        .to_string();
    Ok(SystemRef {
        id: occurrence_key(&id, input.start.unwrap_or_else(Utc::now)),
        last_modified: opt_from_nsdate(unsafe { event.lastModifiedDate() }),
    })
}

pub fn update_event(key: &str, input: &SystemEventInput) -> Result<SystemRef, SystemError> {
    require_access(SystemEntity::Calendars)?;
    let store = unsafe { EKEventStore::new() };
    let event = find_occurrence(&store, key)?;
    apply_input(&event, input)?;

    // `EKSpanThisEvent` detaches this occurrence from its series
    // rather than rewriting every future one. That matches what a
    // user tapping a single day means, and it's also the only span
    // the app's editor can honestly offer: it edits one occurrence's
    // fields, with no UI for "and all following".
    unsafe { store.saveEvent_span_error(&event, EKSpan::ThisEvent) }.map_err(backend_error)?;

    let id = unsafe { event.eventIdentifier() }
        .ok_or_else(|| SystemError::Backend("the saved event has no identifier".into()))?
        .to_string();
    let start = from_nsdate(&*unsafe { event.startDate() }).unwrap_or_else(Utc::now);
    Ok(SystemRef {
        id: occurrence_key(&id, start),
        last_modified: opt_from_nsdate(unsafe { event.lastModifiedDate() }),
    })
}

pub fn delete_event(key: &str) -> Result<(), SystemError> {
    require_access(SystemEntity::Calendars)?;
    let store = unsafe { EKEventStore::new() };
    let event = find_occurrence(&store, key)?;
    unsafe { store.removeEvent_span_error(&event, EKSpan::ThisEvent) }.map_err(backend_error)
}

fn apply_input(event: &EKEvent, input: &SystemEventInput) -> Result<(), SystemError> {
    unsafe {
        event.setTitle(Some(&nsstring(&input.title)));
        event.setNotes(input.notes.as_deref().map(nsstring).as_deref());
        event.setLocation(input.location.as_deref().map(nsstring).as_deref());
        event.setURL(
            input
                .url
                .as_deref()
                .and_then(|u| NSURL::URLWithString(&nsstring(u)))
                .as_deref(),
        );
        // Set `allDay` first: EventKit normalises the dates it is
        // handed against the flag that is already set, so assigning
        // the flag afterwards would re-snap dates it had normalised
        // as timed ones.
        event.setAllDay(input.all_day);
        if let Some(start) = input.start {
            let date = if input.all_day {
                local_all_day(start, (0, 0, 0))
            } else {
                Some(to_nsdate(start))
            };
            event.setStartDate(date.as_deref());
        }
        if let Some(end) = input.end {
            let date = if input.all_day {
                local_all_day(end, (23, 59, 59))
            } else {
                Some(to_nsdate(end))
            };
            event.setEndDate(date.as_deref());
        }

        // Alarms are replaced wholesale: the editor shows the full
        // set it's about to save, so anything not in `input` was
        // removed by the user.
        event.setAlarms(Some(&NSArray::from_retained_slice(&[])));
        for minutes in &input.alarms_minutes_before {
            let offset = -(*minutes as f64) * 60.0;
            event.addAlarm(&EKAlarm::alarmWithRelativeOffset(offset));
        }
    }
    Ok(())
}

/// Find the one `EKEvent` an occurrence key points at.
///
/// `eventWithIdentifier:` answers with the *first* occurrence of a
/// series, which is the right answer for a one-off event and the
/// wrong one for the third Tuesday of a standup. So: take that
/// answer when its start matches, and otherwise re-query the day the
/// occurrence falls on and pick the instance with the same identifier
/// and start.
fn find_occurrence(store: &EKEventStore, key: &str) -> Result<Retained<EKEvent>, SystemError> {
    let (event_id, start) = parse_occurrence_key(key)
        .ok_or_else(|| SystemError::NotFound(format!("malformed event reference '{key}'")))?;

    let first = unsafe { store.eventWithIdentifier(&nsstring(&event_id)) };
    if let Some(event) = &first
        && from_nsdate(&*unsafe { event.startDate() }) == Some(start)
    {
        return Ok(event.clone());
    }

    // A day either side covers every timezone the occurrence could
    // have been recorded in without widening the query enough to slow
    // it down.
    let window_start = start - chrono::Duration::days(1);
    let window_end = start + chrono::Duration::days(1);
    let predicate = unsafe {
        store.predicateForEventsWithStartDate_endDate_calendars(
            &to_nsdate(window_start),
            &to_nsdate(window_end),
            None,
        )
    };
    let matches = unsafe { store.eventsMatchingPredicate(&predicate) };
    matches
        .to_vec()
        .into_iter()
        .find(|candidate| {
            unsafe { candidate.eventIdentifier() }.is_some_and(|id| id.to_string() == event_id)
                && from_nsdate(&*unsafe { candidate.startDate() }) == Some(start)
        })
        .ok_or_else(|| {
            SystemError::NotFound(format!(
                "the event '{event_id}' no longer has an occurrence at {start}"
            ))
        })
}
