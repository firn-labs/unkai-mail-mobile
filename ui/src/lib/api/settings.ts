/**
 * App settings, the Nextcloud-hosted settings backup, and the
 * encrypted vault (passphrase unlock, wipe policy).
 *
 * Generated wrappers over the backend commands (#473) — one typed
 * function per `#[tauri::command]`. Argument keys mirror the Rust
 * parameter names (camelCased, as Tauri expects on the wire).
 *
 * Gone relative to the desktop build: hardware-key (FIDO) unlock,
 * the MCP server, imported themes, and the file-dialog-paired
 * settings export / import — a phone has no security-key port, no
 * localhost clients, and no user-chosen filesystem path. The
 * Nextcloud bundle (`ncProbe` / `ncRestore`) is how settings move
 * between a desktop install and this one.
 */

import { call } from './core'
import type {
  AppSettings,
  DatabaseStatusView,
  FidoStatusView,
  SettingsSyncStateView,
  WipePolicyView,
} from './types'

export function getAppSettings(): Promise<AppSettings> {
  return call('get_app_settings')
}

export function updateAppSettings(args: { newSettings: AppSettings }): Promise<void> {
  return call('update_app_settings', args)
}

export function notifySettingsChanged(args: {
  localStorage: Record<string, unknown>
}): Promise<void> {
  return call('notify_settings_changed', args)
}

export function setSettingsSyncTarget(args: { targetNcId?: string | null }): Promise<void> {
  return call('set_settings_sync_target', args)
}

export function getSettingsSyncState(): Promise<SettingsSyncStateView> {
  return call('get_settings_sync_state')
}

export function ncProbeSettingsBundle(args: { ncId: string }): Promise<string | null> {
  return call('nc_probe_settings_bundle', args)
}

export function ncRestoreSettingsBundle(args: { ncId: string }): Promise<Record<string, string>> {
  return call('nc_restore_settings_bundle', args)
}

export function setWipePolicy(args: { policy: WipePolicyView }): Promise<void> {
  return call('set_wipe_policy', args)
}

export function getWipePolicy(): Promise<WipePolicyView> {
  return call('get_wipe_policy')
}

export function databaseStatus(): Promise<DatabaseStatusView> {
  return call('database_status')
}

export function unlockWithPassphrase(args: { passphrase: string }): Promise<void> {
  return call('unlock_with_passphrase', args)
}

/* ── The local vault ─────────────────────────────────────────────
 *
 * The `fido*` names are the desktop build's, where the same key
 * envelope also holds hardware-key wraps. Mobile exposes only the
 * passphrase half — same envelope, so a vault set up on a desktop
 * install opens here and vice versa. */

/** Which unlock methods the cache key is currently wrapped under,
 *  and whether a plaintext key still sits in the keychain. */
export function fidoStatus(): Promise<FidoStatusView> {
  return call('fido_status')
}

/** Wrap the cache key under a passphrase. Requires the cache to be
 *  unlocked (the key has to be readable to be re-wrapped). */
export function fidoEnrollPassphrase(args: {
  passphrase: string
  label: string
}): Promise<void> {
  return call('fido_enroll_passphrase', args)
}

export function fidoVerifyPassphrase(args: { passphrase: string }): Promise<boolean> {
  return call('fido_verify_passphrase', args)
}

export function fidoRemove(args: { credentialIdB64: string }): Promise<void> {
  return call('fido_remove', args)
}

/** Drop the plaintext key from the keychain, so the passphrase
 *  becomes the only way in. */
export function enableFidoOnlyMode(): Promise<void> {
  return call('enable_fido_only_mode')
}

export function disableFidoOnlyMode(): Promise<void> {
  return call('disable_fido_only_mode')
}
