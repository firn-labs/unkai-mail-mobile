<script lang="ts">
  /**
   * Where calendars, contacts and tasks come from.
   *
   * The backend has always supported four kinds of source
   * (`DavSourceKind`) but the mobile UI only ever offered one, so a
   * Nextcloud was effectively required to have a calendar at all.
   * This screen exposes all four:
   *
   *   * **Nextcloud** — the full connection (Talk, Files, Notes on
   *     top of DAV). Its own screen owns the login flow and the
   *     settings backup, so this one links there rather than
   *     duplicating it.
   *   * **CalDAV / CardDAV** — any standards-compliant server.
   *   * **This device** — iOS's own Calendar, Reminders and Contacts
   *     (EventKit + Contacts). Which means iCloud, Google and
   *     Exchange too: whatever the user already set up in iOS
   *     Settings shows up here, because the system databases are
   *     where those accounts deliver.
   *   * **On this phone only** — a store with no remote at all.
   *
   * They coexist. The calendar and contacts views iterate over every
   * source, so adding the device's calendars next to a Nextcloud
   * needs no further wiring.
   *
   * "Connect a cloud" is a provider picker on top of those four kinds
   * (`lib/cloudProviders.ts` says which provider takes which route and
   * why): iCloud, Google, Microsoft and Yahoo go through the device
   * source, the DAV providers get a form with their server filled in.
   * No new source kind and no new backend path — a preset is the generic
   * DAV form with the address already known.
   */
  import * as api from '../api'
  import type { NextcloudAccount, SystemAccessView } from '../api'
  import Icon, { type IconName } from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import Confirm from '../ui/Confirm.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import Toggle from '../Toggle.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import {
    CLOUD_PROVIDERS,
    planDavSources,
    providerLabel,
    type CloudProvider,
  } from '../cloudProviders'
  import { m } from '../../paraglide/messages'

  let accounts = $state<NextcloudAccount[]>([])
  let access = $state<SystemAccessView | null>(null)
  let loading = $state(true)
  let busy = $state(false)
  let picking = $state(false)
  let confirmRemove = $state<NextcloudAccount | null>(null)

  /** Which add-form is open, if any. */
  let form = $state<'dav' | 'system' | 'local' | null>(null)

  /** The provider the DAV form or the device guide is for. */
  let provider = $state<CloudProvider | null>(null)
  /** The "through your iPhone" guide for this provider, when open. */
  let deviceGuide = $state<CloudProvider | null>(null)

  /* The DAV form. */
  let davName = $state('')
  let davUrl = $state('')
  let davUser = $state('')
  let davPassword = $state('')

  function hostOf(url: string): string {
    return url.replace(/^https?:\/\//, '')
  }

  /* Shared between the DAV, system and local forms. */
  let useContacts = $state(true)
  let useCalendars = $state(true)
  let useReminders = $state(true)
  let localName = $state('')

  /** The calls the DAV form would make right now — also what enables
   *  its button, so the button and the submit can never disagree. */
  const davPlan = $derived(
    provider
      ? planDavSources(provider, {
          server: davUrl,
          username: davUser,
          password: davPassword,
          useCalendars,
          useContacts,
          name: davName,
        })
      : [],
  )

  /** A preset with fixed servers shows them rather than asking. */
  const fixedServers = $derived(
    provider && provider.calendarServer && provider.contactsServer
      ? provider.calendarServer === provider.contactsServer
        ? [{ label: m.mobile_cloud_server(), host: hostOf(provider.calendarServer) }]
        : [
            { label: m.mobile_tab_calendar(), host: hostOf(provider.calendarServer) },
            { label: m.mobile_tab_contacts(), host: hostOf(provider.contactsServer) },
          ]
      : [],
  )

  const hasSystemSource = $derived(accounts.some((a) => a.kind === 'system'))

  async function load() {
    try {
      accounts = await api.nextcloud.getNextcloudAccounts()
    } catch (e) {
      toasts.error(m.mobile_sources_load_failed(), e)
    }
    try {
      access = await api.nextcloud.systemAccessStatus()
    } catch {
      // A platform without the bridge answers nothing; the device
      // option is then simply not offered.
      access = null
    }
    loading = false
  }

  $effect(() => {
    void load()
  })

  function kindLabel(account: NextcloudAccount): string {
    if (account.kind === 'system') return m.mobile_source_kind_system()
    if (account.kind === 'local') return m.mobile_source_kind_local()
    if (account.kind === 'dav') return account.server_url || m.mobile_source_kind_dav()
    return account.server_url || m.mobile_source_kind_nextcloud()
  }

  function kindIcon(account: NextcloudAccount): IconName {
    if (account.kind === 'system') return 'settings'
    if (account.kind === 'local') return 'home'
    return 'cloud'
  }

  /** What each source actually provides, as a short line. */
  function capabilityLine(account: NextcloudAccount): string {
    const caps = account.capabilities ?? {}
    const parts: string[] = []
    if (caps.caldav) parts.push(m.mobile_tab_calendar())
    if (caps.carddav) parts.push(m.mobile_tab_contacts())
    if (caps.tasks) parts.push(m.mobile_tab_tasks())
    return parts.join(' · ')
  }

  const deviceProviders = CLOUD_PROVIDERS.filter((p) => p.route === 'device')
  const directProviders = CLOUD_PROVIDERS.filter(
    (p) => p.route === 'dav' || p.route === 'nextcloud',
  )
  const offlineProviders = CLOUD_PROVIDERS.filter((p) => p.route === 'local')

  async function openIosSettings() {
    try {
      await api.system.openAppSettings()
    } catch (e) {
      toasts.error(m.mobile_privacy_open_settings(), e)
    }
  }

  function choose(chosen: CloudProvider) {
    picking = false
    if (chosen.route === 'nextcloud') {
      nav.push('settings-nextcloud')
    } else if (chosen.route === 'device') {
      deviceGuide = chosen
    } else if (chosen.route === 'local') {
      openForm('local')
    } else {
      openForm('dav', chosen)
    }
  }

  function openForm(which: 'dav' | 'system' | 'local', preset: CloudProvider | null = null) {
    provider = preset
    davName = ''
    davUrl = ''
    davUser = ''
    davPassword = ''
    localName = ''
    useContacts = true
    useCalendars = true
    useReminders = true
    form = which
  }

  /**
   * Connect a DAV provider: one source, or two when the provider serves
   * calendars and contacts from different hosts (`planDavSources`).
   *
   * The calls run one after the other, not together, so a wrong
   * password produces one clear error rather than two racing ones; and a
   * half that succeeded is kept and said so, rather than silently rolled
   * back or silently reported as a full success.
   */
  async function addDav() {
    const target = provider
    const plans = davPlan
    if (!target || plans.length === 0) return
    busy = true
    let connected = 0
    try {
      for (const plan of plans) {
        try {
          await api.nextcloud.addDavAccount({
            displayName: plan.displayName,
            serverUrl: plan.serverUrl,
            username: plan.username,
            password: plan.password,
            useContacts: plan.useContacts,
            useCalendars: plan.useCalendars,
          })
          connected++
        } catch (e) {
          if (plans.length > 1 && plan.covers !== 'both') {
            const service =
              plan.covers === 'calendars' ? m.mobile_tab_calendar() : m.mobile_tab_contacts()
            toasts.error(m.mobile_cloud_partial_failed({ service }), e)
          } else {
            toasts.error(m.mobile_source_add_failed(), e)
          }
        }
      }
      if (connected > 0) {
        form = null
        await load()
        toasts.success(m.mobile_cloud_connected({ provider: providerLabel(target) }))
      }
    } finally {
      busy = false
    }
  }

  async function addSystem() {
    busy = true
    try {
      // The backend prompts for each permission before it stores the
      // source, so this call can sit for a while — that's the iOS
      // dialog, not a hang.
      await api.nextcloud.addSystemDavAccount({
        displayName: m.mobile_source_kind_system(),
        useContacts,
        useCalendars,
        useReminders,
      })
      form = null
      await load()
      toasts.success(m.mobile_source_added())
    } catch (e) {
      toasts.error(m.mobile_source_add_failed(), e)
    } finally {
      busy = false
    }
  }

  async function addLocal() {
    busy = true
    try {
      await api.nextcloud.addLocalDavAccount({
        displayName: localName.trim() || m.mobile_source_kind_local(),
        useContacts,
        useCalendars,
      })
      form = null
      await load()
      toasts.success(m.mobile_source_added())
    } catch (e) {
      toasts.error(m.mobile_source_add_failed(), e)
    } finally {
      busy = false
    }
  }

  async function remove(account: NextcloudAccount) {
    confirmRemove = null
    try {
      await api.nextcloud.removeNextcloudAccount({ id: account.id })
      await load()
      toasts.show(m.mobile_source_removed())
    } catch (e) {
      toasts.error(m.mobile_source_remove_failed(), e)
    }
  }

  /** Ask iOS for one database the user declined (or never answered)
   *  at add time, so a half-granted device source can be completed
   *  without removing and re-adding it. */
  async function requestAccess(entity: 'calendars' | 'reminders' | 'contacts') {
    try {
      access = await api.nextcloud.requestSystemAccess({ entity })
      if (access[entity] !== 'granted') toasts.show(m.mobile_source_access_denied())
    } catch (e) {
      toasts.error(m.mobile_source_access_failed(), e)
    }
  }

  const systemRows = $derived<{ entity: 'calendars' | 'reminders' | 'contacts'; label: string }[]>([
    { entity: 'calendars', label: m.mobile_tab_calendar() },
    { entity: 'reminders', label: m.mobile_source_reminders() },
    { entity: 'contacts', label: m.mobile_tab_contacts() },
  ])
</script>

<div class="screen">
  <NavBar
    title={m.mobile_settings_sources()}
    backLabel={m.mobile_settings()}
    onback={() => nav.pop()}
    large
  >
    {#snippet actions()}
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_source_add()}
        onclick={() => (picking = true)}
      >
        <Icon name="plus" size={20} />
      </button>
    {/snippet}
  </NavBar>

  <div class="screen__body">
    {#if loading}
      <Spinner />
    {:else}
      {#if accounts.length === 0}
        <EmptyState
          icon="cloud"
          title={m.mobile_sources_empty_title()}
          body={m.mobile_sources_empty_body()}
        />
        <div class="px-3 pt-2">
          <button type="button" class="wide-button" onclick={() => (picking = true)}>
            {m.mobile_source_add()}
          </button>
        </div>
      {:else}
        <div class="list-group">
          {#each accounts as account (account.id)}
            <div class="list-row">
              <Icon name={kindIcon(account)} size={20} />
              <span class="min-w-0 flex-1">
                <span class="block truncate list-row__title">
                  {account.display_name || account.username || kindLabel(account)}
                </span>
                <span class="block truncate list-row__detail">
                  {kindLabel(account)}{capabilityLine(account) ? ` — ${capabilityLine(account)}` : ''}
                </span>
              </span>
              <button
                type="button"
                class="bar-button bar-button--destructive"
                aria-label={m.mobile_remove()}
                onclick={() => (confirmRemove = account)}
              >
                <Icon name="trash" size={18} />
              </button>
            </div>
          {/each}
        </div>
      {/if}

      {#if hasSystemSource && access?.available}
        <div class="list-header">{m.mobile_source_system_access()}</div>
        <div class="list-group">
          {#each systemRows as row (row.entity)}
            {@const granted = access[row.entity] === 'granted'}
            <div class="list-row">
              <Icon name={granted ? 'success' : 'warning'} size={20} />
              <span class="flex-1">
                <span class="block list-row__title">{row.label}</span>
                <span class="block list-row__detail">
                  {granted ? m.mobile_permission_granted() : m.mobile_permission_missing()}
                </span>
              </span>
              {#if !granted}
                <button
                  type="button"
                  class="value-button"
                  onclick={() => requestAccess(row.entity)}
                >
                  {m.mobile_allow()}
                </button>
              {/if}
            </div>
          {/each}
        </div>
        <p class="list-footer">{m.mobile_source_system_access_hint()}</p>
      {/if}

      <div class="h-16"></div>
    {/if}
  </div>
</div>

{#snippet providerRow(p: CloudProvider)}
  <button type="button" class="list-row" onclick={() => choose(p)}>
    <span class="provider-tile" style="--tile-hue: {p.hue}" aria-hidden="true">{p.monogram}</span>
    <span class="min-w-0 flex-1">
      <span class="block truncate list-row__title">{providerLabel(p)}</span>
      <span class="block list-row__detail">{p.detail()}</span>
    </span>
    <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
  </button>
{/snippet}

{#if picking}
  <Sheet kind="full" label={m.mobile_cloud_title()} onclose={() => (picking = false)}>
    <div class="screen">
      <NavBar title={m.mobile_cloud_title()}>
        {#snippet leading()}
          <button
            type="button"
            class="bar-button bar-button--text"
            onclick={() => (picking = false)}
          >
            {m.mobile_cancel()}
          </button>
        {/snippet}
      </NavBar>
      <div class="screen__body">
        <div class="list-header">{m.mobile_cloud_group_device()}</div>
        <div class="list-group">
          {#each deviceProviders as p (p.id)}
            {@render providerRow(p)}
          {/each}
        </div>
        <p class="list-footer">{m.mobile_cloud_group_device_footer()}</p>

        <div class="list-header">{m.mobile_cloud_group_direct()}</div>
        <div class="list-group">
          {#each directProviders as p (p.id)}
            {@render providerRow(p)}
          {/each}
        </div>
        <p class="list-footer">{m.mobile_cloud_group_direct_footer()}</p>

        <div class="list-header">{m.mobile_cloud_group_offline()}</div>
        <div class="list-group">
          {#each offlineProviders as p (p.id)}
            {@render providerRow(p)}
          {/each}
        </div>
        <div class="h-16"></div>
      </div>
    </div>
  </Sheet>
{/if}

{#if deviceGuide}
  {@const guide = deviceGuide}
  <Sheet
    label={m.mobile_cloud_device_title({ provider: guide.name })}
    onclose={() => (deviceGuide = null)}
  >
    <div class="px-4 pb-6 pt-1">
      <h2 class="text-base font-semibold">{m.mobile_cloud_device_title({ provider: guide.name })}</h2>
      <ol class="device-steps">
        <li>{m.mobile_cloud_device_step1({ provider: guide.name })}</li>
        <li>{m.mobile_cloud_device_step2()}</li>
        <li>{m.mobile_cloud_device_step3()}</li>
      </ol>

      {#if !access?.available}
        <p class="mt-3 text-sm ink-muted">{m.mobile_cloud_device_unavailable()}</p>
      {:else if hasSystemSource}
        <p class="mt-3 flex items-start gap-2 text-sm">
          <span class="ink-success shrink-0"><Icon name="success" size={18} /></span>
          <span>{m.mobile_cloud_device_connected()}</span>
        </p>
      {:else}
        <button
          type="button"
          class="wide-button mt-4"
          onclick={() => {
            deviceGuide = null
            openForm('system')
          }}
        >
          {m.mobile_cloud_device_connect()}
        </button>
      {/if}
      <button
        type="button"
        class="wide-button wide-button--quiet mt-2"
        onclick={() => void openIosSettings()}
      >
        {m.mobile_privacy_open_settings()}
      </button>
    </div>
  </Sheet>
{/if}

{#if form === 'dav' && provider}
  {@const preset = provider}
  <Sheet
    label={m.mobile_cloud_dav_title({ provider: providerLabel(preset) })}
    onclose={() => (form = null)}
  >
    <div class="form-group px-4 pb-6 pt-1">
      <h2 class="text-base font-semibold">
        {m.mobile_cloud_dav_title({ provider: providerLabel(preset) })}
      </h2>
      {#if fixedServers.length > 0}
        <dl class="fixed-servers">
          {#each fixedServers as server (server.label)}
            <div>
              <dt>{server.label}</dt>
              <dd class="selectable">{server.host}</dd>
            </div>
          {/each}
        </dl>
      {:else}
        <p class="mb-3 mt-1 text-sm ink-muted">{m.mobile_source_dav_hint()}</p>
        <input
          type="url"
          inputmode="url"
          autocapitalize="none"
          autocorrect="off"
          spellcheck="false"
          bind:value={davUrl}
          aria-label={m.mobile_sr_server_address()}
          placeholder="dav.example.com"
          disabled={busy}
        />
      {/if}
      <input
        type="text"
        inputmode={preset.calendarServer ? 'email' : 'text'}
        autocomplete="username"
        autocapitalize="none"
        autocorrect="off"
        spellcheck="false"
        class="mt-2"
        bind:value={davUser}
        aria-label={preset.usernameHint?.() ?? m.mobile_username()}
        placeholder={preset.usernameHint?.() ?? m.mobile_username()}
        disabled={busy}
      />
      <input
        type="password"
        autocomplete="current-password"
        class="mt-2"
        bind:value={davPassword}
        aria-label={m.mobile_password()}
        placeholder={m.mobile_password()}
        disabled={busy}
      />
      {#if preset.passwordHint}
        <p class="mt-1 text-xs ink-muted">{preset.passwordHint()}</p>
      {/if}
      <input
        type="text"
        class="mt-2"
        bind:value={davName}
        aria-label={m.mobile_source_name_optional()}
        placeholder={m.mobile_source_name_optional()}
        disabled={busy}
      />

      <div class="list-group mt-4">
        <div class="list-row">
          <span class="flex-1 list-row__title">{m.mobile_tab_calendar()}</span>
          <Toggle
            checked={useCalendars}
            label={m.mobile_tab_calendar()}
            onchange={(v) => (useCalendars = v)}
          />
        </div>
        <div class="list-row">
          <span class="flex-1 list-row__title">{m.mobile_tab_contacts()}</span>
          <Toggle
            checked={useContacts}
            label={m.mobile_tab_contacts()}
            onchange={(v) => (useContacts = v)}
          />
        </div>
      </div>

      <button
        type="button"
        class="wide-button mt-4"
        disabled={busy || davPlan.length === 0}
        onclick={addDav}
      >
        {busy ? m.mobile_working() : m.mobile_connect()}
      </button>
    </div>
  </Sheet>
{/if}

{#if form === 'system'}
  <Sheet label={m.mobile_source_add_system()} onclose={() => (form = null)}>
    <div class="form-group px-4 pb-6 pt-1">
      <h2 class="text-base font-semibold">{m.mobile_source_add_system()}</h2>
      <p class="mb-3 mt-1 text-sm ink-muted">{m.mobile_source_system_hint()}</p>

      <div class="list-group">
        <div class="list-row">
          <span class="flex-1 list-row__title">{m.mobile_tab_calendar()}</span>
          <Toggle
            checked={useCalendars}
            label={m.mobile_tab_calendar()}
            onchange={(v) => (useCalendars = v)}
          />
        </div>
        <div class="list-row">
          <span class="flex-1 list-row__title">{m.mobile_source_reminders()}</span>
          <Toggle
            checked={useReminders}
            label={m.mobile_source_reminders()}
            onchange={(v) => (useReminders = v)}
          />
        </div>
        <div class="list-row">
          <span class="flex-1 list-row__title">{m.mobile_tab_contacts()}</span>
          <Toggle
            checked={useContacts}
            label={m.mobile_tab_contacts()}
            onchange={(v) => (useContacts = v)}
          />
        </div>
      </div>

      <button
        type="button"
        class="wide-button mt-4"
        disabled={busy || (!useCalendars && !useContacts && !useReminders)}
        onclick={addSystem}
      >
        {busy ? m.mobile_source_asking() : m.mobile_connect()}
      </button>
      <p class="mt-3 text-center text-xs ink-muted">
        {m.mobile_source_system_permission_note()}
      </p>
    </div>
  </Sheet>
{/if}

{#if form === 'local'}
  <Sheet label={m.mobile_source_add_local()} onclose={() => (form = null)}>
    <div class="form-group px-4 pb-6 pt-1">
      <h2 class="text-base font-semibold">{m.mobile_source_add_local()}</h2>
      <p class="mb-3 mt-1 text-sm ink-muted">{m.mobile_source_local_hint()}</p>
      <input
        type="text"
        bind:value={localName}
        aria-label={m.mobile_source_name_optional()}
        placeholder={m.mobile_source_name_optional()}
        disabled={busy}
      />

      <div class="list-group mt-4">
        <div class="list-row">
          <span class="flex-1 list-row__title">{m.mobile_tab_calendar()}</span>
          <Toggle
            checked={useCalendars}
            label={m.mobile_tab_calendar()}
            onchange={(v) => (useCalendars = v)}
          />
        </div>
        <div class="list-row">
          <span class="flex-1 list-row__title">{m.mobile_tab_contacts()}</span>
          <Toggle
            checked={useContacts}
            label={m.mobile_tab_contacts()}
            onchange={(v) => (useContacts = v)}
          />
        </div>
      </div>

      <button
        type="button"
        class="wide-button mt-4"
        disabled={busy || (!useCalendars && !useContacts)}
        onclick={addLocal}
      >
        {busy ? m.mobile_working() : m.mobile_create()}
      </button>
    </div>
  </Sheet>
{/if}

{#if confirmRemove}
  {@const account = confirmRemove}
  <Confirm
    title={m.mobile_source_remove()}
    body={m.mobile_source_remove_body({
      name: account.display_name || kindLabel(account),
    })}
    confirmLabel={m.mobile_remove()}
    onconfirm={() => remove(account)}
    oncancel={() => (confirmRemove = null)}
  />
{/if}

<style>
  /* A monogram, not a logo — see `cloudProviders.ts`. Decorative: the
     provider's name is the row's text, so the tile is aria-hidden and
     its lettering carries no information of its own. */
  .provider-tile {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 36px;
    height: 36px;
    border-radius: var(--r-sm);
    background: oklch(0.48 0.13 var(--tile-hue));
    color: #fff;
    font-size: var(--t-caption);
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .device-steps {
    margin-top: var(--s-3);
    padding-left: var(--s-5);
    list-style: decimal;
    font-size: var(--t-body);
    line-height: var(--t-body-lh);
    color: var(--ui-ink);
  }

  .device-steps li + li {
    margin-top: var(--s-2);
  }

  .fixed-servers {
    margin: var(--s-2) 0 var(--s-3);
    padding: var(--s-2) var(--s-3);
    border-radius: var(--r-sm);
    background: color-mix(in oklab, var(--color-surface-500) 12%, transparent);
    font-size: var(--t-callout);
    line-height: var(--t-callout-lh);
  }

  .fixed-servers div {
    display: flex;
    gap: var(--s-2);
  }

  .fixed-servers dt {
    color: var(--ui-ink-muted);
    min-width: 5.5rem;
  }

  .fixed-servers dd {
    color: var(--ui-ink);
    overflow-wrap: anywhere;
  }
</style>
