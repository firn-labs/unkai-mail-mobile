<script lang="ts">
  /** Settings hub. Categories mirror the desktop's, minus the ones
   *  a phone has no equivalent for (window behaviour, autostart,
   *  the local MCP server, hardware security keys).
   *
   *  In test mode it opens with a card that says so and offers the way
   *  out — the one place the introduction points to. Leaving deletes
   *  the sample data, so it asks first (`Confirm` is for exactly the
   *  irreversible). */
  import Icon, { type IconName } from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Confirm from '../ui/Confirm.svelte'
  import { nav, type ScreenKind } from '../nav.svelte'
  import { accountsStore } from '../accountsStore.svelte'
  import { LANGUAGE_NAMES } from '../locale'
  import { getLocale } from '../../paraglide/runtime'
  import { m } from '../../paraglide/messages'

  interface Props {
    /** Leave test mode: delete the demo, then route (the shell does). */
    onleavedemo?: () => Promise<void>
    /** Replay the introduction. */
    onshowintro?: () => void
  }

  let { onleavedemo, onshowintro }: Props = $props()

  const inTestMode = $derived(accountsStore.list.some((a) => a.demo))
  let confirmLeave = $state(false)
  let leaving = $state(false)

  async function leave() {
    confirmLeave = false
    if (!onleavedemo) return
    leaving = true
    try {
      await onleavedemo()
    } finally {
      leaving = false
    }
  }

  type Entry = {
    kind: ScreenKind
    icon: IconName
    label: string
    detail?: string
  }

  /**
   * Four groups, each with a heading, in the order a customer looks for
   * things: *what is connected*, *how the app behaves*, *what keeps it
   * safe*, *help*. Every row leads with its glyph in muted ink (v3): the
   * group headings carry the scanning, and a colour per category — v2's
   * tiles — put five saturated colours on a screen whose rule is one.
   */
  const groups: { title: string; entries: Entry[] }[] = $derived([
    {
      title: m.mobile_settings_group_accounts(),
      entries: [
        {
          kind: 'settings-accounts',
          icon: 'email-envelope',
          label: m.mobile_settings_accounts(),
          detail: String(accountsStore.list.length),
        },
        { kind: 'settings-nextcloud', icon: 'cloud', label: m.mobile_settings_nextcloud() },
        { kind: 'settings-sources', icon: 'calendar', label: m.mobile_settings_sources() },
      ],
    },
    {
      title: m.mobile_settings_group_app(),
      entries: [
        { kind: 'settings-mail', icon: 'compose', label: m.mobile_settings_mail() },
        { kind: 'settings-features', icon: 'more', label: m.mobile_settings_features() },
        { kind: 'settings-notifications', icon: 'notification', label: m.mobile_settings_notifications() },
        { kind: 'settings-appearance', icon: 'design-palette', label: m.mobile_settings_appearance() },
        {
          kind: 'settings-language',
          icon: 'translate',
          label: m.mobile_settings_language(),
          detail: LANGUAGE_NAMES[getLocale()],
        },
      ],
    },
    {
      title: m.mobile_settings_group_security(),
      entries: [
        { kind: 'settings-security', icon: 'lock', label: m.mobile_settings_security() },
        { kind: 'settings-encryption', icon: 'encrypted', label: m.mobile_settings_encryption() },
        { kind: 'settings-privacy', icon: 'eye-off', label: m.mobile_settings_privacy() },
      ],
    },
  ])
</script>

<div class="screen">
  <NavBar title={m.mobile_settings()} backLabel={m.mobile_more()} onback={() => nav.pop()} large />
  <div class="screen__body">
    {#if inTestMode}
      <section class="testmode" aria-labelledby="testmode-title">
        <div class="testmode__head">
          <span class="testmode__icon icon-tile icon-tile--warning" aria-hidden="true"><Icon name="eye" size={18} /></span>
          <h2 id="testmode-title" class="testmode__title">{m.mobile_testmode_title()}</h2>
        </div>
        <p class="testmode__body">{m.mobile_testmode_body()}</p>
        <button
          type="button"
          class="wide-button"
          disabled={leaving || !onleavedemo}
          onclick={() => (confirmLeave = true)}
        >
          {leaving ? m.mobile_working() : m.mobile_testmode_leave()}
        </button>
      </section>
    {/if}

    {#each groups as group (group.title)}
      <h2 class="list-header">{group.title}</h2>
      <div class="list-group">
        {#each group.entries as entry (entry.kind)}
          <button class="list-row" onclick={() => nav.push(entry.kind)}>
            <span class="icon-tile" aria-hidden="true">
              <Icon name={entry.icon} size={18} />
            </span>
            <span class="flex-1 list-row__title">{entry.label}</span>
            {#if entry.detail}<span class="list-row__detail">{entry.detail}</span>{/if}
            <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
          </button>
        {/each}
      </div>
    {/each}

    <h2 class="list-header">{m.mobile_settings_group_help()}</h2>
    <div class="list-group">
      {#if onshowintro}
        <button class="list-row" onclick={() => onshowintro?.()}>
          <span class="icon-tile" aria-hidden="true"><Icon name="help" size={18} /></span>
          <span class="flex-1 list-row__title">{m.mobile_settings_intro()}</span>
          <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
        </button>
      {/if}
      <button class="list-row" onclick={() => nav.push('settings-about')}>
        <span class="icon-tile" aria-hidden="true"><Icon name="info" size={18} /></span>
        <span class="flex-1 list-row__title">{m.mobile_settings_about()}</span>
        <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
      </button>
    </div>
    <div class="h-8"></div>
  </div>
</div>

{#if confirmLeave}
  <Confirm
    title={m.mobile_testmode_leave_confirm_title()}
    body={m.mobile_testmode_leave_confirm_body()}
    confirmLabel={m.mobile_testmode_leave()}
    onconfirm={leave}
    oncancel={() => (confirmLeave = false)}
  />
{/if}

<style>
  /* A state the app is in, not one more setting: a card of its own above
     the groups, its glyph in the warning ink, its title in the display
     face. The text stays ink — coloured text on a tint is the pairing
     the contrast solver cannot vouch for. */
  .testmode {
    margin: var(--s-2) var(--gutter) 0;
    padding: var(--s-4);
    border-radius: var(--r-lg);
    border: 1px solid var(--ui-card-border);
    background: var(--ui-card);
  }

  .testmode__head {
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }

  .testmode__icon {
    display: inline-flex;
    width: 24px;
    height: 24px;
  }

  .testmode__title {
    font-family: var(--font-display);
    font-size: var(--t-headline);
    line-height: var(--t-headline-lh);
    font-weight: 750;
    color: var(--ui-ink);
  }

  .testmode__body {
    margin: var(--s-2) 0 var(--s-4);
    font-size: var(--t-callout);
    line-height: var(--t-callout-lh);
    color: var(--ui-ink-muted);
  }
</style>
