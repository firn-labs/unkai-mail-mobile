<script lang="ts">
  /**
   * Every mailbox the user can open — one tap from the inbox.
   *
   * It used to be the Mail tab's root. It is now pushed from the
   * inbox's "Mailboxes" button: nearly every session is about new mail,
   * so the app opens there and this list is where you go when you
   * want something else.
   *
   * Structure mirrors what the desktop keeps in its permanent
   * sidebar, collapsed into one scrollable list:
   *   1. Unified mailboxes across all accounts (only when there is
   *      more than one account — with one account they'd duplicate
   *      that account's own folders).
   *   2. One section per account: special-use folders first in
   *      canonical order, then the user's own folders.
   *
   * Unread counts come from the cache, so the list is accurate
   * offline and repaints when the sync loop pushes a new count.
   *
   * Special-use folders are named by their **role**, in the user's
   * language ("Entwürfe", not the server's "Drafts"). The server name
   * is an implementation detail of whichever provider the account is
   * on; the role is what the user is looking for.
   */
  import * as api from '../api'
  import type { Account, Folder } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import PullToRefresh from '../ui/PullToRefresh.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import { nav } from '../nav.svelte'
  import { accountsStore } from '../accountsStore.svelte'
  import { toasts } from '../toast.svelte'
  import { iconFor, leafName, sortFolders, specialUse } from '../mail/folders'
  import { roleName } from '../mail/folderNames'
  import {
    UNIFIED_ARCHIVE_FOLDER,
    UNIFIED_DRAFTS_FOLDER,
    UNIFIED_JUNK_FOLDER,
    UNIFIED_SENT_FOLDER,
    UNIFIED_TRASH_FOLDER,
  } from '../unifiedFolders'
  import { singleFlight } from '../schedule'
  import { m } from '../../paraglide/messages'

  interface Props {
    /** Unread per account id, pushed from the shell's event wiring. */
    unreadByAccount: Record<string, number>
    outboxCount: number
  }

  let { unreadByAccount, outboxCount }: Props = $props()

  const accounts = $derived(accountsStore.list)
  const multiAccount = $derived(accounts.length > 1)

  /** account id → its folder list, loaded from cache then refreshed. */
  let folders = $state<Record<string, Folder[]>>({})
  let loading = $state(true)

  const totalUnread = $derived(
    Object.values(unreadByAccount).reduce((sum, n) => sum + n, 0),
  )

  /**
   * Read every account's folder list out of the cache.
   *
   * Accounts run **concurrently**. These are independent SQLite reads
   * behind independent IPC calls, and awaiting them one after another
   * made the screen's paint latency scale with the number of accounts
   * for no reason at all.
   */
  async function loadCached() {
    const results = await Promise.all(
      accounts.map(async (acc) => {
        try {
          return [acc.id, await api.mail.getCachedFolders({ accountId: acc.id })] as const
        } catch {
          // A cache miss is normal on a fresh account; the network
          // refresh below fills it in.
          return [acc.id, [] as Folder[]] as const
        }
      }),
    )
    folders = Object.fromEntries(results)
    loading = false
  }

  /**
   * Re-LIST folders (structure) and check mail (counts).
   *
   * Two things about the shape:
   *
   *   * **Accounts refresh in parallel.** Each one is a separate
   *     server, a separate connection and a separate `LIST` +
   *     per-folder `STATUS` conversation — dozens of round trips. Run
   *     back to back, a second account doubled how long the spinner
   *     sat there while contributing nothing to the first one's
   *     result. Nothing here is ordered between accounts, so nothing
   *     needs to wait.
   *   * **Best-effort per account.** `allSettled`, so one unreachable
   *     server reports itself and leaves the others' folders on
   *     screen rather than blanking the list.
   *
   * `singleFlight` covers the overlap that a phone makes routine: a
   * pull started while the mount's refresh is still out, or a second
   * pull because the first is taking a while. Two of these in flight
   * means two `checkMailNow` sweeps polling the same INBOXes over
   * competing connections — which makes the wait longer, not shorter.
   */
  const refresh = singleFlight(async () => {
    const next: Record<string, Folder[]> = { ...folders }
    const listed = await Promise.allSettled(
      accounts.map(async (acc) => [acc.id, await api.mail.fetchFolders({ accountId: acc.id })] as const),
    )
    listed.forEach((result, i) => {
      if (result.status === 'fulfilled') {
        const [id, list] = result.value
        next[id] = list
      } else {
        toasts.error(
          m.mobile_mailboxes_refresh_failed({ account: accounts[i].email }),
          result.reason,
        )
      }
    })
    folders = next
    try {
      await api.mail.checkMailNow()
    } catch {
      // The sync loop reports its own failures through the shell;
      // a failed manual check just leaves the counts as they were.
    }
    await loadCached()
  })

  $effect(() => {
    // Re-read whenever the account list changes (setup finished, an
    // account was removed).
    void accounts.length
    void (async () => {
      await loadCached()
      // A freshly added account has nothing cached, and "no folders
      // yet" is a dead end — fetch once instead of waiting for the
      // user to guess that a pull would help.
      const empty = accounts.filter((a) => (folders[a.id] ?? []).length === 0)
      if (empty.length > 0) await refresh()
    })()
  })

  function openFolder(accountId: string | null, folder: string, title: string) {
    nav.push('mail-list', { accountId, folder, title })
  }

  function accountUnread(id: string): number {
    return unreadByAccount[id] ?? 0
  }

  /** Unread for one folder: the cache's per-folder count, which is
   *  only meaningful for the Inbox on most servers — other folders
   *  report 0 until they're opened. */
  function folderUnread(f: Folder): number {
    return f.unread_count ?? 0
  }

  /**
   * The account's folders minus the ones it has hidden
   * (Settings → Accounts → the account → Visible folders).
   *
   * Only this list is filtered. Unread counts, the unified
   * mailboxes and search all still cover the whole account — hiding
   * a folder is about what takes up a row here, not about what the
   * app pays attention to.
   */
  function visibleFolders(account: Account): Folder[] {
    const list = folders[account.id] ?? []
    const hidden = account.hidden_folders
    if (!hidden || hidden.length === 0) return list
    return list.filter((f) => !hidden.includes(f.name))
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_mailboxes_title()}
    backLabel={m.folder_name_inbox()}
    onback={() => nav.pop()}
    large
  />

  {#if accounts.length === 0}
    <EmptyState
      icon="add-account"
      title={m.mobile_no_accounts_title()}
      body={m.mobile_no_accounts_body()}
      action={{
        label: m.mobile_add_account(),
        run: () => nav.go('more', 'settings-accounts', {}),
      }}
    />
  {:else}
    <PullToRefresh onrefresh={refresh}>
      {#if multiAccount}
        <div class="list-header">{m.mobile_all_accounts()}</div>
        <div class="list-group">
          <button
            class="list-row"
            onclick={() => openFolder(null, 'INBOX', m.folder_name_inbox())}
          >
            <span class="folder-icon"><Icon name="global-inbox" size={22} /></span>
            <span class="flex-1 list-row__title">{m.mobile_all_inboxes()}</span>
            {#if totalUnread > 0}
              <span class="count-pill count-pill--signal">{totalUnread}</span>
            {/if}
            <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
          </button>
          {#each [{ f: UNIFIED_DRAFTS_FOLDER, label: m.folder_name_drafts(), icon: 'drafts' }, { f: UNIFIED_SENT_FOLDER, label: m.folder_name_sent(), icon: 'sent' }, { f: UNIFIED_ARCHIVE_FOLDER, label: m.folder_name_archive(), icon: 'archive' }, { f: UNIFIED_JUNK_FOLDER, label: m.folder_name_junk(), icon: 'spam' }, { f: UNIFIED_TRASH_FOLDER, label: m.folder_name_trash(), icon: 'trash' }] as entry (entry.f)}
            <button class="list-row" onclick={() => openFolder(null, entry.f, entry.label)}>
              <span class="folder-icon"><Icon name={entry.icon as never} size={22} /></span>
              <span class="flex-1 list-row__title">{entry.label}</span>
              <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
            </button>
          {/each}
        </div>
      {/if}

      {#if outboxCount > 0}
        <div class="list-group">
          <button class="list-row" onclick={() => nav.push('outbox', {})}>
            <span class="folder-icon"><Icon name="sent" size={22} /></span>
            <span class="flex-1 list-row__title">{m.mobile_outbox()}</span>
            <span class="count-pill">{outboxCount}</span>
            <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
          </button>
        </div>
      {/if}

      {#each accounts as account (account.id)}
        {@const list = visibleFolders(account)}
        {@const sorted = sortFolders(list)}
        <div class="list-header flex items-center gap-2">
          {#if account.emoji}<span>{account.emoji}</span>{/if}
          <span class="truncate">{account.display_name || account.email}</span>
          {#if accountUnread(account.id) > 0}
            <span class="count-pill count-pill--signal ml-auto">{accountUnread(account.id)}</span>
          {/if}
        </div>
        <div class="list-group">
          {#if list.length === 0}
            <!-- Three different situations, three different
                 sentences: still loading, nothing on the server, and
                 everything hidden by the user's own filter. The last
                 one has to say so, or the account looks broken. -->
            <div class="list-row">
              <span class="list-row__detail">
                {loading
                  ? m.mobile_loading()
                  : (folders[account.id] ?? []).length > 0
                    ? m.mobile_account_folders_all_hidden()
                    : m.mobile_no_folders()}
              </span>
            </div>
            {#if !loading && (folders[account.id] ?? []).length > 0}
              <button
                class="list-row"
                onclick={() =>
                  nav.go('more', 'settings-account-folders', { accountId: account.id })}
              >
                <span class="flex-1 list-row__title">{m.mobile_account_folders()}</span>
                <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
              </button>
            {/if}
          {:else}
            {#each sorted.special as entry (entry.folder.name)}
              {@const label = roleName(entry.kind)}
              <button
                class="list-row"
                onclick={() => openFolder(account.id, entry.folder.name, label)}
              >
                <span class="folder-icon"><Icon name={iconFor(entry.kind)} size={22} /></span>
                <span class="flex-1 list-row__title truncate">{label}</span>
                {#if folderUnread(entry.folder) > 0}
                  <span class="count-pill count-pill--signal"
                    >{folderUnread(entry.folder)}</span
                  >
                {/if}
                <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
              </button>
            {/each}
            {#each sorted.custom as entry (entry.folder.name)}
              <button
                class="list-row"
                style="padding-left: {16 + entry.depth * 14}px"
                onclick={() =>
                  openFolder(account.id, entry.folder.name, leafName(entry.folder))}
              >
                <span class="folder-icon"><Icon name={iconFor(specialUse(entry.folder))} size={22} /></span>
                <span class="flex-1 list-row__title truncate">{leafName(entry.folder)}</span>
                {#if folderUnread(entry.folder) > 0}
                  <span class="count-pill">{folderUnread(entry.folder)}</span>
                {/if}
                <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
              </button>
            {/each}
          {/if}
        </div>
      {/each}

      <div class="h-8"></div>
    </PullToRefresh>
  {/if}
</div>

<style>
  /* Folder glyphs in muted ink, like every row glyph (v3): the shape is
     what the eye matches ("the tray", "the bin"), and the colour here is
     the unread count's — the one thing on this screen that is *new*. */
  .folder-icon {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--ui-ink-muted);
  }
</style>
