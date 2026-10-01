/**
 * Shell-level affordances that aren't domain data: handing a URL to
 * the OS, saving bytes into the app's Documents directory, the
 * Nextcloud-hosted viewer for Office attachments, and the app
 * version string.
 *
 * Generated wrappers over the backend commands (#473) — one typed
 * function per `#[tauri::command]`. Argument keys mirror the Rust
 * parameter names (camelCased, as Tauri expects on the wire).
 *
 * The desktop build's half of this module (native "Save As",
 * printing, system-font enumeration, window identity, deep-link
 * drains) has no mobile counterpart and is gone — see the mobile
 * shell's `lib.rs` for the full ledger.
 */

import { call } from './core'
import type { OfficeOpenResult } from './types'

export function getAppVersion(): Promise<string> {
  return call('get_app_version')
}

/** Hand a URL to the OS. On iOS this is UIKit's `openURL`, which
 *  leaves the app — used by the Nextcloud login flow and by links
 *  the user taps in a message body. */
export function openUrl(args: { url: string }): Promise<void> {
  return call('open_url', args)
}

/** Open this app's page in the iOS Settings app, where its permissions
 *  (notifications, calendars, contacts, photos…) can be changed. */
export function openAppSettings(): Promise<void> {
  return call('open_app_settings')
}

/**
 * Save an attachment into the app's Documents directory, which the
 * Files app surfaces under "On My iPhone → Unkai Mail".
 *
 * The mobile answer to a "Save As…" dialog: a sandboxed app can't
 * ask the user for a path, so the destination is fixed and the
 * command returns where the file actually landed (it de-duplicates
 * rather than overwriting).
 */
export function saveAttachmentToDocuments(args: {
  accountId: string
  folder: string
  uid: number
  partId: number
  filename: string
}): Promise<string> {
  return call('save_attachment_to_documents', args)
}

/** `saveAttachmentToDocuments` for bytes the frontend already holds
 *  (a decrypted attachment, a generated `.ics`). */
export function saveBytesToDocuments(args: {
  filename: string
  bytes: number[]
}): Promise<string> {
  return call('save_bytes_to_documents', args)
}

/* ── Office viewer ───────────────────────────────────────────────
 *
 * A phone can't render a `.docx` on its own, but the user's own
 * Nextcloud can: upload the attachment to a per-user temp folder,
 * open the returned Files deep link with `openUrl`, delete the temp
 * file afterwards. `officeSweepTemp` clears what an interrupted
 * session left behind. */

export function officeOpenAttachment(args: {
  ncId: string
  filename: string
  data: number[]
  contentType?: string | null
}): Promise<OfficeOpenResult> {
  return call('office_open_attachment', args)
}

export function officeCloseAttachment(args: { ncId: string; tempPath: string }): Promise<void> {
  return call('office_close_attachment', args)
}

export function officeSweepTemp(args: { ncId: string }): Promise<number> {
  return call('office_sweep_temp', args)
}
