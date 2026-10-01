<script lang="ts" module>
  import type { IconName } from '../Icon.svelte'

  export interface SheetAction {
    label: string
    icon?: IconName
    /** Renders in the destructive tint and, when it's the only
     *  destructive action, gets separated from the rest. */
    destructive?: boolean
    disabled?: boolean
    /** Secondary line under the label — the file a share points at,
     *  the folder a move targets. */
    detail?: string
    /** The current choice, when the sheet picks one of several (the
     *  inbox's account). Marked with a check and `aria-current`. */
    selected?: boolean
    run: () => void
  }
</script>

<script lang="ts">
  /**
   * The mobile answer to a right-click menu.
   *
   * Every row action that isn't the row's primary tap lands here:
   * the desktop build's `⋯` menu and context menu are the same
   * list, and on a phone that list belongs at the bottom of the
   * screen where a thumb reaches it — not anchored to the row,
   * which a finger is already covering.
   */
  import Icon from '../Icon.svelte'
  import Sheet from './Sheet.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    title?: string
    /** Secondary line under the title — usually what the actions
     *  will act on, so the user can confirm they hit the right row. */
    subtitle?: string
    actions: SheetAction[]
    onclose: () => void
  }

  let { title = '', subtitle = '', actions, onclose }: Props = $props()

  function pick(action: SheetAction) {
    if (action.disabled) return
    onclose()
    // Let the sheet's close transition start before the action runs;
    // an action that opens another sheet otherwise stacks two
    // dialogs on the same frame.
    setTimeout(action.run, 0)
  }
</script>

<Sheet label={title || m.mobile_actions_title()} {onclose}>
  {#if title}
    <div class="action-head">
      <p class="action-head__title">{title}</p>
      {#if subtitle}
        <p class="action-head__subtitle">{subtitle}</p>
      {/if}
    </div>
  {/if}

  <!-- One rounded group, the same shape as a settings card, so a sheet
       reads as a short list of choices rather than a menu of links. -->
  <ul class="action-group">
    <!-- Keyed by position, not label: two accounts or two folders can
         share a name, and a duplicate key is a runtime error. -->
    {#each actions as action, i (i)}
      <li>
        <button
          type="button"
          class="action-row"
          class:action-row--destructive={action.destructive}
          disabled={action.disabled}
          aria-current={action.selected ? 'true' : undefined}
          onclick={() => pick(action)}
        >
          {#if action.icon}
            <span class="action-row__icon"><Icon name={action.icon} size={20} /></span>
          {:else}
            <span class="action-row__icon"></span>
          {/if}
          <span class="flex-1 min-w-0">
            <span class="block truncate">{action.label}</span>
            {#if action.detail}
              <span class="action-row__detail">{action.detail}</span>
            {/if}
          </span>
          {#if action.selected}
            <span class="action-row__check"><Icon name="check" size={18} /></span>
          {/if}
        </button>
      </li>
    {/each}
  </ul>

  <div class="action-cancel">
    <button type="button" class="wide-button wide-button--quiet" onclick={onclose}>
      {m.mobile_cancel()}
    </button>
  </div>
</Sheet>

<style>
  .action-head {
    padding: var(--s-1) var(--s-6) var(--s-3);
    text-align: center;
  }

  .action-head__title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-display);
    font-size: var(--t-headline);
    line-height: var(--t-headline-lh);
    font-weight: 700;
    color: var(--ui-ink);
  }

  .action-head__subtitle {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-footnote);
    line-height: var(--t-footnote-lh);
    color: var(--ui-ink-muted);
  }

  .action-group {
    margin: 0 var(--s-3);
    border-radius: var(--r-lg);
    background: var(--ui-fill-quiet);
    overflow: hidden;
  }

  .action-group li + li {
    border-top: 1px solid var(--ui-hairline);
  }

  .action-row {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    width: 100%;
    min-height: 52px;
    padding: var(--s-2) var(--s-4);
    font-size: var(--t-control);
    text-align: left;
    color: var(--ui-ink);
  }

  .action-row:active {
    background: var(--ui-press);
  }

  .action-row:disabled {
    opacity: 0.4;
  }

  /* Muted, not accent: every row in a sheet is an action, so colouring
     every glyph with the action colour says nothing and adds a column
     of colour. The destructive row keeps its colour, because there the
     colour *is* the information. */
  .action-row__icon {
    display: inline-flex;
    width: 20px;
    flex-shrink: 0;
    color: var(--ui-ink-muted);
  }

  .action-row__detail {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-footnote);
    color: var(--ui-ink-muted);
  }

  /* The current choice: a check in the action ink, the one place a
     sheet uses the accent — here it marks state, not an action. */
  .action-row__check {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--ui-accent-ink);
  }

  .action-row--destructive,
  .action-row--destructive .action-row__icon {
    color: var(--ui-danger-ink);
  }

  .action-cancel {
    padding: var(--s-3) var(--s-3) var(--s-1);
  }
</style>
