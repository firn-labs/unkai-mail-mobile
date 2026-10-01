<script lang="ts">
  /**
   * Tasks from the user's Nextcloud task lists.
   *
   * One screen rather than list-then-tasks: on a phone, the useful
   * question is "what do I have to do", not "what's in list #3". So
   * the lists become a filter chip row at the top and everything
   * else is one prioritised list, with completed items tucked behind
   * a segment.
   */
  import * as api from '../api'
  import type { NextcloudAccount, Task, TaskList } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import PullToRefresh from '../ui/PullToRefresh.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import Segmented from '../ui/Segmented.svelte'
  import Fab from '../ui/Fab.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import SwipeRow from '../ui/SwipeRow.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { dayHeading, toDateInput, parseLocalDate } from '../format'
  import { m } from '../../paraglide/messages'

  let ncAccounts = $state<NextcloudAccount[]>([])
  let lists = $state<TaskList[]>([])
  let tasks = $state<Task[]>([])
  let loading = $state(true)
  let syncing = $state(false)
  let mode = $state<'open' | 'done'>('open')
  let listFilter = $state<string | null>(null)

  /** The create/edit sheet's working copy. `null` = closed. */
  let editing = $state<{
    task: Task | null
    listId: string
    summary: string
    description: string
    due: string
    priority: number
  } | null>(null)

  const visibleLists = $derived(lists.filter((l) => !l.hidden))

  const filtered = $derived(
    tasks
      .filter((t: Task) => (mode === 'done' ? t.status === 'COMPLETED' : t.status !== 'COMPLETED'))
      .filter((t: Task) => (listFilter ? t.task_list_id === listFilter : true))
      .filter((t: Task) => visibleLists.some((l) => l.id === t.task_list_id))
      .sort((a: Task, b: Task) => {
        // Due first (soonest at the top), then by priority, then
        // alphabetically — the order someone works down a list in.
        const ad = a.due ? new Date(a.due).getTime() : Number.POSITIVE_INFINITY
        const bd = b.due ? new Date(b.due).getTime() : Number.POSITIVE_INFINITY
        if (ad !== bd) return ad - bd
        const ap = a.priority || 9
        const bp = b.priority || 9
        if (ap !== bp) return ap - bp
        return a.summary.localeCompare(b.summary)
      }),
  )

  async function load() {
    try {
      ncAccounts = await api.nextcloud.getNextcloudAccounts()
      const allLists: TaskList[] = []
      const allTasks: Task[] = []
      for (const acc of ncAccounts) {
        try {
          allLists.push(...(await api.tasks.listNextcloudTaskLists({ ncId: acc.id })))
          allTasks.push(...(await api.tasks.listNextcloudTasks({ ncId: acc.id })))
        } catch {
          // Cached data for the other accounts is still worth showing.
        }
      }
      lists = allLists
      tasks = allTasks
    } catch (e) {
      toasts.error(m.mobile_tasks_load_failed(), e)
    } finally {
      loading = false
    }
  }

  async function sync() {
    syncing = true
    try {
      for (const acc of ncAccounts) {
        const synced = await api.tasks.syncNextcloudTaskLists({ ncId: acc.id })
        for (const list of synced) {
          if (list.hidden) continue
          await api.tasks.syncNextcloudTasks({ ncId: acc.id, listId: list.id })
        }
      }
      await load()
      toasts.success(m.mobile_tasks_synced())
    } catch (e) {
      toasts.error(m.mobile_tasks_sync_failed(), e)
    } finally {
      syncing = false
    }
  }

  $effect(() => {
    void load()
  })

  function ncIdFor(task: Task): string {
    return lists.find((l) => l.id === task.task_list_id)?.nextcloud_account_id ?? ''
  }

  async function toggleDone(task: Task) {
    const done = task.status === 'COMPLETED'
    // Optimistic: ticking a checkbox has to feel immediate, and the
    // server call is a CalDAV round-trip.
    tasks = tasks.map((t) =>
      t.uid === task.uid ? { ...t, status: done ? 'NEEDS-ACTION' : 'COMPLETED' } : t,
    )
    try {
      const updated = await api.tasks.updateNextcloudTask({
        ncId: ncIdFor(task),
        listId: task.task_list_id,
        uid: task.uid,
        etag: task.etag,
        status: done ? 'NEEDS-ACTION' : 'COMPLETED',
        ...(done
          ? { clearCompleted: true }
          : { completedUnix: Math.floor(Date.now() / 1000) }),
      })
      tasks = tasks.map((t) => (t.uid === task.uid ? updated : t))
    } catch (e) {
      tasks = tasks.map((t) => (t.uid === task.uid ? task : t))
      toasts.error(m.mobile_task_update_failed(), e)
    }
  }

  async function remove(task: Task) {
    const previous = tasks
    tasks = tasks.filter((t) => t.uid !== task.uid)
    try {
      await api.tasks.deleteNextcloudTask({
        ncId: ncIdFor(task),
        listId: task.task_list_id,
        uid: task.uid,
      })
      toasts.show(m.mobile_task_deleted())
    } catch (e) {
      tasks = previous
      toasts.error(m.mobile_task_delete_failed(), e)
    }
  }

  function openEditor(task: Task | null) {
    const listId = task?.task_list_id ?? listFilter ?? visibleLists[0]?.id ?? ''
    if (!listId) {
      toasts.error(m.mobile_no_task_lists())
      return
    }
    editing = {
      task,
      listId,
      summary: task?.summary ?? '',
      description: task?.description ?? '',
      due: task?.due ? toDateInput(new Date(task.due)) : '',
      priority: task?.priority ?? 0,
    }
  }

  async function saveEditor() {
    if (!editing) return
    const { task, listId, summary, description, due, priority } = editing
    if (!summary.trim()) {
      toasts.error(m.mobile_task_needs_title())
      return
    }
    const list = lists.find((l) => l.id === listId)
    const ncId = list?.nextcloud_account_id ?? ''
    const dueDate = due ? parseLocalDate(due) : null
    const dueUnix = dueDate ? Math.floor(dueDate.getTime() / 1000) : null

    try {
      if (task) {
        const updated = await api.tasks.updateNextcloudTask({
          ncId,
          listId,
          uid: task.uid,
          etag: task.etag,
          summary: summary.trim(),
          description: description.trim() || null,
          priority,
          ...(dueUnix
            ? { dueUnix, dueTz: Intl.DateTimeFormat().resolvedOptions().timeZone }
            : { clearDue: true }),
        })
        tasks = tasks.map((t) => (t.uid === task.uid ? updated : t))
      } else {
        const created = await api.tasks.createNextcloudTask({
          ncId,
          listId,
          summary: summary.trim(),
          description: description.trim() || null,
          priority: priority || null,
          ...(dueUnix
            ? { dueUnix, dueTz: Intl.DateTimeFormat().resolvedOptions().timeZone }
            : {}),
        })
        tasks = [...tasks, created]
      }
      editing = null
    } catch (e) {
      toasts.error(m.mobile_task_save_failed(), e)
    }
  }

  function dueLabel(task: Task): string {
    if (!task.due) return ''
    return dayHeading(new Date(task.due))
  }

  function overdue(task: Task): boolean {
    if (!task.due || task.status === 'COMPLETED') return false
    return new Date(task.due).getTime() < Date.now()
  }
</script>

<div class="screen">
  <!-- A feature is a tab root *or* a page under More, depending on how
       the user arranged the bar (Settings → Features). Back appears
       exactly when there is somewhere to go back to. -->
  <NavBar
    title={m.mobile_tasks()}
    backLabel={nav.canGoBack ? m.mobile_more() : null}
    onback={() => nav.pop()}
    busy={syncing}
    large
  >
    {#snippet actions()}
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_sync()}
        onclick={sync}
        disabled={syncing || ncAccounts.length === 0}
      >
        <Icon name={syncing ? 'loading' : 'sync'} size={20} />
      </button>
    {/snippet}
    {#snippet below()}
      <Segmented
        value={mode}
        options={[
          { value: 'open', label: m.mobile_tasks_open() },
          { value: 'done', label: m.mobile_tasks_done() },
        ]}
        onchange={(v) => (mode = v as 'open' | 'done')}
      />
    {/snippet}
  </NavBar>

  {#if loading}
    <Spinner />
  {:else if visibleLists.length === 0}
    <EmptyState
      icon="tasks"
      title={m.mobile_no_task_lists_title()}
      body={m.mobile_no_task_lists_body()}
      action={{
        label: m.mobile_connect_nextcloud(),
        run: () => nav.go('more', 'settings-nextcloud'),
      }}
    />
  {:else}
    {#if visibleLists.length > 1}
      <div class="chip-row">
        <button
          type="button"
          class="chip-filter"
          class:chip-filter--on={listFilter === null}
          onclick={() => (listFilter = null)}
        >
          {m.mobile_all_lists()}
        </button>
        {#each visibleLists as list (list.id)}
          <button
            type="button"
            class="chip-filter"
            class:chip-filter--on={listFilter === list.id}
            onclick={() => (listFilter = list.id)}
          >
            {#if list.color}
              <span class="chip-dot" style="background: {list.color}"></span>
            {/if}
            {list.display_name || list.name}
          </button>
        {/each}
      </div>
    {/if}

    {#if filtered.length === 0}
      <EmptyState
        icon="success"
        title={mode === 'done' ? m.mobile_no_done_tasks() : m.mobile_no_open_tasks()}
        body={mode === 'done' ? '' : m.mobile_no_open_tasks_body()}
      />
    {:else}
      <PullToRefresh onrefresh={sync}>
        <ul>
          {#each filtered as task (task.uid)}
            <li>
              <SwipeRow
                onclick={() => openEditor(task)}
                trailing={[
                  {
                    label: m.mobile_delete(),
                    icon: 'trash',
                    tone: 'error',
                    run: () => remove(task),
                  },
                ]}
                leading={[
                  {
                    label: task.status === 'COMPLETED' ? m.mobile_task_reopen() : m.mobile_task_done(),
                    icon: task.status === 'COMPLETED' ? 'not-done' : 'success',
                    tone: 'success',
                    run: () => toggleDone(task),
                  },
                ]}
              >
                <div class="task-row">
                  <button
                    type="button"
                    class="task-check"
                    class:task-check--on={task.status === 'COMPLETED'}
                    aria-label={task.status === 'COMPLETED'
                      ? m.mobile_task_reopen()
                      : m.mobile_task_done()}
                    onclick={(e) => {
                      e.stopPropagation()
                      void toggleDone(task)
                    }}
                  >
                    {#if task.status === 'COMPLETED'}<Icon name="check" size={14} />{/if}
                  </button>
                  <span class="min-w-0 flex-1">
                    <span
                      class="block truncate t-body"
                      class:line-through={task.status === 'COMPLETED'}
                      class:ink-muted={task.status === 'COMPLETED'}
                    >
                      {task.summary}
                    </span>
                    <span class="flex items-center gap-2">
                      {#if task.due}
                        <span
                          class="list-row__detail"
                          class:ink-danger={overdue(task)}
                        >
                          {dueLabel(task)}
                        </span>
                      {/if}
                      {#if task.priority > 0 && task.priority <= 4}
                        <span class="ink-danger"><Icon name="important" size={13} /></span>
                      {/if}
                      {#if !listFilter && visibleLists.length > 1}
                        <span class="list-row__detail truncate">
                          {visibleLists.find((l) => l.id === task.task_list_id)?.display_name ?? ''}
                        </span>
                      {/if}
                    </span>
                  </span>
                </div>
              </SwipeRow>
            </li>
          {/each}
        </ul>
        <div class="h-24"></div>
      </PullToRefresh>
    {/if}

    <Fab label={m.mobile_new_task()} onclick={() => openEditor(null)} />
  {/if}
</div>

{#if editing}
  <Sheet label={editing.task ? m.mobile_edit_task() : m.mobile_new_task()} onclose={() => (editing = null)}>
    <div class="form-group px-4 pb-6 pt-1">
      <h2 class="mb-3 text-base font-semibold">
        {editing.task ? m.mobile_edit_task() : m.mobile_new_task()}
      </h2>
      <input
        type="text"
        bind:value={editing.summary}
        aria-label={m.mobile_task_title_placeholder()}
        placeholder={m.mobile_task_title_placeholder()}
      />
      <textarea
        class="mt-2"
        rows="3"
        bind:value={editing.description}
        aria-label={m.mobile_task_notes_placeholder()}
        placeholder={m.mobile_task_notes_placeholder()}
      ></textarea>
      <div class="mt-2 flex gap-2">
        <label class="flex-1 text-xs">
          {m.mobile_task_due()}
          <input type="date" bind:value={editing.due} class="mt-1" />
        </label>
        <label class="flex-1 text-xs">
          {m.mobile_task_priority()}
          <select bind:value={editing.priority} class="mt-1">
            <option value={0}>{m.mobile_priority_none()}</option>
            <option value={1}>{m.mobile_priority_high()}</option>
            <option value={5}>{m.mobile_priority_medium()}</option>
            <option value={9}>{m.mobile_priority_low()}</option>
          </select>
        </label>
      </div>
      <label class="mt-2 block text-xs">
        {m.mobile_task_list()}
        <select bind:value={editing.listId} class="mt-1">
          {#each visibleLists as list (list.id)}
            <option value={list.id}>{list.display_name || list.name}</option>
          {/each}
        </select>
      </label>
      <button type="button" class="wide-button mt-4" onclick={saveEditor}>
        {m.mobile_save()}
      </button>
    </div>
  </Sheet>
{/if}

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
    display: inline-flex;
    align-items: center;
    gap: 5px;
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

  .chip-dot {
    width: 8px;
    height: 8px;
    border-radius: var(--r-full);
  }

  .task-row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 12px;
    min-height: 56px;
  }

  li + li .task-row {
    border-top: 1px solid color-mix(in oklab, var(--color-surface-500) 16%, transparent);
  }

  .task-check {
    width: 24px;
    height: 24px;
    flex-shrink: 0;
    position: relative;
    border-radius: var(--r-full);
    border: 1.5px solid var(--ui-control);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--ui-on-success);
  }

  /* 44pt to tap; the drawn circle stays 24px. */
  .task-check::before {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    width: 44px;
    height: 44px;
    transform: translate(-50%, -50%);
  }

  .task-check--on {
    background: var(--ui-success-fill);
    border-color: var(--ui-success-fill);
  }
</style>
