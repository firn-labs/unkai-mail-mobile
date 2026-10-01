<script lang="ts" module>
  /**
   * Every sheet currently open, oldest first.
   *
   * Escape and focus containment belong to the *topmost* sheet only. An
   * action sheet opened from inside another sheet used to close both on
   * one Escape, because each listened on the window independently.
   */
  const openSheets: symbol[] = []

  /** Whether any sheet is up — the shell's edge-swipe must not pop the
   *  screen out from under a dialog. */
  export function hasOpenSheet(): boolean {
    return openSheets.length > 0
  }
</script>

<script lang="ts">
  /**
   * A modal surface presented over the whole app.
   *
   * Two shapes, one component:
   *   * `full` — a page in its own right (Compose, the event
   *     editor). Slides up from the bottom, covers the tab bar,
   *     owns its own nav bar.
   *   * `sheet` — a partial-height card anchored to the bottom
   *     (action lists, pickers, confirmations). Sized by its
   *     content, capped at 85% of the viewport.
   *
   * Both dismiss on backdrop tap and on Escape (a hardware keyboard
   * is common on iPad and in the simulator). `full` sheets can opt
   * out with `dismissible={false}` so a half-written message can't
   * evaporate on a stray tap.
   *
   * ## A dialog, not just a box that looks like one
   *
   * What makes a modal usable without sight or without a touchscreen
   * (WCAG 2.4.3, 4.1.2), and what this component therefore does for
   * every caller:
   *
   *   * **Focus moves in** when it opens — otherwise VoiceOver stays on
   *     the button that opened it, behind the sheet, and the user has
   *     no idea anything appeared.
   *   * **Focus stays in.** Tab wraps around inside the sheet; the page
   *     underneath is `aria-modal`, which WebKit honours for VoiceOver.
   *   * **Focus goes back** to the control that opened it on close.
   *   * **There is always a way out that is not a gesture.** Dragging
   *     the grabber and tapping the backdrop are both pointer-only, so
   *     a dismissible card also carries a Close button — visually
   *     hidden, since sighted touch users have the grabber, and shown
   *     when a keyboard focuses it.
   */
  import type { Snippet } from 'svelte'
  import { tick } from 'svelte'
  import { fade, fly } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import { m } from '../../paraglide/messages'

  interface Props {
    kind?: 'full' | 'sheet'
    open?: boolean
    onclose?: () => void
    dismissible?: boolean
    /** Accessible name — required, since a sheet is a dialog. */
    label: string
    /** `alertdialog` for a confirmation that interrupts; `dialog` otherwise. */
    role?: 'dialog' | 'alertdialog'
    /** Id of the element whose text describes the dialog. */
    describedby?: string
    children: Snippet
  }

  let {
    kind = 'sheet',
    open = true,
    onclose,
    dismissible = true,
    label,
    role = 'dialog',
    describedby,
    children,
  }: Props = $props()

  const id = Symbol('sheet')
  let surface: HTMLDivElement | undefined = $state()

  function isTopmost(): boolean {
    return openSheets[openSheets.length - 1] === id
  }

  /* Register while open; move focus in, and hand it back on close. */
  $effect(() => {
    if (!open) return
    openSheets.push(id)
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null
    /** Kept locally: `bind:this` may already be cleared at teardown. */
    let node: HTMLElement | undefined
    void tick().then(() => {
      node = surface
      // Focus the dialog itself rather than its first control: landing
      // on "Delete" because it happens to come first is exactly what a
      // confirmation must not do. VoiceOver then reads the dialog's name.
      if (isTopmost()) node?.focus({ preventScroll: true })
    })
    return () => {
      const index = openSheets.indexOf(id)
      if (index !== -1) openSheets.splice(index, 1)
      // Hand focus back only if it is still ours to give — inside this
      // sheet, or dropped to <body> by the removal. Anywhere else, the
      // user or another sheet has already put it somewhere deliberate.
      const active = document.activeElement
      const ours = !active || active === document.body || !!node?.contains(active)
      if (opener?.isConnected && ours) opener.focus({ preventScroll: true })
    }
  })

  function requestClose() {
    if (dismissible) onclose?.()
  }

  const FOCUSABLE =
    'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'

  function onkeydown(e: KeyboardEvent) {
    if (!open || !isTopmost()) return
    if (e.key === 'Escape') {
      e.stopPropagation()
      requestClose()
      return
    }
    if (e.key === 'Tab' && surface) {
      const items = [...surface.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
        (el) => el.offsetParent !== null || el === document.activeElement,
      )
      if (items.length === 0) {
        e.preventDefault()
        surface.focus()
        return
      }
      const first = items[0]
      const last = items[items.length - 1]
      const active = document.activeElement
      if (e.shiftKey && (active === first || active === surface)) {
        e.preventDefault()
        last.focus()
      } else if (!e.shiftKey && active === last) {
        e.preventDefault()
        first.focus()
      } else if (!surface.contains(active)) {
        e.preventDefault()
        first.focus()
      }
    }
  }

  /* Drag-to-dismiss for partial sheets. Tracking the pointer by
     hand rather than leaning on a library keeps the interaction
     honest about one thing: a downward drag only dismisses if it
     went far enough OR fast enough, so a slow 20px nudge snaps
     back instead of closing under the user's thumb. */
  let dragY = $state(0)
  let dragging = $state(false)
  let startY = 0
  let startTime = 0

  function onPointerDown(e: PointerEvent) {
    if (kind !== 'sheet' || !dismissible) return
    dragging = true
    startY = e.clientY
    startTime = performance.now()
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return
    dragY = Math.max(0, e.clientY - startY)
  }

  function onPointerUp() {
    if (!dragging) return
    const elapsed = Math.max(1, performance.now() - startTime)
    const velocity = dragY / elapsed // px per ms
    dragging = false
    if (dragY > 120 || velocity > 0.5) {
      dragY = 0
      requestClose()
    } else {
      dragY = 0
    }
  }
</script>

<svelte:window {onkeydown} />

{#if open}
  <div class="sheet-root" role="presentation">
    <div
      class="sheet-backdrop"
      transition:fade={{ duration: 150 }}
      onclick={requestClose}
      role="presentation"
      aria-hidden="true"
    ></div>

    {#if kind === 'full'}
      <div
        class="sheet-full"
        bind:this={surface}
        {role}
        aria-modal="true"
        aria-label={label}
        aria-describedby={describedby}
        tabindex="-1"
        transition:fly={{ y: 400, duration: 260, easing: cubicOut }}
      >
        {@render children()}
      </div>
    {:else}
      <div
        class="sheet-card"
        bind:this={surface}
        {role}
        aria-modal="true"
        aria-label={label}
        aria-describedby={describedby}
        tabindex="-1"
        transition:fly={{ y: 300, duration: 220, easing: cubicOut }}
        style="transform: translateY({dragY}px); transition: {dragging
          ? 'none'
          : 'transform 180ms ease-out'}"
      >
        {#if dismissible}
          <div
            class="sheet-grabber-zone"
            onpointerdown={onPointerDown}
            onpointermove={onPointerMove}
            onpointerup={onPointerUp}
            onpointercancel={onPointerUp}
            role="presentation"
          >
            <div class="sheet-grabber"></div>
          </div>
          <button type="button" class="sheet-close" onclick={requestClose}>
            {m.mobile_close()}
          </button>
        {/if}
        <div class="sheet-body">
          {@render children()}
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .sheet-root {
    position: fixed;
    inset: 0;
    /* Above the keyboard, so a field in a sheet stays visible. */
    bottom: var(--keyboard-inset, 0px);
    z-index: 60;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
  }

  /* The theme's night, not pure black: a scrim tinted like the canvas
     reads as the same world dimmed rather than a hole cut in it. */
  .sheet-backdrop {
    position: absolute;
    inset: 0;
    background: color-mix(in oklab, var(--color-surface-950) 42%, transparent);
  }

  .sheet-full {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--color-surface-50);
  }

  :global([data-mode='dark']) .sheet-full {
    background: var(--color-surface-900);
  }

  /* The dialog takes focus programmatically so VoiceOver announces it;
     that is not a focus the user moved, so it draws no ring. */
  .sheet-full:focus,
  .sheet-card:focus {
    outline: none;
  }

  /* v3: the card floats — inset from the screen's edges and rounded on
     all four corners, sitting just above the home indicator like the
     tab bar and the reader's toolbar. Everything that is laid *over*
     the app shares that one shape, so a sheet reads as part of the
     same set of controls instead of a panel bolted to the bottom edge. */
  .sheet-card {
    position: relative;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    margin: 0 var(--s-2)
      max(var(--s-2), calc(env(safe-area-inset-bottom) - var(--s-3) - var(--keyboard-inset, 0px)));
    border-radius: var(--r-xl);
    background: var(--ui-card);
    --field-surface: var(--ui-card);
    padding-bottom: var(--s-2);
    box-shadow: var(--e-3);
    overflow: hidden;
  }

  .sheet-grabber-zone {
    padding: var(--s-2) 0 var(--s-1);
    display: flex;
    justify-content: center;
    touch-action: none;
    cursor: grab;
  }

  .sheet-grabber {
    width: 36px;
    height: 5px;
    border-radius: var(--r-full);
    background: var(--ui-control);
  }

  .sheet-close {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  .sheet-close:focus-visible {
    z-index: 1;
    top: var(--s-2);
    right: var(--s-3);
    width: auto;
    height: auto;
    min-height: 44px;
    margin: 0;
    padding: var(--s-2) var(--s-4);
    clip: auto;
    border-radius: var(--r-full);
    background: var(--ui-accent-fill);
    color: var(--ui-on-accent);
    font-size: var(--t-callout);
    font-weight: 600;
  }

  .sheet-body {
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    overscroll-behavior: contain;
  }
</style>
