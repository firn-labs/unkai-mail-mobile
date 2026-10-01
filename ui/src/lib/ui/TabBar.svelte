<script lang="ts">
  /**
   * The tab bar along the bottom — the app's spine.
   *
   * Mail is always first and More always last; between them sit the
   * features the user put there (Settings -> Features), defaulting
   * to Calendar, Tasks and Contacts — what a mail client's users
   * actually reach for mid-triage. Everything not on the bar stays
   * reachable through More, which is why the two fixed ends are
   * fixed: the bar can be rearranged, never made a dead end.
   */
  import Icon, { type IconName } from '../Icon.svelte'
  import { nav, type TabId } from '../nav.svelte'
  import { features, featureById } from '../features.svelte'
  import { m } from '../../paraglide/messages'

  interface Props {
    /** Unread mail across every account — badges the Mail tab. */
    unread?: number
    /** Queued outgoing mail, badged on More (where Outbox lives). */
    outbox?: number
  }

  let { unread = 0, outbox = 0 }: Props = $props()

  /**
   * The bar floats over the bottom of the screen (v3) instead of taking
   * its own strip, so everything that scrolls has to know how much of
   * the screen it covers. Measured rather than computed: the labels
   * follow Dynamic Type (up to the chrome cap) and the offset follows
   * the safe area, and a ResizeObserver catches both. Published on
   * `<html>` because toasts and sheets live outside the shell; removed
   * when the bar unmounts (the reader hides it), so screens without a
   * bar get their space back.
   */
  let bar: HTMLElement | undefined = $state()

  $effect(() => {
    if (!bar) return
    const el = bar
    const root = document.documentElement
    // From the bar's own box — its height plus its resolved `bottom`
    // offset — not from `innerHeight - top`. At a cold start the shell
    // can mount the bar before the webview's viewport has settled; the
    // old measure then caught a stale height, nothing re-measured it (the
    // bar itself never changed size), and the compose pill sat under the
    // bar until something remounted it.
    const publish = () => {
      const bottom = parseFloat(getComputedStyle(el).bottom) || 0
      root.style.setProperty('--bar-clearance', `${Math.round(el.offsetHeight + bottom)}px`)
    }
    publish()
    // The bar's size follows Dynamic Type; the page's size follows the
    // viewport (and with it the safe area the offset is made of).
    const observer = new ResizeObserver(publish)
    observer.observe(el)
    observer.observe(root)
    window.addEventListener('resize', publish)
    return () => {
      observer.disconnect()
      window.removeEventListener('resize', publish)
      root.style.removeProperty('--bar-clearance')
    }
  })

  const tabs = $derived<{ id: TabId; icon: IconName; label: () => string }[]>([
    { id: 'mail', icon: 'email-envelope', label: () => m.mobile_tab_mail() },
    ...features.tabs.flatMap((id) => {
      const feature = featureById(id)
      return feature ? [{ id: feature.id as TabId, icon: feature.icon, label: feature.label }] : []
    }),
    { id: 'more', icon: 'more', label: () => m.mobile_tab_more() },
  ])

  function badgeFor(id: TabId): number {
    if (id === 'mail') return unread
    if (id === 'more') return outbox
    return 0
  }

  /** Cap the number so a 4-digit unread count can't stretch the
   *  bubble across the tab. */
  function badgeText(n: number): string {
    return n > 99 ? '99+' : String(n)
  }
</script>

<nav class="tab-bar" aria-label={m.mobile_tab_bar_label()} bind:this={bar}>
  {#each tabs as tab (tab.id)}
    {@const active = nav.tab === tab.id}
    {@const badge = badgeFor(tab.id)}
    <button
      type="button"
      class="tab-bar__item"
      class:tab-bar__item--active={active}
      aria-current={active ? 'page' : undefined}
      onclick={() => nav.selectTab(tab.id)}
    >
      <span class="tab-bar__icon">
        <Icon name={tab.icon} size={24} />
        {#if badge > 0}
          <span class="tab-bar__badge" aria-hidden="true">{badgeText(badge)}</span>
        {/if}
      </span>
      <span class="tab-bar__label">{tab.label()}</span>
      {#if badge > 0}
        <span class="sr-only">{m.mobile_tab_badge_sr({ count: badge })}</span>
      {/if}
    </button>
  {/each}
</nav>

<style>
  /* v3: a floating capsule above the home indicator, the list scrolling
     beneath it. Two reasons beyond the look. The bar is the one piece of
     chrome on every screen, and as a full-width strip it read as part
     of the page; as an object it reads as the app's controls. And it
     frees the strip it used to occupy, which on a phone is two more
     rows of mail. Opaque rather than blurred, for the reason
     `.glass-panel` is: a live backdrop blur re-samples the screen on
     every frame of every scroll. */
  .tab-bar {
    position: absolute;
    left: var(--s-3);
    right: var(--s-3);
    /* Sits just above the home indicator, not a full safe area above
       it; on a phone without one (SE) it keeps a small margin. */
    bottom: max(var(--s-2), calc(env(safe-area-inset-bottom) - var(--s-3)));
    z-index: 20;
    display: flex;
    align-items: stretch;
    padding: var(--s-1);
    border-radius: var(--r-full);
    background: var(--ui-bar);
    box-shadow: var(--e-float);
  }

  .tab-bar__item {
    flex: 1;
    /* Without this a flex item refuses to shrink below its label's
       width, and the ellipsis below never gets the chance to apply. */
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1px;
    min-height: 52px;
    padding-block: var(--s-1);
    border-radius: var(--r-full);
    color: var(--ui-ink-muted);
    transition:
      color var(--m-base) var(--m-ease),
      background-color var(--m-base) var(--m-ease),
      transform var(--m-fast) var(--m-ease);
  }

  /* The current tab is a lozenge inside the capsule: a footprint, not
     only a colour, so it is findable at a glance and by anyone who does
     not separate the two hues well (WCAG 1.4.1). The label also goes
     bold. Quiet fill, accent ink: the tint pill behind the icon alone
     was a framework default. */
  .tab-bar__item--active {
    color: var(--ui-accent-ink);
    background: var(--ui-fill-quiet);
  }

  /* The press dips the whole item rather than fading it. Opacity on a
     tab bar reads as "disabled" for the instant it lasts; a scale
     reads as "pressed", which is what actually happened. */
  .tab-bar__item:active {
    transform: scale(0.94);
  }

  .tab-bar__icon {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 26px;
  }

  .tab-bar__item--active .tab-bar__label {
    font-weight: 700;
  }

  /* Five labels share the width; at the larger text sizes one ends in
     an ellipsis rather than touching its neighbour. The button keeps
     the full name for VoiceOver either way. */
  .tab-bar__label {
    max-width: 100%;
    padding-inline: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-bar-label);
    line-height: 1.15;
    font-weight: 550;
  }

  /* Unread mail is the signal colour — the same red as the unread mark
     on the row it counts. */
  .tab-bar__badge {
    position: absolute;
    top: -3px;
    left: 62%;
    min-width: calc(17px * var(--dt-chrome, 1));
    height: calc(17px * var(--dt-chrome, 1));
    padding-inline: 5px;
    border-radius: var(--r-full);
    /* A ring in the bar's own colour separates the bubble from the icon
       it overlaps. */
    box-shadow: 0 0 0 2px var(--ui-bar);
    background: var(--ui-now-fill);
    color: var(--ui-on-now);
    font-family: var(--font-display);
    font-size: var(--t-badge);
    font-weight: 750;
    line-height: calc(17px * var(--dt-chrome, 1));
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  /* On the active lozenge the ring takes the lozenge's colour instead. */
  .tab-bar__item--active .tab-bar__badge {
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--ui-bar) 87%, var(--color-surface-500));
  }
</style>
