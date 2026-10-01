<script lang="ts">
  /**
   * Browse the user's Nextcloud files.
   *
   * Tapping a file downloads it and hands the bytes to the webview,
   * which renders images, PDFs and text inline. Anything else gets
   * the two actions that always work on a phone: save it into the
   * app's Documents folder (visible in the Files app), or share it
   * as a link.
   */
  import { formatLocale } from '../locale'
  import * as api from '../api'
  import type { FileEntry, NextcloudAccount } from '../api'
  import Icon from '../Icon.svelte'
  import FileTypeIcon from '../FileTypeIcon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import PullToRefresh from '../ui/PullToRefresh.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import SearchField from '../ui/SearchField.svelte'
  import ActionSheet from '../ui/ActionSheet.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { fileSize } from '../format'
  import { formatError } from '../errors'
  import { m } from '../../paraglide/messages'

  let accounts = $state<NextcloudAccount[]>([])
  let ncId = $state('')
  let path = $state('/')
  let entries = $state<FileEntry[]>([])
  let loading = $state(true)
  let error = $state('')
  let query = $state('')
  let busyPath = $state<string | null>(null)
  let sheetFor = $state<FileEntry | null>(null)
  let newFolderName = $state<string | null>(null)

  const segments = $derived(path.split('/').filter(Boolean))
  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase()
    if (!q) return entries
    return entries.filter((e: FileEntry) => e.name.toLowerCase().includes(q))
  })

  async function loadAccounts() {
    try {
      accounts = await api.nextcloud.getNextcloudAccounts()
      if (!ncId && accounts.length > 0) ncId = accounts[0].id
    } catch (e) {
      error = formatError(e)
    } finally {
      if (accounts.length === 0) loading = false
    }
  }

  async function list() {
    if (!ncId) return
    loading = true
    error = ''
    try {
      const all = await api.nextcloud.listNextcloudFiles({ ncId, path })
      entries = all.filter((e: FileEntry) => e.path.replace(/\/$/, '') !== path.replace(/\/$/, ''))
    } catch (e) {
      error = formatError(e)
      entries = []
    } finally {
      loading = false
    }
  }

  $effect(() => {
    void loadAccounts()
  })

  $effect(() => {
    void ncId
    void path
    // A filter scoped to one folder is rarely useful in the next one.
    query = ''
    void list()
  })

  function enter(entry: FileEntry) {
    if (entry.is_dir) {
      path = entry.path.endsWith('/') ? entry.path : `${entry.path}/`
    } else {
      void open(entry)
    }
  }

  async function open(entry: FileEntry) {
    busyPath = entry.path
    try {
      const bytes = await api.nextcloud.downloadNextcloudFile({ ncId, path: entry.path })
      const blob = new Blob([new Uint8Array(bytes)], {
        type: entry.content_type ?? 'application/octet-stream',
      })
      const url = URL.createObjectURL(blob)
      window.open(url, '_blank')
      setTimeout(() => URL.revokeObjectURL(url), 60_000)
    } catch (e) {
      toasts.error(m.mobile_file_open_failed(), e)
    } finally {
      busyPath = null
    }
  }

  async function saveToDevice(entry: FileEntry) {
    busyPath = entry.path
    try {
      const bytes = await api.nextcloud.downloadNextcloudFile({ ncId, path: entry.path })
      const saved = await api.system.saveBytesToDocuments({ filename: entry.name, bytes })
      toasts.success(m.mobile_file_saved({ name: saved.split('/').pop() ?? entry.name }))
    } catch (e) {
      toasts.error(m.mobile_file_save_failed(), e)
    } finally {
      busyPath = null
    }
  }

  async function share(entry: FileEntry) {
    try {
      const result = await api.nextcloud.createNextcloudShare({ ncId, path: entry.path })
      await navigator.clipboard?.writeText(result.url)
      toasts.success(m.mobile_share_link_copied())
    } catch (e) {
      toasts.error(m.mobile_share_link_failed(), e)
    }
  }

  async function createFolder() {
    const name = (newFolderName ?? '').trim()
    newFolderName = null
    if (!name) return
    try {
      await api.nextcloud.createNextcloudDirectory({ ncId, path: `${path}${name}` })
      await list()
      toasts.success(m.mobile_folder_created())
    } catch (e) {
      toasts.error(m.mobile_folder_create_failed(), e)
    }
  }

  function goTo(depth: number) {
    path = depth === 0 ? '/' : `/${segments.slice(0, depth).join('/')}/`
  }
</script>

<div class="screen">
  <!-- A feature is a tab root *or* a page under More, depending on how
       the user arranged the bar (Settings → Features). Back appears
       exactly when there is somewhere to go back to. -->
  <NavBar
    title={m.mobile_files()}
    backLabel={nav.canGoBack ? m.mobile_more() : null}
    onback={() => nav.pop()}
  >
    {#snippet actions()}
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_new_folder()}
        onclick={() => (newFolderName = '')}
        disabled={!ncId}
      >
        <Icon name="add-folder" size={20} />
      </button>
    {/snippet}
    {#snippet below()}
      <div class="flex flex-col gap-2">
        {#if accounts.length > 1}
          <select
            bind:value={ncId}
            class="rounded-lg border border-surface-300/50 p-1.5 text-sm"
            aria-label={m.mobile_sr_nextcloud_account()}
          >
            {#each accounts as acc (acc.id)}
              <option value={acc.id}>{acc.display_name || acc.username}</option>
            {/each}
          </select>
        {/if}
        <SearchField bind:value={query} placeholder={m.mobile_files_search_placeholder()} />
        <nav class="flex flex-wrap items-center gap-1 text-xs" aria-label={m.mobile_breadcrumb()}>
          <button type="button" class="crumb" onclick={() => goTo(0)}>
            <Icon name="cloud" size={14} />
          </button>
          {#each segments as segment, index (index)}
            <span class="opacity-40">/</span>
            <button type="button" class="crumb" onclick={() => goTo(index + 1)}>{segment}</button>
          {/each}
        </nav>
      </div>
    {/snippet}
  </NavBar>

  {#if accounts.length === 0 && !loading}
    <EmptyState
      icon="cloud"
      title={m.mobile_no_nextcloud_title()}
      body={m.mobile_no_nextcloud_body()}
      action={{
        label: m.mobile_connect_nextcloud(),
        run: () => nav.push('settings-nextcloud'),
      }}
    />
  {:else if loading}
    <Spinner />
  {:else if error}
    <EmptyState icon="warning" title={m.mobile_files_error_title()} body={error} />
  {:else if entries.length === 0}
    <EmptyState icon="files" title={m.mobile_folder_empty()} />
  {:else if filtered.length === 0}
    <EmptyState
      icon="search"
      title={m.mobile_files_no_match_title()}
      body={m.mobile_files_no_match_body({ query })}
    />
  {:else}
    <PullToRefresh onrefresh={list}>
      <ul class="list-group">
        {#each filtered as entry (entry.path)}
          <li class="flex">
            <button class="list-row flex-1" onclick={() => enter(entry)}>
              {#if busyPath === entry.path}
                <Icon name="loading" size={20} />
              {:else if entry.is_dir}
                <Icon name="move-to-folder" size={20} />
              {:else}
                <FileTypeIcon
                  filename={entry.name}
                  contentType={entry.content_type}
                  class="w-5 h-5"
                />
              {/if}
              <span class="min-w-0 flex-1">
                <span class="block truncate list-row__title">{entry.name}</span>
                {#if !entry.is_dir}
                  <span class="block list-row__detail">
                    {fileSize(entry.size)}
                    {#if entry.modified}
                      · {new Date(entry.modified).toLocaleDateString(formatLocale())}
                    {/if}
                  </span>
                {/if}
              </span>
              {#if entry.is_dir}
                <Icon name="nav-forward" size={16} />
              {/if}
            </button>
            <button
              type="button"
              class="row-more"
              aria-label={m.mobile_more_actions()}
              onclick={() => (sheetFor = entry)}
            >
              <Icon name="more" size={18} />
            </button>
          </li>
        {/each}
      </ul>
      <div class="h-20"></div>
    </PullToRefresh>
  {/if}
</div>

{#if sheetFor}
  {@const entry = sheetFor}
  <ActionSheet
    title={entry.name}
    actions={[
      ...(entry.is_dir
        ? []
        : [
            { label: m.mobile_open(), icon: 'open-link' as const, run: () => open(entry) },
            {
              label: m.mobile_save_to_device(),
              icon: 'download' as const,
              run: () => saveToDevice(entry),
            },
          ]),
      { label: m.mobile_copy_share_link(), icon: 'share-links', run: () => share(entry) },
    ]}
    onclose={() => (sheetFor = null)}
  />
{/if}

{#if newFolderName !== null}
  <Sheet label={m.mobile_new_folder()} onclose={() => (newFolderName = null)}>
    <div class="form-group px-4 pb-6 pt-1">
      <h2 class="mb-3 text-base font-semibold">{m.mobile_new_folder()}</h2>
      <input
        type="text"
        bind:value={newFolderName}
        aria-label={m.mobile_folder_name_placeholder()}
        placeholder={m.mobile_folder_name_placeholder()}
        onkeydown={(e) => e.key === 'Enter' && createFolder()}
      />
      <button type="button" class="wide-button mt-4" onclick={createFolder}>
        {m.mobile_create()}
      </button>
    </div>
  </Sheet>
{/if}

<style>
  .crumb {
    padding: 2px 6px;
    border-radius: var(--r-xs);
    color: var(--ui-accent-ink);
  }
  .row-more {
    display: flex;
    align-items: center;
    min-width: 44px;
    padding-inline: 14px;
    color: var(--ui-ink-muted);
  }
  .row-more:active {
    background: color-mix(in oklab, var(--color-primary-500) 10%, transparent);
  }
</style>
