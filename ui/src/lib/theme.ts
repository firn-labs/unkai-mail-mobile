/**
 * Theme application helper.
 *
 * The Rust `AppSettings` struct stores two preferences:
 *   - `theme_name` — which Skeleton theme to use (cerberus, pine, …)
 *     OR a user-imported theme id (#132 tier 2).
 *   - `theme_mode` — `"system" | "light" | "dark"`
 *
 * This module turns those into the `data-theme` and `data-mode`
 * attributes on `<html>` that Skeleton's CSS variables and the
 * Tailwind `dark:` variant (overridden in `app.css`) react to.
 *
 * `system` mode means "follow the OS". We don't hand that string to
 * the CSS — instead this module reads `prefers-color-scheme` and sets
 * `data-mode="light"` or `"dark"` itself, then keeps doing so as the
 * OS preference changes. That way the CSS only ever has to look at a
 * concrete `light` / `dark` value and never branches on "system".
 *
 * Issue #132 adds tier-2 imported themes: a CSS file the user picked
 * via the Settings → Design "Import theme…" flow.  We inject the
 * file via a runtime `<link rel="stylesheet">` whose href swaps to
 * the just-picked theme on every theme switch — Skeleton's variables
 * match against `data-theme` regardless of where they're loaded from,
 * so the rest of the app works unchanged.
 */

import * as api from './api'
import { CUSTOM_THEME_ID, buildThemeCss, type CustomThemeInput } from './customTheme'
import { m } from '../paraglide/messages'

export type ThemeMode = 'system' | 'light' | 'dark'

export interface ThemeOption {
  /** Skeleton theme slug for stock themes — matches the file name
      under `@skeletonlabs/skeleton/themes/<slug>.css`.  For custom
      themes (#132) this is the slug declared in the imported CSS's
      `[data-theme="…"]` selector. */
  id: string
  /** Human-readable label for the picker. */
  label: string
  /** One-line description shown next to the label so the user knows
      what they're picking without trying it first. */
  description: string
  /** True for tier-2 user-imported themes — drives the "Custom"
      tag and Remove button in the picker.  Stock themes leave this
      undefined. */
  custom?: boolean
}

/**
 * Stock Skeleton themes available in every build (#132 tier 1), led by
 * the house theme `unkai` (`src/themes/unkai.css`), which is the default.
 * Adding more is four steps:
 *   1. Add an `@import` for the theme in `app.css`.
 *   2. Add an entry here, and its `mobile_theme_desc_<id>` message in
 *      both `messages/en.json` and `messages/de.json`.
 *   3. Regenerate `accessible-colors.css`: `npm --prefix ui test -- -u`
 *      (the contrast test fails until you do).
 *   4. Check the new theme's row in `accessible-colors.css` looks sane.
 *
 * Names are proper nouns and stay untranslated; descriptions are getters
 * so they read the active locale.
 */
export const STOCK_THEMES: ThemeOption[] = [
  { id: 'unkai', label: 'Unkai', get description() { return m.mobile_theme_desc_unkai() } },
  { id: 'cerberus', label: 'Cerberus', get description() { return m.mobile_theme_desc_cerberus() } },
  { id: 'modern', label: 'Modern', get description() { return m.mobile_theme_desc_modern() } },
  { id: 'pine', label: 'Pine', get description() { return m.mobile_theme_desc_pine() } },
  { id: 'rose', label: 'Rose', get description() { return m.mobile_theme_desc_rose() } },
  { id: 'vintage', label: 'Vintage', get description() { return m.mobile_theme_desc_vintage() } },
  { id: 'catppuccin', label: 'Catppuccin', get description() { return m.mobile_theme_desc_catppuccin() } },
  { id: 'concord', label: 'Concord', get description() { return m.mobile_theme_desc_concord() } },
  { id: 'crimson', label: 'Crimson', get description() { return m.mobile_theme_desc_crimson() } },
  { id: 'fennec', label: 'Fennec', get description() { return m.mobile_theme_desc_fennec() } },
  { id: 'hamlindigo', label: 'Hamlindigo', get description() { return m.mobile_theme_desc_hamlindigo() } },
  { id: 'legacy', label: 'Legacy', get description() { return m.mobile_theme_desc_legacy() } },
  { id: 'mint', label: 'Mint', get description() { return m.mobile_theme_desc_mint() } },
  { id: 'mona', label: 'Mona', get description() { return m.mobile_theme_desc_mona() } },
  { id: 'nosh', label: 'Nosh', get description() { return m.mobile_theme_desc_nosh() } },
  { id: 'nouveau', label: 'Nouveau', get description() { return m.mobile_theme_desc_nouveau() } },
  { id: 'reign', label: 'Reign', get description() { return m.mobile_theme_desc_reign() } },
  { id: 'rocket', label: 'Rocket', get description() { return m.mobile_theme_desc_rocket() } },
  { id: 'sahara', label: 'Sahara', get description() { return m.mobile_theme_desc_sahara() } },
  { id: 'seafoam', label: 'Seafoam', get description() { return m.mobile_theme_desc_seafoam() } },
  { id: 'terminus', label: 'Terminus', get description() { return m.mobile_theme_desc_terminus() } },
  { id: 'vox', label: 'Vox', get description() { return m.mobile_theme_desc_vox() } },
  { id: 'wintry', label: 'Wintry', get description() { return m.mobile_theme_desc_wintry() } },
]

/** Live list of imported themes — refreshed by App.svelte after
 *  every `import_custom_theme` / `remove_custom_theme` IPC. */
let customThemes: ThemeOption[] = []

/** Replace the runtime catalogue of imported themes.  Caller is
 *  expected to have already pushed each theme's path through
 *  `registerCustomThemePath` so a subsequent `applyTheme` call can
 *  load the right CSS file. */
export function setCustomThemes(list: ThemeOption[]): void {
  customThemes = list.map((t) => ({ ...t, custom: true }))
}

/** All themes the picker should render — stock first, then any
 *  user-imported ones flagged with `custom: true`. */
export function listThemes(): ThemeOption[] {
  return [CUSTOM_THEME, ...STOCK_THEMES, ...customThemes]
}

/** Back-compat shim — older callers read `THEMES` directly.  Proxy
 *  resolves to the live `listThemes()` snapshot on every access so
 *  imports don't need to refactor. */
export const THEMES = new Proxy([] as ThemeOption[], {
  get(_target, prop, _receiver) {
    const live = listThemes()
    return Reflect.get(live, prop, live)
  },
})

/**
 * The user-built theme.
 *
 * Always present in the picker, unlike the tier-2 imported themes: it
 * needs no file and no import step, so there is never a state where the
 * entry should be hidden. Picking it switches
 * `<html data-theme="custom">`; the colours behind it come from
 * `AppSettings.custom_theme_*` and are compiled to CSS by
 * `customTheme.ts`.
 */
export const CUSTOM_THEME: ThemeOption = {
  id: CUSTOM_THEME_ID,
  // Getters, so the strings follow the active locale rather than being
  // frozen in one language at import time — this entry used to be
  // German for everyone.
  get label() {
    return m.mobile_theme_custom_name()
  },
  get description() {
    return m.mobile_theme_custom_desc()
  },
}

/** Fallback if a saved `theme_name` no longer exists in the live
    list (e.g. a removed custom theme, or a build downgrade). */
export const DEFAULT_THEME_ID = 'unkai'

const DARK_MEDIA = '(prefers-color-scheme: dark)'

/** Map from a custom theme slug → its absolute on-disk path.
 *  Maintained alongside `customThemes` so `applyTheme` can swap
 *  the runtime `<link>` href when the user picks one. */
const customThemePathById = new Map<string, string>()
export function registerCustomThemePath(id: string, path: string): void {
  customThemePathById.set(id, path)
}
export function unregisterCustomThemePath(id: string): void {
  customThemePathById.delete(id)
}

const CUSTOM_LINK_ID = 'unkai-custom-theme'
const GENERATED_STYLE_ID = 'unkai-generated-theme'

/**
 * Compile the user's colours into a stylesheet and keep it in `<head>`.
 *
 * A `<style>` element rather than inline variables on `<html>`, for two
 * reasons: the generated CSS also carries a `body` font rule, which has
 * nowhere to live in an inline style; and a single element that can be
 * replaced wholesale makes "the theme changed" one DOM write instead of
 * ~150 `setProperty` calls on every keystroke in the editor.
 *
 * The sheet stays installed even while a stock theme is active. It is
 * scoped to `[data-theme="custom"]`, so it costs nothing when unused
 * and means switching back to a custom theme is instant.
 */
export function applyCustomTheme(input: CustomThemeInput): void {
  let style = document.getElementById(GENERATED_STYLE_ID) as HTMLStyleElement | null
  if (!style) {
    style = document.createElement('style')
    style.id = GENERATED_STYLE_ID
    document.head.appendChild(style)
  }
  const css = buildThemeCss(input)
  // Skip an identical write: the editor calls this live as sliders
  // move, and re-assigning `textContent` invalidates style resolution
  // for the whole document whether or not anything changed.
  if (style.textContent !== css) style.textContent = css
}

function ensureCustomThemeLoaded(themeId: string): void {
  const path = customThemePathById.get(themeId)
  let link = document.getElementById(CUSTOM_LINK_ID) as HTMLLinkElement | null
  if (!path) {
    if (link) link.parentElement?.removeChild(link)
    return
  }
  // `assetUrl` rewrites the absolute path into the protocol
  // the Tauri webview can fetch (`asset://` on most platforms).
  const href = api.platform.assetUrl(path)
  if (!link) {
    link = document.createElement('link')
    link.id = CUSTOM_LINK_ID
    link.rel = 'stylesheet'
    document.head.appendChild(link)
  }
  if (link.href !== href) link.href = href
}

/**
 * Apply a theme + mode to the document. Idempotent — safe to call
 * on every settings change without diffing.
 *
 * For `system` mode this resolves the OS preference once and writes
 * a concrete `light`/`dark` value. The matchMedia listener installed
 * by `installSystemModeListener` takes care of the live updates as
 * the OS preference flips later.
 */
export function applyTheme(name: string, mode: ThemeMode): void {
  const live = listThemes()
  const themeId = live.some((t) => t.id === name) ? name : DEFAULT_THEME_ID
  ensureCustomThemeLoaded(themeId)
  document.documentElement.dataset.theme = themeId

  const concreteMode: 'light' | 'dark' =
    mode === 'system' ? (prefersDark() ? 'dark' : 'light') : mode
  document.documentElement.dataset.mode = concreteMode

  try {
    localStorage.setItem(LAST_THEME_KEY, JSON.stringify({ name: themeId, mode }))
  } catch {
    // Without storage the next start paints from the system mode.
  }
}

const LAST_THEME_KEY = 'unkai.lastTheme'

/**
 * Paint the first frame in the theme the app had last time.
 *
 * The real theme comes from the backend settings, an IPC round trip
 * after the page has loaded. Until then the page had no `data-mode`
 * and drew in its light tokens — in dark mode a light-grey screen for
 * the length of the boot, the first thing every start showed. This
 * runs synchronously before the app mounts, from what `applyTheme`
 * last stored (or from the system's mode on a first start); the
 * settings then confirm or correct it.
 */
export function applyLastTheme(): void {
  try {
    const raw = localStorage.getItem(LAST_THEME_KEY)
    if (raw) {
      const last = JSON.parse(raw) as { name?: string; mode?: ThemeMode }
      if (last.name && (last.mode === 'light' || last.mode === 'dark' || last.mode === 'system')) {
        applyTheme(last.name, last.mode)
        return
      }
    }
  } catch {
    // Unreadable record: fall through to the system's mode.
  }
  document.documentElement.dataset.theme = DEFAULT_THEME_ID
  document.documentElement.dataset.mode = prefersDark() ? 'dark' : 'light'
}

/**
 * Subscribe to OS theme changes so `system` mode follows them live.
 * Returns an unsubscribe function. Caller owns the lifecycle — the
 * App-level `$effect` in `App.svelte` re-installs whenever the user
 * switches mode (tearing down the old listener for `light`/`dark`).
 *
 * For non-system modes this is a no-op subscription that just hands
 * back a no-op cleanup, so callers don't have to branch.
 */
export function installSystemModeListener(
  mode: ThemeMode,
  themeName: string,
): () => void {
  if (mode !== 'system' || !window.matchMedia) return () => {}

  const mql = window.matchMedia(DARK_MEDIA)
  const handler = () => applyTheme(themeName, 'system')
  mql.addEventListener('change', handler)
  return () => mql.removeEventListener('change', handler)
}

function prefersDark(): boolean {
  return !!window.matchMedia && window.matchMedia(DARK_MEDIA).matches
}
