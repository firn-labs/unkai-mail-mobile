/**
 * Loading envelopes for a mailbox, whichever kind of mailbox it is.
 *
 * The list screen shouldn't have to know that "All Inboxes",
 * "All Sent" and "INBOX on this account" are three different
 * backend commands with three different pagination stories. It asks
 * for a page; this module picks the call.
 *
 * Three cases:
 *   * **One account, one folder** — cursor pagination by UID
 *     (`fetchOlderEnvelopes`), the cheap and exact path.
 *   * **Unified inbox** — a merged fetch that pages by a per-account
 *     UID cursor.
 *   * **Unified special-use** (All Sent / Drafts / …) — the backend
 *     resolves each account's real folder per call and has no cursor
 *     form, so "load more" re-fetches with a larger limit. Fine at
 *     the sizes these mailboxes are actually browsed at, and honest:
 *     the alternative is a cursor the server can't express.
 */

import * as api from '../api'
import type { EmailEnvelope } from '../api'
import { unifiedSpecialKind } from '../unifiedFolders'

export const PAGE_SIZE = 50

export interface MailboxRef {
  /** `null` means "every account" — a unified mailbox. */
  accountId: string | null
  folder: string
}

export function isUnified(ref: MailboxRef): boolean {
  return ref.accountId === null
}

/** Cached-first read, so a mailbox paints before the network answers. */
export async function loadCached(ref: MailboxRef, limit = PAGE_SIZE): Promise<EmailEnvelope[]> {
  const special = unifiedSpecialKind(ref.folder)
  if (isUnified(ref)) {
    if (special) return api.mail.getUnifiedSpecialCachedEnvelopes({ special, limit })
    return api.mail.getUnifiedCachedEnvelopes({ folder: ref.folder, limit })
  }
  return api.mail.getCachedEnvelopes({ accountId: ref.accountId!, folder: ref.folder, limit })
}

/** Authoritative read from the server, which also refreshes the cache. */
export async function loadLive(ref: MailboxRef, limit = PAGE_SIZE): Promise<EmailEnvelope[]> {
  const special = unifiedSpecialKind(ref.folder)
  if (isUnified(ref)) {
    if (special) return api.mail.fetchUnifiedSpecialEnvelopes({ special, limit })
    return api.mail.fetchUnifiedEnvelopes({ folder: ref.folder, limit })
  }
  return api.mail.fetchEnvelopes({ accountId: ref.accountId!, folder: ref.folder, limit })
}

/**
 * The next page below `loaded`.
 *
 * Returns the *whole* list for the special-unified case (which
 * re-fetches with a bigger window) and only the new tail for the
 * cursor cases, so callers get one uniform contract: whatever comes
 * back replaces the list from `loaded.length` onward.
 */
export async function loadOlder(
  ref: MailboxRef,
  loaded: EmailEnvelope[],
  limit = PAGE_SIZE,
): Promise<{ envelopes: EmailEnvelope[]; replaceAll: boolean }> {
  const special = unifiedSpecialKind(ref.folder)

  if (isUnified(ref)) {
    if (special) {
      const envelopes = await api.mail.fetchUnifiedSpecialEnvelopes({
        special,
        limit: loaded.length + limit,
      })
      return { envelopes, replaceAll: true }
    }
    // Oldest UID seen per account is the cursor the merged fetch
    // needs — each account's stream is paged independently.
    const beforeUidPerAccount: Record<string, number> = {}
    for (const env of loaded) {
      const current = beforeUidPerAccount[env.account_id]
      if (current === undefined || env.uid < current) beforeUidPerAccount[env.account_id] = env.uid
    }
    const envelopes = await api.mail.fetchOlderUnifiedEnvelopes({
      folder: ref.folder,
      beforeUidPerAccount,
      limit,
    })
    return { envelopes, replaceAll: false }
  }

  const oldest = loaded.reduce<number | null>(
    (min, env) => (min === null || env.uid < min ? env.uid : min),
    null,
  )
  if (oldest === null) return { envelopes: [], replaceAll: false }
  const envelopes = await api.mail.fetchOlderEnvelopes({
    accountId: ref.accountId!,
    folder: ref.folder,
    beforeUid: oldest,
    limit,
  })
  return { envelopes, replaceAll: false }
}

/**
 * Collapse a list into one row per conversation.
 *
 * Keeps the newest message of each thread and hangs the thread's
 * size off it. Messages with no `thread_id` (most servers, most
 * mail) pass through untouched, so this is safe to run
 * unconditionally when the preference is on.
 */
export function collapseThreads(envelopes: EmailEnvelope[]): EmailEnvelope[] {
  // Value *and* output position per thread. The position is the
  // point: this runs on every list mutation, and looking the row up
  // with `indexOf` made collapsing a thread-heavy mailbox quadratic
  // — which is exactly where it hurts, since a mailbox has to be
  // large before threading matters at all.
  const byThread = new Map<string, { row: EmailEnvelope; at: number }>()
  const out: EmailEnvelope[] = []

  for (const env of envelopes) {
    const key = env.thread_id ? `${env.account_id}::${env.thread_id}` : null
    if (!key) {
      out.push(env)
      continue
    }
    const seen = byThread.get(key)
    if (!seen) {
      byThread.set(key, { row: env, at: out.length })
      out.push(env)
      continue
    }
    const existing = seen.row
    // Keep the newest as the visible row, but let unread anywhere in
    // the thread show on it — a thread with one unread reply must
    // not look read just because its newest message is.
    const merged = new Date(env.date) > new Date(existing.date) ? env : existing
    const other = merged === env ? existing : env
    const combined: EmailEnvelope = {
      ...merged,
      is_read: merged.is_read && other.is_read,
      is_starred: merged.is_starred || other.is_starred,
      thread_total_count: Math.max(
        merged.thread_total_count ?? 1,
        (existing.thread_total_count ?? 1) + 1,
      ),
    }
    seen.row = combined
    out[seen.at] = combined
  }

  return out
}

/** Newest first — the order every mailbox is read in.
 *
 *  Each date is parsed once into a decorated array rather than
 *  inside the comparator: a sort calls its comparator O(n log n)
 *  times, so parsing there means building ~2 · n · log n `Date`
 *  objects for a list that only has n distinct dates in it. */
export function sortByDate(envelopes: EmailEnvelope[]): EmailEnvelope[] {
  return envelopes
    .map((env) => ({ env, at: new Date(env.date).getTime() }))
    .sort((a, b) => b.at - a.at)
    .map((entry) => entry.env)
}

/** De-duplicate by (account, folder, uid) after a merge of cached +
 *  live results, which overlap by design. */
export function dedupe(envelopes: EmailEnvelope[]): EmailEnvelope[] {
  const seen = new Set<string>()
  const out: EmailEnvelope[] = []
  for (const env of envelopes) {
    const key = `${env.account_id}::${env.folder}::${env.uid}`
    if (seen.has(key)) continue
    seen.add(key)
    out.push(env)
  }
  return out
}
