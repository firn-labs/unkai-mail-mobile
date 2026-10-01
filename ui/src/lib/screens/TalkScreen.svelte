<script lang="ts">
  /**
   * Nextcloud Talk rooms.
   *
   * The app doesn't implement the chat protocol — it manages rooms
   * and gets you into one. That's the integration that matters from
   * a mail client: create a room for the thread you're reading,
   * paste its link into a reply, or jump into the call. Messages
   * themselves belong to the Talk app, which `Join` opens.
   */
  import { formatLocale } from '../locale'
  import * as api from '../api'
  import type { NextcloudAccount, TalkRoom } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import PullToRefresh from '../ui/PullToRefresh.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import SearchField from '../ui/SearchField.svelte'
  import ActionSheet from '../ui/ActionSheet.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import Fab from '../ui/Fab.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    oncompose: (initial: { body: string; subject?: string }) => void
  }

  let { oncompose }: Props = $props()

  let accounts = $state<NextcloudAccount[]>([])
  let ncId = $state('')
  let rooms = $state<TalkRoom[]>([])
  let loading = $state(true)
  let query = $state('')
  let sheetFor = $state<TalkRoom | null>(null)
  let newRoomName = $state<string | null>(null)

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase()
    const list = [...rooms].sort((a, b) => b.last_activity - a.last_activity)
    if (!q) return list
    return list.filter((r: TalkRoom) => r.display_name.toLowerCase().includes(q))
  })

  async function load() {
    loading = true
    try {
      accounts = await api.nextcloud.getNextcloudAccounts()
      if (!ncId && accounts.length > 0) ncId = accounts[0].id
      if (ncId) rooms = await api.talk.listTalkRooms({ ncId })
    } catch (e) {
      toasts.error(m.mobile_talk_load_failed(), e)
    } finally {
      loading = false
    }
  }

  $effect(() => {
    void ncId
    void load()
  })

  function join(room: TalkRoom) {
    void api.system
      .openUrl({ url: room.web_url })
      .catch((e) => toasts.error(m.mobile_open_failed(), e))
  }

  async function shareInMail(room: TalkRoom) {
    oncompose({
      subject: m.mobile_talk_invite_subject({ room: room.display_name }),
      body: m.mobile_talk_invite_body({ room: room.display_name, url: room.web_url }),
    })
  }

  async function copyLink(room: TalkRoom) {
    try {
      await navigator.clipboard.writeText(room.web_url)
      toasts.success(m.mobile_link_copied())
    } catch {
      toasts.error(m.mobile_copy_failed())
    }
  }

  async function remove(room: TalkRoom) {
    try {
      await api.talk.deleteTalkRoom({ ncId, roomToken: room.token })
      rooms = rooms.filter((r) => r.token !== room.token)
      toasts.show(m.mobile_room_deleted())
    } catch (e) {
      toasts.error(m.mobile_room_delete_failed(), e)
    }
  }

  async function create() {
    const name = (newRoomName ?? '').trim()
    newRoomName = null
    if (!name || !ncId) return
    try {
      const room = await api.talk.createTalkRoom({ ncId, roomName: name, participants: [] })
      // Public by default: a room created from a mail client exists
      // to be shared with people who may not have an account on this
      // Nextcloud.
      try {
        await api.talk.setTalkRoomPublic({ ncId, roomToken: room.token, public: true })
      } catch {
        // Some servers forbid public rooms by policy; the room still
        // works for internal participants.
      }
      await load()
      toasts.success(m.mobile_room_created())
    } catch (e) {
      toasts.error(m.mobile_room_create_failed(), e)
    }
  }
</script>

<div class="screen">
  <!-- A feature is a tab root *or* a page under More, depending on how
       the user arranged the bar (Settings → Features). Back appears
       exactly when there is somewhere to go back to. -->
  <NavBar
    title={m.mobile_talk()}
    backLabel={nav.canGoBack ? m.mobile_more() : null}
    onback={() => nav.pop()}
  >
    {#snippet actions()}
      <button type="button" class="bar-button" aria-label={m.mobile_refresh()} onclick={load}>
        <Icon name="refresh" size={20} />
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
        <SearchField bind:value={query} placeholder={m.mobile_talk_search_placeholder()} />
      </div>
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
  {:else if rooms.length === 0}
    <EmptyState icon="meetings" title={m.mobile_no_rooms_title()} body={m.mobile_no_rooms_body()} />
  {:else if filtered.length === 0}
    <EmptyState
      icon="search"
      title={m.mobile_rooms_no_match_title()}
      body={m.mobile_rooms_no_match_body({ query })}
    />
  {:else}
    <PullToRefresh onrefresh={load}>
      <ul class="list-group">
        {#each filtered as room (room.token)}
          <li class="flex">
            <button class="list-row flex-1" onclick={() => join(room)}>
              <Icon name="meetings" size={20} />
              <span class="min-w-0 flex-1">
                <span class="block truncate list-row__title">{room.display_name}</span>
                <span class="block list-row__detail">
                  {new Date(room.last_activity * 1000).toLocaleString(formatLocale())}
                </span>
              </span>
              {#if room.unread_messages > 0}
                <span class="count-pill count-pill--accent">{room.unread_messages}</span>
              {/if}
            </button>
            <button
              type="button"
              class="row-more"
              aria-label={m.mobile_more_actions()}
              onclick={() => (sheetFor = room)}
            >
              <Icon name="more" size={18} />
            </button>
          </li>
        {/each}
      </ul>
      <div class="h-24"></div>
    </PullToRefresh>

    <Fab label={m.mobile_new_room()} onclick={() => (newRoomName = '')} />
  {/if}
</div>

{#if sheetFor}
  {@const room = sheetFor}
  <ActionSheet
    title={room.display_name}
    actions={[
      { label: m.mobile_join(), icon: 'open-in-browser', run: () => join(room) },
      { label: m.mobile_share_in_email(), icon: 'compose', run: () => shareInMail(room) },
      { label: m.mobile_copy_link(), icon: 'copy', run: () => copyLink(room) },
      {
        label: m.mobile_delete_room(),
        icon: 'trash',
        destructive: true,
        run: () => remove(room),
      },
    ]}
    onclose={() => (sheetFor = null)}
  />
{/if}

{#if newRoomName !== null}
  <Sheet label={m.mobile_new_room()} onclose={() => (newRoomName = null)}>
    <div class="form-group px-4 pb-6 pt-1">
      <h2 class="mb-1 text-base font-semibold">{m.mobile_new_room()}</h2>
      <p class="mb-3 text-sm ink-muted">{m.mobile_new_room_hint()}</p>
      <input
        type="text"
        bind:value={newRoomName}
        aria-label={m.mobile_room_name_placeholder()}
        placeholder={m.mobile_room_name_placeholder()}
        onkeydown={(e) => e.key === 'Enter' && create()}
      />
      <button type="button" class="wide-button mt-4" onclick={create}>{m.mobile_create()}</button>
    </div>
  </Sheet>
{/if}

<style>
  .row-more {
    display: flex;
    align-items: center;
    min-width: 44px;
    padding-inline: 14px;
    color: var(--ui-ink-muted);
  }
</style>
