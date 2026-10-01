<script lang="ts">
  /**
   * The one search input in the app.
   *
   * Bound value plus an explicit `onchange`: the clear button
   * assigns `value` programmatically, which fires no native `input`
   * event — a caller that debounces on `oninput` alone would never
   * re-run its search when the field is cleared. Callers get told
   * either way through `onchange`.
   */
  import Icon from '../Icon.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    value: string
    placeholder?: string
    onchange?: (value: string) => void
    onsubmit?: (value: string) => void
    onfocus?: () => void
    autofocus?: boolean
  }
  let {
    value = $bindable(),
    placeholder = '',
    onchange,
    onsubmit,
    onfocus,
    autofocus = false,
  }: Props = $props()

  let input: HTMLInputElement | undefined = $state()

  // Focus in an effect rather than through the `autofocus`
  // attribute: on iOS the attribute is ignored anyway, and doing it
  // ourselves keeps the search screen's "open and type" behaviour
  // without the a11y warning that a page-load focus steal earns.
  $effect(() => {
    if (autofocus) input?.focus()
  })

  function set(next: string) {
    value = next
    onchange?.(next)
  }

  export function focus() {
    input?.focus()
  }
</script>

<div class="search-field" role="search">
  <span class="search-field__icon"><Icon name="search" size={18} /></span>
  <!-- A placeholder is not a label: it disappears on the first
       keystroke, and assistive technology is not required to read it
       (WCAG 3.3.2, 4.1.2). -->
  <input
    bind:this={input}
    class="search-field__input"
    aria-label={placeholder || m.mobile_search()}
    type="search"
    inputmode="search"
    enterkeyhint="search"
    autocapitalize="none"
    autocorrect="off"
    spellcheck="false"
    {placeholder}
    {value}
    oninput={(e) => set(e.currentTarget.value)}
    onfocus={() => onfocus?.()}
    onkeydown={(e) => {
      if (e.key === 'Enter') {
        e.currentTarget.blur()
        onsubmit?.(value)
      }
    }}
  />
  {#if value}
    <button
      type="button"
      class="search-field__clear"
      aria-label={m.mobile_clear_search()}
      onclick={() => {
        set('')
        input?.focus()
      }}
    >
      <span class="search-field__clear-dot"><Icon name="close" size={14} /></span>
    </button>
  {/if}
</div>

<style>
  .search-field {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    /* The same box as `.search-trigger`, which opens the screen this
       field lives on — tapping the trigger should feel like the field
       simply woke up. */
    min-height: 44px;
    padding-inline: var(--s-4) var(--s-2);
    border-radius: var(--r-full);
    background: var(--ui-fill-quiet);
    color: var(--ui-ink-muted);
  }
  .search-field__input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    outline: none;
    font-size: var(--t-input); /* never < 16px: iOS zooms the page on focus */
    color: var(--ui-ink);
  }
  .search-field__input::placeholder {
    color: var(--ui-ink-muted);
  }
  /* The field draws its own focus state on the pill, not a ring on the
     bare input inside it. */
  .search-field:focus-within {
    box-shadow: 0 0 0 2px var(--ui-accent-ink);
  }
  .search-field__input::-webkit-search-cancel-button {
    display: none;
  }
  /* A 44pt target around the 20px dot the design draws — the dot
     alone was a quarter of Apple's minimum. The negative margin keeps
     the pill at its drawn height. */
  .search-field__clear {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    margin: 0 calc(-1 * var(--s-3)) 0 calc(-1 * var(--s-2));
  }
  .search-field__clear-dot {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: var(--r-full);
    background: var(--ui-neutral-fill);
    color: var(--ui-on-neutral);
  }
</style>
