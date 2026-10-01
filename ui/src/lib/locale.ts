/**
 * Which language the app speaks.
 *
 * By default the iPhone's: the device's preferred languages, in order,
 * against the ones the app ships. Settings → Language can pin one
 * instead. Before this module the app used Paraglide's `localStorage`
 * strategy, which *stored* whatever language it resolved on the very
 * first start — so changing the iPhone's language later changed
 * nothing, and there was no way to pick one either.
 *
 * A pinned language lives under its own key (`unkai.locale`) and is
 * written only by the language screen; a language worked out from the
 * device is never stored, so it keeps following the device.
 */
import { defineCustomClientStrategy, locales } from '../paraglide/runtime'

export const PINNED_LOCALE_KEY = 'unkai.locale'

/** What Paraglide's old strategy left behind on every install. */
const LEGACY_LOCALE_KEY = 'PARAGLIDE_LOCALE'

/**
 * Each language in its own name — the way a language list has to be
 * written, since someone looking for their language may not read the
 * one the app is in right now.
 */
export const LANGUAGE_NAMES: Record<string, string> = {
  en: 'English',
  de: 'Deutsch',
  es: 'Español',
  fr: 'Français',
  it: 'Italiano',
  pt: 'Português',
  ja: '日本語',
  zh: '简体中文',
  ko: '한국어',
  ru: 'Русский',
}

/**
 * The first of the device's languages the app has, or `undefined`.
 *
 * Matching is by language, not region — `de-AT` and `de-CH` are German,
 * `pt-PT` gets the (Brazilian-leaning) Portuguese — with one exception:
 * the app's Chinese is Simplified, so Traditional (`zh-Hant`, and
 * Taiwan, Hong Kong and Macao without a script) is passed over for the
 * next preference rather than shown a script its reader did not ask
 * for. Pure, so it is tested without a browser.
 */
export function negotiateLocale(
  preferred: readonly string[],
  available: readonly string[],
): string | undefined {
  for (const tag of preferred) {
    const [base, ...rest] = tag.toLowerCase().split(/[-_]/)
    if (base === 'zh') {
      const traditional =
        rest.includes('hant') ||
        (!rest.includes('hans') && rest.some((part) => ['tw', 'hk', 'mo'].includes(part)))
      if (!traditional && available.includes('zh')) return 'zh'
      continue
    }
    if (available.includes(base)) return base
  }
  return undefined
}

/** The language picked in Settings, or `null` to follow the device. */
export function pinnedLocale(): string | null {
  try {
    const pinned = localStorage.getItem(PINNED_LOCALE_KEY)
    return pinned && (locales as readonly string[]).includes(pinned) ? pinned : null
  } catch {
    return null
  }
}

let formatLocaleCache: string | undefined | null = null

/**
 * The locale for dates, times and sorting: the language picked in
 * Settings, or `undefined` — the device's own regional formats — when
 * the app follows the device. With a pinned language the dates have to
 * follow it, or a Spanish screen showed "Mi" and "23.09." from a German
 * phone. Read once: changing the pin reloads the page.
 */
export function formatLocale(): string | undefined {
  if (formatLocaleCache === null) {
    const pinned = pinnedLocale()
    formatLocaleCache = pinned === 'zh' ? 'zh-Hans' : (pinned ?? undefined)
  }
  return formatLocaleCache
}

/** The language the device asks for, among the app's. */
export function deviceLocale(): string | undefined {
  const preferred = navigator.languages?.length ? navigator.languages : [navigator.language]
  return negotiateLocale(preferred, locales)
}

/**
 * Register the strategy. Must run before the first message is
 * formatted — `main.ts` imports `localeSetup.ts` first for that.
 */
export function installLocaleStrategy(): void {
  try {
    localStorage.removeItem(LEGACY_LOCALE_KEY)
  } catch {
    // Nothing to clean up without storage.
  }
  defineCustomClientStrategy('custom-unkai', {
    getLocale: () => pinnedLocale() ?? deviceLocale(),
    // Paraglide reports every locale it resolves here, including the
    // one worked out from the device. Storing that is exactly the old
    // bug, so the pin is written by `chooseLocale` alone.
    setLocale: () => {},
  })
}

/**
 * Pin a language (or `null`: follow the device again) and restart the
 * interface in it. A reload, because every string on screen was
 * formatted in the old language — the same thing iOS does when the
 * system language changes.
 */
export function chooseLocale(locale: string | null): void {
  try {
    if (locale) localStorage.setItem(PINNED_LOCALE_KEY, locale)
    else localStorage.removeItem(PINNED_LOCALE_KEY)
  } catch {
    // Without storage the choice cannot outlive the reload.
    return
  }
  window.location.reload()
}
