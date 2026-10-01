/**
 * Platform affordances that aren't backend commands (#473): the
 * native file picker, OS notifications, and the custom-protocol
 * asset URLs (`contact-photo://`, `unkai-logo://`) that
 * `convertFileSrc` turns into webview-loadable URLs.
 *
 * Same rationale as `api/core`: components stay free of direct
 * `@tauri-apps/*` imports so the platform-only surface is
 * enumerable in one file — this module IS the list of what a
 * non-Tauri build would have to re-provide.
 *
 * Autostart is gone relative to the desktop build (iOS has no
 * login-item model), and so is the save dialog: saving goes to the
 * app's Documents directory through `api.system.save*ToDocuments`.
 */

import { convertFileSrc } from '@tauri-apps/api/core'
import { open as dialogOpen, type OpenDialogOptions } from '@tauri-apps/plugin-dialog'
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
  type Options as NotificationOptions,
} from '@tauri-apps/plugin-notification'

/* ── file picking ──────────────────────────────────────────────── */
//
// Backed by UIDocumentPickerViewController on iOS: the user picks
// out of Files (iCloud Drive, on-device, third-party providers).
// Used for key / certificate import and `.ics` / `.vcf` import.
// Picking *attachments* deliberately does NOT go through here — a
// plain `<input type="file">` in the webview gives the user the
// photo library and camera as well, which is what people reach for
// on a phone.

export function openFileDialog(options?: OpenDialogOptions): Promise<string | string[] | null> {
  return dialogOpen(options)
}

/* ── notifications ─────────────────────────────────────────────── */
//
// Local notifications only (there is no push server): new mail
// found by the foreground sync loop, event reminders, message
// reminders. iOS asks the user for the permission the first time
// `requestNotificationsPermission` runs.

export function notificationsPermissionGranted(): Promise<boolean> {
  return isPermissionGranted()
}

export function requestNotificationsPermission(): Promise<NotificationPermission> {
  return requestPermission()
}

export function showNotification(options: NotificationOptions | string): void {
  sendNotification(options)
}

/* ── custom-protocol asset URLs ────────────────────────────────── */

export function assetUrl(filePath: string, protocol?: string): string {
  return convertFileSrc(filePath, protocol)
}
