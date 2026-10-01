<script lang="ts">
  /** The mail-account list: add, open one, or remove it. */
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import { nav } from '../nav.svelte'
  import { accountsStore } from '../accountsStore.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    onaddaccount: () => void
  }

  let { onaddaccount }: Props = $props()

  const accounts = $derived(accountsStore.list)

  $effect(() => {
    void accountsStore.load()
  })
</script>

<div class="screen">
  <NavBar
    title={m.mobile_settings_accounts()}
    backLabel={m.mobile_settings()}
    onback={() => nav.pop()}
  >
    {#snippet actions()}
      <button type="button" class="bar-button" aria-label={m.mobile_add_account()} onclick={onaddaccount}>
        <Icon name="plus" size={20} />
      </button>
    {/snippet}
  </NavBar>

  <div class="screen__body">
    {#if accounts.length === 0}
      <EmptyState
        icon="add-account"
        title={m.mobile_no_accounts_title()}
        body={m.mobile_no_accounts_body()}
        action={{ label: m.mobile_add_account(), run: onaddaccount }}
      />
    {:else}
      <div class="list-group">
        {#each accounts as account (account.id)}
          <button
            class="list-row"
            onclick={() => nav.push('settings-account', { accountId: account.id })}
          >
            <span class="account-emoji">{account.emoji || '✉️'}</span>
            <span class="min-w-0 flex-1">
              <span class="block truncate list-row__title">
                {account.display_name || account.email}
              </span>
              <span class="block truncate list-row__detail">{account.email}</span>
            </span>
            <Icon name="nav-forward" size={16} />
          </button>
        {/each}
      </div>
      <div class="px-3 pt-4">
        <button type="button" class="wide-button wide-button--quiet" onclick={onaddaccount}>
          {m.mobile_add_account()}
        </button>
      </div>
    {/if}
    <div class="h-16"></div>
  </div>
</div>

<style>
  .account-emoji {
    font-size: calc(22px * var(--dt-content, 1));
    width: 28px;
    text-align: center;
  }
</style>
