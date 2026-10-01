<script lang="ts">
  // Reusable iOS-style switch.  Replaces native checkboxes
  // across Settings (#164 follow-up) for a lighter, more modern
  // look.  Smaller than the original Security toggle (h-5 / w-9
  // vs h-6 / w-11) so it sits comfortably on dense settings
  // rows without dominating the line.
  //
  // Use:
  //   <Toggle bind:checked={value} label="Enable feature" />
  //
  // Accessible: rendered as a real <button role="switch"> with
  // aria-checked, focusable, Space / Enter activates.
  //
  // Colours are the contrast-solved pair, not raw theme shades
  // (WCAG 1.4.11 — a switch's state has to be visible at 3:1):
  //   * on  — track in the accent fill, knob in the label colour
  //           solved against it, so the knob stays visible even on a
  //           pale accent where a white one disappeared;
  //   * off — track in `--ui-control` (3:1 against the card), knob in
  //           the card colour, so the knob is 3:1 against its track by
  //           construction.
  // v3: the visible switch is 46×28 (it was 36×20, a web checkbox's
  // size that read as a toy next to 16pt text); an invisible ring
  // extends the touch target to Apple's 44pt minimum. The knob travels
  // on the spring curve, so the state change has a little weight.

  interface Props {
    checked: boolean
    /** Optional aria label.  When omitted callers should
     *  associate the toggle with surrounding text via context. */
    label?: string
    disabled?: boolean
    onchange?: (checked: boolean) => void
    class?: string
  }
  let {
    checked = $bindable(),
    label = '',
    disabled = false,
    onchange,
    class: cls = '',
  }: Props = $props()

  function flip() {
    if (disabled) return
    checked = !checked
    onchange?.(checked)
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label || undefined}
  disabled={disabled || undefined}
  onclick={flip}
  class="toggle {cls}"
  class:toggle--on={checked}
>
  <span class="toggle__knob"></span>
</button>

<style>
  .toggle {
    position: relative;
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    width: 46px;
    height: 28px;
    border-radius: var(--r-full);
    background: var(--ui-control);
    transition: background-color var(--m-base) var(--m-ease);
  }

  /* The touch target: 44pt around the drawn switch. */
  .toggle::before {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    width: 54px;
    height: 44px;
    transform: translate(-50%, -50%);
  }

  .toggle--on {
    background: var(--ui-accent-fill);
  }

  .toggle:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .toggle__knob {
    display: inline-block;
    width: 22px;
    height: 22px;
    border-radius: var(--r-full);
    background: var(--ui-card);
    box-shadow: 0 1px 3px color-mix(in oklab, var(--color-surface-950) 30%, transparent);
    transform: translateX(3px);
    transition:
      transform var(--m-base) var(--m-spring),
      background-color var(--m-base) var(--m-ease);
  }

  .toggle--on .toggle__knob {
    background: var(--ui-on-accent);
    transform: translateX(21px);
  }
</style>
