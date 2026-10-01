//! The Apple (iOS / macOS) implementation of the system bridge.
//!
//! Three frameworks, two permission scopes, one shape of code: open
//! a store, ask it a question, convert the answer to plain Rust,
//! drop the store.
//!
//! ## Why a fresh store per call
//!
//! `EKEventStore` and `CNContactStore` are Objective-C objects, and
//! objc2's `Retained<T>` is neither `Send` nor `Sync` — so a
//! long-lived store can't be parked in a `static` and shared across
//! the async runtime's worker threads. Creating one per call costs a
//! few milliseconds and buys thread-safety we'd otherwise have to
//! build by hand with a dedicated thread and a command channel. Sync
//! runs are minutes apart; the trade is easily worth it.
//!
//! A second, subtler reason: a cached `EKEventStore` serves a
//! *snapshot*. Apple's own guidance is to call `reset` (or use a new
//! store) after an external change, and a store created at the top of
//! each call is that, for free.
//!
//! ## Why everything blocks
//!
//! The two asynchronous APIs here (the permission prompt and the
//! reminders fetch) hand their answer to a completion block on an
//! arbitrary queue. Rather than infect the whole crate with futures,
//! each one parks on a `Condvar` until the block fires. Callers are
//! expected to be inside `spawn_blocking` — which is also true for
//! the synchronous calls, since a large `eventsMatchingPredicate` is
//! not fast either.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_event_kit::{EKAuthorizationStatus, EKEntityType, EKEventStore};
use objc2_foundation::{NSCalendar, NSCalendarUnit, NSDate, NSDateComponents, NSError, NSString};

use crate::{PermissionState, SystemEntity, SystemError};

mod calendar;
mod contacts;
mod reminders;

pub use calendar::{create_event, delete_event, list_calendars, list_events, update_event};
pub use contacts::{create_contact, delete_contact, list_contacts, update_contact};
pub use reminders::{
    create_reminder, delete_reminder, list_reminder_lists, list_reminders, update_reminder,
};

/// How long we wait for a completion block before giving up.
///
/// Only the permission prompt can realistically approach this — it
/// stays on screen until the user answers. Ten minutes is "the user
/// walked away", not "the framework is slow", and returning an error
/// then is better than pinning a runtime thread forever.
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(600);

/* ── Permissions ─────────────────────────────────────────────────*/

impl From<EKAuthorizationStatus> for PermissionState {
    fn from(status: EKAuthorizationStatus) -> Self {
        match status {
            EKAuthorizationStatus::NotDetermined => PermissionState::NotDetermined,
            EKAuthorizationStatus::FullAccess => PermissionState::Granted,
            EKAuthorizationStatus::WriteOnly => PermissionState::WriteOnly,
            // Restricted (device policy) and Denied both mean "not
            // ours to change" — the app's only move is to point at
            // the Settings app, which is the same message either way.
            _ => PermissionState::Denied,
        }
    }
}

pub fn access_status(entity: SystemEntity) -> PermissionState {
    match entity {
        SystemEntity::Calendars => {
            unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) }.into()
        }
        SystemEntity::Reminders => {
            unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Reminder) }.into()
        }
        SystemEntity::Contacts => contacts::access_status(),
    }
}

pub fn request_access(entity: SystemEntity) -> Result<PermissionState, SystemError> {
    // Asking again after a denial does nothing — iOS shows the prompt
    // exactly once per install — so short-circuit rather than let the
    // caller believe a second try might help.
    let current = access_status(entity);
    if current != PermissionState::NotDetermined {
        return Ok(current);
    }

    match entity {
        SystemEntity::Contacts => contacts::request_access()?,
        SystemEntity::Calendars | SystemEntity::Reminders => {
            let store = unsafe { EKEventStore::new() };
            let waiter = Waiter::new();
            let block = waiter.access_block();
            // iOS 17 split the old single "calendars" grant into
            // full-access and write-only. We ask for full access:
            // write-only would let us add events to a calendar we
            // then couldn't display, and a calendar the app can't
            // read is one it can't honestly show.
            unsafe {
                match entity {
                    SystemEntity::Reminders => {
                        store.requestFullAccessToRemindersWithCompletion(block_ptr(&block))
                    }
                    _ => store.requestFullAccessToEventsWithCompletion(block_ptr(&block)),
                }
            }
            waiter.wait(entity)?;
        }
    }

    Ok(access_status(entity))
}

/// Fail early when the caller would otherwise get a confusingly empty
/// answer: without permission every EventKit query returns zero rows
/// rather than an error, which would read to the user as "your
/// calendar is empty".
pub(crate) fn require_access(entity: SystemEntity) -> Result<(), SystemError> {
    if access_status(entity).is_usable() {
        Ok(())
    } else {
        Err(SystemError::AccessDenied(entity))
    }
}

/* ── Completion-block plumbing ───────────────────────────────────*/

/// A one-shot rendezvous between a completion block and the thread
/// that started the call.
///
/// The state has to be `Send + Sync`: the block runs on whichever
/// queue the framework picks. An `Arc<(Mutex, Condvar)>` of plain
/// data is the smallest thing that is.
pub(crate) struct Waiter {
    state: Arc<WaiterState>,
}

/// The slot the block writes into, plus the variable the caller
/// sleeps on. `None` means "not answered yet".
type WaiterState = (Mutex<Option<Result<(), String>>>, Condvar);

impl Waiter {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new((Mutex::new(None), Condvar::new())),
        }
    }

    /// A `(BOOL granted, NSError *error)` completion block that
    /// resolves this waiter.
    pub(crate) fn access_block(&self) -> block2::RcBlock<dyn Fn(Bool, *mut NSError) + 'static> {
        let state = Arc::clone(&self.state);
        block2::RcBlock::new(move |granted: Bool, error: *mut NSError| {
            let outcome = if granted.as_bool() {
                Ok(())
            } else {
                Err(error_message(error).unwrap_or_else(|| "access was not granted".into()))
            };
            let (lock, cv) = &*state;
            if let Ok(mut slot) = lock.lock() {
                *slot = Some(outcome);
            }
            cv.notify_all();
        })
    }

    /// Park until the block fires, or give up.
    pub(crate) fn wait(&self, entity: SystemEntity) -> Result<(), SystemError> {
        let (lock, cv) = &*self.state;
        let guard = lock
            .lock()
            .map_err(|_| SystemError::Backend("permission wait poisoned".into()))?;
        let (guard, timeout) = cv
            .wait_timeout_while(guard, COMPLETION_TIMEOUT, |slot| slot.is_none())
            .map_err(|_| SystemError::Backend("permission wait poisoned".into()))?;
        if timeout.timed_out() {
            return Err(SystemError::Backend(format!(
                "the system {entity} permission prompt was never answered"
            )));
        }
        match guard.clone() {
            Some(Ok(())) => Ok(()),
            // A refusal is not a malfunction: report it as the
            // permission state it is, so the UI offers Settings
            // rather than a retry button.
            Some(Err(_)) => Err(SystemError::AccessDenied(entity)),
            None => Err(SystemError::Backend("permission wait ended early".into())),
        }
    }
}

/// Hand a block to an Objective-C method expecting a raw block
/// pointer. The block stays owned by the caller's `RcBlock`, which
/// must outlive the call — every caller here keeps it in scope until
/// its `Waiter` has resolved.
pub(crate) fn block_ptr<F: ?Sized>(block: &block2::RcBlock<F>) -> *mut block2::DynBlock<F> {
    let ptr: *const block2::DynBlock<F> = &**block;
    ptr.cast_mut()
}

/* ── Small conversions ───────────────────────────────────────────*/

/// `NSError` → a message worth showing, if there is one.
pub(crate) fn error_message(error: *mut NSError) -> Option<String> {
    // SAFETY: the framework hands us either null or a valid, still-live
    // NSError for the duration of the completion block.
    let error = unsafe { error.as_ref() }?;
    Some(error.localizedDescription().to_string())
}

/// `Retained<NSError>` → our error type.
pub(crate) fn backend_error(error: Retained<NSError>) -> SystemError {
    SystemError::Backend(error.localizedDescription().to_string())
}

pub(crate) fn nsstring(value: &str) -> Retained<NSString> {
    NSString::from_str(value)
}

/// `NSString` → `Option<String>`, folding the empty string to `None`.
///
/// Cocoa uses `@""` where Rust would use `None` in about half these
/// properties (`location`, `notes`, `organizationName` …); collapsing
/// the two here keeps every call site from repeating the check.
pub(crate) fn opt_string(value: Option<Retained<NSString>>) -> Option<String> {
    value
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
}

pub(crate) fn to_nsdate(value: DateTime<Utc>) -> Retained<NSDate> {
    // NSDate's epoch reference is the same as Unix's here — the
    // `…Since1970` constructor exists precisely to avoid the 2001
    // reference date NSDate uses internally.
    NSDate::dateWithTimeIntervalSince1970(value.timestamp() as f64)
}

pub(crate) fn from_nsdate(value: &NSDate) -> Option<DateTime<Utc>> {
    let secs = value.timeIntervalSince1970();
    if !secs.is_finite() {
        return None;
    }
    DateTime::from_timestamp(secs.trunc() as i64, 0)
}

pub(crate) fn opt_from_nsdate(value: Option<Retained<NSDate>>) -> Option<DateTime<Utc>> {
    value.and_then(|d| from_nsdate(&d))
}

/* ── All-day events live in the device's timezone ────────────────
 *
 * EventKit stores an all-day event as local midnight to local
 * 23:59:59. The app stores one as *UTC* midnight to UTC 23:59:59 —
 * the convention `input_to_calendar_event` establishes and every
 * CalDAV `DTSTART;VALUE=DATE` parses into.
 *
 * Passing those instants through unconverted works out east of
 * Greenwich by luck and breaks west of it: 2026-03-01T00:00Z is
 * 2026-02-28 19:00 in New York, so a birthday would land a day
 * early. These two helpers do the translation, and the calendar
 * module uses them on both sides of every all-day event. */

/// The calendar date an instant falls on **in the device's timezone**.
pub(crate) fn local_date_parts(date: &NSDate) -> (isize, isize, isize) {
    let units = NSCalendarUnit::Year | NSCalendarUnit::Month | NSCalendarUnit::Day;
    let comps = NSCalendar::currentCalendar().components_fromDate(units, date);
    (comps.year(), comps.month(), comps.day())
}

/// The instant a wall-clock time falls on **in the device's timezone**.
pub(crate) fn local_instant(
    (year, month, day): (isize, isize, isize),
    (hour, minute, second): (isize, isize, isize),
) -> Option<Retained<NSDate>> {
    let comps = NSDateComponents::new();
    comps.setYear(year);
    comps.setMonth(month);
    comps.setDay(day);
    comps.setHour(hour);
    comps.setMinute(minute);
    comps.setSecond(second);
    NSCalendar::currentCalendar().dateFromComponents(&comps)
}
