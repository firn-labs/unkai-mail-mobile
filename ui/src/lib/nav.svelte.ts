/**
 * Navigation: five tabs, each owning its own screen stack.
 *
 * This is the mobile replacement for the desktop shell's three-pane
 * layout. Where the desktop keeps folder list, message list and
 * reader on screen at once, a phone shows exactly one screen and
 * remembers how you got there — so navigation state is a stack per
 * tab, and switching tabs preserves each stack's position (the
 * behaviour every iOS tab bar has).
 *
 * Two rules keep this honest:
 *
 *   * **Screens are data, not components.** A stack entry is
 *     `{ kind, props }`; `App.svelte` maps `kind` to a component.
 *     That keeps the stack serialisable (so a future "restore where
 *     I was" is a JSON write) and stops screens from capturing each
 *     other's state in closures.
 *   * **Modals are not screens.** Compose, the event editor and the
 *     other full-height sheets present *over* the tab bar and are
 *     tracked separately in `App.svelte` — pushing them onto a tab
 *     stack would let a tab switch strand a half-written message.
 */

/**
 * Every tab that *can* exist. Which of them are actually on the bar
 * — and in what order — is the user's choice, held in
 * `features.svelte.ts`; this type is only the set of stacks the
 * navigator keeps.
 *
 * Mail and More are always present. The rest are features, and a
 * feature keeps its stack even while it isn't on the bar: putting
 * Files back on a tab should return the user to the folder they were
 * in, not to the root.
 */
export type TabId =
  | 'mail'
  | 'more'
  | 'calendar'
  | 'tasks'
  | 'contacts'
  | 'files'
  | 'talk'
  | 'notes'
  | 'shares'

/** Every screen the stacks can hold. The payload each one expects
 *  is documented on `ScreenProps` below. */
export type ScreenKind =
  | 'mail-inbox'
  | 'mail-home'
  | 'mail-list'
  | 'mail-message'
  | 'mail-search'
  | 'outbox'
  | 'calendar-home'
  | 'event-detail'
  | 'tasks-home'
  | 'contacts-home'
  | 'contact-detail'
  | 'more-home'
  | 'files'
  | 'talk'
  | 'notes'
  | 'note-detail'
  | 'shares'
  | 'settings'
  | 'settings-accounts'
  | 'settings-account'
  | 'settings-account-folders'
  | 'settings-appearance'
  | 'settings-language'
  | 'settings-theme-editor'
  | 'settings-notifications'
  | 'settings-mail'
  | 'settings-security'
  | 'settings-encryption'
  | 'settings-nextcloud'
  | 'settings-sources'
  | 'settings-features'
  | 'settings-privacy'
  | 'settings-about'

export interface Screen {
  kind: ScreenKind
  /** Props handed to the screen component. Deliberately loose: each
   *  screen validates what it needs, and the alternative (a discriminated
   *  union of 25 payload types) buys little for a private API. */
  props: Record<string, unknown>
  /** Monotonic id so `{#each}` keying survives pushing the same
   *  screen kind twice (message → same message from a search). */
  id: number
}

let nextId = 1

function screen(kind: ScreenKind, props: Record<string, unknown> = {}): Screen {
  return { kind, props, id: nextId++ }
}

/**
 * The root screen of each tab — never popped.
 *
 * Mail's root is the **inbox**, not the mailbox list. Reading new mail
 * is what nearly every session is for, and a folder list as the first
 * screen charged a tap for it every time. The mailboxes are one tap
 * away, through the inbox's leading "Mailboxes" button (`mail-home`).
 */
const ROOTS: Record<TabId, ScreenKind> = {
  mail: 'mail-inbox',
  calendar: 'calendar-home',
  tasks: 'tasks-home',
  contacts: 'contacts-home',
  files: 'files',
  talk: 'talk',
  notes: 'notes',
  shares: 'shares',
  more: 'more-home',
}

const TAB_IDS = Object.keys(ROOTS) as TabId[]

/** A fresh stack for every tab, each holding just its root. */
function initialStacks(): Record<TabId, Screen[]> {
  return Object.fromEntries(TAB_IDS.map((tab) => [tab, [screen(ROOTS[tab])]])) as Record<
    TabId,
    Screen[]
  >
}

class Navigation {
  /** Which tab is on screen. */
  tab = $state<TabId>('mail')

  /** One stack per tab; index 0 is always the tab's root screen. */
  stacks = $state<Record<TabId, Screen[]>>(initialStacks())

  /**
   * Which direction the last navigation moved, so the screen
   * transition can slide the right way. Read once per render by
   * `App.svelte`; not part of the stack's identity.
   */
  direction = $state<'forward' | 'back'>('forward')

  get stack(): Screen[] {
    return this.stacks[this.tab]
  }

  get current(): Screen {
    const s = this.stack
    return s[s.length - 1]
  }

  get canGoBack(): boolean {
    return this.stack.length > 1
  }

  /** Push a screen onto the active tab's stack. */
  push(kind: ScreenKind, props: Record<string, unknown> = {}): void {
    this.direction = 'forward'
    this.stacks[this.tab] = [...this.stack, screen(kind, props)]
  }

  /** Replace the top screen — used when a screen re-targets itself
   *  (opening a different message from a thread strip) and a second
   *  stack entry would make Back feel like a treadmill. */
  replace(kind: ScreenKind, props: Record<string, unknown> = {}): void {
    this.direction = 'forward'
    this.stacks[this.tab] = [...this.stack.slice(0, -1), screen(kind, props)]
  }

  pop(): void {
    if (!this.canGoBack) return
    this.direction = 'back'
    this.stacks[this.tab] = this.stack.slice(0, -1)
  }

  /** Drop everything above the tab's root. */
  popToRoot(): void {
    if (!this.canGoBack) return
    this.direction = 'back'
    this.stacks[this.tab] = [this.stack[0]]
  }

  /**
   * Switch tabs. Tapping the tab you're already on pops that tab to
   * its root — the standard iOS gesture, and the fastest way back to
   * the folder list from three screens deep.
   */
  selectTab(tab: TabId): void {
    if (this.tab === tab) {
      this.popToRoot()
      return
    }
    this.direction = 'forward'
    this.tab = tab
  }

  /** Jump straight to a screen in another tab (a notification tap
   *  landing on a message, "Open in Calendar" from a mail invite). */
  go(tab: TabId, kind: ScreenKind, props: Record<string, unknown> = {}): void {
    this.direction = 'forward'
    this.tab = tab
    this.stacks[tab] =
      kind === ROOTS[tab] ? [screen(kind, props)] : [screen(ROOTS[tab]), screen(kind, props)]
  }

  /** Reset every stack — after a profile wipe or an account teardown
   *  that invalidates the ids screens are holding. */
  resetAll(): void {
    this.direction = 'back'
    this.tab = 'mail'
    this.stacks = initialStacks()
  }
}

export const nav = new Navigation()
