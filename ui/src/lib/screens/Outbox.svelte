<script lang="ts">
  /**
   * The outbox: messages the app accepted but couldn't deliver.
   *
   * On a phone this matters more than on a desktop — sending while
   * the signal drops is normal, not exceptional. Every row shows why
   * it's still here and offers the two things the user can do about
   * it: try again, or throw it away.
   */
  import * as api from '../api'
  import type { OutboxRowDto } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import PullToRefresh from '../ui/PullToRefresh.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import ActionSheet from '../ui/ActionSheet.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { fullDate } from '../format'
  import { m } from '../../paraglide/messages'

  let rows = $state<OutboxRowDto[]>([])
  let loading = $state(true)
  let sheetFor = $state<OutboxRowDto | null>(null)

  async function load() {
    try {
      rows = await api.compose.listAllOutbox()
    } catch (e) {
      toasts.error(m.mobile_outbox_load_failed(), e)
    } finally {
      loading = false
    }
  }

  $effect(() => {
    void load()
  })

  async function retry(row: OutboxRowDto) {
    try {
      await api.compose.retryOutboxEntry({ id: row.id })
      toasts.show(m.mobile_outbox_retrying())
      await load()
    } catch (e) {
      toasts.error(m.mobile_outbox_retry_failed(), e)
    }
  }

  async function remove(row: OutboxRowDto) {
    try {
      await api.compose.deleteOutboxEntry({ id: row.id })
      rows = rows.filter((r) => r.id !== row.id)
      toasts.show(m.mobile_outbox_deleted())
    } catch (e) {
      toasts.error(m.mobile_outbox_delete_failed(), e)
    }
  }
</script>

<div class="screen">
  <NavBar title={m.mobile_outbox()} backLabel={m.mobile_back()} onback={() => nav.pop()} />

  {#if loading}
    <Spinner />
  {:else if rows.length === 0}
    <EmptyState
      icon="sent"
      title={m.mobile_outbox_empty_title()}
      body={m.mobile_outbox_empty_body()}
    />
  {:else}
    <PullToRefresh onrefresh={load}>
      <ul class="list-group">
        {#each rows as row (row.id)}
          <li>
            <button class="list-row" onclick={() => (sheetFor = row)}>
              <Icon name={row.last_error ? 'warning' : 'loading'} size={20} />
              <span class="min-w-0 flex-1">
                <span class="block truncate list-row__title">
                  {row.subject || m.mobile_no_subject()}
                </span>
                <span class="block truncate list-row__detail">
                  {m.mobile_outbox_to({ recipients: row.to_display })}
                </span>
                {#if row.last_error}
                  <span class="mt-0.5 block text-xs ink-danger">{row.last_error}</span>
                {/if}
                <span class="block list-row__detail">
                  {m.mobile_outbox_attempts({
                    count: row.attempt_count,
                    when: fullDate(new Date(row.queued_at * 1000)),
                  })}
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
  {@const row = sheetFor}
  <ActionSheet
    title={row.subject || m.mobile_no_subject()}
    actions={[
      { label: m.mobile_outbox_retry(), icon: 'sync', run: () => retry(row) },
      {
        label: m.mobile_delete(),
        icon: 'trash',
        destructive: true,
        run: () => remove(row),
      },
    ]}
    onclose={() => (sheetFor = null)}
  />
{/if}
