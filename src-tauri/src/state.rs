//! The mobile runtime's single application context.
//!
//! The desktop shell hosts N profile contexts and routes every
//! command through a window-label → profile map (#533).  A phone
//! has exactly one window and one user, so the mobile shell keeps
//! the *storage* layout profiles introduced (`profiles/<id>/`, one
//! SQLCipher key per profile — see `unkai_store::paths`) but hosts
//! only the startup profile.  That keeps `unkai-store` untouched
//! and leaves the door open for a profile switcher later: adding
//! one means swapping the handle inside [`AppState`], not
//! reintroducing a window registry.
//!
//! Every `#[tauri::command]` shim that touches profile state takes
//! `state: State<'_, AppState>` and resolves once through
//! [`app_ctx`] — never an inline lookup, the same rule the desktop
//! shell holds for `profile_ctx`.

use std::sync::{Arc, RwLock};

use tauri::AppHandle;
use unkai_commands::background::{
    background_sync_loop, message_reminder_loop, prerender_inboxes_on_launch, settings_sync_worker,
};
use unkai_commands::state::{
    AppContext, EventReminderState, ProfileInfo, SettingsSyncNotify, SharedLocalStorage,
    SharedSettings,
};
use unkai_core::UnkaiError;
use unkai_mcp::McpServer;
use unkai_store::{Cache, ProfilePaths, SharedCache, account_store, app_settings};

use crate::notifier::MobileNotifier;

/// Everything that exists once per hosted profile.
///
/// Mirrors the desktop shell's `ProfileHandle` minus the window
/// bookkeeping.  The `mcp` member is a parked, never-started
/// [`McpServer`]: the application layer's settings command takes
/// one so it can reconcile the listener on every settings write,
/// and the mobile build keeps `mcp_enabled` pinned to `false`
/// (see [`load_mobile_settings`]) so it never binds a port.
pub struct ProfileHandle {
    pub ctx: AppContext,
    /// Parked MCP server — mobile never enables it, but
    /// `update_app_settings` wants the handle to reconcile against.
    pub mcp: McpServer,
    /// The frontend's latest `localStorage` snapshot, pushed into
    /// the Nextcloud settings backup bundle (#168).
    pub local_storage: SharedLocalStorage,
    /// Wake channel for the settings-sync worker.
    pub sync_notify: SettingsSyncNotify,
}

/// Managed state: the storage layout plus the one live context.
///
/// The context is built inside `.setup()` (it needs an `AppHandle`
/// for the notifier), so the handle slot starts empty and every
/// command resolves through [`app_ctx`], which turns "not built
/// yet" into a real error instead of a panic.
pub struct AppState {
    paths: ProfilePaths,
    shared: SharedCache,
    profile_id: String,
    handle: RwLock<Option<Arc<ProfileHandle>>>,
}

impl AppState {
    pub fn new(paths: ProfilePaths, shared: SharedCache, profile_id: String) -> Self {
        Self {
            paths,
            shared,
            profile_id,
            handle: RwLock::new(None),
        }
    }

    pub fn paths(&self) -> &ProfilePaths {
        &self.paths
    }

    pub fn shared(&self) -> &SharedCache {
        &self.shared
    }

    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub fn set_handle(&self, handle: Arc<ProfileHandle>) {
        *self.handle.write().expect("app handle lock poisoned") = Some(handle);
    }

    pub fn handle(&self) -> Option<Arc<ProfileHandle>> {
        self.handle
            .read()
            .expect("app handle lock poisoned")
            .clone()
    }

    /// Settings handle of the live context, for the machine-level
    /// URLhaus worker's "is link checking on" gate (#532).
    pub fn settings_handles(&self) -> Vec<SharedSettings> {
        self.handle()
            .map(|h| vec![h.ctx.settings.clone()])
            .unwrap_or_default()
    }
}

/// THE resolution helper: every command shim calls this — and only
/// this — to reach the running context.
pub fn app_ctx(state: &tauri::State<'_, AppState>) -> Result<Arc<ProfileHandle>, UnkaiError> {
    state
        .handle()
        .ok_or_else(|| UnkaiError::Storage("the app context is not open yet".into()))
}

/// Load the profile's settings with the desktop-only switches
/// forced off.
///
/// `mcp_enabled` would bind a localhost HTTP listener that nothing
/// on a phone can reach, and `autostart_enabled` / `start_minimized`
/// / `minimize_to_tray` describe a desktop session model that has
/// no iOS equivalent.  Pinning them here means a settings file
/// synced down from a desktop install can never switch on machinery
/// the mobile build doesn't ship.
pub fn load_mobile_settings(
    paths: &ProfilePaths,
    profile_id: &str,
) -> unkai_core::models::AppSettings {
    let mut settings =
        app_settings::load_settings(&paths.app_settings(profile_id)).unwrap_or_default();
    settings.mcp_enabled = false;
    settings.autostart_enabled = false;
    settings.start_minimized = false;
    settings.minimize_to_tray = false;
    settings
}

/// Open the profile's cache, run the boot repairs, and spawn the
/// background loops — the mobile twin of the desktop shell's
/// `build_profile_handle`.
///
/// A cache-open failure is fatal for the same reason it is on the
/// desktop: without the cache the write-through path is broken and
/// the app would silently lose its offline store.
pub fn build_context(
    app: &AppHandle,
    profile_id: &str,
    paths: &ProfilePaths,
    shared: &SharedCache,
) -> Result<Arc<ProfileHandle>, UnkaiError> {
    let cache = Cache::open_for_profile(&paths.cache_db(profile_id), profile_id)?;

    // Scrub cache rows left behind by accounts that were removed
    // while a previous run was mid-write.  Cheap, and it keeps the
    // unified inbox from painting envelopes whose account is gone.
    match account_store::load_accounts(&cache) {
        Ok(accounts) => {
            let active_ids: Vec<String> = accounts.iter().map(|a| a.id.clone()).collect();
            if let Err(e) = cache.prune_orphan_accounts(&active_ids) {
                tracing::warn!("startup orphan-account prune failed: {e}");
            }
        }
        Err(e) => {
            tracing::warn!("skipping startup orphan-account prune — load_accounts failed: {e}")
        }
    }

    // One-time backfill for `addresses_json` on contacts synced by
    // an older build (the column was added with an empty default
    // and CardDAV delta-sync never re-pulls unchanged contacts).
    match cache.backfill_addresses(|raw| {
        let p = unkai_carddav::parse_vcard(raw).ok()?;
        Some(
            p.addresses
                .into_iter()
                .map(|a| unkai_core::models::ContactAddress {
                    kind: a.kind,
                    street: a.street,
                    locality: a.locality,
                    region: a.region,
                    postal_code: a.postal_code,
                    country: a.country,
                })
                .collect(),
        )
    }) {
        Ok(0) => {}
        Ok(n) => tracing::info!("contact backfill: rewrote addresses_json on {n} rows"),
        Err(e) => tracing::warn!("contact backfill failed: {e}"),
    }

    let settings = load_mobile_settings(paths, profile_id);
    let shared_settings: SharedSettings = Arc::new(tokio::sync::RwLock::new(settings));

    let ctx = AppContext {
        cache: cache.clone(),
        shared: shared.clone(),
        settings: shared_settings.clone(),
        reminders: Arc::new(EventReminderState::default()),
        ui: Arc::new(MobileNotifier::new(app.clone())),
        profile: Arc::new(ProfileInfo {
            id: profile_id.to_string(),
            paths: paths.clone(),
        }),
    };

    // Parked, never reconciled into a listener (see the struct doc).
    let mcp = McpServer::new(cache, shared_settings, None);
    let local_storage: SharedLocalStorage =
        Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));
    let sync_notify = SettingsSyncNotify(Arc::new(tokio::sync::Notify::new()));

    // ── Background loops ────────────────────────────────────────
    //
    // The same set the desktop spawns, minus the tray plumbing.
    // Spawned and detached: the context lives for the whole
    // process on mobile (there is no profile switch to tear one
    // down), so nothing ever needs to join or abort them.  iOS
    // suspends the process shortly after the app leaves the
    // foreground, so these run while the app is on screen and for
    // the OS-granted grace period after backgrounding — see the
    // mobile-sync note in CLAUDE.md.
    let bg_ctx = ctx.clone();
    tauri::async_runtime::spawn(async move {
        background_sync_loop(bg_ctx).await;
    });

    // Deliberately not gated on the background-sync setting: a
    // reminder the user set must fire even with polling off.
    let reminder_ctx = ctx.clone();
    tauri::async_runtime::spawn(async move {
        message_reminder_loop(reminder_ctx).await;
    });

    // Warm the message cache for the newest INBOX envelopes so the
    // first tap paints from cache instead of waiting on IMAP.
    let prerender_ctx = ctx.clone();
    tauri::async_runtime::spawn(async move {
        prerender_inboxes_on_launch(&prerender_ctx).await;
    });

    let sync_ctx = ctx.clone();
    let sync_storage = local_storage.clone();
    let worker_notify = sync_notify.0.clone();
    tauri::async_runtime::spawn(async move {
        settings_sync_worker(sync_ctx, sync_storage, worker_notify).await;
    });
    if unkai_store::settings_sync::load_state(&ctx.profile.settings_sync_file())
        .map(|s| s.pending && s.target_nc_id.is_some())
        .unwrap_or(false)
    {
        sync_notify.0.notify_one();
    }

    Ok(Arc::new(ProfileHandle {
        ctx,
        mcp,
        local_storage,
        sync_notify,
    }))
}
