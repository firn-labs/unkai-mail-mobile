//! EventKit's reminder half — the Reminders app.
//!
//! Reminders are `VTODO`s wearing an Apple badge: a list, a title, an
//! optional due date, a completion flag and a 0–9 priority that is
//! literally iCalendar's. The app's Tasks tab already renders exactly
//! that shape, so this module is a straight field mapping.
//!
//! The one wrinkle is the fetch: unlike events, reminders come back
//! through a completion block, so `list_reminders` parks on a
//! condition variable until it fires.

use std::sync::{Arc, Condvar, Mutex};

use chrono::{DateTime, Utc};
use objc2::rc::Retained;
use objc2_event_kit::{EKEntityType, EKEventStore, EKReminder};
use objc2_foundation::{NSArray, NSCalendar, NSCalendarUnit, NSDateComponents, NSURL};

use super::{
    COMPLETION_TIMEOUT, backend_error, calendar::to_system_calendar, from_nsdate, nsstring,
    opt_from_nsdate, opt_string, require_access,
};
use crate::{
    SystemCalendar, SystemEntity, SystemError, SystemRef, SystemReminder, SystemReminderInput,
};

pub fn list_reminder_lists() -> Result<Vec<SystemCalendar>, SystemError> {
    require_access(SystemEntity::Reminders)?;
    let store = unsafe { EKEventStore::new() };
    let lists = unsafe { store.calendarsForEntityType(EKEntityType::Reminder) };
    Ok(lists
        .to_vec()
        .iter()
        .map(|c| to_system_calendar(c))
        .collect())
}

pub fn list_reminders() -> Result<Vec<SystemReminder>, SystemError> {
    require_access(SystemEntity::Reminders)?;
    let store = unsafe { EKEventStore::new() };
    // `nil` calendars = every list, completed reminders included.
    // Filtering completed ones out is the Tasks view's job — it has a
    // Completed bucket and would have nothing to put in it otherwise.
    let predicate = unsafe { store.predicateForRemindersInCalendars(None) };

    // The completion fires on an arbitrary queue, so the shared slot
    // has to be `Sync`, and the conversion to plain Rust has to happen
    // *inside* the block — an `NSArray` handle can't cross back out.
    let state: Arc<(Mutex<Option<Vec<SystemReminder>>>, Condvar)> =
        Arc::new((Mutex::new(None), Condvar::new()));
    let block_state = Arc::clone(&state);
    let block = block2::RcBlock::new(move |reminders: *mut NSArray<EKReminder>| {
        // SAFETY: null on failure, otherwise a live array for the
        // duration of the block.
        let rows = unsafe { reminders.as_ref() }
            .map(|array| {
                array
                    .to_vec()
                    .iter()
                    .filter_map(|r| to_system_reminder(r))
                    .collect()
            })
            .unwrap_or_default();
        let (lock, cv) = &*block_state;
        if let Ok(mut slot) = lock.lock() {
            *slot = Some(rows);
        }
        cv.notify_all();
    });

    // SAFETY: `block` stays alive until the wait below returns.
    let _request = unsafe { store.fetchRemindersMatchingPredicate_completion(&predicate, &block) };

    let (lock, cv) = &*state;
    let guard = lock
        .lock()
        .map_err(|_| SystemError::Backend("the reminder fetch lock was poisoned".into()))?;
    let (mut guard, timeout) = cv
        .wait_timeout_while(guard, COMPLETION_TIMEOUT, |slot| slot.is_none())
        .map_err(|_| SystemError::Backend("the reminder fetch lock was poisoned".into()))?;
    if timeout.timed_out() {
        return Err(SystemError::Backend(
            "the system reminder fetch did not finish".into(),
        ));
    }
    Ok(guard.take().unwrap_or_default())
}

fn to_system_reminder(reminder: &EKReminder) -> Option<SystemReminder> {
    let calendar = unsafe { reminder.calendar() }?;
    Some(SystemReminder {
        id: unsafe { reminder.calendarItemIdentifier() }.to_string(),
        list_id: unsafe { calendar.calendarIdentifier() }.to_string(),
        title: unsafe { reminder.title() }.to_string(),
        notes: opt_string(unsafe { reminder.notes() }),
        url: unsafe { reminder.URL() }
            .and_then(|u| u.absoluteString())
            .map(|s| s.to_string()),
        completed: unsafe { reminder.isCompleted() },
        completed_at: opt_from_nsdate(unsafe { reminder.completionDate() }),
        due: due_date(reminder),
        created: opt_from_nsdate(unsafe { reminder.creationDate() }),
        last_modified: opt_from_nsdate(unsafe { reminder.lastModifiedDate() }),
        // EventKit clamps priority to 0..=9 itself; the cast is safe
        // and a defensive `min` keeps a future widening honest.
        priority: (unsafe { reminder.priority() }).min(9) as u8,
    })
}

/// A reminder's due date is `NSDateComponents`, not an instant — the
/// Reminders app lets you say "the 4th" without saying when on the
/// 4th. Resolving through the *current* calendar is what turns that
/// back into the same instant the user sees on their screen.
fn due_date(reminder: &EKReminder) -> Option<DateTime<Utc>> {
    let comps = unsafe { reminder.dueDateComponents() }?;
    let date = NSCalendar::currentCalendar().dateFromComponents(&comps)?;
    from_nsdate(&date)
}

fn due_components(due: DateTime<Utc>) -> Retained<NSDateComponents> {
    let units = NSCalendarUnit::Year
        | NSCalendarUnit::Month
        | NSCalendarUnit::Day
        | NSCalendarUnit::Hour
        | NSCalendarUnit::Minute
        | NSCalendarUnit::Second;
    NSCalendar::currentCalendar().components_fromDate(units, &super::to_nsdate(due))
}

/* ── Writes ──────────────────────────────────────────────────────*/

pub fn create_reminder(
    list_id: &str,
    input: &SystemReminderInput,
) -> Result<SystemRef, SystemError> {
    require_access(SystemEntity::Reminders)?;
    let store = unsafe { EKEventStore::new() };
    let list = unsafe { store.calendarWithIdentifier(&nsstring(list_id)) }
        .ok_or_else(|| SystemError::NotFound(format!("reminder list '{list_id}'")))?;
    if !unsafe { list.allowsContentModifications() } {
        return Err(SystemError::Backend(format!(
            "the system reminder list '{}' is read-only",
            unsafe { list.title() }
        )));
    }

    let reminder = unsafe { EKReminder::reminderWithEventStore(&store) };
    unsafe { reminder.setCalendar(Some(&list)) };
    apply_input(&reminder, input);
    unsafe { store.saveReminder_commit_error(&reminder, true) }.map_err(backend_error)?;

    Ok(SystemRef {
        id: unsafe { reminder.calendarItemIdentifier() }.to_string(),
        last_modified: opt_from_nsdate(unsafe { reminder.lastModifiedDate() }),
    })
}

pub fn update_reminder(id: &str, input: &SystemReminderInput) -> Result<SystemRef, SystemError> {
    require_access(SystemEntity::Reminders)?;
    let store = unsafe { EKEventStore::new() };
    let reminder = fetch(&store, id)?;
    apply_input(&reminder, input);
    unsafe { store.saveReminder_commit_error(&reminder, true) }.map_err(backend_error)?;

    Ok(SystemRef {
        id: id.to_string(),
        last_modified: opt_from_nsdate(unsafe { reminder.lastModifiedDate() }),
    })
}

pub fn delete_reminder(id: &str) -> Result<(), SystemError> {
    require_access(SystemEntity::Reminders)?;
    let store = unsafe { EKEventStore::new() };
    let reminder = fetch(&store, id)?;
    unsafe { store.removeReminder_commit_error(&reminder, true) }.map_err(backend_error)
}

fn fetch(store: &EKEventStore, id: &str) -> Result<Retained<EKReminder>, SystemError> {
    let item = unsafe { store.calendarItemWithIdentifier(&nsstring(id)) }
        .ok_or_else(|| SystemError::NotFound(format!("reminder '{id}'")))?;
    // `calendarItemWithIdentifier:` answers with events too — a stale
    // id that now belongs to an event must not be silently saved as
    // one.
    item.downcast::<EKReminder>()
        .map_err(|_| SystemError::NotFound(format!("reminder '{id}'")))
}

fn apply_input(reminder: &EKReminder, input: &SystemReminderInput) {
    unsafe {
        reminder.setTitle(Some(&nsstring(&input.title)));
        reminder.setNotes(input.notes.as_deref().map(nsstring).as_deref());
        reminder.setURL(
            input
                .url
                .as_deref()
                .and_then(|u| NSURL::URLWithString(&nsstring(u)))
                .as_deref(),
        );
        reminder.setDueDateComponents(input.due.map(due_components).as_deref());
        // Set `completed` last: EventKit derives `completionDate` from
        // it, and setting the date first would be overwritten.
        reminder.setCompleted(input.completed);
        reminder.setPriority(input.priority.min(9) as usize);
    }
}
