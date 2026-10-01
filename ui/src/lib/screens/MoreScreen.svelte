<script lang="ts">
  /**
   * The More tab: a hub, not a leftover list.
   *
   * Three regions, top to bottom, in the order people come here for them:
   *
   *   1. **Who you are** — a card with the account (or the number of
   *      accounts). It is how a customer checks "which address am I
   *      using?" and the way into managing accounts.
   *   2. **Features** — every feature that is not on the tab bar, as a
   *      grid of tiles. A grid shows all of them at once without
   *      scrolling, and tiles read as *places*, which is what they are.
   *      Which features land here is the user's choice (Settings →
   *      Features), so the grid is derived rather than written out: any
   *      feature that isn't hidden and isn't on the tab bar appears.
   *      That is what makes the tab bar safe to rearrange — nothing can
   *      be taken off it and lost.
   *   3. **Outbox and Settings.** Settings is never removable: it is the
   *      way back from any arrangement.
   *
   * Search used to be listed here as well. It lives on the inbox, as a
   * visible field, and one place is easier to learn than two.
   */
  import Icon from '../Icon.svelte'
  import Avatar from '../Avatar.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import { nav } from '../nav.svelte'
  import { features } from '../features.svelte'
  import { accountsStore } from '../accountsStore.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    outboxCount: number
    ncConnected: boolean
  }

  let { outboxCount, ncConnected }: Props = $props()

  const accounts = $derived(accountsStore.list)
  const single = $derived(accounts.length === 1 ? accounts[0] : null)
  const inTestMode = $derived(accounts.some((a) => a.demo))
</script>

<div class="screen">
  <NavBar title={m.mobile_more()} large />

  <div class="screen__body">
    <!-- Who you are -->
    <button type="button" class="account-card" onclick={() => nav.push('settings-accounts')}>
      {#if single}
        <span aria-hidden="true" class="flex shrink-0">
          <Avatar
            displayName={single.display_name || single.email}
            size={52}
          />
        </span>
        <span class="account-card__text">
          <span class="account-card__name">{single.display_name || single.email}</span>
          <span class="account-card__detail">{single.email}</span>
        </span>
      {:else}
        <span class="icon-tile icon-tile--lg" aria-hidden="true">
          <Icon name="add-account" size={22} />
        </span>
        <span class="account-card__text">
          <span class="account-card__name">{m.mobile_more_manage_accounts()}</span>
          <span class="account-card__detail">
            {m.mobile_more_accounts_count({ count: accounts.length })}
          </span>
        </span>
      {/if}
      {#if inTestMode}
        <span class="account-card__badge">{m.mobile_testmode_title()}</span>
      {/if}
      <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
    </button>

    <!-- Features -->
    {#if features.inMore.length > 0}
      <h2 class="section-title">{m.mobile_settings_features()}</h2>
      <div class="tile-grid">
        {#each features.inMore as feature (feature.id)}
          <button type="button" class="tile" onclick={() => nav.push(feature.root)}>
            <span class="icon-tile icon-tile--lg" aria-hidden="true">
              <Icon name={feature.icon} size={22} />
            </span>
            <span>
              <span class="tile__title">{feature.label()}</span>
              {#if feature.needsNextcloud && !ncConnected}
                <span class="tile__detail">{m.mobile_not_connected()}</span>
              {/if}
            </span>
          </button>
        {/each}
      </div>
    {/if}

    <!-- Outbox and Settings -->
    <div class="list-group">
      <button class="list-row" onclick={() => nav.push('outbox')}>
        <span class="icon-tile" aria-hidden="true"><Icon name="sent" size={18} /></span>
        <span class="flex-1 list-row__title">{m.mobile_outbox()}</span>
        {#if outboxCount > 0}
          <span class="count-pill count-pill--accent">{outboxCount}</span>
        {/if}
        <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
      </button>
      <button class="list-row" onclick={() => nav.push('settings')}>
        <span class="icon-tile" aria-hidden="true"><Icon name="settings" size={18} /></span>
        <span class="flex-1 list-row__title">{m.mobile_settings()}</span>
        <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
      </button>
    </div>

    <div class="h-8"></div>
  </div>
</div>

<style>
  /* Who you are, first: the one card on this screen that is about a
     person rather than a place. Flat like every card (v3), on the
     gutter like the title above it. */
  .account-card {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    width: calc(100% - 2 * var(--gutter));
    margin: var(--s-2) var(--gutter) 0;
    padding: var(--s-4);
    border-radius: var(--r-lg);
    border: 1px solid var(--ui-card-border);
    background: var(--ui-card);
    text-align: left;
    transition: background-color var(--m-fast) var(--m-ease);
  }

  .account-card:active {
    background: color-mix(in oklab, var(--ui-card) 90%, var(--color-surface-500));
  }

  .account-card__text {
    display: block;
    flex: 1;
    min-width: 0;
  }

  .account-card__name {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-headline);
    line-height: var(--t-headline-lh);
    font-weight: 650;
    color: var(--ui-ink);
  }

  .account-card__detail {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-footnote);
    line-height: var(--t-footnote-lh);
    color: var(--ui-ink-muted);
  }

  /* Ink on a warning tint, not warning-coloured text: the tint carries
     the "this is not a real account" signal, the word carries it for
     everyone else. */
  .account-card__badge {
    flex-shrink: 0;
    padding: 2px var(--s-2);
    border-radius: var(--r-full);
    background: var(--ui-tint-warning);
    color: var(--ui-ink);
    font-size: var(--t-micro);
    line-height: var(--t-micro-lh);
    font-weight: 650;
  }
</style>
