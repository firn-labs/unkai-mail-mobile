/**
 * Tells every mounted message list about a change made elsewhere.
 *
 * The reader acts on one message while the list it came from is still
 * mounted underneath (`App.svelte` keeps mail lists alive across a
 * push, see `KEEP_ALIVE`). Archive, Delete and Move used to wait for the
 * server before going back — 1–3 seconds on a phone connection, with
 * nothing on screen moving — and only then let the list re-read its
 * cache. Now the reader goes back at once and tells the list here; the
 * server call finishes behind it, and a failure puts the row back.
 *
 * Function calls, not events: there is one window, so the reader and
 * the list share a process and a module graph. Lists register on mount
 * and unregister on destroy, so a message is only ever hidden in lists
 * that actually show it.
 */
import type { EmailEnvelope } from '../api'

export interface ListHandle {
  /** Take the row out of the list while the server works on it. */
  hide(key: string): void
  /** Put a row taken out by `hide` back (the server refused). */
  unhide(key: string): void
  /** The server confirmed: the row stays gone for good. Until then a
   *  list keeps it filtered even out of fresh loads, which may still
   *  see the message where it was. */
  settle(key: string): void
  /** Change fields of a row in place (read / flagged state). */
  patch(key: string, fields: Partial<EmailEnvelope>): void
}

const lists = new Set<ListHandle>()

/** The key every list uses for a row: account, folder and UID. */
export function envelopeKey(ref: { account_id: string; folder: string; uid: number }): string {
  return `${ref.account_id}::${ref.folder}::${ref.uid}`
}

export const mailLists = {
  /** Register a list; the returned function unregisters it. */
  register(handle: ListHandle): () => void {
    lists.add(handle)
    return () => lists.delete(handle)
  },
  hide(key: string): void {
    for (const list of lists) list.hide(key)
  },
  unhide(key: string): void {
    for (const list of lists) list.unhide(key)
  },
  settle(key: string): void {
    for (const list of lists) list.settle(key)
  },
  patch(key: string, fields: Partial<EmailEnvelope>): void {
    for (const list of lists) list.patch(key, fields)
  },
}
