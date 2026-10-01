/**
 * Typed event channel from the backend to the UI (#473).
 *
 * `AppEventPayloads` is the single registry of every event name in
 * the app. Subscribing or emitting an unregistered name is a
 * compile error, which is the point: event names used to be bare
 * strings scattered across components.
 *
 * The desktop build also carried a second family of names here —
 * the popout-window → main-window handoffs (`compose-from-mail`,
 * `event-editor-saved-from-popout`, …). Mobile has one window and
 * one navigation stack, so those handoffs are plain function calls
 * now and the registry lists only what the backend pushes.
 *
 * Handlers receive the raw Tauri `Event<T>` (payload under
 * `event.payload`).
 */

import { emit, listen, type Event, type UnlistenFn } from '@tauri-apps/api/event'

export interface AppEventPayloads {
  /* ── backend → UI push channels ─────────────────────────────── */
  /**
   * The sync loop found new mail in a folder.
   *
   * One event per folder per poll, not one per message: `count` is
   * the size of the whole batch and `from` / `subject` / `uid`
   * describe its newest message. See `NewMailPayload` in
   * `crates/unkai-commands/src/notify.rs` — collapsing the burst in
   * Rust is what keeps a 40-message first sync from firing 40
   * notifications.
   */
  'new-mail': {
    accountId: string
    folder: string
    count: number
    from?: string
    subject?: string
    uid?: number
  }
  /** Flags (read / flagged / pinned) changed server-side. */
  'mail-flags-updated': { accountId: string; folder: string }
  'unread-count-updated': number
  'unread-count-by-account-updated': Record<string, number>
  'outbox-updated': unknown
  'event-reminder': {
    uid: string
    summary: string
    start: string
    minutesBefore: number
    location?: string | null
    meetingUrl?: string | null
    attendees?: string[]
  }
  'message-reminder': {
    accountId: string
    folder: string
    uid: number
    from: string
    subject: string
  }
  'calendars-updated': unknown
  'custom-themes-changed': unknown
  /** The machine-global profile registry changed. Mobile hosts one
   *  profile, so this only ever fires from a settings restore. */
  'profiles-changed': null
}

export type AppEventName = keyof AppEventPayloads

/**
 * Subscribe to an app event.
 *
 * A plain global `listen()`: the mobile shell has exactly one
 * window, so there is no per-window targeting to respect (the
 * desktop build scopes each listener to its profile's window).
 */
export function onAppEvent<K extends AppEventName>(
  name: K,
  handler: (event: Event<AppEventPayloads[K]>) => void,
): Promise<UnlistenFn> {
  return listen(name, handler)
}

/** Emit an app event — used by tests and by the few places the UI
 *  wants to re-broadcast something it computed itself. */
export function emitAppEvent<K extends AppEventName>(
  name: K,
  payload: AppEventPayloads[K],
): Promise<void> {
  return emit(name, payload)
}
