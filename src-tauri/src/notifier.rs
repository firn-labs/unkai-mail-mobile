//! The mobile implementation of [`UiNotifier`].
//!
//! `unkai-commands` decides *when* the user should be told
//! something and *what* to tell them; this file is the only place
//! that knows the answer is "emit a Tauri event to the app's single
//! webview".  Swapping the shell means writing another one of
//! these and nothing else.
//!
//! Unlike the desktop notifier there is no tray, no taskbar badge,
//! and no per-profile window targeting: one window, one profile, so
//! every push is a plain broadcast.  The *user-visible* toast is
//! raised by the frontend through the notification plugin (iOS
//! local notifications), exactly as it is on the desktop.

use std::collections::HashMap;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use unkai_commands::notify::{
    CalendarsUpdatedPayload, EventReminderPayload, MailFlagsUpdatedPayload, MessageReminderPayload,
    NewMailPayload, OutboxUpdatedPayload, UiNotifier,
};
use unkai_core::UnkaiError;

/// Pushes the application layer's notifications out over Tauri IPC.
pub struct MobileNotifier {
    app: AppHandle,
}

impl MobileNotifier {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    /// One place for the "emit, log on failure, carry on" shape
    /// every push channel wants.  None of the ~40 call sites in the
    /// application layer can do anything useful with an emit error.
    fn push<P: Serialize + Clone>(&self, event: &str, payload: P) {
        if let Err(e) = self.app.emit(event, payload) {
            tracing::warn!("failed to emit {event} event: {e}");
        }
    }
}

impl UiNotifier for MobileNotifier {
    fn new_mail(&self, payload: &NewMailPayload) {
        self.push("new-mail", payload);
    }

    fn mail_flags_updated(&self, payload: &MailFlagsUpdatedPayload) {
        self.push("mail-flags-updated", payload);
    }

    fn outbox_updated(&self, payload: &OutboxUpdatedPayload) {
        self.push("outbox-updated", payload);
    }

    fn calendars_updated(&self, payload: &CalendarsUpdatedPayload) {
        self.push("calendars-updated", payload);
    }

    fn event_reminder(&self, payload: &EventReminderPayload) {
        self.push("event-reminder", payload);
    }

    fn message_reminder(&self, payload: &MessageReminderPayload) -> Result<(), UnkaiError> {
        self.app
            .emit("message-reminder", payload)
            .map_err(|e| UnkaiError::Other(format!("emit message-reminder: {e}")))
    }

    /// The unread total drives the in-app badges on the Mail tab
    /// and the account rows.  There is no OS-level app-icon badge:
    /// setting one on iOS requires the notification permission and
    /// a `UNUserNotificationCenter` round-trip that Tauri does not
    /// expose, so the frontend attaches the count to the local
    /// notifications it raises instead.
    fn unread_total_changed(&self, total: u32) {
        self.push("unread-count-updated", total);
    }

    fn unread_by_account_changed(&self, by_account: &HashMap<String, u32>) {
        self.push("unread-count-by-account-updated", by_account);
    }

    fn custom_themes_changed(&self) {
        self.push("custom-themes-changed", ());
    }

    fn profiles_changed(&self) {
        self.push("profiles-changed", ());
    }

    /// The logo style only reaches native chrome on the desktop
    /// (tray + window icon).  On iOS the app icon is baked into
    /// the bundle, so the style is a pure frontend concern — the
    /// UI reads `logo_style` and paints its own header art.
    fn apply_logo_style(&self, _style: &str) -> Result<(), UnkaiError> {
        Ok(())
    }
}
