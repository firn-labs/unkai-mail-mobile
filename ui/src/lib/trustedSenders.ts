// Trusted-senders allow-list for the "Always show images from
// [sender]" affordance.
//
// Stored in `localStorage` as a JSON array of lower-cased bare
// email addresses under `unkai-trusted-senders`, which doubles as
// the settings-bundle wire name so the list follows the user to
// their desktop install and back.
//
// (The desktop build namespaces this key per profile because all
// its windows share one storage origin. The mobile app hosts a
// single profile, so the plain key is the profile-scoped key.)
//
// Every mutation calls `notifySettingsChanged()` so the auto-sync
// worker picks the change up immediately — otherwise the list
// lives only on this device until some *other* settings mutation
// happens to push the bundle, and a Nextcloud restore in between
// would wipe the entry (#295).

import { notifySettingsChanged } from './settingsBundle'

/** Settings-bundle wire name, and the local storage key. */
export const TRUSTED_SENDERS_BUNDLE_KEY = 'unkai-trusted-senders'

/**
 * Strip the angle-bracketed address out of an RFC 5322 `From:`
 * string and lower-case it. `"Jane Doe" <jane@example.org>` →
 * `jane@example.org`. When the input has no brackets we treat the
 * whole string as an address.
 */
export function getSenderAddress(from: string): string {
  const m = from.match(/<([^>]+)>/)
  return (m ? m[1] : from).trim().toLowerCase()
}

function readList(): string[] {
  try {
    const raw = localStorage.getItem(TRUSTED_SENDERS_BUNDLE_KEY)
    return raw ? (JSON.parse(raw) as string[]) : []
  } catch {
    return []
  }
}

function writeList(list: string[]): boolean {
  try {
    localStorage.setItem(TRUSTED_SENDERS_BUNDLE_KEY, JSON.stringify(list))
    return true
  } catch {
    return false
  }
}

/** All trusted addresses, sorted for stable display. */
export function listTrustedSenders(): string[] {
  return readList().sort((a, b) => a.localeCompare(b))
}

export function isSenderTrusted(from: string): boolean {
  const addr = getSenderAddress(from)
  if (!addr) return false
  return readList().includes(addr)
}

export function addTrustedSender(from: string): void {
  const addr = getSenderAddress(from)
  if (!addr) return
  const list = readList()
  if (list.includes(addr)) return
  if (writeList([...list, addr])) void notifySettingsChanged()
}

export function removeTrustedSender(fromOrAddr: string): void {
  const addr = getSenderAddress(fromOrAddr)
  if (!addr) return
  const list = readList()
  const next = list.filter((a) => a !== addr)
  if (next.length === list.length) return
  if (writeList(next)) void notifySettingsChanged()
}
