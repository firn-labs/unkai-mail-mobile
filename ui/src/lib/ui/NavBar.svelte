<script lang="ts">
  /**
   * The bar at the top of every screen: back affordance, title,
   * and up to a handful of actions.
   *
   * Deliberately one component rather than per-screen markup — the
   * title's truncation, the safe-area inset, and the 44pt tap
   * targets are the things a phone UI gets wrong when every screen
   * rolls its own header.
   *
   * `subtitle` carries the secondary line (the folder a message is
   * in, the account a list belongs to) so the title stays a short
   * noun the user can scan.
   */
  import type { Snippet } from 'svelte'
  import Icon from '../Icon.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    title: string
    subtitle?: string
    /** Label of the screen underneath, shown next to the chevron.
     *  Omitted → no back affordance. */
    backLabel?: string | null
    onback?: () => void
    /** Right-hand action buttons. */
    actions?: Snippet
    /** Left-hand slot replacing the back button (a Cancel button in
     *  a modal, an account switcher on a root screen). */
    leading?: Snippet
    /** A search field or segmented control below the title row. */
    below?: Snippet
    /** Progress hairline under the bar while a refresh is in flight. */
    busy?: boolean
    /**
     * Render a large, left-aligned title that collapses into the bar
     * as the screen scrolls.
     *
     * For **destinations** — a screen you go to and stay in, whose
     * title names a place worth stating properly. Tab roots, and
     * long-lived screens like Settings.
     *
     * Not for transient views: a message, a contact, an event editor.
     * There the title is a breadcrumb for something the user is
     * already looking at, and 30px of display type would shout the
     * obvious at someone who just tapped their way in.
     *
     * Also not for a title that is really a *control* — the calendar's
     * month sits between prev/next arrows, and collapsing it away
     * would strand them around an empty gap.
     */
    large?: boolean
    /**
     * Makes the subtitle a control: it opens a choice of what the screen
     * shows (the inbox's account). Drawn in the action ink with a
     * down-chevron, in both the large and the compact title.
     */
    onsubtitle?: () => void
    /** Accessible name of that control — the subtitle alone ("All
     *  accounts") does not say that tapping it switches. */
    subtitleLabel?: string
  }

  let {
    title,
    subtitle = '',
    backLabel = null,
    onback,
    actions,
    leading,
    below,
    busy = false,
    large = false,
    onsubtitle,
    subtitleLabel,
  }: Props = $props()

  /**
   * Collapse state for the large title.
   *
   * The bar hears its own screen's scrolling rather than taking the
   * offset as a prop. `.screen` is always `NavBar + scrolling body`, so
   * the scroller is a known sibling — and threading a scroll position
   * through all 32 screens to tell the header something it can see
   * for itself would be plumbing nobody maintains.
   */
  let collapsed = $state(false)
  let header: HTMLElement | undefined = $state()

  /**
   * Whether the back label fits its slot.
   *
   * The bar's side slots are about a quarter of the width each, and a
   * German screen name ("Einstellungen") does not fit next to the
   * chevron. Cut to "Einst…" it reads as a glitch, so — as the platform
   * does — a label that does not fit is replaced by plain "Back".
   *
   * Measured, not guessed from the character count: a hidden copy of
   * the full label is compared against the width actually available,
   * and a ResizeObserver repeats that when the text size changes.
   */
  let backBox: HTMLElement | undefined = $state()
  let backMeasure: HTMLElement | undefined = $state()
  let backFits = $state(true)

  $effect(() => {
    if (!backBox || !backMeasure) return
    const box = backBox
    const measure = backMeasure
    const check = () => {
      backFits = measure.offsetWidth <= box.clientWidth + 1
    }
    check()
    const observer = new ResizeObserver(check)
    observer.observe(box)
    observer.observe(measure)
    return () => observer.disconnect()
  })

  /** Past this many px the large title has travelled far enough that
   *  swapping to the compact one reads as a transition rather than a
   *  flicker. */
  const COLLAPSE_AT = 20

  $effect(() => {
    const screen = header?.parentElement
    if (!large || !screen) return

    // Listened for on the screen, in the capture phase, rather than on
    // a scroller looked up once: scroll events do not bubble, but they
    // do pass their ancestors on the way down. A lookup at mount missed
    // every list that renders a spinner first — the inbox among them —
    // so its title never collapsed.
    //
    // rAF-throttled: iOS emits scroll events well above the display
    // refresh rate during a flick, and this runs on every one of them.
    let frame = 0
    const onScroll = (event: Event) => {
      const scroller = event.target
      if (!(scroller instanceof HTMLElement) || !scroller.matches('.screen__body, .ptr__scroller')) {
        return
      }
      if (frame) return
      frame = requestAnimationFrame(() => {
        frame = 0
        const next = scroller.scrollTop > COLLAPSE_AT
        if (next !== collapsed) collapsed = next
      })
    }
    screen.addEventListener('scroll', onScroll, { capture: true, passive: true })
    return () => {
      screen.removeEventListener('scroll', onScroll, { capture: true })
      if (frame) cancelAnimationFrame(frame)
    }
  })
</script>

<header
  class="nav-bar"
  class:nav-bar--large={large}
  class:nav-bar--collapsed={collapsed}
  bind:this={header}
>
  <div class="nav-bar__row">
    <div class="nav-bar__side">
      <!-- `backLabel` wins over `leading`: a screen that supplies
           both (a list with a Cancel button in selection mode) sets
           `backLabel` to null exactly when its leading slot should
           take over, and testing `leading` first hid Back entirely. -->
      {#if backLabel !== null}
        <!-- `min-w-0` on both the button and its label is what keeps a
             long back label from pushing into the title: a flex item
             defaults to `min-width: auto`, so without it the button
             refuses to shrink below its text and simply overflows its
             third of the bar. The chevron never shrinks — it is the
             part that has to stay tappable. -->
        <button
          type="button"
          class="nav-bar__back"
          onclick={() => onback?.()}
          aria-label={backLabel ? m.mobile_back_to({ screen: backLabel }) : m.mobile_back()}
        >
          <span class="shrink-0"><Icon name="nav-backward" size={22} /></span>
          {#if backLabel}
            <span class="nav-bar__back-label" bind:this={backBox}>
              <span class="nav-bar__back-measure" aria-hidden="true" bind:this={backMeasure}
                >{backLabel}</span
              >
              {backFits ? backLabel : m.mobile_back()}
            </span>
          {/if}
        </button>
      {:else if leading}
        {@render leading()}
      {/if}
    </div>

    <!-- In large mode only one of the two titles is ever visible, and
         only that one may be in the accessibility tree: VoiceOver
         otherwise reads the screen's heading twice, once from an
         element at opacity 0. -->
    <div
      class="nav-bar__compact flex flex-col items-center justify-center min-w-0 flex-[2] shrink text-center px-1"
      aria-hidden={large && !collapsed ? 'true' : undefined}
    >
      <!-- A screen whose content opens with its own heading (a message's
           subject) passes an empty title; an empty <h1> would be read by
           VoiceOver as a heading with no name. -->
      {#if title}<h1 class="nav-bar__title truncate w-full">{title}</h1>{/if}
      {#if subtitle && onsubtitle}
        <button
          type="button"
          class="nav-bar__switch nav-bar__switch--compact"
          aria-label={subtitleLabel}
          aria-haspopup="dialog"
          onclick={() => onsubtitle?.()}
        >
          <span class="truncate">{subtitle}</span>
          <span class="nav-bar__switch-chevron" aria-hidden="true"><Icon name="nav-forward" size={12} /></span>
        </button>
      {:else if subtitle}
        <p class="nav-bar__subtitle truncate w-full">{subtitle}</p>
      {/if}
    </div>

    <div class="nav-bar__side nav-bar__side--end">
      {#if actions}
        {@render actions()}
      {/if}
    </div>
  </div>

  {#if large}
    <!-- The display title. It scales and fades rather than simply
         hiding, so the compact title in the bar above reads as the
         same object arriving somewhere else — the transition is what
         tells the user the header collapsed instead of the title
         being replaced. `transform-origin` keeps it pinned to its
         leading edge so it shrinks *towards* the bar title. -->
    <div class="nav-bar__large" aria-hidden={collapsed ? 'true' : undefined}>
      <h1 class="nav-bar__large-title">{title}</h1>
      {#if subtitle && onsubtitle}
        <!-- The switch sits where the subtitle would: it *is* the
             subtitle — what this screen shows — made tappable. -->
        <button
          type="button"
          class="nav-bar__switch"
          aria-label={subtitleLabel}
          aria-haspopup="dialog"
          onclick={() => onsubtitle?.()}
        >
          <span class="truncate">{subtitle}</span>
          <span class="nav-bar__switch-chevron" aria-hidden="true"><Icon name="nav-forward" size={14} /></span>
        </button>
      {:else if subtitle}
        <p class="nav-bar__large-subtitle">{subtitle}</p>
      {/if}
    </div>
  {/if}

  {#if below}
    <div class="nav-bar__below">
      {@render below()}
    </div>
  {/if}

  {#if busy}
    <!-- Indeterminate hairline: a spinner in the bar would fight the
         title for the eye, and a full-screen overlay would block a
         list the user can already read from cache. -->
    <div class="nav-bar__progress" aria-hidden="true"></div>
  {/if}
</header>

<style>
  .nav-bar {
    padding-top: env(safe-area-inset-top);
    position: relative;
    z-index: 20;
    flex-shrink: 0;
    background: var(--ui-canvas);
    /* No border by default. A hairline under every header is the
       reflex of a design that doesn't trust its own spacing — the
       recessed canvas already separates the bar from the cards below.
       It appears only when content actually scrolls under the bar,
       which is the moment the separation stops being obvious. */
    border-bottom: 1px solid transparent;
    transition: border-color var(--m-base) var(--m-ease);
  }

  .nav-bar--collapsed {
    border-bottom-color: var(--ui-hairline);
  }

  /* The bar's own row. Its inset puts a text button's first letter
     (which carries 8px of padding) on the gutter, so "Mailboxes" at the
     top left lines up with the title below it. */
  .nav-bar__row {
    display: flex;
    align-items: center;
    gap: var(--s-1);
    min-height: 44px;
    padding-inline: calc(var(--gutter) - var(--s-2));
  }

  /* The side slots share the width with the compact title, but never
     squeeze a short word ("Mailboxes", "Select") into an ellipsis to keep
     the title centred: the title is the one that can truncate. At large
     text sizes a slot stops at 40% and its own label ellipsises. */
  .nav-bar__side {
    display: flex;
    align-items: center;
    flex: 1 1 0;
    min-width: min-content;
    max-width: 40%;
  }

  .nav-bar__side--end {
    justify-content: flex-end;
    gap: 2px;
  }

  .nav-bar__below {
    padding: 0 var(--gutter) var(--s-3);
  }

  /* The button spans its slot, so the label box below knows how much
     room there really is (and the target grows with it). */
  .nav-bar__back {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    width: 100%;
    min-width: 0;
    min-height: 44px;
    /* The chevron hangs into the margin: the label starts near the
       gutter, the arrow sits before it. */
    margin-left: calc(-1 * var(--s-2));
    padding-inline: var(--s-1);
    font-size: var(--t-bar-body);
    color: var(--ui-accent-ink);
    border-radius: var(--r-sm);
    transition: opacity var(--m-fast) var(--m-ease);
  }

  .nav-bar__back-label {
    position: relative;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: left;
  }

  .nav-bar__back-measure {
    position: absolute;
    left: 0;
    top: 0;
    visibility: hidden;
    white-space: nowrap;
    pointer-events: none;
  }

  .nav-bar__back:active {
    opacity: 0.5;
  }

  .nav-bar__title {
    font-family: var(--font-display);
    font-size: var(--t-bar-title);
    line-height: 1.3;
    font-weight: 700;
    color: var(--ui-ink);
  }

  .nav-bar__subtitle {
    font-size: var(--t-bar-label);
    line-height: 1.3;
    color: var(--ui-ink-faint);
  }

  /* In large mode the compact title starts invisible and fades in as
     the display title leaves — the two are never both legible, so the
     eye only ever tracks one title. */
  .nav-bar--large .nav-bar__compact {
    opacity: 0;
    transform: translateY(4px);
    transition:
      opacity var(--m-base) var(--m-ease),
      transform var(--m-base) var(--m-ease);
  }

  .nav-bar--large.nav-bar--collapsed .nav-bar__compact {
    opacity: 1;
    transform: none;
  }

  .nav-bar__large {
    padding: var(--s-1) var(--gutter) var(--s-3);
    transform-origin: left top;
    transition:
      opacity var(--m-base) var(--m-ease),
      transform var(--m-base) var(--m-ease),
      max-height var(--m-base) var(--m-ease),
      padding-bottom var(--m-base) var(--m-ease);
    /* Room for a two-line title at the largest text size; the collapse
       animates this down to 0, so it must be a length, not `none`.
       100, not 90: the account switch under the inbox title is 28pt. */
    max-height: calc(100px * var(--dt-content, 1));
    overflow: hidden;
  }

  .nav-bar--collapsed .nav-bar__large {
    opacity: 0;
    /* Shrinks toward the bar title rather than just fading: the size
       change is what makes the two read as one object moving. */
    transform: scale(0.62) translateY(-8px);
    max-height: 0;
    padding-bottom: 0;
  }

  /* The one loud thing on a screen: the place you are, in the app's
     rounded display face, heavy and tightly set. Everything around it
     stays quiet so it can be. */
  .nav-bar__large-title {
    font-family: var(--font-display);
    font-size: var(--t-display);
    line-height: var(--t-display-lh);
    font-weight: 800;
    letter-spacing: -0.028em;
    color: var(--ui-ink);
    /* German titles are long single words ("Posteingang",
       "Einstellungen"); at the largest text sizes one no longer fits the
       width and must break — hyphenated by the document's language
       (`<html lang>` follows the locale) — rather than run off screen. */
    overflow-wrap: break-word;
    -webkit-hyphens: auto;
    hyphens: auto;
  }

  .nav-bar__large-subtitle {
    margin-top: 2px;
    font-size: var(--t-callout);
    line-height: var(--t-callout-lh);
    color: var(--ui-ink-muted);
  }

  /* The subtitle as a switch: action ink, a down-chevron, and a 44pt
     target that hugs the text (the visible part stays a line of type,
     not a button-shaped box — it is a value you can change). */
  .nav-bar__switch {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: var(--s-1);
    max-width: 100%;
    min-height: 28px;
    font-size: var(--t-callout);
    line-height: var(--t-callout-lh);
    font-weight: 600;
    color: var(--ui-accent-ink);
    border-radius: var(--r-sm);
    transition: opacity var(--m-fast) var(--m-ease);
  }

  .nav-bar__switch::before {
    content: '';
    position: absolute;
    inset: -6px -8px;
  }

  .nav-bar__switch:active {
    opacity: 0.5;
  }

  .nav-bar__switch--compact {
    min-height: 0;
    margin-top: 0;
    font-size: var(--t-bar-label);
    line-height: 1.3;
  }

  .nav-bar__switch-chevron {
    display: inline-flex;
    flex-shrink: 0;
    transform: rotate(90deg);
  }

  .nav-bar__progress {
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 2px;
    overflow: hidden;
    background: var(--ui-tint-accent);
  }

  .nav-bar__progress::after {
    content: '';
    position: absolute;
    inset-block: 0;
    width: 40%;
    background: var(--ui-accent-ink);
    animation: nav-progress 1.1s ease-in-out infinite;
  }

  @keyframes nav-progress {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(250%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .nav-bar__progress::after {
      animation-duration: 2.4s;
    }
  }
</style>
