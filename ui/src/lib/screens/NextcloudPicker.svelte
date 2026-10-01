<script lang="ts">
  /**
   * A file picker over the user's Nextcloud, presented as a sheet.
   *
   * Used wherever the app needs to reach into the cloud without
   * leaving what the user was doing — attaching a share link to a
   * message being the case that matters. Browsing is deliberately
   * plain: breadcrumb, folder list, tap to descend, tap a file to
   * pick it.
   */
  import * as api from '../api'
  import type { FileEntry, NextcloudAccount } from '../api'
  import Icon from '../Icon.svelte'
  import FileTypeIcon from '../FileTypeIcon.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import { toasts } from '../toast.svelte'
  import { fileSize } from '../format'
  import { formatError } from '../errors'
  import { m } from '../../paraglide/messages'

  interface Props {
    /** `file` picks a file; `folder` picks the directory itself. */
    mode?: 'file' | 'folder'
    onpick: (ncId: string, path: string) => void
    onclose: () => void
  }

  let { mode = 'file', onpick, onclose }: Props = $props()

  let accounts = $state<NextcloudAccount[]>([])
  let ncId = $state('')
  let path = $state('/')
  let entries = $state<FileEntry[]>([])
  let loading = $state(true)
  let error = $state('')

  const segments = $derived(path.split('/').filter(Boolean))

  async function loadAccounts() {
    try {
      accounts = await api.nextcloud.getNextcloudAccounts()
      if (accounts.length > 0 && !ncId) ncId = accounts[0].id
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
      // The listing includes the directory itself; drop it so the
      // first row isn't a link back to where you already are.
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
    void list()
  })

  function goTo(depth: number) {
    path = depth === 0 ? '/' : `/${segments.slice(0, depth).join('/')}/`
  }

  function pick(entry: FileEntry) {
    if (entry.is_dir) {
      path = entry.path.endsWith('/') ? entry.path : `${entry.path}/`
      return
    }
    if (mode === 'folder') return
    onpick(ncId, entry.path)
  }
</script>

<Sheet label={m.mobile_nextcloud_picker_title()} {onclose}>
  <div class="px-4 pb-1 pt-1">
    <div class="flex items-center gap-2">
      <h2 class="flex-1 text-base font-semibold">{m.mobile_nextcloud_picker_title()}</h2>
      {#if mode === 'folder'}
        <button type="button" class="value-button" onclick={() => onpick(ncId, path)}>
          {m.mobile_choose_this_folder()}
        </button>
      {/if}
    </div>

    {#if accounts.length > 1}
      <select
        bind:value={ncId}
        class="mt-2 w-full rounded-lg border border-surface-300/50 p-2 text-sm"
        aria-label={m.mobile_sr_nextcloud_account()}
      >
        {#each accounts as acc (acc.id)}
          <option value={acc.id}>{acc.display_name || acc.username} · {acc.server_url}</option>
        {/each}
      </select>
    {/if}

    <nav class="mt-2 flex flex-wrap items-center gap-1 text-xs" aria-label={m.mobile_breadcrumb()}>
      <button type="button" class="crumb" onclick={() => goTo(0)}>
        <Icon name="cloud" size={14} />
      </button>
      {#each segments as segment, index (index)}
        <span class="opacity-50">/</span>
        <button type="button" class="crumb" onclick={() => goTo(index + 1)}>{segment}</button>
      {/each}
    </nav>
  </div>

  {#if accounts.length === 0 && !loading}
    <EmptyState
      icon="cloud"
      title={m.mobile_no_nextcloud_title()}
      body={m.mobile_no_nextcloud_body()}
    />
  {:else if loading}
    <div class="py-10"><Spinner /></div>
  {:else if error}
    <p class="px-4 py-6 text-center text-sm ink-danger">{error}</p>
  {:else if entries.length === 0}
    <p class="px-4 py-8 text-center text-sm ink-muted">{m.mobile_folder_empty()}</p>
  {:else}
    <ul class="max-h-[55vh] overflow-y-auto">
      {#each entries as entry (entry.path)}
        <li>
          <button type="button" class="list-row" onclick={() => pick(entry)}>
            {#if entry.is_dir}
              <Icon name="files" size={20} />
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
                <span class="block list-row__detail">{fileSize(entry.size)}</span>
              {/if}
            </span>
            {#if entry.is_dir}
              <Icon name="nav-forward" size={16} />
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</Sheet>

<style>
  .crumb {
    padding: 2px 6px;
    border-radius: var(--r-xs);
    color: var(--ui-accent-ink);
  }
  .crumb:active {
    opacity: 0.6;
  }
</style>
