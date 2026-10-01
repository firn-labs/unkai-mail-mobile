//! Unkai Mail — the **mobile shell** (iOS today, Android next).
//!
//! Same split as the desktop shell it was derived from: the
//! application layer — every command body, the background loops,
//! the crypto bridge — lives in the transport-agnostic
//! `unkai-commands` crate, and what remains here is
//!
//!   * one thin `#[tauri::command]` shim per command, which
//!     resolves the app context out of managed state and delegates,
//!   * the mobile chrome: the custom URI schemes the webview loads
//!     avatars and logo art through, and the single-window setup,
//!   * [`notifier::MobileNotifier`] — the mobile implementation of
//!     `unkai_commands::UiNotifier`, the seam the application layer
//!     talks back to the UI through.
//!
//! Keep it that way: new command logic goes in `unkai-commands`
//! (in the module mirroring the frontend's `ui/src/lib/api/`
//! domain), and only its shim is added here.
//!
//! **A synchronous shim is `#[tauri::command(async)]`, never a bare
//! `#[tauri::command]`.** Tauri runs a non-async command *on the main
//! thread*, and on iOS that is the thread that draws the webview and
//! delivers its touches: every cache read, keychain lookup and vCard
//! parse behind a bare attribute froze the screen for as long as it
//! took — and the busiest of them (envelopes, unread counts, accounts,
//! contacts) run on every navigation. `(async)` runs the same function
//! on Tauri's thread pool. The two exceptions, `open_url` and
//! `open_app_settings`, hand off to UIKit and stay where UIKit is.
//!
//! **What is deliberately absent** relative to the desktop shell:
//! the tray, menus, the multi-window/profile registry, the
//! in-app updater, `mailto:` deep links and file associations,
//! autostart, the MCP server, system-font enumeration, and every
//! command that pairs a native file dialog with a filesystem path.
//! A phone has one window, one user, and a sandboxed filesystem;
//! see `CLAUDE.md` for the full ledger.

mod logo;
mod notifier;
mod state;

use std::sync::Arc;

use tauri::{AppHandle, Manager, State, UriSchemeContext};
use unkai_commands as cmds;
use unkai_commands::mail::refresh_unread_badge;
use unkai_core::UnkaiError;
use unkai_core::models::{
    Account, AppSettings, CalendarEvent, Contact, Email, EmailEnvelope, Folder, NextcloudAccount,
    OutgoingEmail, Task, TaskList,
};
use unkai_nextcloud::{FileEntry, LoginFlowInit};
use unkai_store::cache::{SearchFilters, SearchHit, SearchScope};
use unkai_store::link_check;

use unkai_commands::accounts::ProbedCert;
use unkai_commands::calendar::{
    AttendeeAvailability, CalendarEventInput, CalendarSummary, ImportCalendarReport, InviteSummary,
    NextcloudMapsCapability, SyncCalendarsReport,
};
use unkai_commands::compose::{
    DraftReplaceSource, OutboxRowDto, OutboxSourceRef, RepliedToRef, SavedDraft,
};
use unkai_commands::contacts::{
    AddressbookSummary, ContactCategoryView, ContactGroupView, ContactInput, ContactPhoto,
    ImportContactsReport, MailingListView, SyncContactsReport,
};
use unkai_commands::crypto::{PgpKeyStatus, PgpPublicKeyDto, SmimeCertDto, SmimeCertStatus};
use unkai_commands::mail::{AttachmentPreviewView, InlineImageView, LinkVerdict};
use unkai_commands::nextcloud::{
    NextcloudGroupView, NextcloudShareResult, NextcloudShareRow, NextcloudUserLookup,
};
use unkai_commands::settings::{DatabaseStatusView, SettingsSyncStateView, WipePolicyView};
use unkai_commands::support::SyncStatus;

use state::{AppState, app_ctx, build_context};

// ── Custom URI schemes ──────────────────────────────────────────
//
// Two read-only schemes the webview loads binary assets through,
// so image bytes never have to ride the IPC bridge as base64:
// `contact-photo://<id>` for cached avatars and `unkai-logo://
// <style>` for the embedded brand art.  Both work identically on
// iOS (WKURLSchemeHandler) and on the desktop.

fn contact_photo_protocol(
    ctx: UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<std::borrow::Cow<'static, [u8]>> {
    let id = percent_decode(request.uri().path().trim_start_matches('/'));
    let Some(handle) = ctx.app_handle().state::<AppState>().handle() else {
        tracing::warn!("contact-photo request before the app context was open");
        return tauri::http::Response::builder()
            .status(503)
            .body(std::borrow::Cow::Owned(Vec::new()))
            .expect("build 503");
    };
    match handle.ctx.cache.get_contact_photo(&id) {
        Ok(Some((mime, bytes))) => tauri::http::Response::builder()
            .status(200)
            .header("Content-Type", mime)
            // The bytes are immutable per (id, etag) — but we don't
            // know the etag here.  A short cache window is enough to
            // dedupe the burst of requests scrolling a contact list
            // produces.
            .header("Cache-Control", "private, max-age=300")
            .body(std::borrow::Cow::Owned(bytes))
            .expect("build photo response"),
        Ok(None) => tauri::http::Response::builder()
            .status(404)
            .body(std::borrow::Cow::Owned(Vec::new()))
            .expect("build 404"),
        Err(e) => {
            tracing::warn!("contact-photo lookup for '{id}' failed: {e}");
            tauri::http::Response::builder()
                .status(500)
                .body(std::borrow::Cow::Owned(Vec::new()))
                .expect("build 500")
        }
    }
}

/// Minimal RFC 3986 percent-decoder.  Avoids pulling in a dep just
/// to undo what `encodeURIComponent` did on the JS side.
/// Unrecognised `%xx` sequences are passed through verbatim.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn logo_protocol(
    _ctx: UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<std::borrow::Cow<'static, [u8]>> {
    // The style can arrive as either the host or the path, and which
    // one it is depends on the engine: `unkai-logo://storm` parses
    // `storm` as the *authority*, leaving the path empty, while
    // `unkai-logo://x/storm` puts it in the path. Reading only the
    // path meant every request resolved to an empty slug and fell
    // through to the default — which happened to be `storm`, so the
    // bug was invisible until a second style was added and refused to
    // appear.
    let uri = request.uri();
    let style = match uri.host() {
        Some(host) if !host.is_empty() => host.to_string(),
        _ => uri.path().trim_start_matches('/').to_string(),
    };
    let bytes = logo::logo_bytes_for(&style);
    tauri::http::Response::builder()
        .status(200)
        .header("Content-Type", "image/png")
        .header("Cache-Control", "private, max-age=300")
        .header("Access-Control-Allow-Origin", "*")
        .body(std::borrow::Cow::Borrowed(bytes))
        .expect("build logo response")
}

// ── Mobile-only commands ────────────────────────────────────────

/// Hand a URL to the OS.
///
/// The desktop shell calls the `open` crate, which has no iOS
/// backend (there is no `xdg-open` equivalent — a sandboxed app
/// asks UIKit to open the URL for it).  The opener plugin does
/// exactly that on iOS and keeps the desktop behaviour on every
/// other platform, so the command name and contract stay identical
/// for the frontend.
#[tauri::command]
fn open_url(url: String, app: tauri::AppHandle) -> Result<(), UnkaiError> {
    use tauri_plugin_opener::OpenerExt;
    // Only ever web URLs reach this command (Nextcloud Login Flow
    // v2, a link tapped in a message body).  Refusing everything
    // else keeps a crafted `file://` or `javascript:` href in mail
    // from being handed to UIKit.
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:"))
    {
        return Err(UnkaiError::Other(format!(
            "refusing to open a non-web URL: {url}"
        )));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| UnkaiError::Other(format!("could not open the URL: {e}")))
}

/// Open this app's page in the iOS Settings app.
///
/// Where every permission the app holds can be withdrawn — which the
/// privacy notice has to be able to point at, not just describe. A
/// separate command rather than an allowance in `open_url`: that one
/// refuses non-web schemes because its input can come from a message
/// body, and this URL is a constant nothing outside the binary chooses.
/// `app-settings:` is the value of UIKit's
/// `UIApplication.openSettingsURLString`.
#[tauri::command]
fn open_app_settings(app: tauri::AppHandle) -> Result<(), UnkaiError> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url("app-settings:", None::<&str>)
        .map_err(|e| UnkaiError::Other(format!("could not open Settings: {e}")))
}

/// Save an attachment into the app's Documents directory.
///
/// The mobile answer to the desktop's "Save As…" dialog: iOS has
/// no user-chosen save path, but the bundle declares
/// `UIFileSharingEnabled` + `LSSupportsOpeningDocumentsInPlace`,
/// so everything written here shows up under "On My iPhone → Unkai
/// Mail" in the Files app, where the user can move or share it.
///
/// Returns the absolute path so the frontend can show where the
/// file landed.
#[tauri::command]
async fn save_attachment_to_documents(
    account_id: String,
    folder: String,
    uid: u32,
    part_id: u32,
    filename: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, UnkaiError> {
    let h = app_ctx(&state)?;
    let bytes =
        cmds::mail::download_email_attachment(account_id, folder, uid, part_id, &h.ctx.cache)
            .await?;
    write_to_documents(&app, &filename, &bytes)
}

/// Save bytes the frontend already holds (a decrypted attachment,
/// a generated `.ics`) into the Documents directory.
#[tauri::command(async)]
fn save_bytes_to_documents(
    filename: String,
    bytes: Vec<u8>,
    app: tauri::AppHandle,
) -> Result<String, UnkaiError> {
    write_to_documents(&app, &filename, &bytes)
}

/// Shared tail of the two save commands: sanitise the filename,
/// de-duplicate against what is already there, write.
fn write_to_documents(
    app: &tauri::AppHandle,
    filename: &str,
    bytes: &[u8],
) -> Result<String, UnkaiError> {
    let dir = app
        .path()
        .document_dir()
        .map_err(|e| UnkaiError::Other(format!("no document directory: {e}")))?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| UnkaiError::Other(format!("could not create the document directory: {e}")))?;

    // Strip any path structure a sender put in the filename —
    // `../../` in an attachment name must never escape the
    // sandbox's Documents directory.
    let base = std::path::Path::new(filename)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty() && s != "." && s != "..")
        .unwrap_or_else(|| "attachment".to_string());

    // Never overwrite: "report.pdf" → "report (2).pdf".
    let mut candidate = dir.join(&base);
    if candidate.exists() {
        let stem = std::path::Path::new(&base)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "attachment".into());
        let ext = std::path::Path::new(&base)
            .extension()
            .map(|s| format!(".{}", s.to_string_lossy()))
            .unwrap_or_default();
        for n in 2..1000 {
            candidate = dir.join(format!("{stem} ({n}){ext}"));
            if !candidate.exists() {
                break;
            }
        }
    }

    std::fs::write(&candidate, bytes)
        .map_err(|e| UnkaiError::Other(format!("could not write the file: {e}")))?;
    Ok(candidate.to_string_lossy().into_owned())
}

// ── Local vault (passphrase-wrapped cache key) ──────────────────
//
// The `fido_*` command names come from the desktop build, where
// the same envelope also holds hardware-key (WebAuthn PRF) wraps.
// A phone has no security-key port, so the mobile shell exposes
// only the passphrase half — same storage, same envelope format,
// so a vault set up on the desktop opens here and vice versa.

#[tauri::command(async)]
fn fido_status(
    state: State<'_, AppState>,
) -> Result<unkai_commands::settings::FidoStatusView, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::fido_status(&h.ctx.profile.id)
}

#[tauri::command(async)]
fn fido_enroll_passphrase(
    passphrase: String,
    label: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::fido_enroll_passphrase(passphrase, label, &h.ctx.cache, &h.ctx.profile.id)
}

#[tauri::command(async)]
fn fido_verify_passphrase(
    passphrase: String,
    state: State<'_, AppState>,
) -> Result<bool, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::fido_verify_passphrase(passphrase, &h.ctx.profile.id)
}

#[tauri::command(async)]
fn fido_remove(credential_id_b64: String, state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::fido_remove(credential_id_b64, &h.ctx.profile.id)
}

/// Drop the plaintext key from the keychain so the cache can only
/// be opened with the enrolled passphrase.
#[tauri::command(async)]
fn enable_fido_only_mode(state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::enable_fido_only_mode(&h.ctx.profile.id)
}

#[tauri::command(async)]
fn disable_fido_only_mode(state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::disable_fido_only_mode(&h.ctx.cache, &h.ctx.profile.id)
}

// ── Office-attachment viewer ────────────────────────────────────
//
// A phone cannot open a `.docx` on its own, but the user's own
// Nextcloud can: upload the attachment to a per-user temp folder,
// hand the resulting Files deep link to `open_url` (Safari renders
// it through Collabora), and delete the temp file when the user
// comes back.  `office_sweep_temp` clears anything an interrupted
// session left behind.

#[tauri::command]
async fn office_open_attachment(
    nc_id: String,
    filename: String,
    data: Vec<u8>,
    content_type: Option<String>,
    state: State<'_, AppState>,
) -> Result<unkai_commands::system::OfficeOpenResult, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::system::office_open_attachment(nc_id, filename, data, content_type, &h.ctx.cache).await
}

#[tauri::command]
async fn office_close_attachment(
    nc_id: String,
    temp_path: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::system::office_close_attachment(nc_id, temp_path, &h.ctx.cache).await
}

#[tauri::command]
async fn office_sweep_temp(nc_id: String, state: State<'_, AppState>) -> Result<u32, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::system::office_sweep_temp(nc_id, &h.ctx.cache).await
}

// ── Command shims ───────────────────────────────────────────────
//
// One per `#[tauri::command]`: extract the app context out of
// managed state, delegate to `unkai-commands`, return.  Any logic
// that grows past that shape belongs in the application layer.

#[tauri::command(async)]
fn add_account(
    account: Account,
    password: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::accounts::add_account(account, password, &h.ctx.cache, &h.sync_notify)
}

#[tauri::command]
async fn add_contact_to_category(
    contact_id: String,
    category: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::add_contact_to_category(contact_id, category, &h.ctx.cache).await
}

#[allow(clippy::too_many_arguments)] // Tauri command: each arg maps to a frontend invoke parameter
#[tauri::command]
async fn add_dav_account(
    display_name: String,
    server_url: String,
    username: String,
    password: String,
    use_contacts: bool,
    use_calendars: bool,
    trusted_certs: Option<Vec<unkai_core::models::TrustedCert>>,
    state: State<'_, AppState>,
) -> Result<NextcloudAccount, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::add_dav_account(
        display_name,
        server_url,
        username,
        password,
        use_contacts,
        use_calendars,
        trusted_certs,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command(async)]
fn add_local_dav_account(
    display_name: String,
    use_contacts: bool,
    use_calendars: bool,
    state: State<'_, AppState>,
) -> Result<NextcloudAccount, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::add_local_dav_account(display_name, use_contacts, use_calendars, &h.ctx.cache)
}

#[tauri::command]
async fn add_system_dav_account(
    display_name: String,
    use_contacts: bool,
    use_calendars: bool,
    use_reminders: bool,
    state: State<'_, AppState>,
) -> Result<NextcloudAccount, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::add_system_dav_account(
        display_name,
        use_contacts,
        use_calendars,
        use_reminders,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command(async)]
fn system_access_status() -> cmds::nextcloud::SystemAccessView {
    cmds::nextcloud::system_access_status()
}

#[tauri::command]
async fn request_system_access(
    entity: String,
) -> Result<cmds::nextcloud::SystemAccessView, UnkaiError> {
    cmds::nextcloud::request_system_access(entity).await
}

#[tauri::command]
async fn add_talk_participant(
    nc_id: String,
    room_token: String,
    participant: unkai_nextcloud::ParticipantSource,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::talk::add_talk_participant(nc_id, room_token, participant, &h.ctx.cache).await
}

#[tauri::command]
async fn add_talk_participants(
    nc_id: String,
    room_token: String,
    participants: Vec<unkai_nextcloud::ParticipantSource>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::talk::add_talk_participants(nc_id, room_token, participants, &h.ctx.cache).await
}

#[tauri::command]
async fn archive_message(
    account_id: String,
    folder: String,
    uid: u32,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::archive_message(account_id, folder, uid, &h.ctx.cache).await
}

#[tauri::command]
async fn archive_messages(
    account_id: String,
    folder: String,
    uids: Vec<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<u32>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::archive_messages(account_id, folder, uids, &h.ctx.cache).await
}

#[tauri::command]
async fn check_mail_now(state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::check_mail_now(&h.ctx).await
}

#[tauri::command(async)]
fn check_urls(
    urls: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<LinkVerdict>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::check_urls(urls, &h.ctx.shared, &h.ctx.settings)
}

#[tauri::command]
async fn clear_folder(
    account_id: String,
    folder: String,
    state: State<'_, AppState>,
) -> Result<u32, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::clear_folder(account_id, folder, &h.ctx.cache, h.ctx.ui.as_ref()).await
}

#[tauri::command]
async fn count_outbox(state: State<'_, AppState>) -> Result<u32, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::count_outbox(&h.ctx.cache).await
}

#[tauri::command]
async fn count_outbox_by_account(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, u32>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::count_outbox_by_account(&h.ctx.cache).await
}

#[tauri::command]
async fn create_calendar_event(
    calendar_id: String,
    input: CalendarEventInput,
    state: State<'_, AppState>,
) -> Result<CalendarEvent, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::create_calendar_event(calendar_id, input, &h.ctx.cache, h.ctx.ui.as_ref()).await
}

#[tauri::command]
async fn create_contact(
    nc_id: String,
    addressbook_url: String,
    addressbook_name: String,
    input: ContactInput,
    state: State<'_, AppState>,
) -> Result<Contact, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::create_contact(
        nc_id,
        addressbook_url,
        addressbook_name,
        input,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command]
async fn create_contact_group(
    nc_id: String,
    addressbook_url: String,
    addressbook_name: String,
    display_name: String,
    member_uids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<ContactGroupView, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::create_contact_group(
        nc_id,
        addressbook_url,
        addressbook_name,
        display_name,
        member_uids,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command]
async fn create_folder(
    account_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::create_folder(account_id, name, &h.ctx.cache).await
}

#[tauri::command]
async fn create_nextcloud_calendar(
    nc_id: String,
    display_name: String,
    color: Option<String>,
    state: State<'_, AppState>,
) -> Result<CalendarSummary, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::create_nextcloud_calendar(nc_id, display_name, color, &h.ctx.cache).await
}

#[tauri::command]
async fn create_nextcloud_directory(
    nc_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::create_nextcloud_directory(nc_id, path, &h.ctx.cache).await
}

#[tauri::command]
async fn create_nextcloud_note(
    nc_id: String,
    title: String,
    content: String,
    category: String,
    state: State<'_, AppState>,
) -> Result<unkai_core::models::Note, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::notes::create_nextcloud_note(nc_id, title, content, category, &h.ctx.cache).await
}

#[allow(clippy::too_many_arguments)] // Tauri command: invoke parameters plus the profile-routing pair
#[tauri::command]
async fn create_nextcloud_share(
    nc_id: String,
    path: String,
    password: Option<String>,
    label: Option<String>,
    permissions: Option<u8>,
    expire_date: Option<String>,
    state: State<'_, AppState>,
) -> Result<NextcloudShareResult, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::create_nextcloud_share(
        nc_id,
        path,
        password,
        label,
        permissions,
        expire_date,
        &h.ctx.cache,
    )
    .await
}

#[allow(clippy::too_many_arguments)] // Tauri command: each arg maps to a frontend invoke parameter
#[tauri::command]
async fn create_nextcloud_task(
    nc_id: String,
    list_id: String,
    summary: String,
    description: Option<String>,
    due_unix: Option<i64>,
    due_tz: Option<String>,
    priority: Option<u8>,
    url: Option<String>,
    state: State<'_, AppState>,
) -> Result<Task, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::create_nextcloud_task(
        nc_id,
        list_id,
        summary,
        description,
        due_unix,
        due_tz,
        priority,
        url,
        &h.ctx.cache,
    )
    .await
}

#[allow(clippy::too_many_arguments)] // Tauri command: each arg maps to a frontend invoke parameter
#[tauri::command]
async fn create_nextcloud_task_from_mail(
    nc_id: String,
    list_id: String,
    mail_account_id: String,
    folder: String,
    uid: u32,
    subject: String,
    from: String,
    state: State<'_, AppState>,
) -> Result<Task, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::create_nextcloud_task_from_mail(
        nc_id,
        list_id,
        mail_account_id,
        folder,
        uid,
        subject,
        from,
        &h.ctx.cache,
    )
    .await
}

#[allow(clippy::too_many_arguments)] // Tauri command: invoke parameters plus the profile-routing pair
#[tauri::command]
async fn create_talk_room(
    nc_id: String,
    room_name: String,
    participants: Vec<unkai_nextcloud::ParticipantSource>,
    object_type: Option<String>,
    object_id: Option<String>,
    room_type: Option<u8>,
    state: State<'_, AppState>,
) -> Result<unkai_nextcloud::TalkRoom, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::talk::create_talk_room(
        nc_id,
        room_name,
        participants,
        object_type,
        object_id,
        room_type,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command(async)]
fn database_status(state: State<'_, AppState>) -> Result<DatabaseStatusView, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::database_status(&h.ctx.cache, &h.ctx.profile.id)
}

#[tauri::command(async)]
fn debug_link_check(
    url: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::debug_link_check(url, &h.ctx.shared)
}

#[tauri::command]
async fn decrypt_message(
    account_id: String,
    folder: String,
    uid: u32,
    pgp_passphrase: String,
    state: State<'_, AppState>,
) -> Result<Email, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::decrypt_message(account_id, folder, uid, pgp_passphrase, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_calendar_event(
    event_id: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::delete_calendar_event(event_id, &h.ctx.cache, h.ctx.ui.as_ref()).await
}

#[tauri::command]
async fn delete_contact(contact_id: String, state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::delete_contact(contact_id, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_contact_category(
    name: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::delete_contact_category(name, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_contact_group(
    group_id: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::delete_contact_group(group_id, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_folder(
    account_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::delete_folder(account_id, name, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_message(
    account_id: String,
    folder: String,
    uid: u32,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::delete_message(account_id, folder, uid, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_messages(
    account_id: String,
    folder: String,
    uids: Vec<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<u32>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::delete_messages(account_id, folder, uids, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_nextcloud_calendar(
    calendar_id: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::delete_nextcloud_calendar(calendar_id, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_nextcloud_note(
    nc_id: String,
    note_id: u64,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::notes::delete_nextcloud_note(nc_id, note_id, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_nextcloud_share(
    nc_id: String,
    share_id: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::delete_nextcloud_share(nc_id, share_id, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_nextcloud_task(
    nc_id: String,
    list_id: String,
    uid: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::delete_nextcloud_task(nc_id, list_id, uid, &h.ctx.cache).await
}

#[tauri::command]
async fn delete_outbox_entry(id: i64, state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::delete_outbox_entry(id, &h.ctx.cache, h.ctx.ui.as_ref()).await
}

#[tauri::command]
async fn delete_talk_room(
    nc_id: String,
    room_token: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::talk::delete_talk_room(nc_id, room_token, &h.ctx.cache).await
}

#[tauri::command]
async fn detect_jmap(host: String) -> Result<Option<String>, UnkaiError> {
    cmds::accounts::detect_jmap(host).await
}

#[tauri::command]
async fn detect_nc_maps(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<NextcloudMapsCapability, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::detect_nc_maps(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn discover_account_settings(
    email: String,
) -> Result<Option<unkai_discovery::DiscoveredAccount>, UnkaiError> {
    cmds::accounts::discover_account_settings(email).await
}

#[tauri::command]
async fn dismiss_cancelled_event(
    uid: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::dismiss_cancelled_event(uid, &h.ctx.cache).await
}

#[tauri::command(async)]
fn dismiss_event_reminder(uid: String, state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::dismiss_event_reminder(uid, &h.ctx.reminders)
}

#[tauri::command]
async fn download_calendar_from_message(
    account_id: String,
    folder: String,
    uid: u32,
    state: State<'_, AppState>,
) -> Result<Option<Vec<u8>>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::download_calendar_from_message(account_id, folder, uid, &h.ctx.cache).await
}

#[tauri::command]
async fn download_decrypted_attachment(
    account_id: String,
    folder: String,
    uid: u32,
    part_id: u32,
    pgp_passphrase: String,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::download_decrypted_attachment(
        account_id,
        folder,
        uid,
        part_id,
        pgp_passphrase,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command]
async fn download_email_attachment(
    account_id: String,
    folder: String,
    uid: u32,
    part_id: u32,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::download_email_attachment(account_id, folder, uid, part_id, &h.ctx.cache).await
}

#[tauri::command]
async fn download_nextcloud_file(
    nc_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::download_nextcloud_file(nc_id, path, &h.ctx.cache).await
}

#[tauri::command]
async fn edit_outbox_entry(
    id: i64,
    state: State<'_, AppState>,
) -> Result<OutboxRowDto, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::edit_outbox_entry(id, &h.ctx.cache, h.ctx.ui.as_ref()).await
}

#[tauri::command]
async fn expunge_draft_after_send(
    account_id: String,
    folder: String,
    uid: u32,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::expunge_draft_after_send(account_id, folder, uid, &h.ctx.cache).await
}

#[tauri::command]
async fn fetch_envelopes(
    account_id: String,
    folder: String,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::fetch_envelopes(account_id, folder, limit, &h.ctx.cache).await
}

#[tauri::command]
async fn fetch_folders(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Folder>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::fetch_folders(account_id, &h.ctx.cache).await
}

#[tauri::command]
async fn fetch_inline_images(
    account_id: String,
    folder: String,
    uid: u32,
    pgp_passphrase: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<InlineImageView>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::fetch_inline_images(account_id, folder, uid, pgp_passphrase, &h.ctx.cache).await
}

#[tauri::command]
async fn fetch_message(
    account_id: String,
    folder: String,
    uid: u32,
    state: State<'_, AppState>,
) -> Result<Email, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::fetch_message(h.ctx.ui.as_ref(), account_id, folder, uid, &h.ctx.cache).await
}

#[tauri::command]
async fn fetch_older_envelopes(
    account_id: String,
    folder: String,
    before_uid: u32,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::fetch_older_envelopes(account_id, folder, before_uid, limit, &h.ctx.cache).await
}

#[tauri::command]
async fn fetch_older_unified_envelopes(
    folder: String,
    before_uid_per_account: std::collections::HashMap<String, u32>,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::fetch_older_unified_envelopes(folder, before_uid_per_account, limit, &h.ctx.cache)
        .await
}

#[tauri::command]
async fn fetch_unified_envelopes(
    folder: String,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::fetch_unified_envelopes(folder, limit, &h.ctx.cache).await
}

#[tauri::command]
async fn fetch_unified_special_envelopes(
    special: String,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::fetch_unified_special_envelopes(special, limit, &h.ctx.cache).await
}

#[tauri::command]
async fn find_nextcloud_user_by_email(
    nc_id: String,
    email: String,
    state: State<'_, AppState>,
) -> Result<Option<NextcloudUserLookup>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::find_nextcloud_user_by_email(nc_id, email, &h.ctx.cache).await
}

#[tauri::command]
async fn geocode_search(
    query: String,
    lang: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<cmds::geocode::GeocodeResult>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::geocode_search(query, lang, &h.ctx.cache, &h.ctx.settings).await
}

#[tauri::command(async)]
fn get_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::accounts::get_accounts(&h.ctx.cache)
}

/// Seed the demo account (test mode). See `unkai_commands::demo`.
#[tauri::command]
async fn create_demo_account(
    state: State<'_, AppState>,
) -> Result<cmds::demo::DemoSummary, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::demo::create_demo_account(&h.ctx.cache, h.ctx.ui.as_ref()).await
}

/// Remove the demo account and its data, leaving real accounts alone.
#[tauri::command]
async fn remove_demo_account(state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::demo::remove_demo_account(&h.ctx.cache).await
}

/// Whether this cache already holds the demo account.
#[tauri::command(async)]
fn demo_account_exists(state: State<'_, AppState>) -> Result<bool, UnkaiError> {
    let h = app_ctx(&state)?;
    Ok(cmds::demo::demo_account_exists(&h.ctx.cache))
}

#[tauri::command]
async fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::get_app_settings(&h.ctx.settings).await
}

#[tauri::command(async)]
fn get_app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command(async)]
fn get_attachment_previews(
    account_id: String,
    folder: String,
    uid: u32,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentPreviewView>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_attachment_previews(account_id, folder, uid, &h.ctx.cache)
}

#[tauri::command]
async fn get_attendee_availability(
    nc_id: String,
    attendee_emails: Vec<String>,
    range_start: chrono::DateTime<chrono::Utc>,
    range_end: chrono::DateTime<chrono::Utc>,
    state: State<'_, AppState>,
) -> Result<Vec<AttendeeAvailability>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::get_attendee_availability(
        nc_id,
        attendee_emails,
        range_start,
        range_end,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command(async)]
fn get_cached_calendars(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CalendarSummary>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::get_cached_calendars(nc_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_cached_envelopes(
    account_id: String,
    folder: String,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_cached_envelopes(account_id, folder, limit, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_cached_events(
    calendar_ids: Vec<String>,
    range_start: chrono::DateTime<chrono::Utc>,
    range_end: chrono::DateTime<chrono::Utc>,
    state: State<'_, AppState>,
) -> Result<Vec<CalendarEvent>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::get_cached_events(calendar_ids, range_start, range_end, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_archive_folder(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_archive_folder(account_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_cached_folders(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Folder>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_cached_folders(account_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_cached_message(
    account_id: String,
    folder: String,
    uid: u32,
    state: State<'_, AppState>,
) -> Result<Option<Email>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_cached_message(account_id, folder, uid, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_calendars_sync_status(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<SyncStatus, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::get_calendars_sync_status(nc_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_contact_photo(
    contact_id: String,
    state: State<'_, AppState>,
) -> Result<Option<ContactPhoto>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::get_contact_photo(contact_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_contacts(
    nc_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<Contact>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::get_contacts(nc_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_contacts_sync_status(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<SyncStatus, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::get_contacts_sync_status(nc_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_envelopes_by_thread(
    account_id: String,
    folder: String,
    thread_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_envelopes_by_thread(account_id, folder, thread_id, &h.ctx.cache)
}

#[tauri::command]
async fn get_event_partstat_for_user(
    uid: String,
    attendee_hint: Option<String>,
    state: State<'_, AppState>,
) -> Result<Option<String>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::get_event_partstat_for_user(uid, attendee_hint, &h.ctx.cache).await
}

#[tauri::command(async)]
fn get_link_check_status(
    state: State<'_, AppState>,
) -> Result<link_check::UrlhausStatus, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_link_check_status(&h.ctx.shared)
}

#[tauri::command(async)]
fn get_nextcloud_accounts(state: State<'_, AppState>) -> Result<Vec<NextcloudAccount>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::get_nextcloud_accounts(&h.ctx.cache)
}

#[tauri::command]
async fn get_nextcloud_note(
    nc_id: String,
    note_id: u64,
    state: State<'_, AppState>,
) -> Result<unkai_core::models::Note, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::notes::get_nextcloud_note(nc_id, note_id, &h.ctx.cache).await
}

#[tauri::command]
async fn get_nextcloud_user_email(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::get_nextcloud_user_email(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn get_receipt_status(
    account_id: String,
    message_id: String,
    state: State<'_, AppState>,
) -> Result<Option<unkai_store::SentReceiptStatus>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_receipt_status(account_id, message_id, &h.ctx.cache).await
}

#[tauri::command]
async fn get_rsvp_response(
    uid: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::get_rsvp_response(uid, &h.ctx.cache).await
}

#[tauri::command(async)]
fn get_settings_sync_state(
    state: State<'_, AppState>,
) -> Result<SettingsSyncStateView, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::get_settings_sync_state(&h.ctx.profile)
}

#[tauri::command(async)]
fn get_tasks_sync_status(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<SyncStatus, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::get_tasks_sync_status(nc_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_total_unread(state: State<'_, AppState>) -> Result<u32, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_total_unread(&h.ctx.cache)
}

#[tauri::command(async)]
fn get_unified_cached_envelopes(
    folder: String,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_unified_cached_envelopes(folder, limit, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_unified_special_cached_envelopes(
    special: String,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_unified_special_cached_envelopes(special, limit, &h.ctx.cache)
}

#[tauri::command(async)]
fn get_unread_counts_by_account(
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, u32>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::get_unread_counts_by_account(&h.ctx.cache)
}

#[tauri::command(async)]
fn get_wipe_policy(state: State<'_, AppState>) -> Result<WipePolicyView, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::get_wipe_policy(&h.ctx.profile.id)
}

#[tauri::command]
async fn import_calendar_file(
    calendar_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<ImportCalendarReport, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::import_calendar_file(calendar_id, path, &h.ctx.cache, h.ctx.ui.as_ref()).await
}

#[tauri::command]
async fn import_contacts_file(
    nc_id: String,
    addressbook_url: String,
    addressbook_name: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<ImportContactsReport, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::import_contacts_file(
        nc_id,
        addressbook_url,
        addressbook_name,
        path,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command(async)]
fn is_event_in_calendar(uid: String, state: State<'_, AppState>) -> Result<bool, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::is_event_in_calendar(uid, &h.ctx.cache)
}

#[tauri::command(async)]
fn is_invite_cancelled(uid: String, state: State<'_, AppState>) -> Result<bool, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::is_invite_cancelled(uid, &h.ctx.cache)
}

#[tauri::command]
async fn list_all_outbox(state: State<'_, AppState>) -> Result<Vec<OutboxRowDto>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::list_all_outbox(&h.ctx.cache).await
}

#[tauri::command(async)]
fn list_contact_categories(
    state: State<'_, AppState>,
) -> Result<Vec<ContactCategoryView>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::list_contact_categories(&h.ctx.cache)
}

#[tauri::command(async)]
fn list_contact_groups(state: State<'_, AppState>) -> Result<Vec<ContactGroupView>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::list_contact_groups(&h.ctx.cache)
}

#[tauri::command]
async fn list_mailing_lists(
    state: State<'_, AppState>,
) -> Result<Vec<MailingListView>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::list_mailing_lists(&h.ctx.cache).await
}

#[tauri::command]
async fn list_nextcloud_addressbooks(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AddressbookSummary>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::list_nextcloud_addressbooks(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn list_nextcloud_calendars(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CalendarSummary>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::list_nextcloud_calendars(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn list_nextcloud_files(
    nc_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileEntry>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::list_nextcloud_files(nc_id, path, &h.ctx.cache).await
}

#[tauri::command]
async fn list_nextcloud_groups(
    state: State<'_, AppState>,
) -> Result<Vec<NextcloudGroupView>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::list_nextcloud_groups(&h.ctx.cache).await
}

#[tauri::command(async)]
fn list_nextcloud_notes(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<unkai_core::models::Note>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::notes::list_nextcloud_notes(nc_id, &h.ctx.cache)
}

#[tauri::command]
async fn list_nextcloud_shares(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<NextcloudShareRow>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::list_nextcloud_shares(nc_id, &h.ctx.cache).await
}

#[tauri::command(async)]
fn list_nextcloud_task_lists(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TaskList>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::list_nextcloud_task_lists(nc_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn list_nextcloud_tasks(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Task>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::list_nextcloud_tasks(nc_id, &h.ctx.cache)
}

#[tauri::command]
async fn list_outbox(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<OutboxRowDto>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::list_outbox(account_id, &h.ctx.cache).await
}

#[tauri::command(async)]
fn list_provider_presets() -> Vec<unkai_discovery::ProviderPreset> {
    cmds::accounts::list_provider_presets()
}

#[tauri::command]
async fn list_talk_rooms(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<unkai_nextcloud::TalkRoom>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::talk::list_talk_rooms(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn mark_as_read(
    account_id: String,
    folder: String,
    uid: u32,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::mark_as_read(account_id, folder, uid, &h.ctx.cache, h.ctx.ui.as_ref()).await
}

#[tauri::command]
async fn mark_folder_read(
    account_id: String,
    folder: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::mark_folder_read(account_id, folder, &h.ctx.cache, h.ctx.ui.as_ref()).await
}

#[tauri::command]
async fn move_message(
    account_id: String,
    folder: String,
    uid: u32,
    dest_folder: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::move_message(account_id, folder, uid, dest_folder, &h.ctx.cache).await
}

#[tauri::command]
async fn move_messages(
    account_id: String,
    folder: String,
    uids: Vec<u32>,
    dest_folder: String,
    state: State<'_, AppState>,
) -> Result<Vec<u32>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::move_messages(account_id, folder, uids, dest_folder, &h.ctx.cache).await
}

#[tauri::command]
async fn nc_probe_settings_bundle(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::nc_probe_settings_bundle(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn nc_restore_settings_bundle(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::nc_restore_settings_bundle(nc_id, &h.ctx.cache, &h.ctx.settings, &h.ctx.profile)
        .await
}

#[tauri::command]
async fn nextcloud_file_preview(
    nc_id: String,
    path: String,
    size: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Option<Vec<u8>>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::nextcloud_file_preview(nc_id, path, size, &h.ctx.cache).await
}

#[tauri::command]
async fn notify_settings_changed(
    local_storage: std::collections::HashMap<String, String>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::notify_settings_changed(local_storage, &h.local_storage, &h.sync_notify).await
}

#[tauri::command(async)]
fn parse_event_invite(bytes: Vec<u8>) -> Result<InviteSummary, UnkaiError> {
    cmds::calendar::parse_event_invite(bytes)
}

#[tauri::command(async)]
fn pgp_disable_unlock_automatically(account_id: String) -> Result<(), UnkaiError> {
    cmds::crypto::pgp_disable_unlock_automatically(account_id)
}

#[tauri::command(async)]
fn pgp_enable_unlock_automatically(
    account_id: String,
    passphrase: String,
) -> Result<(), UnkaiError> {
    cmds::crypto::pgp_enable_unlock_automatically(account_id, passphrase)
}

#[tauri::command(async)]
fn pgp_get_account_key_status(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<PgpKeyStatus, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::pgp_get_account_key_status(account_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn pgp_get_keys_for_email(
    email: String,
    state: State<'_, AppState>,
) -> Result<Vec<PgpPublicKeyDto>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::pgp_get_keys_for_email(email, &h.ctx.cache)
}

#[tauri::command(async)]
fn pgp_has_unlock_automatically(account_id: String) -> Result<bool, UnkaiError> {
    cmds::crypto::pgp_has_unlock_automatically(account_id)
}

#[tauri::command]
async fn pgp_import_private_key(
    account_id: String,
    armored_key: String,
    passphrase: String,
    state: State<'_, AppState>,
) -> Result<String, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::pgp_import_private_key(
        account_id,
        armored_key,
        passphrase,
        &h.ctx.cache,
        &h.sync_notify,
    )
    .await
}

#[tauri::command(async)]
fn pgp_import_public_key(
    armored_key: String,
    email_hint: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::pgp_import_public_key(armored_key, email_hint, &h.ctx.cache)
}

#[tauri::command(async)]
fn pgp_list_public_keys(state: State<'_, AppState>) -> Result<Vec<PgpPublicKeyDto>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::pgp_list_public_keys(&h.ctx.cache)
}

#[tauri::command(async)]
fn pgp_remove_private_key(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::pgp_remove_private_key(account_id, &h.ctx.cache, &h.sync_notify)
}

#[tauri::command(async)]
fn pgp_remove_public_key(
    fingerprint: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::pgp_remove_public_key(fingerprint, &h.ctx.cache)
}

#[tauri::command]
async fn poll_nextcloud_login(
    poll_endpoint: String,
    poll_token: String,
    trusted_certs: Option<Vec<unkai_core::models::TrustedCert>>,
    state: State<'_, AppState>,
) -> Result<Option<NextcloudAccount>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::poll_nextcloud_login(poll_endpoint, poll_token, trusted_certs, &h.ctx.cache)
        .await
}

#[tauri::command]
async fn probe_server_certificate(host: String, port: u16) -> Result<ProbedCert, UnkaiError> {
    cmds::accounts::probe_server_certificate(host, port).await
}

#[allow(clippy::too_many_arguments)] // Tauri command: invoke parameters plus the profile-routing pair
#[tauri::command(async)]
fn put_attachment_preview(
    account_id: String,
    folder: String,
    uid: u32,
    part_id: u32,
    mime: String,
    base64: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::put_attachment_preview(account_id, folder, uid, part_id, mime, base64, &h.ctx.cache)
}

#[tauri::command(async)]
fn record_cancelled_invite(uid: String, state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::record_cancelled_invite(uid, &h.ctx.cache)
}

#[tauri::command]
async fn refresh_nextcloud_capabilities(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<NextcloudAccount, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::refresh_nextcloud_capabilities(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn refresh_urlhaus_now(state: State<'_, AppState>) -> Result<u32, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::refresh_urlhaus_now(&h.ctx.shared).await
}

#[tauri::command(async)]
fn remove_account(id: String, state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::accounts::remove_account(id, &h.ctx.cache, &h.sync_notify)
}

#[tauri::command]
async fn remove_contact_from_category(
    contact_id: String,
    category: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::remove_contact_from_category(contact_id, category, &h.ctx.cache).await
}

#[tauri::command(async)]
fn remove_nextcloud_account(id: String, state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::remove_nextcloud_account(id, &h.ctx.cache)
}

#[tauri::command]
async fn rename_contact_category(
    old: String,
    new: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::rename_contact_category(old, new, &h.ctx.cache).await
}

#[tauri::command]
async fn rename_folder(
    account_id: String,
    old_name: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::rename_folder(account_id, old_name, new_name, &h.ctx.cache).await
}

#[tauri::command]
async fn rename_mailing_list(
    id: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::rename_mailing_list(id, new_name, &h.ctx.cache).await
}

#[tauri::command]
async fn rename_talk_room(
    nc_id: String,
    room_token: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::talk::rename_talk_room(nc_id, room_token, new_name, &h.ctx.cache).await
}

#[tauri::command]
async fn respond_mdn_request(
    account_id: String,
    folder: String,
    uid: u32,
    decline: bool,
    automatic: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::respond_mdn_request(account_id, folder, uid, decline, automatic, &h.ctx.cache).await
}

#[tauri::command]
async fn respond_to_invite(
    calendar_id: String,
    raw_ics: String,
    partstat: String,
    attendee_hint: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::respond_to_invite(calendar_id, raw_ics, partstat, attendee_hint, &h.ctx.cache)
        .await
}

#[tauri::command]
async fn retry_outbox_entry(id: i64, state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::retry_outbox_entry(id, &h.ctx).await
}

#[tauri::command]
async fn retry_outbox_entry_with_passphrase(
    id: i64,
    pgp_passphrase: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::retry_outbox_entry_with_passphrase(id, pgp_passphrase, &h.ctx).await
}

#[tauri::command]
async fn rsvp_existing_event(
    event_id: String,
    partstat: String,
    attendee_hint: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::rsvp_existing_event(event_id, partstat, attendee_hint, &h.ctx.cache).await
}

#[tauri::command]
async fn save_draft(
    account_id: String,
    email: OutgoingEmail,
    replace_source: Option<DraftReplaceSource>,
    state: State<'_, AppState>,
) -> Result<SavedDraft, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::save_draft(account_id, email, replace_source, &h.ctx.cache).await
}

#[tauri::command(async)]
fn search_contacts(
    query: String,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<Contact>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::search_contacts(query, limit, &h.ctx.cache)
}

#[tauri::command(async)]
fn search_emails(
    query: String,
    scope: Option<SearchScope>,
    filters: Option<SearchFilters>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchHit>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::search_emails(query, scope, filters, &h.ctx.cache)
}

#[tauri::command]
async fn search_imap_server(
    account_id: String,
    folder: String,
    query: String,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::search_imap_server(account_id, folder, query, limit, &h.ctx.cache).await
}

#[tauri::command]
async fn search_imap_server_older(
    account_id: String,
    folder: String,
    query: String,
    before_uid: u32,
    limit: u32,
    state: State<'_, AppState>,
) -> Result<Vec<EmailEnvelope>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::search_imap_server_older(account_id, folder, query, before_uid, limit, &h.ctx.cache)
        .await
}

#[tauri::command]
async fn send_email(
    account_id: String,
    email: OutgoingEmail,
    replied_to: Option<RepliedToRef>,
    outbox_source: Option<OutboxSourceRef>,
    pgp_passphrase: Option<String>,
    state: State<'_, AppState>,
) -> Result<i64, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::send_email(
        account_id,
        email,
        replied_to,
        outbox_source,
        pgp_passphrase,
        &h.ctx,
    )
    .await
}

#[tauri::command(async)]
fn set_account_password(id: String, password: String) -> Result<(), UnkaiError> {
    cmds::accounts::set_account_password(id, password)
}

#[tauri::command(async)]
fn set_category_use_as_mailing_list(
    name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::set_category_use_as_mailing_list(name, enabled, &h.ctx.cache)
}

#[tauri::command(async)]
fn set_contact_group_emoji(
    group_id: String,
    emoji: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::set_contact_group_emoji(group_id, emoji, &h.ctx.cache)
}

#[tauri::command(async)]
fn set_contact_group_hidden(
    group_id: String,
    hidden: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::set_contact_group_hidden(group_id, hidden, &h.ctx.cache)
}

#[tauri::command(async)]
fn set_folder_icon(
    account_id: String,
    folder_name: String,
    icon: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::set_folder_icon(account_id, folder_name, icon, &h.ctx.cache, &h.sync_notify)
}

#[tauri::command(async)]
fn set_mailing_list_emoji(
    id: String,
    emoji: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::set_mailing_list_emoji(id, emoji, &h.ctx.cache)
}

#[tauri::command(async)]
fn set_mailing_list_hidden(
    id: String,
    hidden: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::set_mailing_list_hidden(id, hidden, &h.ctx.cache)
}

#[tauri::command]
async fn set_message_flagged(
    account_id: String,
    folder: String,
    uid: u32,
    flagged: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::set_message_flagged(account_id, folder, uid, flagged, &h.ctx.cache).await
}

#[tauri::command]
async fn set_message_pinned(
    account_id: String,
    folder: String,
    uid: u32,
    pinned: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::set_message_pinned(account_id, folder, uid, pinned, &h.ctx.cache).await
}

#[tauri::command]
async fn set_message_priority(
    account_id: String,
    folder: String,
    uid: u32,
    priority: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::set_message_priority(account_id, folder, uid, priority, &h.ctx.cache).await
}

#[tauri::command]
async fn set_message_read(
    account_id: String,
    folder: String,
    uid: u32,
    read: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::set_message_read(
        account_id,
        folder,
        uid,
        read,
        &h.ctx.cache,
        h.ctx.ui.as_ref(),
    )
    .await
}

#[tauri::command]
async fn set_messages_read(
    account_id: String,
    folder: String,
    uids: Vec<u32>,
    read: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::set_messages_read(
        account_id,
        folder,
        uids,
        read,
        &h.ctx.cache,
        h.ctx.ui.as_ref(),
    )
    .await
}

#[tauri::command]
async fn set_message_reminder(
    account_id: String,
    folder: String,
    uid: u32,
    remind_at: Option<i64>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::mail::set_message_reminder(account_id, folder, uid, remind_at, &h.ctx.cache).await
}

#[tauri::command(async)]
fn set_nextcloud_calendar_hidden(
    calendar_id: String,
    hidden: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::set_nextcloud_calendar_hidden(calendar_id, hidden, &h.ctx.cache)
}

#[tauri::command(async)]
fn set_nextcloud_calendar_muted(
    calendar_id: String,
    muted: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::set_nextcloud_calendar_muted(calendar_id, muted, &h.ctx.cache)
}

#[tauri::command(async)]
fn set_nextcloud_task_list_hidden(
    task_list_id: String,
    hidden: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::set_nextcloud_task_list_hidden(task_list_id, hidden, &h.ctx.cache)
}

#[tauri::command(async)]
fn set_nextcloud_task_list_muted(
    task_list_id: String,
    muted: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::set_nextcloud_task_list_muted(task_list_id, muted, &h.ctx.cache)
}

#[tauri::command]
async fn set_settings_sync_target(
    target_nc_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::set_settings_sync_target(target_nc_id, &h.sync_notify, &h.ctx.profile).await
}

#[tauri::command]
async fn set_talk_room_public(
    nc_id: String,
    room_token: String,
    public: bool,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::talk::set_talk_room_public(nc_id, room_token, public, &h.ctx.cache).await
}

#[tauri::command(async)]
fn set_wipe_policy(policy: WipePolicyView, state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::set_wipe_policy(policy, &h.ctx.profile.id)
}

#[tauri::command(async)]
fn smime_disable_unlock_automatically(account_id: String) -> Result<(), UnkaiError> {
    cmds::crypto::smime_disable_unlock_automatically(account_id)
}

#[tauri::command(async)]
fn smime_enable_unlock_automatically(
    account_id: String,
    passphrase: String,
) -> Result<(), UnkaiError> {
    cmds::crypto::smime_enable_unlock_automatically(account_id, passphrase)
}

#[tauri::command(async)]
fn smime_get_account_cert_status(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<SmimeCertStatus, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::smime_get_account_cert_status(account_id, &h.ctx.cache)
}

#[tauri::command(async)]
fn smime_get_certs_for_email(
    email: String,
    state: State<'_, AppState>,
) -> Result<Vec<SmimeCertDto>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::smime_get_certs_for_email(email, &h.ctx.cache)
}

#[tauri::command(async)]
fn smime_has_unlock_automatically(account_id: String) -> Result<bool, UnkaiError> {
    cmds::crypto::smime_has_unlock_automatically(account_id)
}

#[tauri::command(async)]
fn smime_import_pkcs12(
    account_id: String,
    pkcs12_base64: String,
    passphrase: String,
    state: State<'_, AppState>,
) -> Result<String, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::smime_import_pkcs12(
        account_id,
        pkcs12_base64,
        passphrase,
        &h.ctx.cache,
        &h.sync_notify,
    )
}

#[tauri::command(async)]
fn smime_import_public_cert(
    cert_data: String,
    email_hint: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::smime_import_public_cert(cert_data, email_hint, &h.ctx.cache)
}

#[tauri::command(async)]
fn smime_list_public_certs(state: State<'_, AppState>) -> Result<Vec<SmimeCertDto>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::smime_list_public_certs(&h.ctx.cache)
}

#[tauri::command(async)]
fn smime_remove_private_cert(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::smime_remove_private_cert(account_id, &h.ctx.cache, &h.sync_notify)
}

#[tauri::command(async)]
fn smime_remove_public_cert(
    fingerprint: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::smime_remove_public_cert(fingerprint, &h.ctx.cache)
}

#[tauri::command(async)]
fn snooze_event_reminder(
    uid: String,
    snooze_until_iso: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::snooze_event_reminder(uid, snooze_until_iso, &h.ctx.reminders)
}

#[tauri::command]
async fn start_nextcloud_login(
    server_url: String,
    trusted_certs: Option<Vec<unkai_core::models::TrustedCert>>,
) -> Result<LoginFlowInit, UnkaiError> {
    cmds::nextcloud::start_nextcloud_login(server_url, trusted_certs).await
}

#[tauri::command]
async fn sync_calendar_by_id(
    calendar_id: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::sync_calendar_by_id(calendar_id, &h.ctx.cache).await
}

#[tauri::command]
async fn sync_nextcloud_calendars(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<SyncCalendarsReport, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::sync_nextcloud_calendars(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn sync_nextcloud_contacts(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<SyncContactsReport, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::sync_nextcloud_contacts(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn sync_nextcloud_notes(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<unkai_core::models::Note>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::notes::sync_nextcloud_notes(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn sync_nextcloud_task_lists(
    nc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TaskList>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::sync_nextcloud_task_lists(nc_id, &h.ctx.cache).await
}

#[tauri::command]
async fn sync_nextcloud_tasks(
    nc_id: String,
    list_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Task>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::sync_nextcloud_tasks(nc_id, list_id, &h.ctx.cache).await
}

#[tauri::command]
async fn test_connection(
    host: String,
    port: u16,
    username: String,
    password: String,
    trusted_certs: Option<Vec<unkai_core::models::TrustedCert>>,
) -> Result<String, UnkaiError> {
    cmds::accounts::test_connection(host, port, username, password, trusted_certs).await
}

#[tauri::command]
async fn test_jmap_connection(
    jmap_url: String,
    username: String,
    password: String,
) -> Result<String, UnkaiError> {
    cmds::accounts::test_jmap_connection(jmap_url, username, password).await
}

#[tauri::command]
async fn tombstone_draft_for_expunge(
    account_id: String,
    folder: String,
    uid: u32,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::compose::tombstone_draft_for_expunge(account_id, folder, uid, &h.ctx.cache).await
}

#[tauri::command]
async fn try_auto_decrypt_message(
    account_id: String,
    folder: String,
    uid: u32,
    state: State<'_, AppState>,
) -> Result<Option<Email>, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::crypto::try_auto_decrypt_message(account_id, folder, uid, &h.ctx.cache).await
}

#[tauri::command(async)]
fn unlock_with_passphrase(
    passphrase: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::unlock_with_passphrase(passphrase, &h.ctx.cache, &h.ctx.profile.id)
}

#[tauri::command(async)]
fn update_account(account: Account, state: State<'_, AppState>) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::accounts::update_account(account, &h.ctx.cache, &h.sync_notify)
}

#[tauri::command]
async fn update_app_settings(
    new_settings: AppSettings,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::settings::update_app_settings(
        new_settings,
        &h.ctx.settings,
        &h.sync_notify,
        &h.mcp,
        &h.ctx.profile,
    )
    .await
}

#[tauri::command]
async fn update_calendar_event(
    event_id: String,
    input: CalendarEventInput,
    state: State<'_, AppState>,
) -> Result<CalendarEvent, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::update_calendar_event(event_id, input, &h.ctx.cache, h.ctx.ui.as_ref()).await
}

#[tauri::command]
async fn update_contact(
    contact_id: String,
    input: ContactInput,
    state: State<'_, AppState>,
) -> Result<Contact, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::update_contact(contact_id, input, &h.ctx.cache).await
}

#[tauri::command]
async fn update_contact_group(
    group_id: String,
    display_name: Option<String>,
    member_uids: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<ContactGroupView, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::contacts::update_contact_group(group_id, display_name, member_uids, &h.ctx.cache).await
}

#[tauri::command(async)]
fn update_nextcloud_account_trusted_certs(
    nc_id: String,
    trusted_certs: Vec<unkai_core::models::TrustedCert>,
    state: State<'_, AppState>,
) -> Result<NextcloudAccount, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::update_nextcloud_account_trusted_certs(nc_id, trusted_certs, &h.ctx.cache)
}

#[tauri::command]
async fn update_nextcloud_calendar(
    calendar_id: String,
    display_name: Option<String>,
    color: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::calendar::update_nextcloud_calendar(calendar_id, display_name, color, &h.ctx.cache).await
}

#[allow(clippy::too_many_arguments)] // Tauri command: each arg maps to a frontend invoke parameter
#[tauri::command]
async fn update_nextcloud_note(
    nc_id: String,
    note_id: u64,
    etag: String,
    title: Option<String>,
    content: Option<String>,
    category: Option<String>,
    favorite: Option<bool>,
    state: State<'_, AppState>,
) -> Result<unkai_core::models::Note, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::notes::update_nextcloud_note(
        nc_id,
        note_id,
        etag,
        title,
        content,
        category,
        favorite,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command]
async fn update_nextcloud_share(
    nc_id: String,
    share_id: String,
    password: Option<String>,
    permissions: Option<u8>,
    expire_date: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::update_nextcloud_share(
        nc_id,
        share_id,
        password,
        permissions,
        expire_date,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command]
async fn update_nextcloud_share_label(
    nc_id: String,
    share_id: String,
    label: String,
    state: State<'_, AppState>,
) -> Result<(), UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::update_nextcloud_share_label(nc_id, share_id, label, &h.ctx.cache).await
}

#[allow(clippy::too_many_arguments)] // Tauri command: each arg maps to a frontend invoke parameter
#[tauri::command]
async fn update_nextcloud_task(
    nc_id: String,
    list_id: String,
    uid: String,
    etag: String,
    summary: Option<String>,
    description: Option<String>,
    status: Option<String>,
    priority: Option<u8>,
    due_unix: Option<i64>,
    due_tz: Option<String>,
    clear_due: Option<bool>,
    completed_unix: Option<i64>,
    clear_completed: Option<bool>,
    url: Option<String>,
    categories: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<Task, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::tasks::update_nextcloud_task(
        nc_id,
        list_id,
        uid,
        etag,
        summary,
        description,
        status,
        priority,
        due_unix,
        due_tz,
        clear_due,
        completed_unix,
        clear_completed,
        url,
        categories,
        &h.ctx.cache,
    )
    .await
}

#[tauri::command]
async fn upload_to_nextcloud(
    nc_id: String,
    path: String,
    data: Vec<u8>,
    content_type: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, UnkaiError> {
    let h = app_ctx(&state)?;
    cmds::nextcloud::upload_to_nextcloud(nc_id, path, data, content_type, &h.ctx.cache).await
}
// ── App entry point ─────────────────────────────────────────────

/// The mobile entry point.
///
/// On iOS the app is a static library the generated Xcode project
/// links against, so the real entry point is this function rather
/// than `fn main` — `tauri::mobile_entry_point` generates the
/// `extern "C"` symbol UIKit calls.  `src/main.rs` calls the same
/// function so a desktop `cargo run` still works for quick UI
/// iteration in a normal window.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        // Local notifications (new mail, event + message
        // reminders).  On iOS the plugin asks for the OS
        // permission the first time the frontend requests it.
        .plugin(tauri_plugin_notification::init())
        // File picking: PGP / S/MIME key import, `.ics` and
        // `.vcf` import.  Backed by UIDocumentPickerViewController
        // on iOS.  There is no save-dialog counterpart — see
        // `save_attachment_to_documents` above for how saving
        // works on a sandboxed filesystem.
        .plugin(tauri_plugin_dialog::init())
        // Hands URLs to UIKit (`open_url`).
        .plugin(tauri_plugin_opener::init())
        .register_uri_scheme_protocol("contact-photo", contact_photo_protocol)
        .register_uri_scheme_protocol("unkai-logo", logo_protocol)
        .setup(|app| {
            // ── Storage layout ──────────────────────────────────
            //
            // Resolved through Tauri's path API rather than the
            // `dirs` crate the desktop shell uses: on iOS the only
            // writable, backed-up location is inside the app
            // sandbox, and `app_config_dir()` is what resolves
            // there.  The layout *inside* that root is the
            // profile-scoped one `unkai-store` already implements
            // (`profiles/<id>/cache.db` + settings), so the store
            // crate needs no mobile branch at all.
            let root = app
                .path()
                .app_config_dir()
                .map_err(|e| format!("cannot resolve the app config directory: {e}"))?;
            let paths = unkai_store::ProfilePaths::at_root(root);

            // Creates the registry on first run.  Fatal on
            // failure: booting past a half-written layout could
            // open — and wipe-recreate — the wrong database.
            let mut registry = unkai_store::profiles::ensure_registry(&paths)
                .map_err(|e| format!("failed to initialise the profile registry: {e}"))?;
            let profile = registry
                .startup_profile()
                .ok_or("the profile registry is empty")?
                .clone();
            if let Err(e) =
                unkai_store::profiles::touch_last_used(&paths, &mut registry, &profile.id)
            {
                tracing::warn!("could not update profile last-used bookkeeping: {e}");
            }

            // The machine-level shared cache: plaintext SQLite
            // holding data identical for every profile and carrying
            // no user content — today exactly the URLhaus feed
            // snapshot.
            let shared = unkai_store::SharedCache::open(&paths.shared_db())
                .map_err(|e| format!("failed to open the shared cache: {e}"))?;

            app.manage(AppState::new(paths, shared, profile.id.clone()));

            // ── The one app context ─────────────────────────────
            //
            // Built here (not before the builder) because the
            // notifier inside it needs an `AppHandle`.  Opening
            // the encrypted cache, running the boot repairs, and
            // spawning the background loops all happen inside.
            let state = app.state::<AppState>();
            let handle = build_context(
                app.handle(),
                state.profile_id(),
                state.paths(),
                state.shared(),
            )
            .map_err(|e| format!("failed to open the profile: {e}"))?;
            state.set_handle(handle.clone());

            // Paint the first unread count from whatever is already
            // cached, so the Mail tab's badge is truthful before the
            // first sync tick lands.
            refresh_unread_badge(&handle.ctx.cache, handle.ctx.ui.as_ref());

            // The URLhaus link-safety snapshot: one refresh worker
            // per process, running against the shared cache and
            // gated on the profile's `link_check_enabled`.
            let urlhaus_app = app.handle().clone();
            let urlhaus_shared = state.shared().clone();
            tauri::async_runtime::spawn(async move {
                cmds::background::urlhaus_refresh_worker(
                    urlhaus_shared,
                    Arc::new(move || urlhaus_app.state::<AppState>().settings_handles()),
                )
                .await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_account,
            add_contact_to_category,
            add_dav_account,
            add_local_dav_account,
            add_system_dav_account,
            add_talk_participant,
            add_talk_participants,
            archive_message,
            archive_messages,
            check_mail_now,
            check_urls,
            clear_folder,
            count_outbox,
            count_outbox_by_account,
            create_calendar_event,
            create_contact,
            create_contact_group,
            create_demo_account,
            create_folder,
            create_nextcloud_calendar,
            create_nextcloud_directory,
            create_nextcloud_note,
            create_nextcloud_share,
            create_nextcloud_task,
            create_nextcloud_task_from_mail,
            create_talk_room,
            database_status,
            debug_link_check,
            decrypt_message,
            delete_calendar_event,
            delete_contact,
            delete_contact_category,
            delete_contact_group,
            delete_folder,
            delete_message,
            delete_messages,
            delete_nextcloud_calendar,
            delete_nextcloud_note,
            delete_nextcloud_share,
            delete_nextcloud_task,
            delete_outbox_entry,
            delete_talk_room,
            demo_account_exists,
            detect_jmap,
            detect_nc_maps,
            disable_fido_only_mode,
            discover_account_settings,
            dismiss_cancelled_event,
            dismiss_event_reminder,
            download_calendar_from_message,
            download_decrypted_attachment,
            download_email_attachment,
            download_nextcloud_file,
            edit_outbox_entry,
            enable_fido_only_mode,
            expunge_draft_after_send,
            fetch_envelopes,
            fetch_folders,
            fetch_inline_images,
            fetch_message,
            fetch_older_envelopes,
            fetch_older_unified_envelopes,
            fetch_unified_envelopes,
            fetch_unified_special_envelopes,
            fido_enroll_passphrase,
            fido_remove,
            fido_status,
            fido_verify_passphrase,
            find_nextcloud_user_by_email,
            geocode_search,
            get_accounts,
            get_app_settings,
            get_app_version,
            get_archive_folder,
            get_attachment_previews,
            get_attendee_availability,
            get_cached_calendars,
            get_cached_envelopes,
            get_cached_events,
            get_cached_folders,
            get_cached_message,
            get_calendars_sync_status,
            get_contact_photo,
            get_contacts,
            get_contacts_sync_status,
            get_envelopes_by_thread,
            get_event_partstat_for_user,
            get_link_check_status,
            get_nextcloud_accounts,
            get_nextcloud_note,
            get_nextcloud_user_email,
            get_receipt_status,
            get_rsvp_response,
            get_settings_sync_state,
            get_tasks_sync_status,
            get_total_unread,
            get_unified_cached_envelopes,
            get_unified_special_cached_envelopes,
            get_unread_counts_by_account,
            get_wipe_policy,
            import_calendar_file,
            import_contacts_file,
            is_event_in_calendar,
            is_invite_cancelled,
            list_all_outbox,
            list_contact_categories,
            list_contact_groups,
            list_mailing_lists,
            list_nextcloud_addressbooks,
            list_nextcloud_calendars,
            list_nextcloud_files,
            list_nextcloud_groups,
            list_nextcloud_notes,
            list_nextcloud_shares,
            list_nextcloud_task_lists,
            list_nextcloud_tasks,
            list_outbox,
            list_provider_presets,
            list_talk_rooms,
            mark_as_read,
            mark_folder_read,
            move_message,
            move_messages,
            nc_probe_settings_bundle,
            nc_restore_settings_bundle,
            nextcloud_file_preview,
            notify_settings_changed,
            office_close_attachment,
            office_open_attachment,
            office_sweep_temp,
            open_app_settings,
            open_url,
            parse_event_invite,
            pgp_disable_unlock_automatically,
            pgp_enable_unlock_automatically,
            pgp_get_account_key_status,
            pgp_get_keys_for_email,
            pgp_has_unlock_automatically,
            pgp_import_private_key,
            pgp_import_public_key,
            pgp_list_public_keys,
            pgp_remove_private_key,
            pgp_remove_public_key,
            poll_nextcloud_login,
            probe_server_certificate,
            put_attachment_preview,
            record_cancelled_invite,
            refresh_nextcloud_capabilities,
            refresh_urlhaus_now,
            remove_account,
            remove_contact_from_category,
            remove_demo_account,
            remove_nextcloud_account,
            rename_contact_category,
            rename_folder,
            rename_mailing_list,
            rename_talk_room,
            request_system_access,
            respond_mdn_request,
            respond_to_invite,
            retry_outbox_entry,
            retry_outbox_entry_with_passphrase,
            rsvp_existing_event,
            save_attachment_to_documents,
            save_bytes_to_documents,
            save_draft,
            search_contacts,
            search_emails,
            search_imap_server,
            search_imap_server_older,
            send_email,
            set_account_password,
            set_category_use_as_mailing_list,
            set_contact_group_emoji,
            set_contact_group_hidden,
            set_folder_icon,
            set_mailing_list_emoji,
            set_mailing_list_hidden,
            set_message_flagged,
            set_message_pinned,
            set_message_priority,
            set_message_read,
            set_message_reminder,
            set_messages_read,
            set_nextcloud_calendar_hidden,
            set_nextcloud_calendar_muted,
            set_nextcloud_task_list_hidden,
            set_nextcloud_task_list_muted,
            set_settings_sync_target,
            set_talk_room_public,
            set_wipe_policy,
            smime_disable_unlock_automatically,
            smime_enable_unlock_automatically,
            smime_get_account_cert_status,
            smime_get_certs_for_email,
            smime_has_unlock_automatically,
            smime_import_pkcs12,
            smime_import_public_cert,
            smime_list_public_certs,
            smime_remove_private_cert,
            smime_remove_public_cert,
            snooze_event_reminder,
            start_nextcloud_login,
            sync_calendar_by_id,
            sync_nextcloud_calendars,
            sync_nextcloud_contacts,
            sync_nextcloud_notes,
            sync_nextcloud_task_lists,
            sync_nextcloud_tasks,
            system_access_status,
            test_connection,
            test_jmap_connection,
            tombstone_draft_for_expunge,
            try_auto_decrypt_message,
            unlock_with_passphrase,
            update_account,
            update_app_settings,
            update_calendar_event,
            update_contact,
            update_contact_group,
            update_nextcloud_account_trusted_certs,
            update_nextcloud_calendar,
            update_nextcloud_note,
            update_nextcloud_share,
            update_nextcloud_share_label,
            update_nextcloud_task,
            upload_to_nextcloud,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Unkai Mail");
}
