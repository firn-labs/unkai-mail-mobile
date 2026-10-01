<script lang="ts">
  /** The screen's one primary create action, floating clear of the
   *  tab bar. One per screen — a second floating button turns the
   *  bottom-right corner into a guessing game.
   *
   *  An extended pill: icon *and* label ("New message", "New event").
   *  A bare pencil or plus is something an expert remembers; the word
   *  is something a first-time user can read. Every label in the app is
   *  a short noun phrase, which is what makes this fit. The accessible
   *  name is the same `label` (see the markup for why it is also set as
   *  `aria-label`). */
  import Icon, { type IconName } from '../Icon.svelte'

  interface Props {
    icon?: IconName
    label: string
    onclick: () => void
  }
  let { icon = 'plus', label, onclick }: Props = $props()

  /**
   * Tucks in while the list scrolls down, back out when it scrolls up.
   *
   * v3's tab bar floats too, so the bottom-right corner now holds two
   * objects over the list, and the pill sat on the last visible row's
   * date. Reading down a list is the moment the pill is least needed
   * and most in the way; scrolling back up — or reaching the top — is
   * the moment someone goes looking for it. The label is hidden by
   * width, not removed, so it stays the button's accessible name.
   *
   * It hears the scroll the way `NavBar` does — on its `.screen`, in
   * the capture phase — rather than through a prop on every screen.
   */
  let button: HTMLButtonElement | undefined = $state()
  let collapsed = $state(false)

  /** Travel in one direction before the pill changes shape, so a
   *  finger resting on the list does not make it flicker. */
  const HYSTERESIS = 24

  $effect(() => {
    const screen = button?.parentElement
    if (!screen) return
    let last = 0
    let travel = 0
    let frame = 0
    // Capture phase on the screen, as `NavBar` does: scroll does not
    // bubble, and the list may mount after the pill.
    const onScroll = (event: Event) => {
      const scroller = event.target
      if (!(scroller instanceof HTMLElement) || !scroller.matches('.screen__body, .ptr__scroller')) {
        return
      }
      if (frame) return
      frame = requestAnimationFrame(() => {
        frame = 0
        // Clamped to the real range: iOS rubber-bands past both ends,
        // and the bounce back from the bottom would otherwise read as
        // an upward scroll and bring the label back out.
        const max = scroller.scrollHeight - scroller.clientHeight
        const y = Math.min(Math.max(scroller.scrollTop, 0), max)
        const delta = y - last
        last = y
        // Direction changed: start counting afresh.
        travel = Math.sign(delta) === Math.sign(travel) ? travel + delta : delta
        const next = y < 40 ? false : travel > HYSTERESIS ? true : travel < -HYSTERESIS ? false : collapsed
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

<!-- `aria-label` repeats the visible label on purpose: while the pill
     is tucked in, the label is still in the DOM but zero-wide, and the
     button's name must not depend on how WebKit treats that. Same text,
     so the visible label stays the name (WCAG 2.5.3). -->
<button
  type="button"
  class="fab"
  class:fab--collapsed={collapsed}
  aria-label={label}
  {onclick}
  bind:this={button}
>
  <Icon name={icon} size={22} />
  <span class="fab__label">{label}</span>
</button>

<style>
  /* Above the floating tab bar, right-aligned with its edge. The shadow
     is neutral: v2 tinted it with the accent, a glow that made the one
     primary action look like an advertisement for itself. */
  .fab {
    position: absolute;
    right: var(--s-4);
    bottom: calc(var(--bar-clearance, env(safe-area-inset-bottom)) + var(--s-3));
    z-index: 15;
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    min-height: 52px;
    max-width: calc(100% - var(--s-8));
    padding: 0 var(--s-5) 0 var(--s-4);
    border-radius: var(--r-full);
    color: var(--ui-on-accent);
    background: var(--ui-accent-fill);
    box-shadow: var(--e-float);
    transition:
      transform var(--m-fast) var(--m-ease),
      padding var(--m-base) var(--m-ease),
      gap var(--m-base) var(--m-ease);
  }

  .fab:active {
    transform: scale(0.95);
  }

  /* Collapsed: a 52pt disc with the glyph centred. */
  .fab--collapsed {
    gap: 0;
    padding: 0 15px;
  }

  /* At the largest text sizes the label ends in an ellipsis rather than
     pushing the pill off the screen; the full text stays the name. */
  .fab__label {
    min-width: 0;
    max-width: 16rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-control);
    font-weight: 650;
    transition:
      max-width var(--m-base) var(--m-ease),
      opacity var(--m-fast) var(--m-ease);
  }

  .fab--collapsed .fab__label {
    max-width: 0;
    opacity: 0;
  }
</style>
