<script lang="ts">
  /**
   * One contact card.
   *
   * Every row is an action, because that's what a contact is for on
   * a phone: tap an address to write to it, a number to call it, an
   * address to open it in Maps. Rows that aren't actionable (notes,
   * birthday) render as plain text.
   */
  import { formatLocale } from '../locale'
  import * as api from '../api'
  import Icon from '../Icon.svelte'
  import Avatar from '../Avatar.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Confirm from '../ui/Confirm.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { contactPhotoSrc, contactsStore, type Contact } from '../contactsStore.svelte'
  import { parseLocalDate } from '../format'
  import { m } from '../../paraglide/messages'

  interface Props {
    contact: Contact
    oncompose: (initial: { to: string }) => void
  }

  let { contact, oncompose }: Props = $props()

  let confirmDelete = $state(false)

  function callNumber(number: string) {
    // `tel:` leaves the app through the same path as any other URL,
    // and iOS shows its own confirmation before dialling.
    void api.system
      .openUrl({ url: `tel:${number.replace(/[^\d+]/g, '')}` })
      .catch(() => toasts.error(m.mobile_call_failed()))
  }

  function openMaps(address: string) {
    const query = encodeURIComponent(address)
    void api.system
      .openUrl({ url: `https://www.openstreetmap.org/search?query=${query}` })
      .catch((e) => toasts.error(m.mobile_open_failed(), e))
  }

  function formatAddress(a: {
    street: string
    locality: string
    region: string
    postal_code: string
    country: string
  }): string {
    return [a.street, [a.postal_code, a.locality].filter(Boolean).join(' '), a.region, a.country]
      .filter(Boolean)
      .join(', ')
  }

  async function remove() {
    confirmDelete = false
    try {
      await api.contacts.deleteContact({ contactId: contact.id })
      await contactsStore.load()
      toasts.show(m.mobile_contact_deleted())
      nav.pop()
    } catch (e) {
      toasts.error(m.mobile_contact_delete_failed(), e)
    }
  }

  const birthday = $derived(contact.birthday ? parseLocalDate(contact.birthday) : null)
</script>

<div class="screen">
  <NavBar title={m.mobile_contact()} backLabel={m.mobile_back()} onback={() => nav.pop()} />

  <div class="screen__body">
    <div class="flex flex-col items-center gap-2 px-4 pb-2 pt-6">
      <Avatar
        photo={contactPhotoSrc(contact)}
        displayName={contact.display_name}
        size={88}
      />
      <h1 class="mt-1 text-xl font-semibold selectable">{contact.display_name}</h1>
      {#if contact.title || contact.organization}
        <p class="text-sm ink-muted">
          {[contact.title, contact.organization].filter(Boolean).join(' · ')}
        </p>
      {/if}

      <div class="mt-3 flex gap-3">
        {#if contact.email[0]}
          <button
            type="button"
            class="quick-action"
            onclick={() => oncompose({ to: contact.email[0].value })}
          >
            <Icon name="compose" size={20} />
            <span>{m.mobile_write()}</span>
          </button>
        {/if}
        {#if contact.phone[0]}
          <button type="button" class="quick-action" onclick={() => callNumber(contact.phone[0].value)}>
            <Icon name="meetings" size={20} />
            <span>{m.mobile_call()}</span>
          </button>
        {/if}
      </div>
    </div>

    {#if contact.email.length > 0}
      <div class="list-header">{m.mobile_email()}</div>
      <div class="list-group">
        {#each contact.email as entry (entry.value)}
          <button class="list-row" onclick={() => oncompose({ to: entry.value })}>
            <span class="min-w-0 flex-1">
              <span class="block list-row__detail">{entry.kind || m.mobile_email()}</span>
              <span class="block truncate ink-accent">{entry.value}</span>
            </span>
            <Icon name="compose" size={16} />
          </button>
        {/each}
      </div>
    {/if}

    {#if contact.phone.length > 0}
      <div class="list-header">{m.mobile_phone()}</div>
      <div class="list-group">
        {#each contact.phone as entry (entry.value)}
          <button class="list-row" onclick={() => callNumber(entry.value)}>
            <span class="min-w-0 flex-1">
              <span class="block list-row__detail">{entry.kind || m.mobile_phone()}</span>
              <span class="block truncate ink-accent">{entry.value}</span>
            </span>
            <Icon name="nav-forward" size={16} />
          </button>
        {/each}
      </div>
    {/if}

    {#if (contact.addresses ?? []).length > 0}
      <div class="list-header">{m.mobile_address()}</div>
      <div class="list-group">
        {#each contact.addresses ?? [] as address, index (index)}
          {@const text = formatAddress(address)}
          <button class="list-row" onclick={() => openMaps(text)}>
            <span class="min-w-0 flex-1">
              <span class="block list-row__detail">{address.kind || m.mobile_address()}</span>
              <span class="block ink-accent">{text}</span>
            </span>
            <Icon name="location" size={16} />
          </button>
        {/each}
      </div>
    {/if}

    {#if birthday || contact.note || (contact.urls ?? []).length > 0}
      <div class="list-header">{m.mobile_details()}</div>
      <div class="list-group">
        {#if birthday}
          <div class="list-row">
            <Icon name="today" size={18} />
            <span class="flex-1">{birthday.toLocaleDateString(formatLocale())}</span>
          </div>
        {/if}
        {#each contact.urls ?? [] as url (url)}
          <button
            class="list-row"
            onclick={() => api.system.openUrl({ url }).catch(() => {})}
          >
            <Icon name="open-in-browser" size={18} />
            <span class="flex-1 truncate ink-accent">{url}</span>
          </button>
        {/each}
        {#if contact.note}
          <p class="whitespace-pre-wrap px-4 py-3 text-sm selectable">{contact.note}</p>
        {/if}
      </div>
    {/if}

    {#if (contact.categories ?? []).length > 0}
      <div class="flex flex-wrap gap-1.5 px-4 py-2">
        {#each contact.categories ?? [] as category (category)}
          <span class="category-chip">{category}</span>
        {/each}
      </div>
    {/if}

    <div class="px-3 pb-10 pt-4">
      <button
        type="button"
        class="wide-button wide-button--destructive"
        onclick={() => (confirmDelete = true)}
      >
        {m.mobile_delete_contact()}
      </button>
    </div>
  </div>
</div>

{#if confirmDelete}
  <Confirm
    title={m.mobile_delete_contact()}
    body={m.mobile_delete_contact_body({ name: contact.display_name })}
    confirmLabel={m.mobile_delete()}
    onconfirm={remove}
    oncancel={() => (confirmDelete = false)}
  />
{/if}

<style>
  .quick-action {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    min-width: 76px;
    padding: 8px 12px;
    border-radius: var(--r-md);
    font-size: var(--t-caption);
    /* Label in the primary ink: accent-coloured text on its own accent
       tint lost too much contrast in the pale themes. The icon keeps
       the accent. */
    color: var(--ui-ink);
    background: color-mix(in oklab, var(--color-primary-500) 12%, transparent);
  }

  .quick-action :global(svg) {
    color: var(--ui-accent-ink);
  }

  .quick-action:active {
    opacity: 0.7;
  }

  .category-chip {
    padding: 3px 9px;
    border-radius: var(--r-full);
    font-size: var(--t-caption);
    background: color-mix(in oklab, var(--color-surface-500) 16%, transparent);
  }
</style>
