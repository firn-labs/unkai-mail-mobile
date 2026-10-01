// Settings backup & sync helpers (#168).
//
// Three roles:
//   1. Probe & restore — read a bundle a desktop install (or an
//      earlier phone) left on the user's Nextcloud and apply it,
//      mirroring its `localStorage` portion into live storage.
//   2. Notify — every settings UI mutation calls
//      `notifySettingsChanged()` to ping the auto-sync worker so
//      it can debounce + push to the configured Nextcloud.
//
// Secrets are deliberately not part of the bundle: passwords stay
// in the OS keychain, the key-envelope wraps stay in the keychain
// envelope, and the SQLCipher database itself isn't exported.
// Restoring on a fresh install gives the user back every
// preference but still requires re-auth on first connect for each
// account.
//
// The desktop build also writes bundles to a user-chosen file via
// a save dialog. A sandboxed phone has no such path, so Nextcloud
// is the only transport here — which is also the one that moves
// settings between devices without the user carrying a file.

import * as api from './api'
import { TRUSTED_SENDERS_BUNDLE_KEY } from './trustedSenders'

/**
 * `localStorage` keys that carry user-visible state we want to
 * back up, stored under the same name locally and in the bundle.
 * Adding a new key here means it'll automatically be included in
 * the next bundle write.  Keep this list curated: arbitrary keys
 * (like one-shot dismissed-banner flags) probably don't deserve
 * to ride along between machines.
 *
 */
const SYNCED_LOCAL_STORAGE_KEYS = [
  // FIDO unlock toggle (#164).  The wraps live in the keychain
  // envelope — only the *user intent* (am I in encrypted mode?)
  // is on the frontend, and that intent should follow the user
  // across machines.
  'unkai.keyEncryption',
  // Display-language pin (#190): Settings → Language, read by
  // `lib/locale.ts` on every start; absent means "follow the device".
  // Survives bundle import so a user who pinned `de` on one device
  // doesn't get English on the next. (It was `PARAGLIDE_LOCALE`, which
  // Paraglide also wrote on its own at first start — a pin nobody set.)
  'unkai.locale',
] as const

/**
 * Read every synced key from `localStorage` into a plain map.
 * Missing keys are skipped (not encoded as null/empty) so the
 * import side can tell "not set" apart from "explicitly empty".
 */
export function collectLocalStorage(): Record<string, string> {
  const out: Record<string, string> = {}
  try {
    for (const key of SYNCED_LOCAL_STORAGE_KEYS) {
      const v = localStorage.getItem(key)
      if (v !== null) out[key] = v
    }
    // Trusted senders, copied verbatim so the import path doesn't
    // have to know the list's inner shape.
    const trusted = localStorage.getItem(TRUSTED_SENDERS_BUNDLE_KEY)
    if (trusted !== null) out[TRUSTED_SENDERS_BUNDLE_KEY] = trusted
  } catch {
    // localStorage may be unavailable in some webview modes; the
    // bundle still works — it'll just carry an empty map.
  }
  return out
}

/**
 * Write each key from `map` back into `localStorage`.  Keys not
 * present in `map` but expected here are removed — restoring a
 * bundle should mirror the source machine's state, not merge with
 * whatever was already there.
 */
export function applyLocalStorage(map: Record<string, string>) {
  try {
    for (const key of SYNCED_LOCAL_STORAGE_KEYS) {
      const v = map[key]
      if (v === undefined) {
        localStorage.removeItem(key)
      } else {
        localStorage.setItem(key, v)
      }
    }
    // Deliberately asymmetric with the plain keys above: a bundle
    // MISSING this entry is far more likely to predate the key
    // than to mean "the user cleared their whole allow-list", and
    // deleting on restore is unrecoverable while keeping a stale
    // local list is visible and user-fixable. So absent ⇒ leave
    // the local value alone.
    const trusted = map[TRUSTED_SENDERS_BUNDLE_KEY]
    if (trusted !== undefined) localStorage.setItem(TRUSTED_SENDERS_BUNDLE_KEY, trusted)
  } catch {
    /* storage unavailable — silent */
  }
}

/**
 * Ping the auto-sync worker.  Call after any settings UI
 * mutation so the bundle on Nextcloud (if a target is set) gets
 * refreshed.  No-op when sync is off or when no NC is reachable
 * — the worker handles failure / retry on its own.  The
 * frontend does NOT await the eventual NC PUT; this returns as
 * soon as the worker's snapshot has been updated.
 */
export async function notifySettingsChanged(): Promise<void> {
  try {
    await api.settings.notifySettingsChanged({ localStorage: collectLocalStorage() })
  } catch (e) {
    // Failing to update the worker's snapshot is not user-
    // visible — log and move on so the UI action that triggered
    // this isn't held up by a backend hiccup.
    console.warn('notify_settings_changed failed:', e)
  }
}

/** Live view of the auto-sync state for the Settings UI. */
export interface SettingsSyncStateView {
  targetNcId: string | null
  pending: boolean
}

export async function getSyncState(): Promise<SettingsSyncStateView> {
  return api.settings.getSettingsSyncState()
}

/**
 * Set (or clear, with `null`) the NC account that auto-sync
 * pushes to.  Setting it kicks off an immediate push so the
 * chosen NC has a fresh copy without waiting for the next
 * settings change.
 */
export async function setSyncTarget(targetNcId: string | null): Promise<void> {
  await api.settings.setSettingsSyncTarget({ targetNcId })
}

/**
 * Check a connected NC for an existing settings bundle.
 * Returns the bundle's `exported_at` timestamp (RFC 3339) when
 * one is found, `null` when the path doesn't exist.  Surfaces
 * server / auth errors as exceptions; callers should catch and
 * stay quiet — this is only a probe for the "found a backup,
 * restore?" prompt.
 */
export async function ncProbeBundle(ncId: string): Promise<string | null> {
  return api.settings.ncProbeSettingsBundle({ ncId })
}

/**
 * Download + apply the bundle stored on a connected NC. Every
 * preference is restored; passwords still need to be re-entered
 * on first connect. Returns the bundle's `localStorage` portion
 * (already applied locally; returned for callers that want to
 * inspect it).
 */
export async function ncRestoreBundle(ncId: string): Promise<Record<string, string>> {
  const localStorageMap = await api.settings.ncRestoreSettingsBundle({ ncId })
  applyLocalStorage(localStorageMap)
  return localStorageMap
}
