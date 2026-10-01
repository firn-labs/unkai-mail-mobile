<script lang="ts" module>
  import type { MailboxRef as MailboxRefType } from '../mail/envelopes'

  /**
   * When each mailbox last completed a live refresh.
   *
   * Module scope on purpose: the point is to survive this component
   * being unmounted and remounted, which is exactly what navigating
   * away and back does. Bounded by the number of mailboxes the user
   * visits in a session, so it needs no eviction.
   */
  const lastRefreshedAt = new Map<string, number>()

  function mailboxKey(ref: MailboxRefType): string {
    return `${ref.accountId ?? '*'}::${ref.folder}`
  }
</script>

<script lang="ts">
  /**
   * The message list — the screen this app is judged on.
   *
   * Three things it has to get right:
   *
   *   * **Instant paint.** Cached envelopes render before the
   *     network is touched; the live fetch merges in behind them.
   *     A mail app that shows a spinner on every open feels broken
   *     even when it's fast.
   *   * **Triage by thumb.** Swipe left to archive or delete, swipe
   *     right to flip read state, long-press for everything else.
   *     Actions apply immediately and offer Undo rather than asking
   *     first (see `SwipeRow`).
   *   * **Honest emptiness.** "No messages" and "nothing matches
   *     your filter" are different sentences.
   */
  import * as api from '../api'
  import type { EmailEnvelope } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import PullToRefresh from '../ui/PullToRefresh.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import SwipeRow from '../ui/SwipeRow.svelte'
  import Fab from '../ui/Fab.svelte'
  import ActionSheet, { type SheetAction } from '../ui/ActionSheet.svelte'
  import Confirm from '../ui/Confirm.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { settingsStore } from '../settingsStore.svelte'
  import { accountsStore } from '../accountsStore.svelte'
  import { listDate, displayName } from '../format'
  import { parseFromHeader } from '../fromHeader'
  import {
    PAGE_SIZE,
    collapseThreads,
    dedupe,
    loadCached,
    loadLive,
    loadOlder,
    sortByDate,
    type MailboxRef,
  } from '../mail/envelopes'
  import { INBOX, specialUse, type FolderLike } from '../mail/folders'
  import { folderLabel } from '../mail/folderNames'
  import { archiveFolderFor, isArchiveFolder } from '../mail/archive'
  import { formatError } from '../errors'
  import { debounce, singleFlight } from '../schedule'
  import { envelopeKey, mailLists } from '../mail/listBus'
  import { inboxScope } from '../mail/inboxScope.svelte'
  import { unifiedSpecialKind } from '../unifiedFolders'
  import { SvelteSet } from 'svelte/reactivity'
  import { m } from '../../paraglide/messages'

  interface Props {
    /** `null` → unified across accounts. */
    accountId: string | null
    folder: string
    title: string
    /**
     * This list is the Mail tab's root — the inbox the app opens on.
     * It gets a large title and, where a pushed list has its Back
     * button, a "Mailboxes" button that opens the folder list.
     */
    root?: boolean
    /**
     * Whether this list is the screen on top. The shell keeps mail lists
     * mounted underneath the reader (`KEEP_ALIVE` in `App.svelte`), so
     * going back is instant and keeps the scroll position; coming back
     * to the top is when a list catches up with the server, under the
     * same throttle a fresh mount used to have.
     */
    active?: boolean
    /** Unread messages per account, for the inbox's account switcher. */
    unreadByAccount?: Record<string, number>
    oncompose: (initial?: Record<string, unknown>) => void
  }

  let {
    accountId,
    folder,
    title,
    root = false,
    active = true,
    unreadByAccount = {},
    oncompose,
  }: Props = $props()

  const ref = $derived<MailboxRef>({ accountId, folder })

  /**
   * Skip the on-mount network fetch for a mailbox refreshed this
   * recently.
   *
   * This screen is destroyed and rebuilt on every navigation — the
   * shell keys the screen host on the stack entry, so opening a
   * message and coming back is a fresh mount. Without a throttle that
   * means a full IMAP round trip *every time the user backs out of a
   * message*: connect, log in, SELECT, FETCH, log out, to re-learn
   * what was on screen fifteen seconds ago. That is the single most
   * common navigation in a mail app, and paying for a sync on each one
   * is why moving around felt slow.
   *
   * Thirty seconds is the floor the background sync loop already
   * enforces on itself (`MIN_SYNC_INTERVAL_SECS`), so within the
   * window the loop is the thing keeping the mailbox current — and it
   * pushes `new-mail`, which lands here as a fresh mount anyway.
   *
   * Deliberately *not* throttled: a pull, the empty state's Retry, and
   * the recovery after emptying a folder. Those are the user asking,
   * and the answer to asking is always to go and look.
   */
  const MOUNT_REFRESH_THROTTLE_MS = 30_000

  function refreshedRecently(target: MailboxRef): boolean {
    const at = lastRefreshedAt.get(mailboxKey(target))
    return at !== undefined && Date.now() - at < MOUNT_REFRESH_THROTTLE_MS
  }

  let envelopes = $state<EmailEnvelope[]>([])
  let loading = $state(true)
  let refreshing = $state(false)
  let loadingMore = $state(false)
  let exhausted = $state(false)
  let error = $state('')

  /** Long-pressed row awaiting its action sheet. */
  let sheetFor = $state<EmailEnvelope | null>(null)
  /** Multi-select mode, entered from the nav bar. */
  let selecting = $state(false)
  let selected = $state<Set<string>>(new Set())

  /**
   * Rows the reader took away while the server is still moving them
   * (`mail/listBus.ts`). Filtered rather than deleted, so a refusal can
   * show the very same row again — and so a load that lands meanwhile,
   * and may still see the message where it was, cannot bring it back.
   */
  const hiddenKeys = new SvelteSet<string>()

  const conversationView = $derived(settingsStore.value?.conversation_view_enabled ?? true)
  const rows = $derived.by(() => {
    const visible = hiddenKeys.size
      ? envelopes.filter((e) => !hiddenKeys.has(keyOf(e)))
      : envelopes
    return sortByDate(conversationView ? collapseThreads(visible) : visible)
  })
  const isDrafts = $derived(
    specialUse({ name: folder, delimiter: '/', attributes: [] }) === 'drafts',
  )
  const isTrash = $derived(
    specialUse({ name: folder, delimiter: '/', attributes: [] }) === 'trash',
  )
  const isJunk = $derived(
    specialUse({ name: folder, delimiter: '/', attributes: [] }) === 'junk',
  )
  /**
   * Emptying a mailbox in one go is only offered where "delete
   * everything" is the routine action rather than a mistake waiting
   * to happen: Spam and Trash. It also needs a single real mailbox —
   * `clear_folder` is one account's IMAP session, so a unified view
   * (`accountId === null`) has no single folder to clear.
   */
  const canEmpty = $derived(accountId !== null && (isTrash || isJunk))
  let confirmEmpty = $state(false)
  let emptying = $state(false)

  async function emptyFolder() {
    confirmEmpty = false
    if (!accountId) return
    emptying = true
    try {
      const deleted = await api.mail.clearFolder({ accountId, folder })
      envelopes = []
      exhausted = true
      toasts.show(
        deleted === 1
          ? m.mobile_empty_folder_done_one()
          : m.mobile_empty_folder_done_many({ count: deleted }),
      )
    } catch (e) {
      toasts.error(m.mobile_empty_folder_failed(), e)
      // The server's view is the one that counts and we no longer
      // know it — re-read rather than leave a stale list on screen.
      await refresh()
    } finally {
      emptying = false
    }
  }

  const keyOf = envelopeKey

  async function initialLoad() {
    loading = true
    error = ''
    try {
      envelopes = dedupe(await loadCached(ref))
    } catch {
      // No cache yet — the live fetch below is the first paint.
    }
    loading = false
    // Whether the network is touched on mount is a separate question
    // from whether the cache is painted — see `refreshedRecently`.
    if (refreshedRecently(ref) && envelopes.length > 0) return
    await refresh()
  }

  /**
   * Pull the authoritative page from the server.
   *
   * `singleFlight` matters more here than it looks. Four things call
   * this — the mount effect, the pull gesture, the empty state's
   * Retry, and the recovery path in `emptyFolder` — and on a phone
   * they overlap constantly: a pull that starts while the mount's
   * refresh is still out, a Retry tapped because the first attempt is
   * taking a while. Two in flight both assign `envelopes` when they
   * land, so the *slower* one wins and can overwrite fresher results
   * with staler ones. Worse, each one holds its own IMAP connection
   * open, which is exactly what makes the second attempt slower than
   * the first.
   *
   * Now the second caller rides the first's promise, and a request
   * that genuinely arrived against stale state gets one re-run after
   * it — see `singleFlight`.
   */
  const refresh = singleFlight(async () => {
    refreshing = true
    try {
      const live = await loadLive(ref)
      // The live page is authoritative for the window it covers, so
      // it goes first and the cached tail fills in below it.
      envelopes = dedupe([...live, ...envelopes])
      exhausted = live.length < PAGE_SIZE
      error = ''
      // Only a refresh that actually reached the server counts
      // against the throttle: a failed one must leave the next mount
      // free to try again.
      lastRefreshedAt.set(mailboxKey(ref), Date.now())
    } catch (e) {
      error = formatError(e)
      // Only shout when there is nothing on screen: a failed refresh
      // over a readable cached list is a hairline, not an alert.
      if (envelopes.length === 0) toasts.error(m.mobile_mail_load_failed(), e)
    } finally {
      refreshing = false
    }
  })

  async function loadMore() {
    if (loadingMore || exhausted || loading) return
    loadingMore = true
    try {
      const { envelopes: page, replaceAll } = await loadOlder(ref, envelopes)
      if (replaceAll) {
        const before = envelopes.length
        envelopes = dedupe(page)
        exhausted = envelopes.length <= before
      } else {
        if (page.length === 0) exhausted = true
        envelopes = dedupe([...envelopes, ...page])
      }
    } catch (e) {
      // Reaching the end of a mailbox and losing the network look
      // the same from here; stop paging either way and let a pull
      // re-arm it.
      exhausted = true
      console.warn('load more failed', e)
    } finally {
      loadingMore = false
    }
  }

  $effect(() => {
    void folder
    void accountId
    void initialLoad()
  })

  /* ── Staying current while kept alive ────────────────────────── */

  /**
   * Back on top after the reader (or another tab): catch up with the
   * server, unless this mailbox was refreshed within the throttle — the
   * check a remount used to make, now made on return instead.
   */
  let wasActive = true
  $effect(() => {
    const isActive = active
    if (isActive && !wasActive && !loading && !refreshedRecently(ref)) void refresh()
    wasActive = isActive
  })

  /** Re-read the cache — cheap, local, and enough after a background
   *  sync, which has already written what it found there. */
  const rereadCache = debounce(async () => {
    try {
      envelopes = dedupe([...(await loadCached(ref)), ...envelopes])
    } catch {
      // The rows on screen stay; the next pull reconciles.
    }
  }, 300)

  function concernsThisList(payload: { accountId: string; folder: string }): boolean {
    if (accountId !== null && payload.accountId !== accountId) return false
    return payload.folder === folder || unifiedSpecialKind(folder) !== null
  }

  /**
   * New mail and flag changes from the background sync. A list used to
   * learn about them only by being rebuilt on the next navigation; kept
   * alive, it has to listen.
   */
  $effect(() => {
    const offs = [
      api.onAppEvent('new-mail', (event) => {
        if (concernsThisList(event.payload)) void rereadCache()
      }),
      api.onAppEvent('mail-flags-updated', (event) => {
        if (concernsThisList(event.payload)) void rereadCache()
      }),
    ]
    return () => {
      rereadCache.cancel()
      for (const off of offs) void off.then((un) => un())
    }
  })

  /** Changes the reader makes to a message this list shows. */
  $effect(() =>
    mailLists.register({
      hide: (key) => hiddenKeys.add(key),
      unhide: (key) => hiddenKeys.delete(key),
      settle(key) {
        envelopes = envelopes.filter((e) => keyOf(e) !== key)
        hiddenKeys.delete(key)
      },
      patch(key, fields) {
        envelopes = envelopes.map((e) => (keyOf(e) === key ? { ...e, ...fields } : e))
      },
    }),
  )

  /* ── The inbox's account switch ──────────────────────────────── */

  /** Several accounts: the inbox title carries a switch between all of
   *  them and each one (`mail/inboxScope`). One account: nothing to pick. */
  const canSwitchAccount = $derived(root && accountsStore.list.length > 1)
  let accountSheet = $state(false)

  function accountName(account: { display_name?: string; email: string }): string {
    return account.display_name || account.email
  }

  const scopeLabel = $derived.by(() => {
    if (accountId === null) return m.mobile_all_accounts()
    const account = accountsStore.list.find((a) => a.id === accountId)
    return account ? accountName(account) : ''
  })

  function unreadDetail(count: number): string | undefined {
    return count > 0 ? m.mobile_unread_count({ count }) : undefined
  }

  function accountActions(): SheetAction[] {
    const total = Object.values(unreadByAccount).reduce((sum, n) => sum + n, 0)
    return [
      {
        label: m.mobile_all_accounts(),
        icon: 'global-inbox',
        detail: unreadDetail(total),
        selected: accountId === null,
        run: () => inboxScope.set(null),
      },
      ...accountsStore.list.map((account) => ({
        label: accountName(account),
        icon: 'email-envelope' as const,
        // The address under the name — unless the name *is* the address.
        detail:
          [
            accountName(account) !== account.email ? account.email : '',
            unreadDetail(unreadByAccount[account.id] ?? 0),
          ]
            .filter(Boolean)
            .join(' · ') || undefined,
        selected: accountId === account.id,
        run: () => inboxScope.set(account.id),
      })),
    ]
  }

  /* ── Archive vs. back to Inbox ───────────────────────────────── */

  /**
   * Each account's archive folder, as the backend picks it.
   *
   * Offering Archive on a message that is already in that folder made
   * `archive_message` answer with its archive-to-self no-op while the
   * list removed the row anyway — the message vanished and came back on
   * the next open. A message in the archive gets "Move to Inbox"
   * instead. Keyed per account because the unified view mixes them.
   */
  let archiveByAccount = $state<Record<string, string | null>>({})

  $effect(() => {
    const ids = new Set(envelopes.map((e) => e.account_id))
    if (accountId) ids.add(accountId)
    for (const id of ids) {
      if (id in archiveByAccount) continue
      void archiveFolderFor(id).then((folderName) => {
        archiveByAccount = { ...archiveByAccount, [id]: folderName }
      })
    }
  })

  function inArchive(env: EmailEnvelope): boolean {
    return isArchiveFolder(env.folder, archiveByAccount[env.account_id])
  }

  /** The whole visible list sits in an archive folder — what the
   *  multi-select bar's button acts on. */
  const listInArchive = $derived(rows.length > 0 && rows.every(inArchive))

  /* ── Row actions ─────────────────────────────────────────────── */

  function removeLocally(env: EmailEnvelope) {
    envelopes = envelopes.filter((e) => keyOf(e) !== keyOf(env))
  }

  function restoreLocally(env: EmailEnvelope) {
    envelopes = dedupe([env, ...envelopes])
  }

  async function archive(env: EmailEnvelope) {
    removeLocally(env)
    try {
      await api.mail.archiveMessage({
        accountId: env.account_id,
        folder: env.folder,
        uid: env.uid,
      })
      toasts.show(m.mobile_archived())
    } catch (e) {
      restoreLocally(env)
      toasts.error(m.mobile_archive_failed(), e)
    }
  }

  async function unarchive(env: EmailEnvelope) {
    removeLocally(env)
    try {
      await api.mail.moveMessage({
        accountId: env.account_id,
        folder: env.folder,
        uid: env.uid,
        destFolder: INBOX,
      })
      toasts.show(m.mobile_unarchived())
    } catch (e) {
      restoreLocally(env)
      toasts.error(m.mobile_move_failed(), e)
    }
  }

  async function trash(env: EmailEnvelope) {
    removeLocally(env)
    try {
      await api.mail.deleteMessage({
        accountId: env.account_id,
        folder: env.folder,
        uid: env.uid,
      })
      toasts.show(isTrash ? m.mobile_deleted() : m.mobile_moved_to_trash())
    } catch (e) {
      restoreLocally(env)
      toasts.error(m.mobile_delete_failed(), e)
    }
  }

  async function setRead(env: EmailEnvelope, read: boolean) {
    const previous = env.is_read
    envelopes = envelopes.map((e) => (keyOf(e) === keyOf(env) ? { ...e, is_read: read } : e))
    try {
      await api.mail.setMessageRead({
        accountId: env.account_id,
        folder: env.folder,
        uid: env.uid,
        read,
      })
    } catch (e) {
      envelopes = envelopes.map((x) =>
        keyOf(x) === keyOf(env) ? { ...x, is_read: previous } : x,
      )
      toasts.error(m.mobile_flag_update_failed(), e)
    }
  }

  async function setFlagged(env: EmailEnvelope, flagged: boolean) {
    const previous = env.is_starred
    envelopes = envelopes.map((e) =>
      keyOf(e) === keyOf(env) ? { ...e, is_starred: flagged } : e,
    )
    try {
      await api.mail.setMessageFlagged({
        accountId: env.account_id,
        folder: env.folder,
        uid: env.uid,
        flagged,
      })
    } catch (e) {
      envelopes = envelopes.map((x) =>
        keyOf(x) === keyOf(env) ? { ...x, is_starred: previous } : x,
      )
      toasts.error(m.mobile_flag_update_failed(), e)
    }
  }

  async function setPinned(env: EmailEnvelope, pinned: boolean) {
    try {
      await api.mail.setMessagePinned({
        accountId: env.account_id,
        folder: env.folder,
        uid: env.uid,
        pinned,
      })
      envelopes = envelopes.map((e) =>
        keyOf(e) === keyOf(env) ? { ...e, is_pinned: pinned } : e,
      )
    } catch (e) {
      toasts.error(m.mobile_flag_update_failed(), e)
    }
  }

  function sheetActions(env: EmailEnvelope): SheetAction[] {
    return [
      {
        label: env.is_read ? m.mobile_mark_unread() : m.mobile_mark_read(),
        icon: env.is_read ? 'unread' : 'read',
        run: () => setRead(env, !env.is_read),
      },
      {
        label: env.is_starred ? m.mobile_unflag() : m.mobile_flag(),
        icon: 'flag',
        run: () => setFlagged(env, !env.is_starred),
      },
      {
        label: env.is_pinned ? m.mobile_unpin() : m.mobile_pin(),
        icon: 'pin',
        run: () => setPinned(env, !env.is_pinned),
      },
      {
        label: m.mobile_move_to_folder(),
        icon: 'move-to-folder',
        run: () => (moveTarget = env),
      },
      inArchive(env)
        ? { label: m.mobile_unarchive(), icon: 'global-inbox', run: () => unarchive(env) }
        : { label: m.mobile_archive(), icon: 'archive', run: () => archive(env) },
      {
        label: isTrash ? m.mobile_delete_permanently() : m.mobile_delete(),
        icon: 'trash',
        destructive: true,
        run: () => trash(env),
      },
    ]
  }

  /* ── Move-to-folder picker ───────────────────────────────────── */

  let moveTarget = $state<EmailEnvelope | null>(null)
  /** Destinations with the name to show — role names for special-use
   *  folders, like the mailbox list (`folderLabel`). */
  let moveFolders = $state<{ name: string; label: string }[]>([])

  $effect(() => {
    const target = moveTarget
    if (!target) return
    void (async () => {
      try {
        const list: FolderLike[] = await api.mail.getCachedFolders({ accountId: target.account_id })
        moveFolders = list
          .filter((f) => f.name !== target.folder)
          .map((f) => ({ name: f.name, label: folderLabel(f) }))
      } catch (e) {
        toasts.error(m.mobile_folders_load_failed(), e)
        moveTarget = null
      }
    })()
  })

  async function moveTo(env: EmailEnvelope, dest: { name: string; label: string }) {
    removeLocally(env)
    try {
      await api.mail.moveMessage({
        accountId: env.account_id,
        folder: env.folder,
        uid: env.uid,
        destFolder: dest.name,
      })
      toasts.show(m.mobile_moved_to({ folder: dest.label }))
    } catch (e) {
      restoreLocally(env)
      toasts.error(m.mobile_move_failed(), e)
    }
  }

  /* ── Multi-select ────────────────────────────────────────────── */

  function toggleSelected(env: EmailEnvelope) {
    const key = keyOf(env)
    const next = new Set(selected)
    if (next.has(key)) next.delete(key)
    else next.add(key)
    selected = next
  }

  function selectedEnvelopes(): EmailEnvelope[] {
    return envelopes.filter((e) => selected.has(keyOf(e)))
  }

  function exitSelection() {
    selecting = false
    selected = new Set()
  }

  async function bulk(action: 'archive' | 'unarchive' | 'delete' | 'read' | 'unread') {
    const targets = selectedEnvelopes()
    if (targets.length === 0) return
    exitSelection()

    // Group by (account, folder) so the batch commands can do one
    // IMAP round-trip per mailbox instead of one per message.
    const groups = new Map<string, EmailEnvelope[]>()
    for (const env of targets) {
      const key = `${env.account_id}::${env.folder}`
      groups.set(key, [...(groups.get(key) ?? []), env])
    }

    // The rows change at once — a selection of ten does not wait for
    // the server any more than a single swipe does. What the server then
    // refuses is put back (or flipped back) with an error.
    const moving = action === 'archive' || action === 'unarchive' || action === 'delete'
    const read = action === 'read'
    const keys = new Set(targets.map(keyOf))
    if (moving) targets.forEach(removeLocally)
    else envelopes = envelopes.map((e) => (keys.has(keyOf(e)) ? { ...e, is_read: read } : e))

    await Promise.all(
      [...groups].map(async ([key, group]) => {
        const [account, mailbox] = key.split('::')
        const uids = group.map((e) => e.uid)
        try {
          if (moving) {
            // Keep out only what the backend reports as gone. The move
            // commands answer a move into the folder the messages are
            // already in with an empty list; dropping the whole group
            // regardless is what made archived rows reappear later.
            const gone = new Set(
              action === 'archive'
                ? await api.mail.archiveMessages({ accountId: account, folder: mailbox, uids })
                : action === 'unarchive'
                  ? await api.mail.moveMessages({
                      accountId: account,
                      folder: mailbox,
                      uids,
                      destFolder: INBOX,
                    })
                  : // One batch per mailbox (`delete_messages`): looping the
                    // single delete paid a connection and a login per
                    // message, one after the other.
                    await api.mail.deleteMessages({ accountId: account, folder: mailbox, uids }),
            )
            group.filter((env) => !gone.has(env.uid)).forEach(restoreLocally)
          } else {
            // One call for the whole group. Looping `setMessageRead`
            // opened an IMAP connection per message, so marking a
            // screenful read took as many connect/login round trips as
            // there were messages — long enough to look like a freeze.
            await api.mail.setMessagesRead({ accountId: account, folder: mailbox, uids, read })
          }
        } catch (e) {
          if (moving) group.forEach(restoreLocally)
          else {
            const back = new Set(group.map(keyOf))
            envelopes = envelopes.map((x) => (back.has(keyOf(x)) ? { ...x, is_read: !read } : x))
          }
          toasts.error(m.mobile_bulk_action_failed(), e)
        }
      }),
    )
  }

  /* ── Opening ─────────────────────────────────────────────────── */

  function open(env: EmailEnvelope) {
    if (selecting) {
      toggleSelected(env)
      return
    }
    if (isDrafts) {
      // A draft opens in the composer, not the reader — tapping a
      // half-written message to read it is never what you wanted.
      oncompose({ draftRef: { accountId: env.account_id, folder: env.folder, uid: env.uid } })
      return
    }
    nav.push('mail-message', {
      accountId: env.account_id,
      folder: env.folder,
      uid: env.uid,
      subject: env.subject,
    })
    if (!env.is_read) {
      // Optimistic: the reader marks it read on the server; keeping
      // the list in step here avoids a visible flip on the way back.
      envelopes = envelopes.map((e) => (keyOf(e) === keyOf(env) ? { ...e, is_read: true } : e))
    }
  }

  function accountLabel(env: EmailEnvelope): string {
    const account = accountsStore.list.find((a) => a.id === env.account_id)
    return account?.display_name || account?.email || ''
  }
</script>

<!-- A plain screen: the list runs edge to edge. Card margins cost a
     mail row ~32px of subject line, and on a phone that is the one
     dimension there is never enough of. -->
<div class="screen screen--plain">
  <NavBar
    {title}
    subtitle={canSwitchAccount
      ? scopeLabel
      : accountId === null && rows.length > 0
        ? m.mobile_all_accounts()
        : ''}
    onsubtitle={canSwitchAccount && !selecting ? () => (accountSheet = true) : undefined}
    subtitleLabel={m.mobile_inbox_switch_sr({ account: scopeLabel })}
    backLabel={selecting || root ? null : m.mobile_mailboxes_back()}
    onback={() => nav.pop()}
    busy={emptying || (refreshing && envelopes.length > 0)}
    large={root && !selecting}
  >
    {#snippet leading()}
      {#if selecting}
        <button type="button" class="bar-button bar-button--text" onclick={exitSelection}>
          {m.mobile_cancel()}
        </button>
      {:else if root}
        <!-- The word alone, where a Back button would sit. The side slot
             is ~96pt wide; with an icon beside it the word was cut to
             "Postf…", and the word is what tells a customer where it
             leads. -->
        <button
          type="button"
          class="bar-button bar-button--text bar-button--labelled"
          onclick={() => nav.push('mail-home')}
        >
          <span class="bar-button__label">{m.mobile_mailboxes_title()}</span>
        </button>
      {/if}
    {/snippet}
    {#snippet actions()}
      {#if selecting}
        <button
          type="button"
          class="bar-button bar-button--text bar-button--strong"
          disabled={selected.size === 0}
          aria-label={m.mobile_mark_read()}
          onclick={() => bulk('read')}
        >
          <!-- The short word: "Als gelesen markieren" broke over three
               lines in the bar's side slot. -->
          {m.mobile_read()}
        </button>
      {:else}
        <button
          type="button"
          class="bar-button bar-button--text"
          onclick={() => (selecting = true)}
        >
          {m.mobile_select()}
        </button>
      {/if}
    {/snippet}
    {#snippet below()}
      {#if !selecting}
        <!-- A visible field, not a corner magnifier: it is what someone
             looking for a message scans for. It opens the search screen,
             where the real input lives. -->
        <button
          type="button"
          class="search-trigger"
          onclick={() => nav.push('mail-search', { accountId, folder })}
        >
          <Icon name="search" size={18} />
          <span>{m.mobile_search_placeholder()}</span>
        </button>
      {/if}
    {/snippet}
  </NavBar>

  {#if loading && envelopes.length === 0}
    <Spinner label={m.mobile_loading()} />
  {:else if rows.length === 0}
    <EmptyState
      icon="email-envelope"
      title={m.mobile_mailbox_empty_title()}
      body={error ? error : m.mobile_mailbox_empty_body()}
      action={{ label: m.mobile_refresh(), run: () => void refresh() }}
    />
  {:else}
    <PullToRefresh onrefresh={refresh} onnearbottom={loadMore}>
      <ul class="mail-list">
        {#each rows as env (keyOf(env))}
          {@const from = parseFromHeader(env.from)}
          <li>
            <SwipeRow
              onlongpress={() => (sheetFor = env)}
              disabled={selecting}
              leading={[
                {
                  label: env.is_read ? m.mobile_unread() : m.mobile_read(),
                  icon: env.is_read ? 'unread' : 'read',
                  tone: 'primary',
                  run: () => setRead(env, !env.is_read),
                },
              ]}
              trailing={[
                {
                  label: isTrash ? m.mobile_delete_permanently() : m.mobile_delete(),
                  icon: 'trash',
                  tone: 'error',
                  run: () => trash(env),
                },
                inArchive(env)
                  ? {
                      label: m.mobile_unarchive_short(),
                      icon: 'global-inbox',
                      tone: 'primary',
                      run: () => unarchive(env),
                    }
                  : {
                      label: m.mobile_archive(),
                      icon: 'archive',
                      tone: 'success',
                      run: () => archive(env),
                    },
              ]}
            >
              <!-- The row is a real button, so VoiceOver, Switch Control
                   and a keyboard can open it — SwipeRow is only the
                   gesture layer around it. Its accessible name is its
                   text in reading order; state that is shown only as a
                   dot, a weight or an icon is spelled out in `sr-only`
                   text (WCAG 1.4.1). Button content must be phrasing
                   content, hence spans. -->
              <button
                type="button"
                class="mail-row"
                class:mail-row--unread={!env.is_read}
                aria-pressed={selecting ? selected.has(keyOf(env)) : undefined}
                onclick={() => open(env)}
              >
                <!-- The margin before the gutter: the unread mark hangs
                     here, the way punctuation hangs outside a justified
                     column, so every row's text starts on the same edge
                     read or unread. In multi-select the selection circle
                     takes a real column instead — the one mode where the
                     row's state is the thing being edited. -->
                {#if selecting}
                  <span class="mail-row__select" aria-hidden="true">
                    <span class="select-dot" class:select-dot--on={selected.has(keyOf(env))}>
                      {#if selected.has(keyOf(env))}<Icon name="check" size={14} />{/if}
                    </span>
                  </span>
                {:else if !env.is_read}
                  <span class="unread-mark" aria-hidden="true"></span>
                {/if}

                <span class="mail-row__text">
                  <span class="mail-row__line">
                    {#if !env.is_read}<span class="sr-only">{m.mobile_unread()},</span>{/if}
                    <span class="mail-row__from">
                      {from.name || from.email || m.mobile_unknown_sender()}
                    </span>
                    {#if (env.thread_total_count ?? 1) > 1}
                      <span class="thread-count" aria-hidden="true">{env.thread_total_count}</span>
                      <span class="sr-only">
                        , {m.mobile_sr_thread_count({ count: env.thread_total_count })}
                      </span>
                    {/if}
                    <span class="mail-row__date">{listDate(env.date)}</span>
                  </span>
                  <span class="mail-row__line">
                    <span class="mail-row__subject">
                      {env.subject || m.mobile_no_subject()}
                    </span>
                    <!-- Status glyphs trail the subject instead of taking a
                         third line of their own: most rows have none, and a
                         line kept free for them left a gap under every row. -->
                    {#if env.is_pinned}
                      <span class="mail-row__icon"><Icon name="pin" size={14} /></span>
                      <span class="sr-only">, {m.mobile_sr_pinned()}</span>
                    {/if}
                    {#if env.is_starred}
                      <span class="mail-row__icon mail-row__icon--warn"><Icon name="flag" size={14} /></span>
                      <span class="sr-only">, {m.mobile_sr_flagged()}</span>
                    {/if}
                    {#if env.protection}
                      <span class="mail-row__icon"><Icon name="encrypted" size={14} /></span>
                      <span class="sr-only">, {m.mobile_sr_encrypted()}</span>
                    {/if}
                    {#if env.priority === 'high'}
                      <span class="mail-row__icon mail-row__icon--error"><Icon name="important" size={14} /></span>
                      <span class="sr-only">, {m.mobile_sr_high_priority()}</span>
                    {/if}
                    {#if env.is_answered}
                      <span class="mail-row__icon"><Icon name="reply" size={14} /></span>
                      <span class="sr-only">, {m.mobile_sr_answered()}</span>
                    {/if}
                  </span>
                  {#if accountId === null}
                    <span class="mail-row__account">{accountLabel(env)}</span>
                  {/if}
                </span>

                <!-- No disclosure chevron. Every row in a message list
                     opens a message, so a per-row arrow states the same
                     thing 50 times and earns none of the width it takes. -->
              </button>
            </SwipeRow>
          </li>
        {/each}
      </ul>

      {#if loadingMore}
        <div class="py-4"><Spinner full={false} /></div>
      {:else if exhausted && rows.length > 12}
        <p class="mail-list__end">
          {m.mobile_end_of_mailbox({ count: rows.length })}
        </p>
      {/if}

      <!-- Below the list rather than in the nav bar: it's a rare,
           irreversible action, and a bar button next to Search is
           exactly where a thumb lands by accident. -->
      {#if canEmpty}
        <div class="mail-list__empty-folder">
          <button
            type="button"
            class="wide-button wide-button--destructive-quiet"
            disabled={emptying}
            onclick={() => (confirmEmpty = true)}
          >
            {emptying
              ? m.mobile_working()
              : isJunk
                ? m.mobile_empty_junk()
                : m.mobile_empty_trash()}
          </button>
        </div>
      {/if}
      <div class="h-24"></div>
    </PullToRefresh>

    {#if selecting}
      <div class="select-bar">
        <button
          type="button"
          class="select-bar__btn"
          disabled={selected.size === 0}
          onclick={() => bulk('unread')}
        >
          <Icon name="unread" size={18} />
          <span>{m.mobile_mark_unread()}</span>
        </button>
        <button
          type="button"
          class="select-bar__btn"
          disabled={selected.size === 0}
          onclick={() => bulk(listInArchive ? 'unarchive' : 'archive')}
        >
          <Icon name={listInArchive ? 'global-inbox' : 'archive'} size={18} />
          <span>{listInArchive ? m.mobile_unarchive_short() : m.mobile_archive()}</span>
        </button>
        <button
          type="button"
          class="select-bar__btn select-bar__btn--danger"
          disabled={selected.size === 0}
          onclick={() => bulk('delete')}
        >
          <Icon name="trash" size={18} />
          <span>{m.mobile_delete()}</span>
        </button>
      </div>
    {:else}
      <Fab icon="compose" label={m.mobile_compose()} onclick={() => oncompose()} />
    {/if}
  {/if}
</div>

{#if sheetFor}
  {@const env = sheetFor}
  <ActionSheet
    title={env.subject || m.mobile_no_subject()}
    subtitle={displayName(env.from)}
    actions={sheetActions(env)}
    onclose={() => (sheetFor = null)}
  />
{/if}

{#if moveTarget}
  {@const env = moveTarget}
  <ActionSheet
    title={m.mobile_move_to_folder()}
    subtitle={env.subject}
    actions={moveFolders.map((dest) => ({
      label: dest.label,
      icon: 'move-to-folder' as const,
      run: () => moveTo(env, dest),
    }))}
    onclose={() => (moveTarget = null)}
  />
{/if}

{#if accountSheet}
  <ActionSheet
    title={m.mobile_inbox_switch_title()}
    actions={accountActions()}
    onclose={() => (accountSheet = false)}
  />
{/if}

{#if confirmEmpty}
  <Confirm
    title={isJunk ? m.mobile_empty_junk() : m.mobile_empty_trash()}
    body={m.mobile_empty_folder_body({ folder: title })}
    confirmLabel={m.mobile_delete_permanently()}
    onconfirm={emptyFolder}
    oncancel={() => (confirmEmpty = false)}
  />
{/if}

<style>
  /**
   * Skip the layout, style and paint work for rows that are off
   * screen. A mailbox is thousands of rows and none of them changes
   * height, so the browser only has to know how tall a skipped one
   * would be — `contain-intrinsic-size: auto 76px` seeds that with
   * the row's own minimum and then remembers each row's real height
   * once it has been on screen, which keeps the scrollbar honest.
   *
   * Ignored by engines that don't implement it (iOS 15 among them),
   * where the list just behaves exactly as it did before.
   */
  li {
    content-visibility: auto;
    contain-intrinsic-size: auto 76px;
  }

  /* ── The message row (v3) ──────────────────────────────────────
     Full-bleed, two lines, type only.

       1. **No avatar.** v2 gave every row a 44pt pastel initials circle:
          66pt of width per row for two letters the sender's name already
          spells out, in a colour that meant nothing. Without it the
          subject line gains a third of the screen, and the list reads as
          what it is — a list of letters, not a list of contacts.
       2. **One left edge.** Sender and subject start on `--gutter`,
          where the screen title starts. The unread mark hangs in the
          margin before it.
       3. **Unread has three signals, none of them alone:** the mark (in
          the signal colour — Morgenrot in the house theme), the
          sender's weight, and the subject in full ink. v2's fourth, a
          coloured date, is gone: a column of coloured times competed
          with the marks it repeated.
       4. **Two lines, always.** Status glyphs trail the subject. The
          unified inbox adds the account as a third line, where it is
          information rather than decoration. */
  .mail-row {
    display: flex;
    align-items: flex-start;
    width: 100%;
    min-height: 72px;
    padding: var(--s-3) var(--gutter);
    text-align: left;
    position: relative;
    transition: background-color var(--m-fast) var(--m-ease);
  }

  /* Press feedback. The row is the tap target, so it is the row that
     acknowledges the tap — no feedback between the touch and the next
     screen reads as a dropped tap on a slow load. */
  :global(.swipe-content:active) .mail-row {
    background: var(--ui-press);
  }

  /* Hairline between rows, from the gutter to the far edge: it starts
     where the text does, so it underlines the column rather than the
     screen. */
  li + li .mail-row::before {
    content: '';
    position: absolute;
    top: 0;
    left: var(--gutter);
    right: 0;
    height: 1px;
    background: var(--ui-hairline);
  }

  /* Centred in the margin, level with the sender's x-height. 8pt: big
     enough to find at a glance, small enough to stay a mark. */
  .unread-mark {
    position: absolute;
    left: calc(var(--gutter) / 2 - 4px);
    top: calc(var(--s-3) + var(--t-headline-lh) / 2 - 4px);
    width: 8px;
    height: 8px;
    border-radius: var(--r-full);
    background: var(--ui-unread);
  }

  .mail-row__select {
    display: flex;
    flex-shrink: 0;
    width: 22px;
    margin-right: var(--s-3);
    padding-top: 1px;
  }

  .mail-row__text {
    display: block;
    flex: 1;
    min-width: 0;
  }

  .mail-row__line {
    display: flex;
    align-items: baseline;
    gap: var(--s-2);
    min-width: 0;
  }

  .mail-row__line + .mail-row__line {
    margin-top: 2px;
  }

  .mail-row__from {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-headline);
    line-height: var(--t-headline-lh);
    font-weight: 500;
    letter-spacing: -0.01em;
    color: var(--ui-ink);
  }

  .mail-row--unread .mail-row__from {
    font-weight: 700;
  }

  /* Tabular figures, so times line up down the column as a scale
     rather than jittering with each digit's width. */
  .mail-row__date {
    flex-shrink: 0;
    font-size: var(--t-footnote);
    line-height: var(--t-footnote-lh);
    font-variant-numeric: tabular-nums;
    color: var(--ui-ink-faint);
  }

  .mail-row--unread .mail-row__date {
    color: var(--ui-ink);
    font-weight: 600;
  }

  .mail-row__subject {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-callout);
    line-height: var(--t-callout-lh);
    color: var(--ui-ink-muted);
  }

  /* Unread promotes the subject to full ink. The weight of the subject
     stays put — shifting both weight and colour makes the list twitch
     as messages are read. */
  .mail-row--unread .mail-row__subject {
    color: var(--ui-ink);
  }

  .mail-row__account {
    display: block;
    margin-top: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-caption);
    line-height: var(--t-caption-lh);
    color: var(--ui-ink-faint);
  }

  .mail-row__icon {
    display: inline-flex;
    align-self: center;
    flex-shrink: 0;
    color: var(--ui-ink-faint);
  }

  .mail-row__icon--warn {
    color: var(--ui-flagged);
  }

  .mail-row__icon--error {
    color: var(--ui-urgent);
  }

  .select-dot {
    width: 22px;
    height: 22px;
    border-radius: var(--r-full);
    border: 1.5px solid var(--ui-control);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: var(--ui-on-accent);
    transition:
      background-color var(--m-fast) var(--m-ease),
      border-color var(--m-fast) var(--m-ease),
      transform var(--m-fast) var(--m-spring);
  }

  .select-dot--on {
    background: var(--ui-accent-fill);
    border-color: var(--ui-accent-fill);
    transform: scale(1.08);
  }

  .thread-count {
    flex-shrink: 0;
    align-self: center;
    font-size: var(--t-micro);
    line-height: var(--t-micro-lh);
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    padding: 1px 6px;
    border-radius: var(--r-xs);
    color: var(--ui-ink-muted);
    background: var(--ui-fill-quiet);
  }

  .mail-list__empty-folder {
    padding: var(--s-4) var(--gutter) 0;
  }

  .mail-list__end {
    padding: var(--s-6) var(--s-4);
    text-align: center;
    font-size: var(--t-footnote);
    color: var(--ui-ink-faint);
  }

  /* Multi-select's actions float as a capsule just above the tab bar,
     in the tab bar's own shape, so the two read as one set of controls
     rather than two stacked toolbars. */
  .select-bar {
    position: absolute;
    left: var(--s-3);
    right: var(--s-3);
    bottom: calc(var(--bar-clearance, env(safe-area-inset-bottom)) + var(--s-2));
    z-index: 15;
    display: flex;
    padding: var(--s-1);
    border-radius: var(--r-full);
    background: var(--ui-bar);
    box-shadow: var(--e-float);
  }

  .select-bar__btn {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    min-height: 52px;
    border-radius: var(--r-full);
    font-size: var(--t-bar-label);
    font-weight: 600;
    color: var(--ui-accent-ink);
  }

  .select-bar__btn:active:not(:disabled) {
    background: var(--ui-press);
  }

  .select-bar__btn:disabled {
    opacity: 0.35;
  }

  .select-bar__btn--danger {
    color: var(--ui-danger-ink);
  }
</style>
