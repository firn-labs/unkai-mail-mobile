<script lang="ts">
  /**
   * Pull-to-refresh around a scrolling region.
   *
   * The gesture every list in this app needs: on a phone there is
   * no room for a Refresh button in a toolbar, and hiding sync
   * behind a menu makes "did it check?" unanswerable.
   *
   * The component owns the scroller (rather than wrapping one)
   * because the pull may only start when the content is already at
   * the top — that check needs `scrollTop`, and asking every caller
   * to wire it up is how this ends up inconsistent.
   *
   * ## Two rules that keep the gesture from getting stuck
   *
   * A pull that starts and never ends leaves the list translated
   * down with no way back, and a refresh that never resolves leaves
   * the spinner turning forever while `refreshing` blocks any new
   * pull. Both were reachable here, and both look identical to the
   * user: "refreshing the mail always hangs". So:
   *
   *   * **The drag is pointer-captured.** Without capture, a finger
   *     that slides off the scroller mid-pull delivers its
   *     `pointerup` to whatever element it happens to be over — the
   *     nav bar, the tab bar — and this component never learns the
   *     gesture ended.
   *   * **The refresh has a deadline.** `onrefresh` reaches the
   *     network, and not every layer under it has a timeout of its
   *     own. Whatever happens, the indicator retracts and the
   *     gesture re-arms.
   *
   * ## Why the transform is written to the DOM by hand
   *
   * `pull` moved as reactive state, so each of the ~120 pointer
   * events in a drag re-rendered an interpolated `style` attribute —
   * meaning the browser re-parsed a full style declaration
   * (transform *and* transition) per event, on the main thread,
   * during a gesture. The visible result is a pull that stutters
   * behind the finger. Now the drag writes `transform` straight to
   * the node inside a `requestAnimationFrame`, so the work is capped
   * at once per displayed frame and touches nothing but the
   * compositor. `pull` stays a plain variable; only the states that
   * actually change the *markup* remain reactive.
   */
  import type { Snippet } from 'svelte'
  import Icon from '../Icon.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    onrefresh: () => Promise<void>
    children: Snippet
    /** Disable while a screen is in a state where refresh is
     *  meaningless (an empty account list, an error screen). */
    enabled?: boolean
    /** Extra classes for the scroll container. */
    class?: string
    /** Fires when the user scrolls near the bottom — the hook the
     *  mail list loads older messages from. */
    onnearbottom?: () => void
  }

  let { onrefresh, children, enabled = true, class: cls = '', onnearbottom }: Props = $props()

  const TRIGGER = 72
  const MAX_PULL = 120

  /** Below this the gesture is still ambiguous; past it the larger
   *  axis wins and the other one is ignored for the rest of the drag.
   *  Same rule `SwipeRow` uses, for the same reason: a diagonal
   *  flick must resolve to one gesture, not half of two. */
  const LOCK_THRESHOLD = 8

  /**
   * How long a refresh may run before the UI gives up on it.
   *
   * Generous on purpose — this is not a network timeout (the mail
   * protocols have their own, see
   * `crates/unkai-imap/src/timeout.rs`), it is the last line of
   * defence for a promise that never settles at all. Anything that
   * takes this long has failed in a way nobody is waiting through.
   */
  const REFRESH_DEADLINE_MS = 60_000

  let scroller: HTMLDivElement | undefined = $state()

  /**
   * Reactive only where it has to be.
   *
   * `refreshing` and `armed` decide what is *rendered* (which icon,
   * whether it spins), so they stay `$state`. The pull distance
   * drives a transform and a height, which are written to the DOM
   * directly — see the component comment.
   */
  let refreshing = $state(false)
  let armed = $state(false)

  let indicator: HTMLDivElement | undefined = $state()

  let pull = 0
  let dragging = false
  let startY = 0
  let startX = 0
  let axis: 'none' | 'y' | 'x' = 'none'
  let pointerId: number | null = null
  /** Set while a `requestAnimationFrame` is queued for the scroll
   *  handler, so a flick that fires scroll events faster than the
   *  display refreshes still only does the work once per frame. */
  let scrollFrame = 0
  /** Same trick for the drag: coalesce pointer moves to one paint. */
  let pullFrame = 0

  /** Push the current `pull` onto the two nodes it moves. */
  function paintPull() {
    pullFrame = 0
    if (scroller) {
      scroller.style.transform = `translate3d(0, ${pull}px, 0)`
      scroller.style.transition = dragging
        ? 'none'
        : 'transform 220ms cubic-bezier(0.2, 0.8, 0.2, 1)'
      scroller.classList.toggle('ptr__scroller--pulling', pull > 0)
    }
    if (indicator) {
      indicator.style.height = `${pull}px`
      indicator.style.opacity = `${Math.min(1, pull / TRIGGER)}`
    }
  }

  /** Set the pull distance, painting at most once per frame. */
  function setPull(next: number) {
    if (next === pull) return
    pull = next
    // `armed` flips the icon, so it has to stay reactive — but only
    // assign on an actual change, or every frame of a drag past the
    // trigger point would invalidate the template again.
    const nextArmed = pull >= TRIGGER
    if (nextArmed !== armed) armed = nextArmed
    if (!pullFrame) pullFrame = requestAnimationFrame(paintPull)
  }

  /** End the gesture and retract, whatever state it was in. */
  function releaseGesture() {
    dragging = false
    axis = 'none'
    // Clear the id *before* releasing capture: releasing fires
    // `lostpointercapture`, which routes back into `onPointerUp`, and
    // the null id is what makes that re-entry a no-op.
    const released = pointerId
    pointerId = null
    if (released !== null) {
      try {
        if (scroller?.hasPointerCapture(released)) scroller.releasePointerCapture(released)
      } catch {
        // Already released, or the pointer is gone — nothing to undo.
      }
    }
  }

  function onPointerDown(e: PointerEvent) {
    if (!enabled || refreshing) return
    if ((scroller?.scrollTop ?? 0) > 0) return
    pointerId = e.pointerId
    startY = e.clientY
    startX = e.clientX
    axis = 'none'
    dragging = true
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging || pointerId !== e.pointerId) return
    const dy = e.clientY - startY
    const dx = e.clientX - startX
    if (axis === 'none') {
      if (Math.abs(dy) < LOCK_THRESHOLD && Math.abs(dx) < LOCK_THRESHOLD) return
      axis = Math.abs(dy) >= Math.abs(dx) ? 'y' : 'x'
      if (axis === 'y' && dy > 0) {
        // Own the rest of the gesture. Without this a finger that
        // drifts off the scroller — onto the nav bar above it, which
        // is exactly where an over-pull ends up — delivers its
        // `pointerup` elsewhere, and the list stays translated down
        // with the spinner showing and no way to retract it.
        try {
          scroller?.setPointerCapture(e.pointerId)
        } catch {
          // Capture is an optimisation for the edge case; the common
          // path (finger stays over the list) works without it.
        }
      }
    }
    // A horizontal drag belongs to whatever row it started on.
    if (axis === 'x') {
      setPull(0)
      return
    }
    if (dy <= 0) {
      setPull(0)
      return
    }
    // Scrolled away from the top mid-gesture (momentum from a
    // previous flick) — abandon the pull rather than fighting it.
    if ((scroller?.scrollTop ?? 0) > 0) {
      setPull(0)
      releaseGesture()
      return
    }
    // Resistance curve: the further you pull, the less it moves, so
    // the trigger point is something you feel rather than overshoot.
    setPull(Math.min(MAX_PULL, dy * 0.5))
  }

  function onPointerUp(e: PointerEvent) {
    if (pointerId !== e.pointerId) return
    const shouldRefresh = armed
    releaseGesture()
    if (!shouldRefresh) {
      setPull(0)
      return
    }
    void runRefresh()
  }

  /**
   * The one place `onrefresh` is called.
   *
   * Guards re-entry (a second pull while one is running would start a
   * competing sync), and always retracts — a caller whose promise
   * rejects, or never settles at all, must still leave a list the
   * user can pull again.
   */
  async function runRefresh(): Promise<void> {
    if (refreshing) return
    refreshing = true
    setPull(TRIGGER * 0.7)
    let watchdog: ReturnType<typeof setTimeout> | undefined
    try {
      await new Promise<void>((resolve, reject) => {
        watchdog = setTimeout(
          () => reject(new Error('refresh timed out')),
          REFRESH_DEADLINE_MS,
        )
        onrefresh().then(resolve, reject)
      })
    } catch (e) {
      // Screens report their own failures through a toast; a refresh
      // that fell over is not this component's story to tell. All it
      // owes the user is a control that works on the next tap.
      console.warn('pull-to-refresh failed', e)
    } finally {
      if (watchdog !== undefined) clearTimeout(watchdog)
      refreshing = false
      setPull(0)
    }
  }

  function onScroll() {
    if (!onnearbottom || !scroller || scrollFrame) return
    // Reading `scrollHeight` forces a layout flush, and iOS emits
    // scroll events well above 60 Hz during a momentum flick — doing
    // that synchronously on every one of them is what makes a long
    // list stutter as it pages.
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = 0
      if (!scroller) return
      const { scrollTop, scrollHeight, clientHeight } = scroller
      if (scrollHeight - (scrollTop + clientHeight) < clientHeight * 0.6) onnearbottom?.()
    })
  }

  $effect(() => () => {
    if (scrollFrame) cancelAnimationFrame(scrollFrame)
    if (pullFrame) cancelAnimationFrame(pullFrame)
  })

  /** Let a screen kick the same refresh path a pull would (a toolbar
   *  Sync button, a retry after an error). */
  export async function refresh(): Promise<void> {
    await runRefresh()
  }

  export function scrollToTop(): void {
    scroller?.scrollTo({ top: 0, behavior: 'smooth' })
  }
</script>

<div class="ptr">
  <div class="ptr__indicator" bind:this={indicator} aria-hidden={!refreshing && !armed}>
    <span class="ptr__spinner" class:ptr__spinner--spinning={refreshing}>
      <Icon name={refreshing ? 'loading' : 'refresh'} size={20} />
    </span>
  </div>

  <div
    class="ptr__scroller {cls}"
    bind:this={scroller}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
    onlostpointercapture={onPointerUp}
    onscroll={onScroll}
    role="presentation"
  >
    <!-- Pulling is a path-based gesture that VoiceOver, Switch Control
         and a keyboard cannot make (WCAG 2.5.1). This is its twin:
         clipped out of sight, first in the list's reading order, and
         shown as a chip the moment a keyboard focuses it. -->
    {#if enabled}
      <button type="button" class="ptr__a11y-refresh" disabled={refreshing} onclick={() => void runRefresh()}>
        {m.mobile_refresh()}
      </button>
    {/if}
    {@render children()}
  </div>

  <!-- Always mounted, so the text change is what gets announced. -->
  <span class="sr-only" role="status">{refreshing ? m.mobile_refreshing() : ''}</span>
</div>

<style>
  .ptr {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .ptr__indicator {
    position: absolute;
    inset-inline: 0;
    top: 0;
    height: 0;
    opacity: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--ui-accent-ink);
    pointer-events: none;
    overflow: hidden;
  }

  .ptr__spinner--spinning {
    animation: ptr-spin 900ms linear infinite;
  }

  @keyframes ptr-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .ptr__scroller {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    overscroll-behavior-y: contain;
    /* Clear of the floating tab bar — see `.screen__body` in app.css. */
    padding-bottom: var(--bar-clearance, 0px);
    scroll-padding-bottom: var(--bar-clearance, 0px);
  }

  .ptr__scroller--pulling {
    will-change: transform;
  }

  .ptr__a11y-refresh {
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

  .ptr__a11y-refresh:focus-visible {
    position: relative;
    display: block;
    width: auto;
    height: auto;
    min-height: 44px;
    margin: var(--s-2) auto;
    padding: var(--s-2) var(--s-5);
    clip: auto;
    border-radius: var(--r-full);
    background: var(--ui-accent-fill);
    color: var(--ui-on-accent);
    font-size: var(--t-callout);
    font-weight: 600;
  }
</style>
