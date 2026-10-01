//! OS-adjacent affordances that need a server round-trip: the
//! Nextcloud-hosted viewer for Office attachments and the sweep
//! that cleans its temp folder up.
//!
//! Mirrors `ui/src/lib/api/system.ts`.  The desktop-only half of
//! this module (native "Save As", printing, system-font
//! enumeration, `open` -crate URL handling) was dropped in the
//! mobile port: a sandboxed phone app has no user-chosen save
//! path, no printer dialog, no font catalogue to enumerate, and
//! reaches the browser through the shell's opener plugin instead.

use serde::Serialize;
use unkai_core::UnkaiError;
use unkai_core::models::NextcloudAccount;
use unkai_store::Cache;
use unkai_store::credentials;

use crate::support::load_nextcloud_account;

// ── Office viewer (issue #65) ────────────────────────────────
//
// Click an Office-compatible attachment in MailView → upload its
// bytes to a per-user temp folder in the user's Nextcloud → return
// the deep-link URL the frontend opens in a Tauri webview window.
// On close, the frontend fires `office_close_attachment` which
// expunges the temp file. A separate `office_sweep_temp` runs at
// connect-time to clean up anything left behind by a crash mid-edit.
//
// Folder layout:
//   /Unkai Mail/temp/<uuid>-<filename>
//
// The UUID prefix lets concurrent edits coexist without filename
// collisions and gives the sweeper an obvious "is-this-ours" gate
// (only delete files inside the temp folder).

/// Root path for Unkai's per-user temp area on the user's
/// Nextcloud. Files-app-visible (no leading dot) so the user can
/// recover anything we somehow lose track of, but tucked under our
/// app's branded folder so the home screen stays uncluttered.
pub const UNKAI_TEMP_ROOT: &str = "/Unkai Mail";

pub const UNKAI_TEMP_DIR: &str = "/Unkai Mail/temp";

/// Result of `office_open_attachment` — the URL the frontend opens
/// in a fresh webview window plus the temp path it should pass back
/// to `office_close_attachment` on close.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficeOpenResult {
    /// Absolute URL into Nextcloud's Files app, which routes the
    /// file id to whichever app is registered as its handler —
    /// Collabora for Office docs, the PDF viewer for `.pdf`. Pasted
    /// directly into a `WebviewWindow` `url` arg.
    pub url: String,
    /// Path on the user's Nextcloud (relative to the user root).
    /// Round-trips back to `office_close_attachment` so the cleanup
    /// targets the file we just uploaded, not "all temp files".
    pub temp_path: String,
}

/// Best-effort `MKCOL` of `/Unkai Mail` and `/Unkai Mail/temp`.
/// Both are idempotent: `create_directory` returns "folder already
/// exists" as `UnkaiError::Nextcloud` which we swallow so a
/// pre-existing folder doesn't fail the open. Anything else
/// propagates so quota / 401 / network errors surface to the user.
pub async fn ensure_temp_dir(
    account: &NextcloudAccount,
    app_password: &str,
) -> Result<(), UnkaiError> {
    for dir in [UNKAI_TEMP_ROOT, UNKAI_TEMP_DIR] {
        match unkai_nextcloud::create_directory(
            &account.server_url,
            &account.username,
            app_password,
            dir,
            &account.trusted_certs,
        )
        .await
        {
            Ok(()) => {}
            Err(UnkaiError::Nextcloud(msg)) if msg.contains("already exists") => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Upload an attachment to the user's Nextcloud temp folder and
/// return the URL to open it in. Used by MailView when the user
/// clicks a `cid:` link or a tray button on an Office-compatible
/// attachment.
pub async fn office_open_attachment(
    nc_id: String,
    filename: String,
    data: Vec<u8>,
    content_type: Option<String>,
    cache: &Cache,
) -> Result<OfficeOpenResult, UnkaiError> {
    let account = load_nextcloud_account(cache, &nc_id)?;
    let app_password = credentials::get_nextcloud_password(&nc_id)?;

    ensure_temp_dir(&account, &app_password).await?;

    // UUID prefix dodges filename collisions between concurrent
    // viewer windows, and gives the sweeper a way to recognise our
    // own files without a metadata round-trip.
    let safe_name = filename.replace(['/', '\\'], "_");
    let temp_path = format!("{}/{}-{}", UNKAI_TEMP_DIR, uuid::Uuid::new_v4(), safe_name);

    unkai_nextcloud::upload_file(
        &account.server_url,
        &account.username,
        &app_password,
        &temp_path,
        data,
        content_type.as_deref(),
        &account.trusted_certs,
    )
    .await?;

    // Resolve the freshly-uploaded file's `oc:fileid` so we can
    // build the canonical `index.php/f/<id>` deep link. That URL
    // routes through Nextcloud's "open with default app" — Files
    // hands `.docx` etc. to Collabora, `.pdf` to the PDF viewer,
    // so the same code path works for both document types without
    // app-specific URL templating on our side.
    let file_id = unkai_nextcloud::propfind_fileid(
        &account.server_url,
        &account.username,
        &app_password,
        &temp_path,
        &account.trusted_certs,
    )
    .await?;

    let server = account.server_url.trim_end_matches('/');
    let url = format!("{server}/index.php/f/{file_id}");

    Ok(OfficeOpenResult { url, temp_path })
}

/// Delete a temp file the frontend opened earlier. Best-effort:
/// 404 is swallowed by `delete_path`, network blips bubble up but
/// the frontend logs and moves on — leftover files get caught by
/// `office_sweep_temp` at next connect.
pub async fn office_close_attachment(
    nc_id: String,
    temp_path: String,
    cache: &Cache,
) -> Result<(), UnkaiError> {
    let account = load_nextcloud_account(cache, &nc_id)?;
    let app_password = credentials::get_nextcloud_password(&nc_id)?;
    unkai_nextcloud::delete_path(
        &account.server_url,
        &account.username,
        &app_password,
        &temp_path,
        &account.trusted_certs,
    )
    .await
}

/// Clean up anything stuck in `/Unkai Mail/temp` from a previous
/// session — say the user closed Unkai mid-edit, or `office_close_
/// attachment` errored on the way out. We list the directory and
/// DELETE every entry whose `last_modified` is older than the cutoff,
/// so an in-flight viewer window in another Unkai instance doesn't
/// have its file pulled out from under it.
pub async fn office_sweep_temp(nc_id: String, cache: &Cache) -> Result<u32, UnkaiError> {
    let account = load_nextcloud_account(cache, &nc_id)?;
    let app_password = credentials::get_nextcloud_password(&nc_id)?;

    // If the temp dir doesn't exist yet (fresh install / first
    // attachment click) treat that as "nothing to sweep". Anything
    // else propagates.
    let entries = match unkai_nextcloud::list_directory(
        &account.server_url,
        &account.username,
        &app_password,
        UNKAI_TEMP_DIR,
        &account.trusted_certs,
    )
    .await
    {
        Ok(e) => e,
        Err(UnkaiError::Nextcloud(msg)) if msg.contains("not found") => return Ok(0),
        Err(e) => return Err(e),
    };

    let cutoff = chrono::Utc::now() - chrono::Duration::hours(1);
    let mut swept = 0u32;
    for entry in entries {
        let stale = entry.modified.map(|t| t < cutoff).unwrap_or(true);
        if !stale {
            continue;
        }
        let target = format!("{UNKAI_TEMP_DIR}/{}", entry.name);
        match unkai_nextcloud::delete_path(
            &account.server_url,
            &account.username,
            &app_password,
            &target,
            &account.trusted_certs,
        )
        .await
        {
            Ok(()) => swept += 1,
            Err(e) => tracing::warn!("office_sweep_temp: failed to delete {target}: {e}"),
        }
    }
    if swept > 0 {
        tracing::info!("office_sweep_temp: cleaned {swept} stale file(s)");
    }
    Ok(swept)
}

// ── On-disk font cache (#142 follow-up) ───────────────────────
//
// Even with the in-memory cache, a cold launch still pays the
// cost of font-kit's catalogue walk — slow on Linux's first-run
// fontconfig and visible enough that the user complained about
// "first compose" lag.  Persist the result to a JSON file in the
// OS cache dir, signed with a cheap fingerprint of the system
// font directories.  Subsequent launches read the JSON in
// microseconds; we only re-run font-kit when the fingerprint
// changes (i.e. the user actually installed or removed a font).
//
// The fingerprint is a SHA-256 of every font-directory mtime
// found by recursive walk.  Adding or removing a file inside any
// directory updates that directory's mtime on every common
// filesystem, so directory mtimes alone catch both additions and
// removals without us needing to stat every individual font file.
