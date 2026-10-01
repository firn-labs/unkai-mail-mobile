<script lang="ts">
  /**
   * Blocking confirmation for the few actions that genuinely can't
   * be undone: removing an account, deleting a folder, wiping the
   * local vault.
   *
   * Everything reversible uses an undo toast instead — a phone that
   * asks "are you sure?" on every swipe is a phone nobody triages
   * mail on.
   */
  import Sheet from './Sheet.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    title: string
    body?: string
    confirmLabel: string
    destructive?: boolean
    onconfirm: () => void
    oncancel: () => void
  }
  let { title, body = '', confirmLabel, destructive = true, onconfirm, oncancel }: Props = $props()

  const bodyId = `confirm-body-${Math.random().toString(36).slice(2)}`
</script>

<!-- `alertdialog`, described by its body: VoiceOver reads the question
     and its consequence together when the sheet opens, before the user
     has to find either button. -->
<Sheet label={title} role="alertdialog" describedby={body ? bodyId : undefined} onclose={oncancel}>
  <div class="confirm-head">
    <h2 class="confirm-head__title">{title}</h2>
    {#if body}<p id={bodyId} class="confirm-head__body">{body}</p>{/if}
  </div>
  <!-- The shared capsule buttons, so a confirmation looks like every
       other decision in the app. -->
  <div class="confirm-actions">
    <button
      type="button"
      class="wide-button"
      class:wide-button--destructive={destructive}
      onclick={onconfirm}
    >
      {confirmLabel}
    </button>
    <button type="button" class="wide-button wide-button--quiet" onclick={oncancel}>
      {m.mobile_cancel()}
    </button>
  </div>
</Sheet>

<style>
  .confirm-head {
    padding: var(--s-2) var(--s-6) var(--s-5);
    text-align: center;
  }

  .confirm-head__title {
    font-family: var(--font-display);
    font-size: var(--t-title-2);
    line-height: var(--t-title-2-lh);
    font-weight: 750;
    color: var(--ui-ink);
  }

  .confirm-head__body {
    margin-top: var(--s-2);
    font-size: var(--t-callout);
    line-height: var(--t-callout-lh);
    color: var(--ui-ink-muted);
  }

  .confirm-actions {
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
    padding: 0 var(--s-3) var(--s-1);
  }
</style>
