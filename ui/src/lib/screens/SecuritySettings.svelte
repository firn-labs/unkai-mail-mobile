<script lang="ts">
  /**
   * The local vault.
   *
   * The mail cache is a SQLCipher database; this screen governs the
   * key that opens it. On a phone the device passcode already gates
   * the app, so an app-level passphrase is opt-in — for people who
   * want the cache unreadable even to someone holding an unlocked
   * phone.
   *
   * The wipe policy is the other half: after N failed attempts the
   * local cache is destroyed. Mail lives on the server, so this
   * costs a re-sync, not data.
   */
  import * as api from '../api'
  import type { DatabaseStatusView, WipePolicyView } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Toggle from '../Toggle.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { m } from '../../paraglide/messages'

  let status = $state<DatabaseStatusView | null>(null)
  let vault = $state<{ has_plain_key: boolean; credentials: { kind: string; credential_id: string; label: string }[] } | null>(null)
  let policy = $state<WipePolicyView | null>(null)
  let loading = $state(true)
  let showEnroll = $state(false)
  let passphrase = $state('')
  let passphraseAgain = $state('')
  let busy = $state(false)

  const passphraseWraps = $derived(
    (vault?.credentials ?? []).filter((c) => c.kind === 'passphrase'),
  )
  const hasPassphrase = $derived(passphraseWraps.length > 0)
  /** Passphrase enrolled AND the plaintext key removed — only then
   *  is the cache actually gated behind it. */
  const passphraseRequired = $derived(hasPassphrase && vault?.has_plain_key === false)

  async function load() {
    try {
      status = await api.settings.databaseStatus()
      vault = await api.settings.fidoStatus()
      policy = await api.settings.getWipePolicy()
    } catch (e) {
      toasts.error(m.mobile_security_load_failed(), e)
    } finally {
      loading = false
    }
  }

  $effect(() => {
    void load()
  })

  /**
   * Enrol a passphrase: wrap the cache key under it, then (by
   * default) drop the plaintext key so the passphrase is actually
   * required. Both steps matter — enrolling alone leaves the key
   * readable from the keychain.
   */
  async function enroll() {
    if (passphrase.length < 8 || passphrase !== passphraseAgain) return
    busy = true
    try {
      await api.settings.fidoEnrollPassphrase({
        passphrase,
        label: m.mobile_vault_passphrase(),
      })
      await api.settings.enableFidoOnlyMode()
      passphrase = ''
      passphraseAgain = ''
      showEnroll = false
      await load()
      toasts.success(m.mobile_vault_enrolled())
    } catch (e) {
      toasts.error(m.mobile_vault_enroll_failed(), e)
    } finally {
      busy = false
    }
  }

  async function toggleRequired(required: boolean) {
    try {
      if (required) await api.settings.enableFidoOnlyMode()
      else await api.settings.disableFidoOnlyMode()
      await load()
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  async function removePassphrase() {
    const wrap = passphraseWraps[0]
    if (!wrap) return
    try {
      // Put the plaintext key back first: removing the only wrap
      // while the cache is passphrase-only would lock the user out
      // of their own cache permanently.
      if (!vault?.has_plain_key) await api.settings.disableFidoOnlyMode()
      await api.settings.fidoRemove({ credentialIdB64: wrap.credential_id })
      await load()
      toasts.show(m.mobile_vault_passphrase_removed())
    } catch (e) {
      toasts.error(m.mobile_vault_remove_failed(), e)
    }
  }

  async function setPolicy(change: Partial<WipePolicyView>) {
    if (!policy) return
    const next = { ...policy, ...change }
    try {
      await api.settings.setWipePolicy({ policy: next })
      policy = next
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_settings_security()}
    backLabel={m.mobile_settings()}
    onback={() => nav.pop()}
  />

  <div class="screen__body form-group">
    {#if loading}
      <Spinner />
    {:else}
      <div class="list-group">
        <div class="list-row">
          <Icon name={passphraseRequired ? 'lock' : 'unlocked'} size={20} />
          <span class="flex-1">
            <span class="block list-row__title">{m.mobile_vault_passphrase()}</span>
            <span class="block list-row__detail">
              {passphraseRequired
                ? m.mobile_vault_passphrase_on()
                : hasPassphrase
                  ? m.mobile_vault_passphrase_enrolled()
                  : m.mobile_vault_passphrase_off()}
            </span>
          </span>
          {#if !hasPassphrase}
            <button type="button" class="value-button" onclick={() => (showEnroll = true)}>
              {m.mobile_set_up()}
            </button>
          {/if}
        </div>
        {#if hasPassphrase}
          <div class="list-row">
            <span class="flex-1">
              <span class="block list-row__title">{m.mobile_vault_require()}</span>
              <span class="block list-row__detail">{m.mobile_vault_require_hint()}</span>
            </span>
            <Toggle
              checked={passphraseRequired}
              label={m.mobile_vault_require()}
              onchange={(v) => toggleRequired(v)}
            />
          </div>
          <button class="list-row" onclick={removePassphrase}>
            <span class="flex-1 list-row__title ink-danger">
              {m.mobile_vault_remove_passphrase()}
            </span>
          </button>
        {/if}
      </div>
      <p class="list-footer">{m.mobile_vault_hint()}</p>

      {#if policy}
        <div class="list-header">{m.mobile_wipe_policy()}</div>
        <div class="list-group">
          <div class="list-row">
            <span class="flex-1">
              <span class="block list-row__title">{m.mobile_wipe_enabled()}</span>
              <span class="block list-row__detail">{m.mobile_wipe_enabled_hint()}</span>
            </span>
            <Toggle
              checked={policy.enabled}
              label={m.mobile_wipe_enabled()}
              onchange={(v) => setPolicy({ enabled: v, max_attempts: policy?.max_attempts ?? 10 })}
            />
          </div>
          {#if policy.enabled}
            <div class="list-row">
              <span class="flex-1 list-row__title">{m.mobile_wipe_attempts()}</span>
              <select
                aria-label={m.mobile_wipe_attempts()}
                value={policy.max_attempts ?? 10}
                onchange={(e) => setPolicy({ max_attempts: Number(e.currentTarget.value) })}
                class="max-w-[40%]"
              >
                {#each [3, 5, 10, 20] as count (count)}
                  <option value={count}>{count}</option>
                {/each}
              </select>
            </div>
          {/if}
        </div>
        <p class="list-footer pb-10">{m.mobile_wipe_hint()}</p>
      {/if}
    {/if}
  </div>
</div>

{#if showEnroll}
  <Sheet label={m.mobile_vault_passphrase()} onclose={() => (showEnroll = false)}>
    <div class="form-group px-4 pb-6 pt-1">
      <h2 class="text-base font-semibold">{m.mobile_vault_passphrase()}</h2>
      <p class="mb-3 mt-1 text-sm ink-muted">{m.mobile_vault_enroll_hint()}</p>
      <input
        type="password"
        autocomplete="new-password"
        bind:value={passphrase}
        aria-label={m.mobile_passphrase_placeholder()}
        placeholder={m.mobile_passphrase_placeholder()}
      />
      <input
        type="password"
        class="mt-2"
        autocomplete="new-password"
        bind:value={passphraseAgain}
        aria-label={m.mobile_passphrase_repeat_placeholder()}
        placeholder={m.mobile_passphrase_repeat_placeholder()}
      />
      <p class="mt-3 text-xs ink-warning">{m.mobile_vault_enroll_warning()}</p>
      <button
        type="button"
        class="wide-button mt-3"
        disabled={busy || passphrase.length < 8 || passphrase !== passphraseAgain}
        onclick={enroll}
      >
        {busy ? m.mobile_working() : m.mobile_save()}
      </button>
    </div>
  </Sheet>
{/if}
