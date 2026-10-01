<script lang="ts">
  /**
   * Search across cached mail, with an explicit server-search
   * escape hatch.
   *
   * The local index answers instantly and works offline, which is
   * what a phone wants; but it only knows messages this device has
   * seen. When the user is looking for something older, the
   * "Search on the server" row runs an IMAP SEARCH against the
   * mailbox they came from. Making that a separate, labelled action
   * rather than an automatic fallback keeps the fast path fast and
   * the slow path honest.
   */
  import * as api from '../api'
  import type { EmailEnvelope, SearchHit } from '../api'
  import Icon from '../Icon.svelte'
  import Avatar from '../Avatar.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import SearchField from '../ui/SearchField.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { listDate } from '../format'
  import { parseFromHeader } from '../fromHeader'
  import { envelopeKey, mailLists } from '../mail/listBus'
  import { m } from '../../paraglide/messages'

  interface Props {
    /** The mailbox the user searched from, if any — the scope the
     *  server search runs against. */
    accountId?: string | null
    folder?: string | null
  }

  let { accountId = null, folder = null }: Props = $props()

  let query = $state('')
  let hits = $state<SearchHit[]>([])
  let serverHits = $state<EmailEnvelope[]>([])
  let searching = $state(false)
  let searchingServer = $state(false)
  let ran = $state(false)

  let debounce: ReturnType<typeof setTimeout> | null = null

  $effect(() => {
    // Depend on `query` explicitly so the clear button (which assigns
    // programmatically and fires no input event) re-runs the search.
    void query
    if (debounce) clearTimeout(debounce)
    if (query.trim().length < 2) {
      hits = []
      serverHits = []
      ran = false
      return
    }
    debounce = setTimeout(() => void runLocal(), 250)
    return () => {
      if (debounce) clearTimeout(debounce)
    }
  })

  async function runLocal() {
    searching = true
    serverHits = []
    try {
      hits = await api.mail.searchEmails({ query: query.trim() })
      ran = true
    } catch (e) {
      toasts.error(m.mobile_search_failed(), e)
      hits = []
    } finally {
      searching = false
    }
  }

  async function runServer() {
    if (!accountId || !folder) return
    searchingServer = true
    try {
      serverHits = await api.mail.searchImapServer({
        accountId,
        folder,
        query: query.trim(),
        limit: 50,
      })
      if (serverHits.length === 0) toasts.show(m.mobile_search_server_empty())
    } catch (e) {
      toasts.error(m.mobile_search_failed(), e)
    } finally {
      searchingServer = false
    }
  }

  /**
   * Results stay on screen under the reader (the shell keeps this screen
   * alive, so Back returns to the same query). A message archived or
   * deleted from there leaves the results too — and comes back if the
   * server refused (`mail/listBus.ts`).
   */
  const hiddenHits = new Map<string, { hit?: SearchHit; server?: EmailEnvelope }>()
  $effect(() =>
    mailLists.register({
      hide(key) {
        const hit = hits.find((h) => envelopeKey(h) === key)
        const server = serverHits.find((h) => envelopeKey(h) === key)
        if (!hit && !server) return
        hiddenHits.set(key, { hit, server })
        hits = hits.filter((h) => envelopeKey(h) !== key)
        serverHits = serverHits.filter((h) => envelopeKey(h) !== key)
      },
      unhide(key) {
        const gone = hiddenHits.get(key)
        if (!gone) return
        hiddenHits.delete(key)
        if (gone.hit) hits = [gone.hit, ...hits]
        if (gone.server) serverHits = [gone.server, ...serverHits]
      },
      settle(key) {
        hiddenHits.delete(key)
      },
      patch() {
        // Results show no read or flag state.
      },
    }),
  )

  function openHit(hit: { account_id: string; folder: string; uid: number; subject?: string }) {
    nav.push('mail-message', {
      accountId: hit.account_id,
      folder: hit.folder,
      uid: hit.uid,
      subject: hit.subject ?? '',
    })
  }
</script>

<div class="screen">
  <NavBar title={m.mobile_search()} backLabel={m.mobile_back()} onback={() => nav.pop()}>
    {#snippet below()}
      <SearchField
        bind:value={query}
        placeholder={m.mobile_search_placeholder()}
        autofocus
        onsubmit={() => void runLocal()}
      />
    {/snippet}
  </NavBar>

  <div class="screen__body">
    {#if searching}
      <Spinner />
    {:else if query.trim().length < 2}
      <EmptyState
        icon="search"
        title={m.mobile_search_prompt_title()}
        body={m.mobile_search_prompt_body()}
      />
    {:else if hits.length === 0 && serverHits.length === 0 && ran}
      <EmptyState
        icon="search"
        title={m.mobile_search_empty_title()}
        body={m.mobile_search_empty_body({ query })}
      />
    {/if}

    {#if hits.length > 0}
      <div class="list-header">{m.mobile_search_local_results({ count: hits.length })}</div>
      <ul class="list-group">
        {#each hits as hit (`${hit.account_id}-${hit.folder}-${hit.uid}`)}
          {@const from = parseFromHeader(hit.from ?? '')}
          <li>
            <button class="list-row" onclick={() => openHit(hit)}>
              <Avatar
                displayName={from.name || from.email || hit.from}
                size={34}
              />
              <span class="min-w-0 flex-1">
                <span class="flex items-baseline gap-2">
                  <span class="truncate text-sm font-medium">{from.name || from.email}</span>
                  <span class="ml-auto shrink-0 list-row__detail">{listDate(hit.date)}</span>
                </span>
                <span class="block truncate text-sm">{hit.subject || m.mobile_no_subject()}</span>
                <span class="block truncate list-row__detail">{hit.folder}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    {#if serverHits.length > 0}
      <div class="list-header">{m.mobile_search_server_results({ count: serverHits.length })}</div>
      <ul class="list-group">
        {#each serverHits as env (env.uid)}
          {@const from = parseFromHeader(env.from)}
          <li>
            <button class="list-row" onclick={() => openHit(env)}>
              <Avatar
                displayName={from.name || from.email || env.from}
                size={34}
              />
              <span class="min-w-0 flex-1">
                <span class="flex items-baseline gap-2">
                  <span class="truncate text-sm font-medium">{from.name || from.email}</span>
                  <span class="ml-auto shrink-0 list-row__detail">{listDate(env.date)}</span>
                </span>
                <span class="block truncate text-sm">{env.subject || m.mobile_no_subject()}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    {#if accountId && folder && query.trim().length >= 2 && !searching}
      <div class="list-group">
        <button class="list-row" onclick={runServer} disabled={searchingServer}>
          <Icon name={searchingServer ? 'loading' : 'sync'} size={20} />
          <span class="flex-1 list-row__title">
            {searchingServer ? m.mobile_searching_server() : m.mobile_search_on_server()}
          </span>
        </button>
      </div>
      <p class="list-footer pb-6">{m.mobile_search_server_hint({ folder })}</p>
    {/if}
  </div>
</div>
