<script lang="ts">
  /**
   * One mail account: identity, signature, servers, password,
   * removal.
   *
   * Saves explicitly rather than per-keystroke — unlike the toggle
   * screens, these are text fields where a half-typed hostname
   * shouldn't be written through and then used by the sync loop.
   */
  import * as api from '../api'
  import type { Account } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import Confirm from '../ui/Confirm.svelte'
  import { nav } from '../nav.svelte'
  import { accountsStore } from '../accountsStore.svelte'
  import { toasts } from '../toast.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    accountId: string
  }

  let { accountId }: Props = $props()

  let account = $state<Account | null>(null)
  let newPassword = $state('')
  let saving = $state(false)
  let confirmRemove = $state(false)

  $effect(() => {
    account = accountsStore.list.find((a) => a.id === accountId) ?? null
  })

  const hiddenCount = $derived(account?.hidden_folders?.length ?? 0)

  async function save() {
    if (!account) return
    saving = true
    try {
      await api.accounts.updateAccount({ account })
      if (newPassword) {
        await api.accounts.setAccountPassword({ id: account.id, password: newPassword })
        newPassword = ''
      }
      await accountsStore.load()
      toasts.success(m.mobile_account_saved())
    } catch (e) {
      toasts.error(m.mobile_account_save_failed(), e)
    } finally {
      saving = false
    }
  }

  async function remove() {
    confirmRemove = false
    if (!account) return
    try {
      await api.accounts.removeAccount({ id: account.id })
      await accountsStore.load()
      toasts.show(m.mobile_account_removed())
      nav.pop()
    } catch (e) {
      toasts.error(m.mobile_account_remove_failed(), e)
    }
  }
</script>

<div class="screen">
  <NavBar
    title={account?.email ?? m.mobile_account()}
    backLabel={m.mobile_settings_accounts()}
    onback={() => nav.pop()}
  >
    {#snippet actions()}
      <button
        type="button"
        class="bar-button bar-button--text bar-button--strong"
        disabled={saving || !account}
        onclick={save}
      >
        {saving ? m.mobile_saving() : m.mobile_save()}
      </button>
    {/snippet}
  </NavBar>

  <div class="screen__body form-group">
    {#if !account}
      <Spinner />
    {:else}
      <div class="list-header">{m.mobile_account_identity()}</div>
      <div class="list-group">
        <div class="px-4 py-2">
          <input
            type="text"
            bind:value={account.person_name}
            aria-label={m.mobile_setup_name_placeholder()}
            placeholder={m.mobile_setup_name_placeholder()}
          />
        </div>
        <div class="px-4 py-2">
          <input
            type="text"
            bind:value={account.display_name}
            aria-label={m.mobile_setup_display_name_placeholder()}
            placeholder={m.mobile_setup_display_name_placeholder()}
          />
        </div>
        <div class="px-4 py-2">
          <input
            type="text"
            bind:value={account.emoji}
            maxlength="4"
            aria-label={m.mobile_account_emoji_placeholder()}
            placeholder={m.mobile_account_emoji_placeholder()}
          />
        </div>
      </div>

      <div class="list-header">{m.mobile_account_signature()}</div>
      <div class="list-group">
        <div class="px-4 py-2">
          <textarea
            rows="4"
            aria-label={m.mobile_account_signature()}
            bind:value={account.signature}
          ></textarea>
        </div>
      </div>
      <p class="list-footer">{m.mobile_account_signature_hint()}</p>

      <div class="list-header">{m.mobile_setup_servers()}</div>
      <div class="list-group">
        <div class="field-pair px-4 py-2">
          <input
            type="text"
            class="field-pair__grow"
            autocapitalize="none"
            autocorrect="off"
            aria-label={m.mobile_setup_imap_host()}
            bind:value={account.imap_host}
          />
          <input
            type="number"
            inputmode="numeric"
            class="field-pair__port"
            aria-label={m.mobile_setup_imap_port()}
            bind:value={account.imap_port}
          />
        </div>
        <div class="field-pair px-4 py-2">
          <input
            type="text"
            class="field-pair__grow"
            autocapitalize="none"
            autocorrect="off"
            aria-label={m.mobile_setup_smtp_host()}
            bind:value={account.smtp_host}
          />
          <input
            type="number"
            inputmode="numeric"
            class="field-pair__port"
            aria-label={m.mobile_setup_smtp_port()}
            bind:value={account.smtp_port}
          />
        </div>
      </div>

      <div class="list-header">{m.mobile_mailboxes_title()}</div>
      <div class="list-group">
        <button
          class="list-row"
          onclick={() => nav.push('settings-account-folders', { accountId })}
        >
          <span class="min-w-0 flex-1">
            <span class="block list-row__title">{m.mobile_account_folders()}</span>
            {#if hiddenCount > 0}
              <span class="block list-row__detail">
                {hiddenCount === 1
                  ? m.mobile_account_folders_hidden_one()
                  : m.mobile_account_folders_hidden_many({ count: hiddenCount })}
              </span>
            {/if}
          </span>
          <Icon name="nav-forward" size={16} />
        </button>
      </div>

      <div class="list-header">{m.mobile_account_password()}</div>
      <div class="list-group">
        <div class="px-4 py-2">
          <input
            type="password"
            autocomplete="new-password"
            bind:value={newPassword}
            aria-label={m.mobile_account_password_placeholder()}
            placeholder={m.mobile_account_password_placeholder()}
          />
        </div>
      </div>
      <p class="list-footer">{m.mobile_account_password_hint()}</p>

      <div class="px-3 pb-10 pt-6">
        <button
          type="button"
          class="wide-button wide-button--destructive"
          onclick={() => (confirmRemove = true)}
        >
          {m.mobile_remove_account()}
        </button>
      </div>
    {/if}
  </div>
</div>

{#if confirmRemove && account}
  <Confirm
    title={m.mobile_remove_account()}
    body={m.mobile_remove_account_body({ email: account.email })}
    confirmLabel={m.mobile_remove()}
    onconfirm={remove}
    oncancel={() => (confirmRemove = false)}
  />
{/if}
