<script lang="ts">
  /**
   * Connecting a Nextcloud, and what's connected already.
   *
   * Uses Login Flow v2: the app opens the server's login page in
   * Safari and polls for the app password the server hands back.
   * That's the right shape on a phone — the user signs in with
   * whatever their server uses (SSO, a hardware key, a password
   * manager) and the app never sees the real password.
   *
   * The settings backup lives here too: it's stored on the
   * Nextcloud, which makes it the path a desktop install's settings
   * travel to this device on.
   */
  import { formatLocale } from '../locale'
  import * as api from '../api'
  import type { NextcloudAccount } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import Confirm from '../ui/Confirm.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { getSyncState, ncProbeBundle, ncRestoreBundle, setSyncTarget } from '../settingsBundle'
  import { m } from '../../paraglide/messages'

  let accounts = $state<NextcloudAccount[]>([])
  let loading = $state(true)
  let connecting = $state(false)
  let serverUrl = $state('')
  let showConnect = $state(false)
  let confirmRemove = $state<NextcloudAccount | null>(null)
  let syncTarget = $state<string | null>(null)
  let restoring = $state(false)

  /** Cancels the login poll when the sheet closes. */
  let pollAbort: { cancelled: boolean } | null = null

  async function load() {
    try {
      accounts = await api.nextcloud.getNextcloudAccounts()
      syncTarget = (await getSyncState()).targetNcId
    } catch (e) {
      toasts.error(m.mobile_nextcloud_load_failed(), e)
    } finally {
      loading = false
    }
  }

  $effect(() => {
    void load()
    return () => {
      if (pollAbort) pollAbort.cancelled = true
    }
  })

  async function connect() {
    const url = serverUrl.trim()
    if (!url) return
    connecting = true
    const abort = { cancelled: false }
    pollAbort = abort
    try {
      const flow = await api.nextcloud.startNextcloudLogin({
        serverUrl: url.startsWith('http') ? url : `https://${url}`,
      })
      await api.system.openUrl({ url: flow.login_url })

      // Poll until the user finishes in the browser. Nextcloud's
      // endpoint 404s until the grant lands, so a null answer is
      // "not yet", not a failure. ~10 minutes at 2s.
      for (let i = 0; i < 300 && !abort.cancelled; i++) {
        await new Promise((resolve) => setTimeout(resolve, 2000))
        const account = await api.nextcloud
          .pollNextcloudLogin({
            pollEndpoint: flow.poll_endpoint,
            pollToken: flow.poll_token,
          })
          .catch(() => null)
        if (account) {
          await load()
          showConnect = false
          serverUrl = ''
          toasts.success(m.mobile_nextcloud_connected({ server: account.server_url }))
          await offerRestore(account.id)
          return
        }
      }
      if (!abort.cancelled) toasts.error(m.mobile_nextcloud_login_timeout())
    } catch (e) {
      toasts.error(m.mobile_nextcloud_connect_failed(), e)
    } finally {
      connecting = false
      pollAbort = null
    }
  }

  /** A freshly connected server may already hold a settings bundle
   *  from the user's desktop — the moment to offer it is now. */
  async function offerRestore(ncId: string) {
    try {
      const exportedAt = await ncProbeBundle(ncId)
      if (!exportedAt) return
      toasts.show(m.mobile_backup_found({ date: new Date(exportedAt).toLocaleDateString(formatLocale()) }), {
        action: { label: m.mobile_restore(), run: () => void restore(ncId) },
        timeout: 12_000,
      })
    } catch {
      // No bundle, or the server said no — nothing to offer.
    }
  }

  async function restore(ncId: string) {
    restoring = true
    try {
      await ncRestoreBundle(ncId)
      toasts.success(m.mobile_backup_restored())
    } catch (e) {
      toasts.error(m.mobile_backup_restore_failed(), e)
    } finally {
      restoring = false
    }
  }

  async function makeSyncTarget(ncId: string | null) {
    try {
      await setSyncTarget(ncId)
      syncTarget = ncId
      toasts.success(ncId ? m.mobile_backup_enabled() : m.mobile_backup_disabled())
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  async function remove(account: NextcloudAccount) {
    confirmRemove = null
    try {
      await api.nextcloud.removeNextcloudAccount({ id: account.id })
      await load()
      toasts.show(m.mobile_nextcloud_removed())
    } catch (e) {
      toasts.error(m.mobile_nextcloud_remove_failed(), e)
    }
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_settings_nextcloud()}
    backLabel={m.mobile_settings()}
    onback={() => nav.pop()}
  >
    {#snippet actions()}
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_connect()}
        onclick={() => (showConnect = true)}
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
        <div class="px-6 pb-2 pt-8 text-center">
          <Icon name="cloud" size={40} />
          <h2 class="mt-2 text-base font-semibold">{m.mobile_no_nextcloud_title()}</h2>
          <p class="mt-1 text-sm ink-muted">{m.mobile_nextcloud_pitch()}</p>
        </div>
        <div class="px-3 pt-2">
          <button type="button" class="wide-button" onclick={() => (showConnect = true)}>
            {m.mobile_connect_nextcloud()}
          </button>
        </div>
      {:else}
        <div class="list-group">
          {#each accounts as account (account.id)}
            <div class="list-row">
              <Icon name="cloud" size={20} />
              <span class="min-w-0 flex-1">
                <span class="block truncate list-row__title">
                  {account.display_name || account.username}
                </span>
                <span class="block truncate list-row__detail">{account.server_url}</span>
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

        <div class="list-header">{m.mobile_settings_backup()}</div>
        <div class="list-group">
          <button class="list-row" onclick={() => makeSyncTarget(null)}>
            <span class="flex-1 list-row__title">{m.mobile_backup_off()}</span>
            {#if syncTarget === null}
              <span class="ink-accent"><Icon name="check" size={18} /></span>
            {/if}
          </button>
          {#each accounts as account (account.id)}
            <button class="list-row" onclick={() => makeSyncTarget(account.id)}>
              <span class="min-w-0 flex-1">
                <span class="block truncate list-row__title">{account.server_url}</span>
                <span class="block list-row__detail">{m.mobile_backup_target_hint()}</span>
              </span>
              {#if syncTarget === account.id}
                <span class="ink-accent"><Icon name="check" size={18} /></span>
              {/if}
            </button>
          {/each}
        </div>
        <p class="list-footer">{m.mobile_backup_hint()}</p>

        <div class="px-3 pt-3">
          <button
            type="button"
            class="wide-button wide-button--quiet"
            disabled={restoring || accounts.length === 0}
            onclick={() => restore(syncTarget ?? accounts[0].id)}
          >
            {restoring ? m.mobile_working() : m.mobile_restore_from_backup()}
          </button>
        </div>
      {/if}
      <div class="h-16"></div>
    {/if}
  </div>
</div>

{#if showConnect}
  <Sheet
    label={m.mobile_connect_nextcloud()}
    onclose={() => {
      if (pollAbort) pollAbort.cancelled = true
      showConnect = false
      connecting = false
    }}
  >
    <div class="form-group px-4 pb-6 pt-1">
      <h2 class="text-base font-semibold">{m.mobile_connect_nextcloud()}</h2>
      <p class="mb-3 mt-1 text-sm ink-muted">{m.mobile_connect_hint()}</p>
      <input
        type="url"
        inputmode="url"
        autocapitalize="none"
        autocorrect="off"
        spellcheck="false"
        bind:value={serverUrl}
        aria-label={m.mobile_sr_server_address()}
        placeholder="cloud.example.com"
        disabled={connecting}
      />
      <button type="button" class="wide-button mt-4" disabled={connecting || !serverUrl} onclick={connect}>
        {connecting ? m.mobile_waiting_for_browser() : m.mobile_connect()}
      </button>
      {#if connecting}
        <p class="mt-3 text-center text-xs ink-muted">
          {m.mobile_connect_polling()}
        </p>
      {/if}
    </div>
  </Sheet>
{/if}

{#if confirmRemove}
  {@const account = confirmRemove}
  <Confirm
    title={m.mobile_nextcloud_remove()}
    body={m.mobile_nextcloud_remove_body({ server: account.server_url })}
    confirmLabel={m.mobile_remove()}
    onconfirm={() => remove(account)}
    oncancel={() => (confirmRemove = null)}
  />
{/if}
