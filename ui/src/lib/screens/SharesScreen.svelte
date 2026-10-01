<script lang="ts">
  /**
   * Public share links the user has handed out.
   *
   * The reason this is its own screen rather than a detail of
   * Files: a share is a standing grant of access, and the thing
   * people want is a list of everything currently exposed — with
   * one tap to revoke.
   */
  import { formatLocale } from '../locale'
  import * as api from '../api'
  import type { NextcloudAccount, NextcloudShareRow } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import PullToRefresh from '../ui/PullToRefresh.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import SearchField from '../ui/SearchField.svelte'
  import ActionSheet from '../ui/ActionSheet.svelte'
  import Confirm from '../ui/Confirm.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { parseLocalDate } from '../format'
  import { m } from '../../paraglide/messages'

  interface Props {
    oncompose: (initial: { body: string }) => void
  }

  let { oncompose }: Props = $props()

  let accounts = $state<NextcloudAccount[]>([])
  let shares = $state<NextcloudShareRow[]>([])
  let loading = $state(true)
  let query = $state('')
  let sheetFor = $state<NextcloudShareRow | null>(null)
  let confirmDelete = $state<NextcloudShareRow | null>(null)

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase()
    if (!q) return shares
    return shares.filter(
      (s: NextcloudShareRow) =>
        s.path.toLowerCase().includes(q) || (s.label ?? '').toLowerCase().includes(q),
    )
  })

  async function load() {
    loading = true
    try {
      accounts = await api.nextcloud.getNextcloudAccounts()
      const all: NextcloudShareRow[] = []
      for (const acc of accounts) {
        try {
          all.push(...(await api.nextcloud.listNextcloudShares({ ncId: acc.id })))
        } catch {
          // One unreachable server shouldn't hide the others' shares.
        }
      }
      shares = all.sort((a, b) => b.stime - a.stime)
    } catch (e) {
      toasts.error(m.mobile_shares_load_failed(), e)
    } finally {
      loading = false
    }
  }

  $effect(() => {
    void load()
  })

  async function copy(share: NextcloudShareRow) {
    try {
      await navigator.clipboard.writeText(share.url)
      toasts.success(m.mobile_link_copied())
    } catch {
      toasts.error(m.mobile_copy_failed())
    }
  }

  async function revoke(share: NextcloudShareRow) {
    confirmDelete = null
    try {
      await api.nextcloud.deleteNextcloudShare({ ncId: share.nc_id, shareId: share.id })
      shares = shares.filter((s) => s.id !== share.id)
      toasts.show(m.mobile_share_revoked())
    } catch (e) {
      toasts.error(m.mobile_share_revoke_failed(), e)
    }
  }

  function fileName(path: string): string {
    return path.split('/').filter(Boolean).pop() ?? path
  }

  function expiryLabel(share: NextcloudShareRow): string {
    if (!share.expiration) return ''
    const date = parseLocalDate(share.expiration)
    return date ? m.mobile_share_expires({ date: date.toLocaleDateString(formatLocale()) }) : ''
  }
</script>

<div class="screen">
  <!-- A feature is a tab root *or* a page under More, depending on how
       the user arranged the bar (Settings → Features). Back appears
       exactly when there is somewhere to go back to. -->
  <NavBar
    title={m.mobile_shares()}
    backLabel={nav.canGoBack ? m.mobile_more() : null}
    onback={() => nav.pop()}
  >
    {#snippet actions()}
      <button type="button" class="bar-button" aria-label={m.mobile_refresh()} onclick={load}>
        <Icon name="refresh" size={20} />
      </button>
    {/snippet}
    {#snippet below()}
      <SearchField bind:value={query} placeholder={m.mobile_shares_search_placeholder()} />
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
  {:else if shares.length === 0}
    <EmptyState
      icon="share-links"
      title={m.mobile_no_shares_title()}
      body={m.mobile_no_shares_body()}
    />
  {:else if filtered.length === 0}
    <EmptyState
      icon="search"
      title={m.mobile_shares_no_match_title()}
      body={m.mobile_shares_no_match_body({ query })}
    />
  {:else}
    <PullToRefresh onrefresh={load}>
      <ul class="list-group">
        {#each filtered as share (share.id)}
          <li>
            <button class="list-row" onclick={() => (sheetFor = share)}>
              <Icon name="share-links" size={20} />
              <span class="min-w-0 flex-1">
                <span class="block truncate list-row__title">
                  {share.label || fileName(share.path)}
                </span>
                <span class="block truncate list-row__detail">{share.path}</span>
                <span class="mt-0.5 flex items-center gap-2 list-row__detail">
                  <span class="inline-flex items-center gap-1">
                    <Icon name={share.has_password ? 'lock' : 'unlocked'} size={12} />
                    {share.has_password ? m.mobile_share_protected() : m.mobile_share_open()}
                  </span>
                  {#if expiryLabel(share)}<span>· {expiryLabel(share)}</span>{/if}
                </span>
              </span>
              <Icon name="more" size={16} />
            </button>
          </li>
        {/each}
      </ul>
      <div class="h-20"></div>
    </PullToRefresh>
  {/if}
</div>

{#if sheetFor}
  {@const share = sheetFor}
  <ActionSheet
    title={share.label || fileName(share.path)}
    subtitle={share.path}
    actions={[
      { label: m.mobile_copy_link(), icon: 'copy', run: () => copy(share) },
      {
        label: m.mobile_open_in_browser(),
        icon: 'open-in-browser',
        run: () => api.system.openUrl({ url: share.url }).catch(() => {}),
      },
      {
        label: m.mobile_share_in_email(),
        icon: 'compose',
        run: () =>
          oncompose({
            body: m.mobile_share_mail_body({
              name: fileName(share.path),
              url: share.url,
            }),
          }),
      },
      {
        label: m.mobile_revoke_share(),
        icon: 'trash',
        destructive: true,
        run: () => (confirmDelete = share),
      },
    ]}
    onclose={() => (sheetFor = null)}
  />
{/if}

{#if confirmDelete}
  {@const share = confirmDelete}
  <Confirm
    title={m.mobile_revoke_share()}
    body={m.mobile_revoke_share_body({ name: fileName(share.path) })}
    confirmLabel={m.mobile_revoke()}
    onconfirm={() => revoke(share)}
    oncancel={() => (confirmDelete = null)}
  />
{/if}
