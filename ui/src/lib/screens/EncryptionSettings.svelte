<script lang="ts">
  /**
   * End-to-end encryption keys, per account.
   *
   * Import a PGP private key (or an S/MIME PKCS#12 bundle) from the
   * Files app and the account can read and write encrypted mail.
   * "Unlock automatically" parks the passphrase in the OS keychain
   * so reading an encrypted message doesn't prompt every time —
   * opt-in, per account, exactly as on the desktop.
   */
  import * as api from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Toggle from '../Toggle.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import { nav } from '../nav.svelte'
  import { accountsStore } from '../accountsStore.svelte'
  import { toasts } from '../toast.svelte'
  import { m } from '../../paraglide/messages'

  interface KeyState {
    accountId: string
    email: string
    pgpFingerprint: string | null
    smimeFingerprint: string | null
    pgpAuto: boolean
  }

  let states = $state<KeyState[]>([])
  let loading = $state(true)
  let importFor = $state<{ accountId: string; kind: 'pgp' | 'smime' } | null>(null)
  let importPassphrase = $state('')
  let importBusy = $state(false)
  let fileInput: HTMLInputElement | undefined = $state()
  let pendingFile = $state<{ name: string; text: string; base64: string } | null>(null)

  async function load() {
    const next: KeyState[] = []
    for (const account of accountsStore.list) {
      try {
        const pgp = await api.crypto.pgpGetAccountKeyStatus({ accountId: account.id })
        const smime = await api.crypto.smimeGetAccountCertStatus({ accountId: account.id })
        const auto = await api.crypto.pgpHasUnlockAutomatically({ accountId: account.id })
        next.push({
          accountId: account.id,
          email: account.email,
          pgpFingerprint: pgp?.fingerprint ?? null,
          smimeFingerprint: smime?.fingerprint ?? null,
          pgpAuto: auto,
        })
      } catch {
        next.push({
          accountId: account.id,
          email: account.email,
          pgpFingerprint: null,
          smimeFingerprint: null,
          pgpAuto: false,
        })
      }
    }
    states = next
    loading = false
  }

  $effect(() => {
    void load()
  })

  function pickFile(accountId: string, kind: 'pgp' | 'smime') {
    importFor = { accountId, kind }
    pendingFile = null
    importPassphrase = ''
    fileInput?.click()
  }

  function onFilePicked(e: Event) {
    const input = e.currentTarget as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    const reader = new FileReader()
    reader.onload = () => {
      const buffer = reader.result as ArrayBuffer
      const bytes = new Uint8Array(buffer)
      // Both forms are needed: PGP keys go over the wire as ASCII
      // armor, PKCS#12 bundles as base64 of the raw DER.
      let binary = ''
      for (const byte of bytes) binary += String.fromCharCode(byte)
      pendingFile = {
        name: file.name,
        text: new TextDecoder().decode(bytes),
        base64: btoa(binary),
      }
    }
    reader.onerror = () => toasts.error(m.mobile_key_read_failed())
    reader.readAsArrayBuffer(file)
  }

  async function runImport() {
    if (!importFor || !pendingFile) return
    importBusy = true
    try {
      if (importFor.kind === 'pgp') {
        await api.crypto.pgpImportPrivateKey({
          accountId: importFor.accountId,
          armoredKey: pendingFile.text,
          passphrase: importPassphrase,
        })
      } else {
        await api.crypto.smimeImportPkcs12({
          accountId: importFor.accountId,
          pkcs12Base64: pendingFile.base64,
          passphrase: importPassphrase,
        })
      }
      toasts.success(m.mobile_key_imported())
      importFor = null
      pendingFile = null
      importPassphrase = ''
      await load()
    } catch (e) {
      toasts.error(m.mobile_key_import_failed(), e)
    } finally {
      importBusy = false
    }
  }

  async function removePgp(accountId: string) {
    try {
      await api.crypto.pgpRemovePrivateKey({ accountId })
      await load()
      toasts.show(m.mobile_key_removed())
    } catch (e) {
      toasts.error(m.mobile_key_remove_failed(), e)
    }
  }

  async function toggleAuto(state: KeyState, enabled: boolean) {
    try {
      if (enabled) {
        // The backend needs the passphrase once to park it in the
        // keychain; asking here keeps that explicit.
        importFor = { accountId: state.accountId, kind: 'pgp' }
        pendingFile = null
        toasts.show(m.mobile_auto_unlock_needs_passphrase())
      } else {
        await api.crypto.pgpDisableUnlockAutomatically({ accountId: state.accountId })
        await load()
      }
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  async function enableAuto(accountId: string) {
    importBusy = true
    try {
      await api.crypto.pgpEnableUnlockAutomatically({ accountId, passphrase: importPassphrase })
      importFor = null
      importPassphrase = ''
      await load()
      toasts.success(m.mobile_auto_unlock_enabled())
    } catch (e) {
      toasts.error(m.mobile_auto_unlock_failed(), e)
    } finally {
      importBusy = false
    }
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_settings_encryption()}
    backLabel={m.mobile_settings()}
    onback={() => nav.pop()}
  />

  <div class="screen__body">
    {#if loading}
      <Spinner />
    {:else if states.length === 0}
      <p class="px-4 py-8 text-center text-sm ink-muted">{m.mobile_no_accounts_body()}</p>
    {:else}
      {#each states as state (state.accountId)}
        <div class="list-header">{state.email}</div>
        <div class="list-group">
          <div class="list-row">
            <Icon name={state.pgpFingerprint ? 'encrypted' : 'unlocked'} size={20} />
            <span class="min-w-0 flex-1">
              <span class="block list-row__title">{m.mobile_pgp_key()}</span>
              <span class="block truncate list-row__detail">
                {state.pgpFingerprint ?? m.mobile_no_key()}
              </span>
            </span>
            {#if state.pgpFingerprint}
              <button
                type="button"
                class="bar-button bar-button--destructive"
                aria-label={m.mobile_remove()}
                onclick={() => removePgp(state.accountId)}
              >
                <Icon name="trash" size={18} />
              </button>
            {:else}
              <button
                type="button"
                class="value-button"
                onclick={() => pickFile(state.accountId, 'pgp')}
              >
                {m.mobile_import()}
              </button>
            {/if}
          </div>

          {#if state.pgpFingerprint}
            <div class="list-row">
              <span class="flex-1">
                <span class="block list-row__title">{m.mobile_auto_unlock()}</span>
                <span class="block list-row__detail">{m.mobile_auto_unlock_hint()}</span>
              </span>
              <Toggle
                checked={state.pgpAuto}
                label={m.mobile_auto_unlock()}
                onchange={(v) => toggleAuto(state, v)}
              />
            </div>
          {/if}

          <div class="list-row">
            <Icon name={state.smimeFingerprint ? 'verified' : 'unlocked'} size={20} />
            <span class="min-w-0 flex-1">
              <span class="block list-row__title">{m.mobile_smime_cert()}</span>
              <span class="block truncate list-row__detail">
                {state.smimeFingerprint ?? m.mobile_no_key()}
              </span>
            </span>
            {#if !state.smimeFingerprint}
              <button
                type="button"
                class="value-button"
                onclick={() => pickFile(state.accountId, 'smime')}
              >
                {m.mobile_import()}
              </button>
            {/if}
          </div>
        </div>
      {/each}
      <p class="list-footer pb-10">{m.mobile_encryption_hint()}</p>
    {/if}
  </div>

  <input
    bind:this={fileInput}
    type="file"
    class="hidden"
    accept=".asc,.gpg,.key,.p12,.pfx,application/x-pkcs12,text/plain"
    onchange={onFilePicked}
  />
</div>

{#if importFor}
  <Sheet
    label={m.mobile_import_key()}
    onclose={() => {
      importFor = null
      pendingFile = null
    }}
  >
    <div class="form-group px-4 pb-6 pt-1">
      <h2 class="text-base font-semibold">
        {pendingFile ? m.mobile_import_key() : m.mobile_auto_unlock()}
      </h2>
      {#if pendingFile}
        <p class="mb-3 mt-1 truncate text-sm ink-muted">{pendingFile.name}</p>
      {:else}
        <p class="mb-3 mt-1 text-sm ink-muted">{m.mobile_auto_unlock_prompt()}</p>
      {/if}
      <input
        type="password"
        autocomplete="off"
        bind:value={importPassphrase}
        aria-label={m.mobile_key_passphrase_placeholder()}
        placeholder={m.mobile_key_passphrase_placeholder()}
      />
      <button
        type="button"
        class="wide-button mt-4"
        disabled={importBusy}
        onclick={() =>
          pendingFile ? runImport() : enableAuto(importFor!.accountId)}
      >
        {importBusy ? m.mobile_working() : m.mobile_continue()}
      </button>
    </div>
  </Sheet>
{/if}
