<script lang="ts" module>
  import type { IconName } from '../Icon.svelte'

  export interface SwipeAction {
    label: string
    icon: IconName
    /** Tailwind-ish colour token name used for the reveal panel. */
    tone: 'primary' | 'success' | 'warning' | 'error' | 'surface'
    run: () => void
  }

  /**
   * The one row currently held open, app-wide.
   *
   * Touching any other row closes it, which is the behaviour every
   * phone list has: without it a fast triage session leaves a trail
   * of half-open rows behind the finger, and the next tap lands on
   * a row that has moved.
   */
  let openRow: { close: () => void } | null = null
</script>

<script lang="ts">
  /**
   * A list row with swipe actions on both edges.
   *
   * This is the triage gesture the whole mail list is built around:
   * swipe left to get a message out of the way, swipe right to
   * change its read state. Two behaviours make it feel native
   * rather than like a web page pretending:
   *
   *   * **Direction locking.** The first few pixels of a drag decide
   *     whether it's a horizontal swipe or a vertical scroll. Once
   *     locked vertical, the row stops tracking entirely — without
   *     this, a fast flick down the list drags rows sideways.
   *   * **Full-swipe commits.** Dragging past ~45% of the row runs
   *     the outermost action on release without waiting for a tap
   *     on the revealed button, which is what makes clearing an
   *     inbox fast.
   *
   * The row applies its action immediately and leaves undo to the
   * caller (a toast) — asking for confirmation on every swipe would
   * defeat the gesture.
   *
   * ## Not only for fingers
   *
   * A swipe is a path-based gesture, and VoiceOver, Switch Control and
   * a hardware keyboard cannot perform one (WCAG 2.5.1). So the row is
   * a *gesture layer*, never the only way in:
   *
   *   * **The caller's content carries the real controls.** Put the
   *     row's primary action in a `<button>` inside `children`. A tap
   *     that starts on a control is left to that control's own `click`,
   *     so it fires exactly once; a tap anywhere else in the row still
   *     reaches `onclick`.
   *   * **Every swipe action has a button twin**, visually hidden until
   *     it takes keyboard focus. With `onlongpress` there is one
   *     "More actions" button instead, since the long-press sheet
   *     already lists everything the swipes do.
   *   * **A click that a gesture produced is swallowed.** After a swipe,
   *     a long-press hand-off, or a tap that only closed an open row,
   *     WebKit can still deliver a `click` to the control under the
   *     finger — which used to open the message the user had just
   *     archived. The capture-phase handler drops it.
   */
  import type { Snippet } from 'svelte'
  import Icon from '../Icon.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    /** Revealed by swiping right-to-left (the destructive edge). */
    trailing?: SwipeAction[]
    /** Revealed by swiping left-to-right. */
    leading?: SwipeAction[]
    children: Snippet
    onclick?: () => void
    /** Long-press opens the row's full action list. */
    onlongpress?: () => void
    disabled?: boolean
  }

  let { trailing = [], leading = [], children, onclick, onlongpress, disabled = false }: Props =
    $props()

  const BUTTON_WIDTH = 76
  const LOCK_THRESHOLD = 8

  /**
   * How far a full swipe has to travel before releasing commits the
   * outermost action.
   *
   * It used to be 45% of the row — on a phone about 170px, which is only
   * ~18px past the 152px the two trailing buttons take to reveal. Any
   * brisk swipe meant to *show* Delete and Archive went past that, so a
   * fast swipe archived the message outright. Now the finger has to
   * cross most of the row, and never less than a thumb's width beyond
   * the revealed buttons; and the row shows the commit before it happens
   * (`armed`, below), so there is always a chance to drag back.
   */
  const FULL_SWIPE_RATIO = 0.65
  const FULL_SWIPE_MIN_BEYOND_REVEAL = 88

  /** Row width, read once when a gesture starts rather than per move. */
  let rowWidth = 320

  function commitDistance(revealWidth: number): number {
    return Math.max(rowWidth * FULL_SWIPE_RATIO, revealWidth + FULL_SWIPE_MIN_BEYOND_REVEAL)
  }

  /** Where the rubber band starts damping the drag. Always beyond the
   *  commit distance, or a single-action edge (76px wide) could never be
   *  dragged far enough to commit at all. */
  function elasticFrom(revealWidth: number): number {
    return Math.max(revealWidth * 1.9, commitDistance(revealWidth) + 24)
  }

  function rubberBand(dx: number, revealWidth: number): number {
    const max = elasticFrom(revealWidth)
    return Math.abs(dx) > max ? Math.sign(dx) * (max + (Math.abs(dx) - max) * 0.2) : dx
  }

  /** Which edge is past its commit distance right now. A class on the
   *  row, toggled only when this changes, stretches that edge's
   *  outermost action across the row — the "let go now and it happens"
   *  signal. Not `$state`: it is written from the paint callback. */
  let armed: 'none' | 'leading' | 'trailing' = 'none'

  /**
   * The drag offset is a **plain variable**, written straight to the
   * node's `transform` inside a `requestAnimationFrame`.
   *
   * It used to be `$state` interpolated into a `style` attribute,
   * which meant every pointer event of a swipe re-rendered that
   * attribute — the browser re-parsing a full style declaration
   * (transform *and* transition) per event, on the main thread,
   * mid-gesture. With a list of rows mounted that is the single
   * biggest source of stutter in the app, and it is entirely
   * avoidable: a transform written directly never touches style
   * resolution at all.
   *
   * What the *markup* depends on stays reactive: `open` (does the
   * row currently reveal its actions) and `direction` (which side).
   * Those change a handful of times per gesture, not 120.
   */
  let offset = 0
  let animating = true

  /** Whether the row is revealing actions, and on which edge. Drives
   *  the action panels' `aria-hidden` / focusability — nothing that
   *  changes per frame. */
  let open = $state<'none' | 'leading' | 'trailing'>('none')

  let rowEl: HTMLDivElement | undefined = $state()
  /** The layer the pointer handlers live on. Pointer capture has to
   *  be taken on *this* element and no other: capturing retargets
   *  every later event for that pointer to the capture element, so
   *  capturing on the wrapper silently cut the moves off from the
   *  handlers below and froze the row a few pixels in. */
  let contentEl: HTMLDivElement | undefined = $state()

  let pointerId: number | null = null
  let startX = 0
  let startY = 0
  let axis: 'none' | 'x' | 'y' = 'none'
  let longPressTimer: ReturnType<typeof setTimeout> | null = null
  let moved = false
  /** The long-press won this gesture: the sheet is up, so the rest
   *  of the drag belongs to it, not to the row. Without this the row
   *  keeps sliding open behind the sheet. */
  let handedOff = false
  /** The gesture began on a control inside the content (a button, a
   *  link), whose own `click` is the activation — see the header. */
  let startedOnControl = false
  /**
   * Until this `performance.now()` time, a `click` is the tail of a
   * gesture rather than a tap.
   *
   * A window and not a flag: WebKit does not always follow a swipe with
   * a click at all, and a flag left standing would then eat the next
   * *real* activation — an Enter pressed on this row a minute later. A
   * gesture's own click arrives within a frame or two of `pointerup`.
   */
  let swallowClicksUntil = 0
  const SWALLOW_WINDOW_MS = 400

  function swallowClick() {
    swallowClicksUntil = performance.now() + SWALLOW_WINDOW_MS
  }

  const CONTROL_SELECTOR = 'button, a[href], input, select, textarea, label, [role="button"]'

  const trailingWidth = $derived(trailing.length * BUTTON_WIDTH)
  const leadingWidth = $derived(leading.length * BUTTON_WIDTH)

  /** Queued frame for the transform write, so a burst of pointer
   *  moves costs one paint rather than one per event. */
  let frame = 0

  function paint() {
    frame = 0
    if (!contentEl) return
    contentEl.style.transform = `translate3d(${offset}px, 0, 0)`
    contentEl.style.transition = animating
      ? 'transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1)'
      : 'none'
    // `will-change` only for the length of the drag: leaving it on
    // every row of a long list would keep hundreds of compositor
    // layers alive and cost more than it saves.
    contentEl.classList.toggle('swipe-content--dragging', !animating)

    const revealWidth = offset < 0 ? trailingWidth : leadingWidth
    const next: typeof armed =
      !animating && revealWidth > 0 && Math.abs(offset) >= commitDistance(revealWidth)
        ? offset < 0
          ? 'trailing'
          : 'leading'
        : 'none'
    if (next !== armed) {
      armed = next
      rowEl?.classList.toggle('swipe-row--armed-trailing', next === 'trailing')
      rowEl?.classList.toggle('swipe-row--armed-leading', next === 'leading')
    }
  }

  /** How long the settle transition in `paint()` runs. The action
   *  panel has to outlive it — see `setOpen`. */
  const SETTLE_MS = 220

  let closeTimer: ReturnType<typeof setTimeout> | null = null

  /**
   * Mount the action panel eagerly, unmount it late.
   *
   * Revealing has to be immediate: the panel must already be in the
   * DOM by the time the content has slid far enough to expose it.
   * Hiding must not be, because the row closes with a 220ms
   * transition — dropping the panel the instant `offset` returns to
   * zero would leave a visible hole under the still-moving content.
   * So the close is deferred by the length of that animation.
   */
  function setOpen(next: 'none' | 'leading' | 'trailing') {
    if (closeTimer) {
      clearTimeout(closeTimer)
      closeTimer = null
    }
    if (next === open) return
    if (next !== 'none') {
      open = next
      return
    }
    if (!animating) {
      // Mid-drag back through centre: no transition to wait on.
      open = 'none'
      return
    }
    closeTimer = setTimeout(() => {
      closeTimer = null
      // Only if the row is still closed — a new swipe may have
      // started while this was pending.
      if (Math.abs(offset) < 1) open = 'none'
    }, SETTLE_MS)
  }

  /**
   * Move the row. `sync` forces the write to happen now rather than
   * next frame — used when the gesture ends, so the settle animation
   * starts from the offset the finger actually released at.
   */
  function setOffset(next: number, sync = false) {
    offset = next
    setOpen(Math.abs(next) < 1 ? 'none' : next > 0 ? 'leading' : 'trailing')
    if (sync) {
      if (frame) {
        cancelAnimationFrame(frame)
        frame = 0
      }
      paint()
      return
    }
    if (!frame) frame = requestAnimationFrame(paint)
  }

  $effect(() => () => {
    if (frame) cancelAnimationFrame(frame)
    if (closeTimer) clearTimeout(closeTimer)
  })

  function cancelLongPress() {
    if (longPressTimer) {
      clearTimeout(longPressTimer)
      longPressTimer = null
    }
  }

  /** Identity handed to the module-level `openRow` slot. */
  const self = {
    close: () => {
      animating = true
      setOffset(0)
    },
  }

  /** Take (or release) the single open-row slot. */
  function claimOpen(open: boolean) {
    if (open) openRow = self
    else if (openRow === self) openRow = null
  }

  $effect(() => () => claimOpen(false))

  function onPointerDown(e: PointerEvent) {
    if (disabled || e.pointerType === 'mouse' && e.button !== 0) return
    if (openRow && openRow !== self) {
      openRow.close()
      openRow = null
    }
    pointerId = e.pointerId
    startX = e.clientX
    startY = e.clientY
    axis = 'none'
    moved = false
    handedOff = false
    swallowClicksUntil = 0
    const control = (e.target as Element | null)?.closest(CONTROL_SELECTOR)
    startedOnControl = !!control && control !== contentEl && !!contentEl?.contains(control)
    rowWidth = rowEl?.offsetWidth || 320
    animating = false
    if (onlongpress) {
      longPressTimer = setTimeout(() => {
        longPressTimer = null
        // `offset` covers the case where moves arrived but the axis
        // lock hasn't flipped yet — a slow drag is still a drag.
        if (axis === 'x' || moved || offset !== 0) return
        // Reset any partial drag before handing over to the sheet so
        // the row isn't left half-open behind the modal.
        handedOff = true
        animating = true
        setOffset(0)
        onlongpress?.()
      }, 500)
    }
  }

  function onPointerMove(e: PointerEvent) {
    if (pointerId !== e.pointerId || handedOff) return
    const dx = e.clientX - startX
    const dy = e.clientY - startY
    if (Math.abs(dx) > 2 || Math.abs(dy) > 2) {
      moved = true
      cancelLongPress()
    }
    if (axis === 'none') {
      if (Math.abs(dx) < LOCK_THRESHOLD && Math.abs(dy) < LOCK_THRESHOLD) return
      axis = Math.abs(dx) > Math.abs(dy) ? 'x' : 'y'
      if (axis === 'x') {
        try {
          contentEl?.setPointerCapture(e.pointerId)
        } catch {
          // The pointer can already be gone (a cancelled touch); the
          // drag still works without capture, it just stops tracking
          // once the finger leaves the row.
        }
      }
    }
    if (axis !== 'x') return

    // The row owns this gesture now. Without this the PullToRefresh
    // wrapping the list also sees the moves and drags the scroller
    // down by whatever vertical drift the swipe had.
    e.stopPropagation()

    // Rubber-band past the revealed width so an over-swipe feels
    // elastic instead of stuck.
    const limit = dx < 0 ? trailingWidth : leadingWidth
    if (limit === 0) {
      setOffset(dx * 0.15)
      return
    }
    setOffset(rubberBand(dx, limit))
  }

  function onPointerUp(e: PointerEvent) {
    if (pointerId !== e.pointerId) return
    cancelLongPress()
    pointerId = null
    animating = true

    if (handedOff) {
      handedOff = false
      axis = 'none'
      swallowClick()
      setOffset(0)
      return
    }
    if (axis === 'x') swallowClick()

    // Settle the offset against the release point. Move events get
    // coalesced (and the last one before release can lag the finger
    // by a long way on a fast flick), so trusting only the moves
    // makes a full swipe read as a half swipe.
    if (axis === 'x') {
      const dx = e.clientX - startX
      const limit = dx < 0 ? trailingWidth : leadingWidth
      if (limit > 0) setOffset(rubberBand(dx, limit), true)
    }

    if (axis !== 'x') {
      const wasOpen = Math.abs(offset) > 1
      setOffset(0)
      claimOpen(false)
      // A tap that only closed an open row is not an activation.
      if (wasOpen) swallowClick()
      // A tap (no drag, nothing open) activates the row — unless it
      // landed on a control, which activates itself.
      if (!moved && !wasOpen && !disabled && !startedOnControl) onclick?.()
      axis = 'none'
      return
    }
    axis = 'none'

    if (offset < 0 && trailing.length > 0) {
      if (-offset >= commitDistance(trailingWidth)) {
        setOffset(0)
        claimOpen(false)
        trailing[trailing.length - 1].run()
        return
      }
      setOffset(-offset > trailingWidth / 2 ? -trailingWidth : 0)
      claimOpen(offset !== 0)
      return
    }
    if (offset > 0 && leading.length > 0) {
      if (offset >= commitDistance(leadingWidth)) {
        setOffset(0)
        claimOpen(false)
        leading[leading.length - 1].run()
        return
      }
      setOffset(offset > leadingWidth / 2 ? leadingWidth : 0)
      claimOpen(offset !== 0)
      return
    }
    setOffset(0)
    claimOpen(false)
  }

  function runAction(action: SwipeAction) {
    animating = true
    setOffset(0)
    claimOpen(false)
    action.run()
  }

  /** Capture phase, so it runs before any control inside the content. */
  function onClickCapture(e: MouseEvent) {
    if (performance.now() > swallowClicksUntil) return
    swallowClicksUntil = 0
    e.preventDefault()
    e.stopPropagation()
  }

  /** The keyboard's and the trackpad's way to the long-press sheet. */
  function onContextMenu(e: MouseEvent) {
    if (!onlongpress || disabled) return
    e.preventDefault()
    onlongpress()
  }

  function onKeyDown(e: KeyboardEvent) {
    if (!onlongpress || disabled) return
    if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
      e.preventDefault()
      onlongpress()
    }
  }

  /** The buttons that stand in for the gestures — see the header. */
  const accessibleActions = $derived<SwipeAction[]>(
    onlongpress
      ? [{ label: m.mobile_more_actions(), icon: 'more', tone: 'surface', run: onlongpress }]
      : [...leading, ...trailing],
  )

  export function close() {
    animating = true
    setOffset(0)
    claimOpen(false)
  }
</script>

<!--
  The action panels are mounted **only while the row is actually
  swiped**, which is what `open` is for.

  Every row in the mail list carries three of them (unread, delete,
  archive), each with an inline SVG icon and a label — so rendering
  them eagerly meant a 200-message inbox built ~600 buttons and 600
  SVGs that were, for all but one row at a time, invisible under the
  content layer. That is a large share of the DOM this app pays to
  create, style and keep alive while scrolling, in exchange for
  nothing until a finger arrives.

  They are cheap to mount on demand: `open` flips once per gesture,
  at the moment the row starts moving, and the panel underneath is
  in place long before the content has slid far enough to reveal it.
-->
<div class="swipe-row" bind:this={rowEl}>
  <!-- The reveal panels are for the finger that just swiped; assistive
       technology gets the always-present twins at the end instead, so
       these stay out of the accessibility tree and the focus order. -->
  {#if leading.length > 0 && open === 'leading'}
    <div class="swipe-actions swipe-actions--leading" aria-hidden="true">
      {#each leading as action (action.label)}
        <button
          type="button"
          tabindex="-1"
          class="swipe-action swipe-action--{action.tone}"
          style="width: {BUTTON_WIDTH}px"
          onclick={() => runAction(action)}
        >
          <Icon name={action.icon} size={20} />
          <span>{action.label}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if trailing.length > 0 && open === 'trailing'}
    <div class="swipe-actions swipe-actions--trailing" aria-hidden="true">
      {#each trailing as action (action.label)}
        <button
          type="button"
          tabindex="-1"
          class="swipe-action swipe-action--{action.tone}"
          style="width: {BUTTON_WIDTH}px"
          onclick={() => runAction(action)}
        >
          <Icon name={action.icon} size={20} />
          <span>{action.label}</span>
        </button>
      {/each}
    </div>
  {/if}

  <div
    class="swipe-content"
    bind:this={contentEl}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    onclickcapture={onClickCapture}
    oncontextmenu={onContextMenu}
    onkeydown={onKeyDown}
    role="presentation"
  >
    {@render children()}
  </div>

  {#if !disabled && accessibleActions.length > 0}
    <div class="swipe-a11y">
      {#each accessibleActions as action (action.label)}
        <button type="button" class="swipe-a11y__button" onclick={() => action.run()}>
          {action.label}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .swipe-row {
    position: relative;
    overflow: hidden;
    /* The row owns horizontal panning; the list keeps vertical. */
    touch-action: pan-y;
  }

  .swipe-actions {
    position: absolute;
    inset-block: 0;
    display: flex;
  }

  .swipe-actions--trailing {
    right: 0;
  }

  .swipe-actions--leading {
    left: 0;
  }

  .swipe-action {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--s-1);
    height: 100%;
    padding-inline: 4px;
    font-size: var(--t-bar-label);
    line-height: 1.2;
    font-weight: 600;
  }

  /* A long label ("Archivieren", a wide typeface, a larger text size)
     ends in an ellipsis inside its 76px panel instead of running into
     the neighbouring action. The full label is on the gesture twin. */
  .swipe-action span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .swipe-action:active {
    filter: brightness(0.9);
  }

  /* Fill and label are solved as a pair per theme. The labels used to
     be white on every tone, which measured 1.1-2:1 on the green and
     amber panels of most stock themes. */
  .swipe-action--primary {
    background: var(--ui-accent-fill);
    color: var(--ui-on-accent);
  }
  .swipe-action--success {
    background: var(--ui-success-fill);
    color: var(--ui-on-success);
  }
  .swipe-action--warning {
    background: var(--ui-warning-fill);
    color: var(--ui-on-warning);
  }
  .swipe-action--error {
    background: var(--ui-danger-fill);
    color: var(--ui-on-danger);
  }
  .swipe-action--surface {
    background: var(--ui-neutral-fill);
    color: var(--ui-on-neutral);
  }

  /* Past the commit distance: the outermost action takes the whole row
     and its label follows the content's edge, so the reveal reads as
     "this will happen" rather than "here are some buttons". The classes
     are toggled from the paint callback, hence :global. */
  .swipe-row:global(.swipe-row--armed-trailing) .swipe-actions--trailing,
  .swipe-row:global(.swipe-row--armed-leading) .swipe-actions--leading {
    left: 0;
    right: 0;
  }

  .swipe-row:global(.swipe-row--armed-trailing) .swipe-actions--trailing .swipe-action:not(:last-child),
  .swipe-row:global(.swipe-row--armed-leading) .swipe-actions--leading .swipe-action:not(:last-child) {
    display: none;
  }

  .swipe-row:global(.swipe-row--armed-trailing) .swipe-actions--trailing .swipe-action:last-child {
    flex: 1;
    align-items: flex-end;
    padding-right: var(--s-6);
  }

  .swipe-row:global(.swipe-row--armed-leading) .swipe-actions--leading .swipe-action:last-child {
    flex: 1;
    align-items: flex-start;
    padding-left: var(--s-6);
  }

  /* The gesture twins. Clipped out of sight like `.sr-only` so VoiceOver
     still reaches them, and revealed as a solid chip over the row's
     trailing edge the moment a keyboard focuses one — an invisible
     focused control is a trap for a sighted keyboard user (2.4.7). */
  .swipe-a11y__button {
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

  .swipe-a11y__button:focus-visible {
    z-index: 2;
    top: 50%;
    right: var(--s-3);
    width: auto;
    height: auto;
    min-height: 44px;
    margin: 0;
    padding: var(--s-2) var(--s-4);
    clip: auto;
    transform: translateY(-50%);
    border-radius: var(--r-full);
    background: var(--ui-accent-fill);
    color: var(--ui-on-accent);
    font-size: var(--t-callout);
    font-weight: 600;
  }

  /* The sliding layer must be opaque and must match the surface it sits
     on, or the action panel underneath shows through it. That surface is
     a card, unless a plain screen says otherwise through `--ui-row`. */
  .swipe-content {
    position: relative;
    background: var(--ui-row, var(--ui-card));
  }

  /* Toggled by `paint()` for the length of the drag only — see the
     comment there. */
  .swipe-content--dragging {
    will-change: transform;
  }

  /* `--ui-card` already carries the dark value; no override needed. */
</style>
