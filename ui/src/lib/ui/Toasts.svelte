<script lang="ts">
  /** Renders the toast queue above the tab bar. Mounted once by
   *  `App.svelte`; everything else calls `toasts.show(...)`.
   *
   *  Two things make a toast usable beyond a glance:
   *
   *    * **It is announced.** Confirmations go into a polite live
   *      region; errors into an assertive one, so VoiceOver interrupts
   *      for "Couldn't send" but not for "Archived". Both regions exist
   *      from the first render — a region inserted together with its
   *      text is often not announced at all.
   *    * **It waits while you are on it.** A finger resting on a toast,
   *      or keyboard/VoiceOver focus inside it, holds the dismissal
   *      timer, so an Undo cannot vanish from under someone who is
   *      still reaching for it (WCAG 2.2.1). */
  import { fly } from 'svelte/transition'
  import { toasts, type Toast } from '../toast.svelte'
  import Icon from '../Icon.svelte'
  import { m } from '../../paraglide/messages'

  const polite = $derived(toasts.list.filter((t) => t.kind !== 'error'))
  const assertive = $derived(toasts.list.filter((t) => t.kind === 'error'))
</script>

{#snippet item(toast: Toast)}
  <div
    class="toast toast--{toast.kind}"
    transition:fly={{ y: 24, duration: 180 }}
    onpointerdown={() => toasts.hold(toast.id)}
    onpointerup={() => toasts.release(toast.id)}
    onpointercancel={() => toasts.release(toast.id)}
    onfocusin={() => toasts.hold(toast.id)}
    onfocusout={() => toasts.release(toast.id)}
    role="presentation"
  >
    {#if toast.kind !== 'info'}
      <Icon name={toast.kind === 'success' ? 'success' : 'warning'} size={16} />
    {/if}
    <span class="flex-1 min-w-0">{toast.text}</span>
    {#if toast.action}
      <button
        type="button"
        class="toast__action"
        onclick={() => {
          toast.action?.run()
          toasts.dismiss(toast.id)
        }}
      >
        {toast.action.label}
      </button>
    {:else}
      <button
        type="button"
        class="toast__dismiss"
        aria-label={m.mobile_dismiss()}
        onclick={() => toasts.dismiss(toast.id)}
      >
        <Icon name="close" size={14} />
      </button>
    {/if}
  </div>
{/snippet}

<div class="toast-stack">
  <div class="toast-region" aria-live="assertive" aria-atomic="false">
    {#each assertive as toast (toast.id)}
      {@render item(toast)}
    {/each}
  </div>
  <div class="toast-region" aria-live="polite" aria-atomic="false">
    {#each polite as toast (toast.id)}
      {@render item(toast)}
    {/each}
  </div>
</div>

<style>
  .toast-stack {
    position: fixed;
    left: var(--s-4);
    right: var(--s-4);
    /* Clear of the floating tab bar, or of the reader's toolbar where
       the tab bar is hidden — whichever reaches higher. */
    bottom: calc(
      max(var(--bar-clearance, 0px), env(safe-area-inset-bottom) + 64px) + var(--s-2) +
        var(--keyboard-inset, 0px)
    );
    z-index: 70;
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
    pointer-events: none;
  }
  .toast-region {
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: var(--s-3);
    padding: var(--s-1) var(--s-1) var(--s-1) var(--s-4);
    min-height: 52px;
    border-radius: var(--r-xl);
    font-size: var(--t-callout);
    line-height: var(--t-callout-lh);
    /* Opaque, and no backdrop-filter. These sit above a list that is
       often mid-scroll, so a blurred backdrop is the expensive case —
       WebKit re-samples it every frame the content behind moves. At
       94% opacity over arbitrary content the blur was barely visible
       anyway; at 100% it is invisible and free. */
    /* The inverse of the page: ink as the ground, the card colour as
       the text. Contrast is symmetric, so the pair inherits the ink's
       guaranteed 7:1 in every theme and both modes — the hard-coded
       platform grey it replaces was the one colour in the app no theme
       could reach. In dark mode that makes a toast light, which is also
       what makes it read as something laid *over* the screen. */
    color: var(--ui-card);
    background: var(--ui-ink);
    box-shadow: var(--e-3);
  }
  /* The tinted kinds take their fill and label as a solved pair: white on
     a theme's pale success green was unreadable. */
  .toast--success {
    background: var(--ui-success-fill);
    color: var(--ui-on-success);
  }
  .toast--error {
    background: var(--ui-danger-fill);
    color: var(--ui-on-danger);
  }
  .toast__action {
    font-weight: 700;
    padding-inline: var(--s-3);
    min-height: 44px;
    min-width: 44px;
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  /* 44pt to hit, 24px to see. */
  .toast__dismiss {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    border-radius: var(--r-full);
    color: inherit;
    opacity: 0.85;
  }
</style>
