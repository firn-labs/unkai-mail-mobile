<script lang="ts">
  /** Segmented control — the mobile stand-in for a row of tabs.
   *  Used where a screen has 2–3 mutually exclusive modes (calendar
   *  month/agenda, tasks open/done). More than three options
   *  belongs in a picker sheet: segments get unreadably narrow. */
  interface Props {
    options: { value: string; label: string }[]
    value: string
    onchange: (value: string) => void
  }
  let { options, value, onchange }: Props = $props()
</script>

<div class="segmented" role="tablist">
  {#each options as opt (opt.value)}
    <button
      type="button"
      role="tab"
      aria-selected={value === opt.value}
      class="segmented__item"
      class:segmented__item--active={value === opt.value}
      onclick={() => onchange(opt.value)}
    >
      {opt.label}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--r-sm);
    background: var(--ui-fill-quiet);
  }
  /* Both states use the primary ink. The unselected label used to be
     surface-700 on a tinted track, which several stock themes put under
     4.5:1; selection is carried by the raised chip and the weight, not
     by making the other options harder to read. The segment itself is
     32px to sit in a toolbar; its padding extends the target. */
  .segmented__item {
    flex: 1;
    min-height: 34px;
    padding: var(--s-1) var(--s-2);
    border-radius: calc(var(--r-sm) - 3px);
    font-size: var(--t-footnote);
    font-weight: 500;
    color: var(--ui-ink);
    transition:
      background-color var(--m-fast) var(--m-ease),
      box-shadow var(--m-fast) var(--m-ease);
  }
  /* The raised card colour is one of the backgrounds the inks are
     solved against, so the selected label's contrast is guaranteed. */
  .segmented__item--active {
    background: var(--ui-card-raised);
    font-weight: 650;
    /* The outline is the control ink, not a hairline: the selected
       state must stand out from the track at 3:1 (WCAG 1.4.11), and the
       raised chip alone does not reach that on a pale theme. */
    box-shadow:
      var(--e-1),
      inset 0 0 0 1px var(--ui-control);
  }
</style>
