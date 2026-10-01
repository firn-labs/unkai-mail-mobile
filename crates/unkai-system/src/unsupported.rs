//! The non-Apple stand-in.
//!
//! Every entry point of the crate exists here with the same
//! signature and answers [`SystemError::Unsupported`]. That keeps
//! `unkai-commands` free of `#[cfg]` blocks: the "System" source
//! type is always compilable, and on a platform without a bridge it
//! simply never becomes usable — which is exactly what the settings
//! UI already renders for a permission that wasn't granted.
//!
//! When the Android port grows a bridge (Android's `CalendarContract`
//! and `ContactsContract` providers), it becomes a third `mod`
//! alongside `apple`, not a change to any caller.

use chrono::{DateTime, Utc};

use crate::{
    PermissionState, SystemCalendar, SystemContact, SystemEntity, SystemError, SystemEvent,
    SystemEventInput, SystemRef, SystemReminder, SystemReminderInput,
};

pub fn access_status(_entity: SystemEntity) -> PermissionState {
    PermissionState::Denied
}

pub fn request_access(entity: SystemEntity) -> Result<PermissionState, SystemError> {
    Err(SystemError::Unsupported(entity))
}

pub fn list_calendars() -> Result<Vec<SystemCalendar>, SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Calendars))
}

pub fn list_events(
    _start: DateTime<Utc>,
    _end: DateTime<Utc>,
) -> Result<Vec<SystemEvent>, SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Calendars))
}

pub fn create_event(
    _calendar_id: &str,
    _input: &SystemEventInput,
) -> Result<SystemRef, SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Calendars))
}

pub fn update_event(
    _occurrence_key: &str,
    _input: &SystemEventInput,
) -> Result<SystemRef, SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Calendars))
}

pub fn delete_event(_occurrence_key: &str) -> Result<(), SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Calendars))
}

pub fn list_contacts() -> Result<Vec<SystemContact>, SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Contacts))
}

pub fn create_contact(_vcard: &str) -> Result<String, SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Contacts))
}

pub fn update_contact(_id: &str, _vcard: &str) -> Result<(), SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Contacts))
}

pub fn delete_contact(_id: &str) -> Result<(), SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Contacts))
}

pub fn list_reminder_lists() -> Result<Vec<SystemCalendar>, SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Reminders))
}

pub fn list_reminders() -> Result<Vec<SystemReminder>, SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Reminders))
}

pub fn create_reminder(
    _list_id: &str,
    _input: &SystemReminderInput,
) -> Result<SystemRef, SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Reminders))
}

pub fn update_reminder(_id: &str, _input: &SystemReminderInput) -> Result<SystemRef, SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Reminders))
}

pub fn delete_reminder(_id: &str) -> Result<(), SystemError> {
    Err(SystemError::Unsupported(SystemEntity::Reminders))
}
