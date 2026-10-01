/**
 * Which features the app shows, and which of them get a tab.
 *
 * The desktop build has an icon rail with room for everything, so it
 * never had to ask. A phone has five tab slots and a More list, and
 * the right answer differs per person: someone who lives in Talk
 * wants it one thumb away and doesn't care about Tasks; someone
 * without a Nextcloud wants neither.
 *
 * So the tab bar is data, not markup:
 *
 *   * **Mail is always the first tab and More always the last.** Mail
 *     is what the app is; More is where everything not on a tab has
 *     to remain reachable, including Settings — a layout that can
 *     strand the user has no business being configurable.
 *   * **The three slots between them are the user's**, filled from
 *     [`FEATURES`] in the order they picked.
 *   * **Hiding a feature removes it everywhere** — off the tab bar
 *     and out of the More list.
 *
 * The choice lives in `AppSettings.mobile_features` rather than in
 * `localStorage` so it rides the Nextcloud settings bundle to the
 * user's next device.
 */

import type { MobileFeatures } from './api'
import type { IconName } from './Icon.svelte'
import type { ScreenKind } from './nav.svelte'
import { settingsStore } from './settingsStore.svelte'
import { m } from '../paraglide/messages'

/** Everything that can be shown, hidden, or put on a tab. */
export type FeatureId = 'calendar' | 'tasks' | 'contacts' | 'files' | 'talk' | 'notes' | 'shares'

export interface FeatureDef {
  id: FeatureId
  icon: IconName
  /** A function, not a string: `m.*()` reads the *current* locale, and
   *  a value captured at module load would freeze the app in whatever
   *  language it booted in. */
  label: () => string
  /** The screen this feature's tab (or More row) opens. */
  root: ScreenKind
  /** True for the surfaces that need a connected Nextcloud to show
   *  anything. Used to explain an empty screen rather than to hide
   *  the feature — a user who is about to connect one should still
   *  be able to find it. */
  needsNextcloud: boolean
}

/** The catalogue, in the order the settings screen lists it. */
export const FEATURES: FeatureDef[] = [
  {
    id: 'calendar',
    icon: 'calendar',
    label: () => m.mobile_tab_calendar(),
    root: 'calendar-home',
    needsNextcloud: false,
  },
  {
    id: 'tasks',
    icon: 'tasks',
    label: () => m.mobile_tab_tasks(),
    root: 'tasks-home',
    needsNextcloud: false,
  },
  {
    id: 'contacts',
    icon: 'contacts',
    label: () => m.mobile_tab_contacts(),
    root: 'contacts-home',
    needsNextcloud: false,
  },
  {
    id: 'files',
    icon: 'files',
    label: () => m.mobile_files(),
    root: 'files',
    needsNextcloud: true,
  },
  {
    id: 'talk',
    icon: 'meetings',
    label: () => m.mobile_talk(),
    root: 'talk',
    needsNextcloud: true,
  },
  {
    id: 'notes',
    icon: 'notes',
    label: () => m.mobile_notes(),
    root: 'notes',
    needsNextcloud: true,
  },
  {
    id: 'shares',
    icon: 'share-links',
    label: () => m.mobile_shares(),
    root: 'shares',
    needsNextcloud: true,
  },
]

const FEATURE_IDS = FEATURES.map((f) => f.id)

/** The stock arrangement: four tabs, Mail · Calendar · Contacts · More.
 *
 *  Only what works out of the box earns a default slot. Tasks used to
 *  have one, and without a connected Nextcloud it opened on an empty
 *  "connect a server" page — the first thing a new user tapped was a
 *  dead end. It lives under More now and can be put back on the bar in
 *  Settings → Features. Four tabs also give each label room to breathe. */
export const DEFAULT_TABS: FeatureId[] = ['calendar', 'contacts']

/**
 * How many features fit between Mail and More.
 *
 * Five tabs is the iOS maximum before the system would collapse the
 * rest behind its own "More" — and on a 390pt screen a sixth label
 * stops being readable well before that.
 */
export const MAX_TABS = 3

export function featureById(id: FeatureId): FeatureDef | undefined {
  return FEATURES.find((f) => f.id === id)
}

function isFeatureId(value: string): value is FeatureId {
  return (FEATURE_IDS as string[]).includes(value)
}

/**
 * The stored layout, typed.
 *
 * `AppSettings` is still an `any` alias, so without this cast every
 * field read off it infers as `any` and quietly disables type
 * checking on the filters below — exactly where a typo would cost
 * the user their arrangement.
 */
function stored(): MobileFeatures | undefined {
  return settingsStore.value?.mobile_features as MobileFeatures | undefined
}

class FeatureLayout {
  /** Ids the user switched off. Unknown ids in the stored list are
   *  dropped rather than trusted: a settings bundle from a newer
   *  build may name a feature this one doesn't have. */
  readonly hidden = $derived<FeatureId[]>((stored()?.hidden ?? []).filter(isFeatureId))

  /**
   * The middle tab slots, in order.
   *
   * `null` in settings means "never configured" and falls back to the
   * default; an empty array is the user's own answer and is honoured
   * — someone who wants only Mail and More gets exactly that.
   */
  readonly tabs = $derived<FeatureId[]>(
    (() => {
      const configured = stored()?.tabs
      const chosen: FeatureId[] =
        configured == null ? DEFAULT_TABS : configured.filter(isFeatureId)
      return chosen.filter((id) => !this.hidden.includes(id)).slice(0, MAX_TABS)
    })(),
  )

  /** Visible features that didn't get a tab — the More list. */
  readonly inMore = $derived<FeatureDef[]>(
    FEATURES.filter((f) => !this.hidden.includes(f.id) && !this.tabs.includes(f.id)),
  )

  isHidden(id: FeatureId): boolean {
    return this.hidden.includes(id)
  }

  isOnTabBar(id: FeatureId): boolean {
    return this.tabs.includes(id)
  }

  /**
   * Show or hide a feature.
   *
   * Hiding one that held a tab slot frees the slot rather than
   * leaving a gap — the tab bar should never render a hole.
   */
  async setHidden(id: FeatureId, hidden: boolean): Promise<void> {
    const nextHidden = hidden
      ? [...new Set([...this.hidden, id])]
      : this.hidden.filter((f) => f !== id)
    const nextTabs = hidden ? this.tabs.filter((f) => f !== id) : this.tabs
    await this.persist(nextHidden, nextTabs)
  }

  /**
   * Put a feature on the tab bar, or take it off.
   *
   * Returns `false` when the bar is already full: the caller shows a
   * toast rather than silently evicting whatever the user picked
   * first, which would feel like the app arguing with them.
   */
  async setOnTabBar(id: FeatureId, onBar: boolean): Promise<boolean> {
    if (onBar && this.tabs.length >= MAX_TABS) return false
    const nextTabs = onBar ? [...this.tabs, id] : this.tabs.filter((f) => f !== id)
    await this.persist(this.hidden, nextTabs)
    return true
  }

  /** Move a tab one slot left or right. */
  async moveTab(id: FeatureId, delta: -1 | 1): Promise<void> {
    const from = this.tabs.indexOf(id)
    const to = from + delta
    if (from < 0 || to < 0 || to >= this.tabs.length) return
    const nextTabs = [...this.tabs]
    ;[nextTabs[from], nextTabs[to]] = [nextTabs[to], nextTabs[from]]
    await this.persist(this.hidden, nextTabs)
  }

  /** Back to the arrangement the app ships with. */
  async reset(): Promise<void> {
    await this.persist([], DEFAULT_TABS)
  }

  private async persist(hidden: FeatureId[], tabs: FeatureId[]): Promise<void> {
    await settingsStore.patch({ mobile_features: { hidden, tabs } })
  }
}

export const features = new FeatureLayout()
