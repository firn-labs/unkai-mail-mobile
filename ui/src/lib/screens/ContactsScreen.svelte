<script lang="ts">
  /**
   * The contact list: search at the top, A–Z sections below.
   *
   * Contacts come from CardDAV via the local cache, so the list is
   * complete offline. Search filters the cached list directly
   * (rather than round-tripping to `search_contacts`) because
   * filtering a few thousand rows in memory is instant and works
   * with no network at all.
   */
  import * as api from '../api'
  import type { NextcloudAccount } from '../api'
  import Icon from '../Icon.svelte'
  import Avatar from '../Avatar.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import SearchField from '../ui/SearchField.svelte'
  import PullToRefresh from '../ui/PullToRefresh.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { contactsStore, contactPhotoSrc, type Contact } from '../contactsStore.svelte'
  import { m } from '../../paraglide/messages'

  let query = $state('')
  let loading = $state(true)
  let syncing = $state(false)

  const contacts = $derived(contactsStore.list)

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase()
    if (!q) return contacts
    return contacts.filter(
      (c: Contact) =>
        c.display_name.toLowerCase().includes(q) ||
        (c.organization ?? '').toLowerCase().includes(q) ||
        c.email.some((e) => e.value.toLowerCase().includes(q)) ||
        c.phone.some((p) => p.value.toLowerCase().includes(q)),
    )
  })

  /** Group into A–Z buckets; everything without a letter lands in #. */
  const sections = $derived.by(() => {
    const map = new Map<string, Contact[]>()
    for (const contact of filtered) {
      const first = (contact.display_name || '?').trim()[0]?.toUpperCase() ?? '#'
      const key = /[A-ZÄÖÜ]/.test(first) ? first : '#'
      map.set(key, [...(map.get(key) ?? []), contact])
    }
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b))
  })

  async function load() {
    await contactsStore.load()
    loading = false
  }

  async function sync() {
    syncing = true
    try {
      const accounts: NextcloudAccount[] = await api.nextcloud.getNextcloudAccounts()
      for (const acc of accounts) {
        await api.contacts.syncNextcloudContacts({ ncId: acc.id })
      }
      await contactsStore.load()
      toasts.success(m.mobile_contacts_synced())
    } catch (e) {
      toasts.error(m.mobile_contacts_sync_failed(), e)
    } finally {
      syncing = false
    }
  }

  $effect(() => {
    void load()
  })
</script>

<!-- A plain screen (v3): an address book is one long list of people,
     like the inbox, not a stack of cards. v2 put every letter in a card
     of its own, so a letter with one person in it cost a header, two
     card margins and a card — four rows of screen for one name. -->
<div class="screen screen--plain">
  <!-- The list is pulled to refresh, like Mail and the calendar agenda.
       A sync button appears only while there is no list to pull — the
       empty state — so there is always exactly one way to ask. -->
  <NavBar
    title={m.mobile_contacts()}
    backLabel={nav.canGoBack ? m.mobile_more() : null}
    onback={() => nav.pop()}
    busy={syncing}
    large
  >
    {#snippet actions()}
      {#if !loading && contacts.length === 0}
        <button
          type="button"
          class="bar-button"
          aria-label={m.mobile_sync()}
          onclick={sync}
          disabled={syncing}
        >
          <Icon name={syncing ? 'loading' : 'sync'} size={20} />
        </button>
      {/if}
    {/snippet}
    {#snippet below()}
      <SearchField bind:value={query} placeholder={m.mobile_contacts_search_placeholder()} />
    {/snippet}
  </NavBar>

  {#if loading}
    <Spinner />
  {:else if contacts.length === 0}
    <EmptyState
      icon="contacts"
      title={m.mobile_no_contacts_title()}
      body={m.mobile_no_contacts_body()}
      action={{
        label: m.mobile_connect_nextcloud(),
        run: () => nav.go('more', 'settings-nextcloud'),
      }}
    />
  {:else if filtered.length === 0}
    <EmptyState
      icon="search"
      title={m.mobile_contacts_no_match_title()}
      body={m.mobile_contacts_no_match_body({ query })}
    />
  {:else}
    <PullToRefresh onrefresh={sync}>
      {#each sections as [letter, entries] (letter)}
        <h2 class="contacts__letter">{letter}</h2>
        <ul class="contacts__group">
          {#each entries as contact (contact.id)}
            <li class="list-lazy">
              <button class="list-row contacts__row" onclick={() => nav.push('contact-detail', { contact })}>
                <Avatar
                  photo={contactPhotoSrc(contact)}
                  displayName={contact.display_name}
                  size={40}
                />
                <span class="min-w-0 flex-1">
                  <span class="block truncate list-row__title">{contact.display_name}</span>
                  {#if contact.organization}
                    <span class="block truncate list-row__detail">{contact.organization}</span>
                  {:else if contact.email[0]}
                    <span class="block truncate list-row__detail">{contact.email[0].value}</span>
                  {/if}
                </span>
                <!-- No chevron: every row opens a contact, so an arrow
                     on each says the same thing a hundred times (the
                     inbox made the same call). -->
              </button>
            </li>
          {/each}
        </ul>
      {/each}
      <div class="h-20"></div>
    </PullToRefresh>
  {/if}
</div>

<style>
  /* The letter is a quiet marker on the gutter, in the display face —
     an index tab, not a section banner. It stays pinned while its
     people scroll under it, so a long letter keeps its bearings. */
  .contacts__letter {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: var(--s-3) var(--gutter) var(--s-1);
    background: var(--ui-canvas);
    font-family: var(--font-display);
    font-size: var(--t-footnote);
    line-height: var(--t-footnote-lh);
    font-weight: 750;
    color: var(--ui-ink-muted);
  }

  /* A row brought into view by keyboard focus stops below the pinned
     letter rather than under it. */
  .contacts__group li {
    scroll-margin-top: calc(var(--t-footnote-lh) + var(--s-4));
  }

  /* Rows on the gutter, their hairlines starting where the name does
     (gutter + avatar + gap), as in the inbox. */
  .contacts__row {
    padding-inline: var(--gutter);
  }

  .contacts__group li + li .contacts__row::before {
    content: '';
    position: absolute;
    top: 0;
    left: calc(var(--gutter) + 40px + var(--s-3));
    right: 0;
    height: 1px;
    background: var(--ui-hairline);
  }
</style>
