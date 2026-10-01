/**
 * Dynamic Type: the text size the user chose in iOS Settings.
 *
 * A native app gets this for free from `UIFont.preferredFont`. A web
 * view does not — its CSS pixels are fixed unless the page asks — and
 * for anyone who has turned text up because they cannot read the
 * default, an app that ignores the setting is simply unusable. It is
 * also a stated criterion on both sides of this app's obligations:
 * "Larger Text" among Apple's App Store accessibility labels, and
 * resize-text (WCAG 1.4.4) under the European Accessibility Act.
 *
 * # How the size is read
 *
 * WebKit on iOS exposes the setting through one CSS font keyword:
 * `font: -apple-system-body` resolves to the body size of the current
 * content-size category — 17px at the default, 14px at the smallest,
 * 53px at the largest accessibility size. A hidden probe element is
 * given that font and measured; the ratio to 17px becomes the
 * `--dt-content` and `--dt-chrome` multipliers `app.css` scales every
 * type token by.
 *
 * Why a probe rather than `html { font: -apple-system-body }`: that
 * would change what `rem` means, and Tailwind builds all *spacing* from
 * rem. Every padding in the app would grow with the text. On a phone
 * that is the wrong trade — text has to grow, the grid has to stay.
 *
 * A `ResizeObserver` on the probe picks up a change made while the app
 * is running (WebKit re-resolves the keyword and the probe's box
 * changes size), and returning to the foreground re-measures as a
 * backstop, since that is when a change made in Settings arrives.
 */

/** Body size at iOS's default content-size category ("Large"). */
export const DEFAULT_BODY_PX = 17

/**
 * Ceiling for content text: 200%, the size WCAG 1.4.4 names. Past that
 * the accessibility sizes reach 312%, where a single sender name would
 * no longer fit a phone's width at all; iOS's own lists truncate there
 * too.
 */
export const CONTENT_MAX = 2

/**
 * Ceiling for bar text — tab labels, a nav bar's title, badges. A tab
 * bar with five labels at twice the size does not fit; iOS caps its own
 * bars and offers the Large Content Viewer instead.
 */
export const CHROME_MAX = 1.3

/**
 * The floor. iOS's smallest category is 14/17 ≈ 0.82; clamping there
 * keeps a mis-measured probe (0, NaN) from collapsing every glyph.
 */
export const SCALE_MIN = 14 / DEFAULT_BODY_PX

export interface TypeScales {
  content: number
  chrome: number
}

/** Map a measured body size to the two multipliers. Pure, so it is
 *  unit-tested rather than trusted. */
export function scalesFor(bodyPx: number): TypeScales {
  if (!Number.isFinite(bodyPx) || bodyPx <= 0) return { content: 1, chrome: 1 }
  const raw = bodyPx / DEFAULT_BODY_PX
  const clamp = (max: number) => Math.round(Math.min(max, Math.max(SCALE_MIN, raw)) * 1000) / 1000
  return { content: clamp(CONTENT_MAX), chrome: clamp(CHROME_MAX) }
}

/**
 * Whether this engine is iOS WebKit, the only one where
 * `-apple-system-body` means the user's text size. macOS WebKit also
 * accepts the keyword but resolves it to a fixed 13px, which would
 * shrink the desktop dev window to three quarters. `-webkit-touch-callout`
 * exists only on iOS.
 */
function followsSystemTextSize(): boolean {
  return typeof CSS !== 'undefined' && CSS.supports('-webkit-touch-callout', 'none')
}

/**
 * Start following the system text size. Returns a stop function.
 * A no-op everywhere but iOS, where the multipliers stay at 1.
 */
export function installDynamicType(doc: Document = document): () => void {
  if (!followsSystemTextSize()) return () => {}

  const probe = doc.createElement('span')
  probe.setAttribute('aria-hidden', 'true')
  probe.textContent = 'M'
  probe.style.cssText =
    'position:absolute;top:0;left:-9999px;visibility:hidden;pointer-events:none;' +
    'white-space:nowrap;font:-apple-system-body'
  doc.body.appendChild(probe)

  const root = doc.documentElement
  let last = ''
  const apply = () => {
    const { content, chrome } = scalesFor(parseFloat(getComputedStyle(probe).fontSize))
    const key = `${content}/${chrome}`
    // Every write invalidates style for the whole document; skip the
    // ones that would change nothing, which is nearly all of them.
    if (key === last) return
    last = key
    root.style.setProperty('--dt-content', String(content))
    root.style.setProperty('--dt-chrome', String(chrome))
  }

  apply()
  const observer = new ResizeObserver(apply)
  observer.observe(probe)
  const onVisible = () => {
    if (doc.visibilityState === 'visible') apply()
  }
  doc.addEventListener('visibilitychange', onVisible)

  return () => {
    observer.disconnect()
    doc.removeEventListener('visibilitychange', onVisible)
    probe.remove()
  }
}
