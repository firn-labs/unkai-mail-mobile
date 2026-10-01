<script lang="ts">
  /**
   * One labelled control in a settings or editor form.
   *
   * Grouped-list layout: label on the left, control on the right,
   * hairline separators between rows inside a `.form-group`. That
   * shape is what makes a settings screen scannable on a phone —
   * stacked label-above-input cards double the vertical space and
   * halve how much fits on screen.
   */
  import type { Snippet } from 'svelte'

  interface Props {
    label: string
    hint?: string
    /** Stack the control under the label — for multi-line inputs and
     *  anything wider than about half the row. */
    stacked?: boolean
    children: Snippet
  }
  let { label, hint = '', stacked = false, children }: Props = $props()
</script>

<div class="form-row" class:form-row--stacked={stacked}>
  <div class="form-row__label">
    <span>{label}</span>
    {#if hint}<span class="form-row__hint">{hint}</span>{/if}
  </div>
  <div class="form-row__control">
    {@render children()}
  </div>
</div>

<style>
  .form-row {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    padding: 8px 16px;
  }
  .form-row--stacked {
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
  }
  .form-row__label {
    display: flex;
    flex-direction: column;
    font-size: var(--t-body);
    flex: 1;
    min-width: 0;
  }
  .form-row__hint {
    font-size: var(--t-caption);
    line-height: 1.35;
    margin-top: 2px;
    color: var(--ui-ink-muted);
  }
  .form-row--stacked .form-row__control {
    width: 100%;
  }
</style>
