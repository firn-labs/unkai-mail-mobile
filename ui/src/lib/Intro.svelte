<script lang="ts">
  /**
   * The introduction: a few pages on where things are and how the app
   * is operated, shown once after the first sign-in — in test mode or
   * with a real account — and replayable from Settings → Introduction.
   *
   * # What it says, and why these pages
   *
   * Only what a new user cannot see: the gestures (a swipe, a long
   * press and an edge swipe leave no visible trace until tried), where
   * the things behind "More" are, and — per variant — how to leave test
   * mode or where to connect calendars. Every sentence names a real
   * control by its on-screen label; if a label or a gesture changes,
   * change the page in the same PR.
   *
   * # Operable without the gesture it teaches
   *
   * Pages advance by swiping *and* by buttons; the dialog is a `Sheet`
   * (focus moves in, Escape and Skip close it); each page change moves
   * focus to the new heading so VoiceOver reads it, and a live region
   * announces "Page 2 of 5". Animation is the app's own CSS, so Reduce
   * Motion removes it.
   */
  import { tick } from 'svelte'
  import { fly } from 'svelte/transition'
  import { cubicOut } from 'svelte/easing'
  import Icon, { type IconName } from './Icon.svelte'
  import Sheet from './ui/Sheet.svelte'
  import { markIntroSeen, type IntroVariant } from './introState'
  import { m } from '../paraglide/messages'

  interface Props {
    variant: IntroVariant
    onclose: () => void
    /** "Connect now" on the last page of the real-account tour. */
    onconnect?: () => void
  }

  let { variant, onclose, onconnect }: Props = $props()

  interface Page {
    icon: IconName
    title: string
    body: string
    action?: { label: string; run: () => void }
  }

  const pages = $derived<Page[]>([
    variant === 'demo'
      ? {
          icon: 'eye',
          title: m.mobile_intro_demo_welcome_title(),
          body: m.mobile_intro_demo_welcome_body(),
        }
      : {
          icon: 'encrypted',
          title: m.mobile_intro_normal_welcome_title(),
          body: m.mobile_intro_normal_welcome_body(),
        },
    { icon: 'more', title: m.mobile_intro_tabs_title(), body: m.mobile_intro_tabs_body() },
    { icon: 'archive', title: m.mobile_intro_swipe_title(), body: m.mobile_intro_swipe_body() },
    { icon: 'nav-backward', title: m.mobile_intro_nav_title(), body: m.mobile_intro_nav_body() },
    variant === 'demo'
      ? {
          icon: 'sign-out',
          title: m.mobile_intro_demo_end_title(),
          body: m.mobile_intro_demo_end_body(),
        }
      : {
          icon: 'cloud',
          title: m.mobile_intro_normal_end_title(),
          body: m.mobile_intro_normal_end_body(),
          action: onconnect
            ? {
                label: m.mobile_intro_normal_end_action(),
                run: () => {
                  // Read the prop *before* closing. Svelte 5 props are
                  // getters over the parent's expression, and closing
                  // changes the parent's state — a prop read after
                  // `finish()` can already see the closed state.
                  const connect = onconnect
                  finish()
                  connect?.()
                },
              }
            : undefined,
        },
  ])

  let index = $state(0)
  /** Which way the last page change went, for the slide direction. */
  let direction = $state<1 | -1>(1)
  let heading: HTMLHeadingElement | undefined = $state()

  const page = $derived(pages[index])
  const isLast = $derived(index === pages.length - 1)

  function finish() {
    markIntroSeen(variant)
    onclose()
  }

  async function go(next: number) {
    if (next < 0 || next >= pages.length || next === index) return
    direction = next > index ? 1 : -1
    index = next
    await tick()
    heading?.focus({ preventScroll: true })
  }

  /* Swipe between pages. A short horizontal flick is enough; the
     buttons are the non-gesture way, so this can stay forgiving. */
  let startX = 0
  let startY = 0
  let tracking: number | null = null

  function onPointerDown(e: PointerEvent) {
    tracking = e.pointerId
    startX = e.clientX
    startY = e.clientY
  }

  function onPointerUp(e: PointerEvent) {
    if (tracking !== e.pointerId) return
    tracking = null
    const dx = e.clientX - startX
    const dy = e.clientY - startY
    if (Math.abs(dx) < 48 || Math.abs(dx) < Math.abs(dy)) return
    void go(index + (dx < 0 ? 1 : -1))
  }
</script>

<Sheet kind="full" label={m.mobile_settings_intro()} onclose={finish}>
  <div class="intro">
    <div class="intro__top">
      <button type="button" class="bar-button bar-button--text" onclick={finish}>
        {m.mobile_intro_skip()}
      </button>
    </div>

    <div
      class="intro__stage"
      onpointerdown={onPointerDown}
      onpointerup={onPointerUp}
      onpointercancel={() => (tracking = null)}
      role="presentation"
    >
      {#key index}
        <section
          class="intro__page"
          aria-roledescription="slide"
          aria-label={m.mobile_intro_page({ n: index + 1, total: pages.length })}
          in:fly={{ x: 48 * direction, duration: 260, easing: cubicOut }}
        >
          <div class="intro__art" aria-hidden="true">
            <Icon name={page.icon} size={44} />
          </div>
          <h2 class="intro__title" tabindex="-1" bind:this={heading}>{page.title}</h2>
          <p class="intro__body">{page.body}</p>
          {#if page.action}
            <button type="button" class="wide-button wide-button--quiet intro__action" onclick={page.action.run}>
              {page.action.label}
            </button>
          {/if}
        </section>
      {/key}
    </div>

    <div class="intro__bottom">
      <div class="intro__dots" aria-hidden="true">
        {#each pages as _, i (i)}
          <span class="intro__dot" class:intro__dot--on={i === index}></span>
        {/each}
      </div>
      <!-- The dots are for the eye; this is the same fact for the ear. -->
      <span class="sr-only" aria-live="polite">
        {m.mobile_intro_page({ n: index + 1, total: pages.length })}
      </span>

      <div class="intro__nav">
        {#if index > 0}
          <button type="button" class="wide-button wide-button--quiet" onclick={() => void go(index - 1)}>
            {m.mobile_back()}
          </button>
        {/if}
        <button
          type="button"
          class="wide-button"
          onclick={() => (isLast ? finish() : void go(index + 1))}
        >
          {isLast ? m.mobile_intro_done() : m.mobile_intro_next()}
        </button>
      </div>
    </div>
  </div>
</Sheet>

<style>
  .intro {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding-top: env(safe-area-inset-top);
    padding-bottom: env(safe-area-inset-bottom);
    background: var(--ui-canvas);
  }

  .intro__top {
    display: flex;
    justify-content: flex-end;
    padding: var(--s-1) var(--s-2);
    flex-shrink: 0;
  }

  /* The page area scrolls on its own, so the largest text sizes never
     push the buttons off the screen. */
  .intro__stage {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    touch-action: pan-y;
  }

  /* Centred in the space between the bars while it fits; at the largest
     text sizes it grows past it and the stage scrolls instead. */
  .intro__page {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 100%;
    text-align: center;
    padding: var(--s-8) var(--s-6) var(--s-4);
  }

  .intro__art {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 96px;
    height: 96px;
    margin-bottom: var(--s-6);
    border-radius: var(--r-xl);
    background: color-mix(in oklab, var(--color-primary-500) 14%, var(--ui-card));
    color: var(--ui-accent-ink);
    box-shadow: var(--e-2);
  }

  .intro__title {
    font-size: var(--t-title);
    line-height: var(--t-title-lh);
    font-weight: 700;
    color: var(--ui-ink);
  }

  /* Focus lands here on every page change so VoiceOver reads it; that
     is not a focus the user moved, so it draws no ring. */
  .intro__title:focus {
    outline: none;
  }

  .intro__body {
    margin-top: var(--s-3);
    max-width: 30rem;
    font-size: var(--t-body);
    line-height: calc(var(--t-body-lh) * 1.1);
    color: var(--ui-ink-muted);
  }

  .intro__action {
    margin-top: var(--s-6);
    max-width: 20rem;
  }

  .intro__bottom {
    flex-shrink: 0;
    padding: var(--s-3) var(--s-4) var(--s-4);
  }

  .intro__dots {
    display: flex;
    justify-content: center;
    gap: var(--s-2);
    margin-bottom: var(--s-4);
  }

  .intro__dot {
    width: 8px;
    height: 8px;
    border-radius: var(--r-full);
    background: var(--ui-control);
    transition:
      width var(--m-base) var(--m-ease),
      background-color var(--m-base) var(--m-ease);
  }

  .intro__dot--on {
    width: 22px;
    background: var(--ui-accent-ink);
  }

  .intro__nav {
    display: flex;
    gap: var(--s-2);
  }
</style>
