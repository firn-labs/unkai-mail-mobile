<script lang="ts">
  /**
   * Which of an account's folders show up in the mailbox list.
   *
   * Stored as a *hidden* list (`Account.hidden_folders`), not a
   * visible one, and that direction is the whole design: a folder
   * created later — on the server, on the desktop app, by a filter
   * rule — appears on its own instead of staying invisible until
   * someone remembers to come back here and tick it.
   *
   * Hiding is display-only. The folder still syncs, still turns up
   * in search, and is still a valid move target; it just stops
   * taking up a row. That's what the footer tells the user, because
   * "hidden" otherwise reads as "no longer checked for mail".
   *
   * Saves on every toggle, like the other settings screens — there
   * is no half-typed state here that a stray save could corrupt.
   */
  import * as api from '../api'
  import type { Account, Folder } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Toggle from '../Toggle.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import { nav } from '../nav.svelte'
  import { accountsStore } from '../accountsStore.svelte'
  import { toasts } from '../toast.svelte'
  import { iconFor, leafName, sortFolders, specialUse } from '../mail/folders'
  import { m } from '../../paraglide/messages'

  interface Props {
    accountId: string
  }

  let { accountId }: Props = $props()

  const account = $derived<Account | null>(
    accountsStore.list.find((a) => a.id === accountId) ?? null,
  )

  let folders = $state<Folder[]>([])
  let loading = $state(true)
  let saving = $state(false)
  /** The list being edited. Kept locally so a toggle paints
   *  immediately and the round-trip to the store happens behind it. */
  let hidden = $state<string[]>([])

  $effect(() => {
    hidden = [...(account?.hidden_folders ?? [])]
  })

  $effect(() => {
    const id = accountId
    void (async () => {
      loading = true
      try {
        folders = await api.mail.getCachedFolders({ accountId: id })
      } catch {
        // Nothing cached yet — the empty state points at the pull
        // that fills it, which is a better answer than an error.
        folders = []
      }
      loading = false
    })()
  })

  const sorted = $derived(sortFolders(folders))

  function isVisible(name: string): boolean {
    return !hidden.includes(name)
  }

  async function persist(next: string[]) {
    const current = account
    if (!current) return
    const previous = hidden
    hidden = next
    saving = true
    try {
      await api.accounts.updateAccount({ account: { ...current, hidden_folders: next } })
      await accountsStore.load()
    } catch (e) {
      hidden = previous
      toasts.error(m.mobile_settings_save_failed(), e)
    } finally {
      saving = false
    }
  }

  function setVisible(name: string, visible: boolean) {
    void persist(visible ? hidden.filter((n) => n !== name) : [...hidden, name])
  }

  function showAll() {
    void persist([])
  }

  function hideAll() {
    void persist(folders.map((f) => f.name))
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_account_folders()}
    backLabel={m.mobile_account()}
    onback={() => nav.pop()}
    busy={saving}
  />

  <div class="screen__body">
    {#if loading}
      <Spinner />
    {:else if folders.length === 0}
      <EmptyState
        icon="files"
        title={m.mobile_no_folders()}
        body={m.mobile_account_folders_none()}
      />
    {:else}
      <div class="list-group">
        {#each sorted.special as entry (entry.folder.name)}
          {@const label =
            entry.kind === 'inbox' ? m.folder_name_inbox() : leafName(entry.folder)}
          <div class="list-row">
            <Icon name={iconFor(entry.kind)} size={20} />
            <span class="min-w-0 flex-1 list-row__title truncate">{label}</span>
            <Toggle
              checked={isVisible(entry.folder.name)}
              label={label}
              onchange={(on) => setVisible(entry.folder.name, on)}
            />
          </div>
        {/each}
        {#each sorted.custom as entry (entry.folder.name)}
          <div class="list-row" style="padding-left: {16 + entry.depth * 14}px">
            <Icon name={iconFor(specialUse(entry.folder))} size={20} />
            <span class="min-w-0 flex-1 list-row__title truncate">
              {leafName(entry.folder)}
            </span>
            <Toggle
              checked={isVisible(entry.folder.name)}
              label={leafName(entry.folder)}
              onchange={(on) => setVisible(entry.folder.name, on)}
            />
          </div>
        {/each}
      </div>
      <p class="list-footer">{m.mobile_account_folders_hint()}</p>

      <div class="flex gap-2 px-3 pb-10 pt-2">
        <button
          type="button"
          class="wide-button wide-button--quiet"
          disabled={hidden.length === 0}
          onclick={showAll}
        >
          {m.mobile_show_all()}
        </button>
        <button
          type="button"
          class="wide-button wide-button--quiet"
          disabled={hidden.length === folders.length}
          onclick={hideAll}
        >
          {m.mobile_hide_all()}
        </button>
      </div>
    {/if}
  </div>
</div>
