/**
 * Whether the introduction has been seen — per variant.
 *
 * Two variants, because test mode and a real account need different
 * last words: one ends with how to leave test mode, the other with how
 * to connect calendars. Someone who explored test mode and then set up
 * a real account has not seen the second, so it is tracked separately.
 *
 * Stored in `localStorage` on purpose, not in the synced settings: it is
 * a per-device convenience, and losing it (a cleared web view store)
 * costs one replay of a skippable screen, nothing more. Every access is
 * guarded — storage can throw, and a failure to remember must never
 * stop the app from starting.
 */

export type IntroVariant = 'demo' | 'normal'

/** Bump to show a rewritten introduction once more to everyone. */
const VERSION = 1

function key(variant: IntroVariant): string {
  return `unkai.lab.intro.v${VERSION}.${variant}`
}

type Store = Pick<Storage, 'getItem' | 'setItem'>

function defaultStore(): Store | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage
  } catch {
    return null
  }
}

export function introSeen(variant: IntroVariant, store: Store | null = defaultStore()): boolean {
  try {
    return store?.getItem(key(variant)) === '1'
  } catch {
    // Unreadable storage: treat as seen rather than showing the intro on
    // every single launch.
    return true
  }
}

export function markIntroSeen(variant: IntroVariant, store: Store | null = defaultStore()): void {
  try {
    store?.setItem(key(variant), '1')
  } catch {
    // Nothing to do: at worst the intro shows once more.
  }
}

/**
 * Which introduction fits the accounts that exist.
 *
 * A real account wins over the demo: someone who added their own
 * mailbox while still in test mode is past "what is test mode" and
 * wants the connected-app tour. No accounts → no introduction (the
 * first-run screen is up).
 */
export function introVariantFor(accounts: readonly { demo?: boolean }[]): IntroVariant | null {
  if (accounts.some((a) => !a.demo)) return 'normal'
  if (accounts.some((a) => a.demo)) return 'demo'
  return null
}
