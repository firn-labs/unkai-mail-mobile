<script lang="ts">
  /**
   * The "nothing here" screen.
   *
   * Always distinguishes an empty *collection* from an empty
   * *filter result* at the call site — the two need different words
   * ("No messages" vs "No results for 'invoice'"), and collapsing
   * them leaves the user unsure whether to clear the search or go
   * looking elsewhere.
   *
   * v3 drops the soft accent disc v2 drew behind the icon — the
   * icon-in-a-circle is the stock empty state of every template. What
   * keeps a blank screen from reading as broken is type, not a badge:
   * a large, quiet glyph, a title in the display face, one sentence
   * that says what to do, and the action to do it.
   */
  import Icon, { type IconName } from '../Icon.svelte'

  interface Props {
    icon?: IconName
    title: string
    body?: string
    action?: { label: string; run: () => void }
  }
  let { icon = 'info', title, body = '', action }: Props = $props()
</script>

<div class="empty-state">
  <span class="empty-state__glyph" aria-hidden="true"><Icon name={icon} size={40} /></span>
  <h2 class="empty-state__title">{title}</h2>
  {#if body}<p class="empty-state__body">{body}</p>{/if}
  {#if action}
    <!-- Not Skeleton's `preset-filled-primary-500`: its label colour is
         the theme's own contrast token, which measured under 3:1 on
         several stock themes. -->
    <button type="button" class="empty-state__action" onclick={action.run}>
      {action.label}
    </button>
  {/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--s-2);
    padding: var(--s-12) var(--s-8);
    text-align: center;
  }

  .empty-state__glyph {
    display: inline-flex;
    margin-bottom: var(--s-2);
    color: var(--ui-ink-faint);
  }

  .empty-state__title {
    font-family: var(--font-display);
    font-size: var(--t-title-2);
    line-height: var(--t-title-2-lh);
    font-weight: 750;
    letter-spacing: -0.01em;
    color: var(--ui-ink);
  }

  .empty-state__body {
    max-width: 20rem;
    font-size: var(--t-callout);
    line-height: var(--t-callout-lh);
    color: var(--ui-ink-muted);
  }

  .empty-state__action {
    min-height: 48px;
    margin-top: var(--s-4);
    padding: var(--s-2) var(--s-6);
    border-radius: var(--r-full);
    background: var(--ui-accent-fill);
    color: var(--ui-on-accent);
    font-size: var(--t-control);
    font-weight: 600;
    transition: transform var(--m-fast) var(--m-ease);
  }

  .empty-state__action:active {
    transform: scale(0.97);
  }
</style>
