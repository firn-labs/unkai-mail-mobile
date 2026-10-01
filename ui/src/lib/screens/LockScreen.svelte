<script lang="ts">
  /**
   * The cache is encrypted and needs its passphrase.
   *
   * Shown instead of the whole app — there is nothing to render
   * until the store opens. The attempt counter is deliberately
   * visible when a wipe policy is armed: destroying the local cache
   * without warning would be a nasty surprise, even though it costs
   * only a re-sync.
   */
  import * as api from '../api'
  import type { DatabaseStatusView } from '../api'
  import Icon from '../Icon.svelte'
  import { formatError } from '../errors'
  import { m } from '../../paraglide/messages'

  interface Props {
    status: DatabaseStatusView
    onunlocked: () => void
  }

  let { status, onunlocked }: Props = $props()

  /* Read once, deliberately: the lock screen is mounted fresh
     for each open and unmounted on close, so the form owns its
     fields from here on. */
  // svelte-ignore state_referenced_locally
  const seed = status

  let passphrase = $state('')
  let busy = $state(false)
  let error = $state('')
  let attemptsLeft = $state<number | null>(seed.attempts_remaining ?? null)

  async function unlock() {
    if (!passphrase) return
    busy = true
    error = ''
    try {
      await api.settings.unlockWithPassphrase({ passphrase })
      passphrase = ''
      onunlocked()
    } catch (e) {
      error = formatError(e).replace(/^Auth:\s*/i, '')
      try {
        const fresh = await api.settings.databaseStatus()
        attemptsLeft = fresh.attempts_remaining ?? null
      } catch {
        // Status is a nicety here; the error above is the message
        // that matters.
      }
    } finally {
      busy = false
    }
  }
</script>

<div class="lock">
  <div class="lock__card">
    <Icon name="lock" size={36} />
    <h1 class="mt-3 text-lg font-semibold">{m.mobile_locked_title()}</h1>
    <p class="mt-1 text-sm ink-muted">{m.mobile_locked_body()}</p>

    <input
      type="password"
      class="lock__input"
      autocomplete="current-password"
      bind:value={passphrase}
      aria-label={m.mobile_passphrase_placeholder()}
      placeholder={m.mobile_passphrase_placeholder()}
      onkeydown={(e) => e.key === 'Enter' && unlock()}
    />

    {#if error}
      <p class="mt-2 text-sm ink-danger">{error}</p>
    {/if}
    {#if attemptsLeft !== null}
      <p class="mt-2 text-xs ink-warning">
        {m.mobile_locked_attempts({ count: attemptsLeft })}
      </p>
    {/if}

    <button type="button" class="wide-button mt-4" disabled={busy || !passphrase} onclick={unlock}>
      {busy ? m.mobile_working() : m.mobile_unlock()}
    </button>
  </div>
</div>

<style>
  .lock {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    padding-top: calc(env(safe-area-inset-top) + 24px);
  }
  .lock__card {
    width: 100%;
    max-width: 22rem;
    text-align: center;
  }
  .lock__input {
    width: 100%;
    margin-top: 16px;
    padding: 12px;
    border-radius: var(--r-md);
    border: 1px solid color-mix(in oklab, var(--color-surface-500) 30%, transparent);
    background: transparent;
  }
</style>
