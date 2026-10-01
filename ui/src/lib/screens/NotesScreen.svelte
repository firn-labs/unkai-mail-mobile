<script lang="ts">
  /**
   * Nextcloud Notes.
   *
   * List + editor, no folder tree: the Notes app's own category
   * field becomes a filter chip row, which is all the structure a
   * phone list needs.
   */
  import { formatLocale } from '../locale'
  import * as api from '../api'
  import type { NextcloudAccount, Note } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import PullToRefresh from '../ui/PullToRefresh.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import SearchField from '../ui/SearchField.svelte'
  import SwipeRow from '../ui/SwipeRow.svelte'
  import Fab from '../ui/Fab.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { snippet } from '../format'
  import { m } from '../../paraglide/messages'

  let accounts = $state<NextcloudAccount[]>([])
  let ncId = $state('')
  let notes = $state<Note[]>([])
  let loading = $state(true)
  let syncing = $state(false)
  let query = $state('')
  let category = $state<string | null>(null)

  const categories = $derived([
    ...new Set(notes.map((n: Note) => n.category).filter((c: string) => c)),
  ])

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase()
    return notes
      .filter((n: Note) => (category ? n.category === category : true))
      .filter(
        (n: Note) =>
          !q || n.title.toLowerCase().includes(q) || n.content.toLowerCase().includes(q),
      )
      .sort((a: Note, b: Note) => b.modified - a.modified)
  })

  async function load() {
    try {
      accounts = await api.nextcloud.getNextcloudAccounts()
      if (!ncId && accounts.length > 0) ncId = accounts[0].id
      if (ncId) notes = await api.notes.listNextcloudNotes({ ncId })
    } catch (e) {
      toasts.error(m.mobile_notes_load_failed(), e)
    } finally {
      loading = false
    }
  }

  async function sync() {
    if (!ncId) return
    syncing = true
    try {
      notes = await api.notes.syncNextcloudNotes({ ncId })
    } catch (e) {
      toasts.error(m.mobile_notes_sync_failed(), e)
    } finally {
      syncing = false
    }
  }

  $effect(() => {
    void ncId
    void load()
  })

  async function remove(note: Note) {
    const previous = notes
    notes = notes.filter((n) => n.id !== note.id)
    try {
      await api.notes.deleteNextcloudNote({ ncId, noteId: note.id })
      toasts.show(m.mobile_note_deleted())
    } catch (e) {
      notes = previous
      toasts.error(m.mobile_note_delete_failed(), e)
    }
  }

  function openNote(note: Note | null) {
    nav.push('note-detail', { note, ncId })
  }
</script>

<div class="screen">
  <!-- A feature is a tab root *or* a page under More, depending on how
       the user arranged the bar (Settings → Features). Back appears
       exactly when there is somewhere to go back to. -->
  <NavBar
    title={m.mobile_notes()}
    backLabel={nav.canGoBack ? m.mobile_more() : null}
    onback={() => nav.pop()}
    busy={syncing}
  >
    {#snippet actions()}
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_sync()}
        onclick={sync}
        disabled={syncing || !ncId}
      >
        <Icon name={syncing ? 'loading' : 'sync'} size={20} />
      </button>
    {/snippet}
    {#snippet below()}
      <SearchField bind:value={query} placeholder={m.mobile_notes_search_placeholder()} />
    {/snippet}
  </NavBar>

  {#if loading}
    <Spinner />
  {:else if accounts.length === 0}
    <EmptyState
      icon="cloud"
      title={m.mobile_no_nextcloud_title()}
      body={m.mobile_no_nextcloud_body()}
      action={{ label: m.mobile_connect_nextcloud(), run: () => nav.push('settings-nextcloud') }}
    />
  {:else}
    {#if categories.length > 0}
      <div class="chip-row">
        <button
          type="button"
          class="chip-filter"
          class:chip-filter--on={category === null}
          onclick={() => (category = null)}
        >
          {m.mobile_all_notes()}
        </button>
        {#each categories as cat (cat)}
          <button
            type="button"
            class="chip-filter"
            class:chip-filter--on={category === cat}
            onclick={() => (category = cat)}
          >
            {cat}
          </button>
        {/each}
      </div>
    {/if}

    {#if notes.length === 0}
      <EmptyState icon="notes" title={m.mobile_no_notes_title()} body={m.mobile_no_notes_body()} />
    {:else if filtered.length === 0}
      <EmptyState
        icon="search"
        title={m.mobile_notes_no_match_title()}
        body={m.mobile_notes_no_match_body({ query })}
      />
    {:else}
      <PullToRefresh onrefresh={sync}>
        <ul>
          {#each filtered as note (note.id)}
            <li>
              <SwipeRow
                onclick={() => openNote(note)}
                trailing={[
                  {
                    label: m.mobile_delete(),
                    icon: 'trash',
                    tone: 'error',
                    run: () => remove(note),
                  },
                ]}
              >
                <div class="note-row">
                  {#if note.favorite}
                    <Icon name="star" size={16} />
                  {/if}
                  <span class="min-w-0 flex-1">
                    <span class="block truncate t-body font-medium">{note.title}</span>
                    <span class="block truncate list-row__detail">{snippet(note.content, 80)}</span>
                    <span class="block list-row__detail">
                      {new Date(note.modified * 1000).toLocaleDateString(formatLocale())}
                      {#if note.category}· {note.category}{/if}
                    </span>
                  </span>
                  <Icon name="nav-forward" size={16} />
                </div>
              </SwipeRow>
            </li>
          {/each}
        </ul>
        <div class="h-24"></div>
      </PullToRefresh>
    {/if}

    <Fab label={m.mobile_new_note()} onclick={() => openNote(null)} />
  {/if}
</div>

<style>
  .chip-row {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    padding: 8px 12px;
    flex-shrink: 0;
    border-bottom: 1px solid color-mix(in oklab, var(--color-surface-500) 16%, transparent);
  }
  .chip-filter {
    flex-shrink: 0;
    padding: 5px 11px;
    min-height: 32px;
    border-radius: var(--r-full);
    font-size: var(--t-footnote);
    background: color-mix(in oklab, var(--color-surface-500) 14%, transparent);
  }
  .chip-filter--on {
    background: var(--ui-accent-fill);
    color: var(--ui-on-accent);
  }
  .note-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px;
    min-height: 66px;
  }
  li + li .note-row {
    border-top: 1px solid color-mix(in oklab, var(--color-surface-500) 16%, transparent);
  }
</style>
