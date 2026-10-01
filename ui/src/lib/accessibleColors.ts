/**
 * Text and control colours with a *guaranteed* contrast ratio.
 *
 * # Why this exists
 *
 * The app's colour tokens used to be fixed recipes over the active
 * theme — "surface-900 at 42% opacity", "primary-500 as link text",
 * "white on the accent". Measured against all 22 stock themes, those
 * recipes failed WCAG 2.1 AA almost everywhere: secondary text landed
 * at 1.8–2.8:1 where 4.5:1 is required, white labels on the green and
 * amber swipe actions at 1.1–2:1, and in a theme whose accent is a
 * pale yellow the accent-as-text was 1.02:1 — invisible.
 *
 * That matters beyond taste. The European Accessibility Act (applied
 * since June 2025) points at EN 301 549, which is WCAG 2.1 AA, and
 * Apple's App Store accessibility labels ask the same question as
 * "Sufficient Contrast".
 *
 * No single CSS recipe can pass in every theme — some stock scales are
 * not even monotonic (Reign's surface-500 is darker than its 900) — so
 * a fixed formula is either too weak for the extreme themes or needlessly
 * heavy-handed for the good ones. Instead each theme's colours are
 * *solved*: start from exactly what the design asks for, and move only
 * as far toward black (light mode) or white (dark mode) as it takes to
 * reach the target. A theme that already passed looks the same as
 * before; one that did not gets the smallest change that fixes it.
 *
 * # How it is used
 *
 * `accessible-colors.css` is generated from this module for every stock
 * theme (see `accessibleColors.test.ts`, which also fails the build if
 * the file goes stale), and `customTheme.ts` calls it at runtime for the
 * user-built theme. Both emit plain colour values, so nothing is
 * computed in the browser.
 *
 * Pure maths, no DOM — like the other `lib/*.ts` modules it runs under
 * node in vitest.
 */

/** Gamma-encoded sRGB, each channel 0…1. */
export type Rgb = readonly [number, number, number]

/** OKLab: L 0…1, a/b roughly −0.4…0.4. */
type Lab = readonly [number, number, number]

/* ── Conversions ──────────────────────────────────────────────────── */

function toLinear(v: number): number {
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4)
}

function fromLinear(v: number): number {
  return v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055
}

function clamp01(v: number): number {
  return Math.min(1, Math.max(0, v))
}

export function rgbToOklab([r, g, b]: Rgb): Lab {
  const lr = toLinear(r)
  const lg = toLinear(g)
  const lb = toLinear(b)
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb)
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb)
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb)
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ]
}

export function oklabToRgb([L, a, b]: Lab): Rgb {
  const l = Math.pow(L + 0.3963377774 * a + 0.2158037573 * b, 3)
  const m = Math.pow(L - 0.1055613458 * a - 0.0638541728 * b, 3)
  const s = Math.pow(L - 0.0894841775 * a - 1.291485548 * b, 3)
  return [
    clamp01(fromLinear(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s)),
    clamp01(fromLinear(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s)),
    clamp01(fromLinear(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s)),
  ]
}

/**
 * Parse the colour syntaxes the themes actually use: `#rgb`, `#rrggbb`,
 * and `oklch(L C H)` with L as a fraction or a percentage, H with or
 * without `deg`, or `none` for an achromatic colour. Returns null for
 * anything else rather than guessing.
 */
export function parseColor(value: string): Rgb | null {
  const v = value.trim().toLowerCase()
  const hex = v.match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/)
  if (hex) {
    const h = hex[1].length === 3 ? [...hex[1]].map((c) => c + c).join('') : hex[1]
    return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255) as unknown as Rgb
  }
  const ok = v.match(/^oklch\(\s*([\d.]+)(%?)\s+([\d.]+|none)\s+([\d.]+|none)(?:deg)?\s*\)$/)
  if (ok) {
    const L = Number(ok[1]) / (ok[2] ? 100 : 1)
    const C = ok[3] === 'none' ? 0 : Number(ok[3])
    const H = ok[4] === 'none' ? 0 : (Number(ok[4]) * Math.PI) / 180
    return oklabToRgb([L, C * Math.cos(H), C * Math.sin(H)])
  }
  if (v === 'white') return [1, 1, 1]
  if (v === 'black') return [0, 0, 0]
  return null
}

export function toHex(rgb: Rgb): string {
  return (
    '#' +
    rgb
      .map((c) =>
        Math.round(clamp01(c) * 255)
          .toString(16)
          .padStart(2, '0'),
      )
      .join('')
  )
}

/* ── Measuring ────────────────────────────────────────────────────── */

/** WCAG 2.x relative luminance. */
export function luminance([r, g, b]: Rgb): number {
  return 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b)
}

/** WCAG 2.x contrast ratio, 1…21. */
export function contrast(x: Rgb, y: Rgb): number {
  const a = luminance(x)
  const b = luminance(y)
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05)
}

/** `fg` at `alpha` painted over an opaque `bg` — how WebKit composites,
 *  in gamma-encoded sRGB. */
export function over(fg: Rgb, alpha: number, bg: Rgb): Rgb {
  return [0, 1, 2].map((i) => fg[i] * alpha + bg[i] * (1 - alpha)) as unknown as Rgb
}

/** Interpolate in OKLab: `t = 0` is `from`, `t = 1` is `to`. Keeps hue
 *  while moving toward a pole, which is what keeps a darkened accent
 *  recognisably the theme's accent. */
export function mix(from: Rgb, to: Rgb, t: number): Rgb {
  const a = rgbToOklab(from)
  const b = rgbToOklab(to)
  return oklabToRgb([0, 1, 2].map((i) => a[i] + (b[i] - a[i]) * t) as unknown as Lab)
}

const WHITE: Rgb = [1, 1, 1]
const BLACK: Rgb = [0, 0, 0]

function minContrast(fg: Rgb, bgs: readonly Rgb[]): number {
  return Math.min(...bgs.map((bg) => contrast(fg, bg)))
}

/**
 * Headroom added to every target.
 *
 * The solved colour is rounded to 8-bit hex and the browser rounds again
 * when it composites an alpha, so a colour solved to exactly 4.500 can
 * render at 4.49. A few hundredths cost nothing visible and make the
 * guarantee survive both roundings — the test re-measures the emitted
 * hex values, not the unrounded ones.
 */
const HEADROOM = 0.08

/**
 * The least movement of `colour` toward `pole` that reaches `target`
 * against every background.
 *
 * Scanned rather than bisected: when the starting colour is *lighter*
 * than a light background (a pale accent), moving toward black first
 * lowers the contrast and only then raises it, so the function is not
 * monotonic and a bisection could stop at the wrong side of the dip.
 * 200 steps then a local refinement is a few milliseconds per theme.
 */
function solveMix(colour: Rgb, pole: Rgb, bgs: readonly Rgb[], target: number): Rgb {
  const goal = target + HEADROOM
  if (minContrast(colour, bgs) >= goal) return colour
  const STEPS = 200
  for (let i = 1; i <= STEPS; i++) {
    const t = i / STEPS
    if (minContrast(mix(colour, pole, t), bgs) >= goal) {
      // Refine inside the last step, where the answer is known to lie.
      let lo = (i - 1) / STEPS
      let hi = t
      for (let k = 0; k < 12; k++) {
        const mid = (lo + hi) / 2
        if (minContrast(mix(colour, pole, mid), bgs) >= goal) hi = mid
        else lo = mid
      }
      return mix(colour, pole, hi)
    }
  }
  return pole
}

/** The least opacity ≥ `floor` at which `ink` reaches `target` over every
 *  background. Opacity toward a contrasting ink is monotonic, so this
 *  one can bisect. */
function solveAlpha(ink: Rgb, floor: number, bgs: readonly Rgb[], target: number): number {
  const goal = target + HEADROOM
  const passes = (alpha: number) => bgs.every((bg) => contrast(over(ink, alpha, bg), bg) >= goal)
  if (passes(floor)) return floor
  let lo = floor
  let hi = 1
  for (let k = 0; k < 20; k++) {
    const mid = (lo + hi) / 2
    if (passes(mid)) hi = mid
    else lo = mid
  }
  // Round *up* so the emitted value can only be stronger than solved.
  return Math.min(1, Math.ceil(hi * 1000) / 1000)
}

/**
 * A filled control and the label on it.
 *
 * Any colour has at least ~4.58:1 against either pure white or pure
 * black (the two curves cross at luminance 0.18), so for AA the fill
 * never has to change — only the label does. White is preferred when
 * it passes, since that is how the design reads in most themes; black
 * takes over on the pale fills where white text was illegible. Only a
 * stricter target (Increase Contrast) can require moving the fill too.
 */
function solveFill(fill: Rgb, target: number): { fill: Rgb; on: Rgb } {
  const goal = target + HEADROOM
  if (contrast(WHITE, fill) >= goal) return { fill, on: WHITE }
  if (contrast(BLACK, fill) >= goal) return { fill, on: BLACK }
  const on = contrast(WHITE, fill) >= contrast(BLACK, fill) ? WHITE : BLACK
  return { fill: solveMix(fill, on === WHITE ? BLACK : WHITE, [on], target), on }
}

/* ── The token set ────────────────────────────────────────────────── */

export type Mode = 'light' | 'dark'
export type ContrastLevel = 'standard' | 'more'

/**
 * WCAG targets per token.
 *
 * Text is 4.5:1 (AA, 1.4.3); the primary ink aims for 7:1 (AAA, 1.4.6)
 * because it is the text people read most, and muted sits between the
 * two so the three inks stay three distinguishable steps. Control
 * boundaries are 3:1 (1.4.11).
 *
 * With iOS "Increase Contrast" on, all text moves to 7:1 and controls
 * to 4.5:1. The secondary inks then converge on the primary one, which
 * is the point of the setting — hierarchy is carried by size and weight
 * instead. 7:1 is also the ceiling that is reachable everywhere: the
 * darkest background of a mid-toned dark theme (a bottom bar on
 * `surface-800`) cannot give even pure white much more than 8:1.
 */
const TARGETS: Record<
  ContrastLevel,
  Record<'ink' | 'muted' | 'faint' | 'text' | 'fill' | 'control', number>
> = {
  standard: { ink: 7, muted: 5.5, faint: 4.5, text: 4.5, fill: 4.5, control: 3 },
  more: { ink: 7, muted: 7, faint: 7, text: 7, fill: 7, control: 4.5 },
}

/** The opacities the design system specified for the two secondary
 *  inks. They are floors: a theme only ever gets *more* opaque ink. */
const DESIGN_ALPHA: Record<Mode, { muted: number; faint: number }> = {
  light: { muted: 0.62, faint: 0.42 },
  dark: { muted: 0.64, faint: 0.42 },
}

/** Every token this module produces, in emission order. */
export const TOKEN_NAMES = [
  '--ui-ink',
  '--ui-ink-muted',
  '--ui-ink-faint',
  '--ui-accent-ink',
  '--ui-danger-ink',
  '--ui-warning-ink',
  '--ui-success-ink',
  '--ui-tertiary-ink',
  '--ui-signal-ink',
  '--ui-accent-fill',
  '--ui-on-accent',
  '--ui-danger-fill',
  '--ui-on-danger',
  '--ui-success-fill',
  '--ui-on-success',
  '--ui-warning-fill',
  '--ui-on-warning',
  '--ui-neutral-fill',
  '--ui-on-neutral',
  '--ui-signal-fill',
  '--ui-on-signal',
  '--ui-control',
] as const

export type TokenName = (typeof TOKEN_NAMES)[number]

/** Looks up `--color-<role>-<shade>` for one theme. */
export type ThemeColors = (name: string) => Rgb

/**
 * The backgrounds text actually sits on, per mode.
 *
 * Each must be named here or its contrast is not guaranteed. Light: the
 * card, the recessed canvas (nav bar, list headers), a sheet, the raised
 * white card, and an unread mail row's 5% accent tint. Dark adds
 * `surface-800` and the raised card.
 */
export function backgroundsFor(color: ThemeColors, mode: Mode): Rgb[] {
  const primary = color('primary-500')
  if (mode === 'light') {
    const card = color('surface-50')
    return [card, color('surface-100'), WHITE, over(primary, 0.05, card)]
  }
  const card = color('surface-900')
  return [
    card,
    color('surface-950'),
    color('surface-800'),
    mix(color('surface-900'), color('surface-800'), 0.7),
    over(primary, 0.05, card),
  ]
}

/**
 * Translucent *washes* that text and glyphs also sit on — mirroring two
 * tokens in `app.css` (change one, change the other):
 *
 *   * `--ui-fill-quiet` — the grey wash under the search field, count
 *     pills, action-sheet rows and quiet buttons. Worst case over the
 *     canvas (light) and the card (dark); none sits on the raised card.
 *   * `--ui-tint-accent` — the accent wash behind the active tab's icon,
 *     an empty state's disc and the inline notices.
 *
 * They get their own list because they get their own target: **AA**
 * (`WASH_TARGET`), not the design targets of `TARGETS`. The 2026-09-30
 * redesign introduced both, and measuring them against the full targets
 * showed why: a wash is by definition lighter (dark mode) or darker
 * (light mode) than the surface under it, and on a theme whose dark card
 * already only just gives pure white 7:1 — Reign — *any* wash puts the
 * primary ink under 7:1. Nothing a solver does to the ink can fix that;
 * white is as far as it goes. So: 7:1 on the surfaces people read on,
 * AA guaranteed on the washes. Before this list existed, muted ink on
 * the quiet fill measured 4.7:1 and faint ink 4.0:1 in the house theme,
 * with nothing to stop a paler theme going lower.
 */
export function washesFor(color: ThemeColors, mode: Mode): Rgb[] {
  const primary = color('primary-500')
  if (mode === 'light') {
    const card = color('surface-50')
    return [over(color('surface-500'), 0.13, color('surface-100')), over(primary, 0.12, card)]
  }
  const card = color('surface-900')
  return [over(color('surface-50'), 0.11, card), over(primary, 0.12, card)]
}

/** What every text token must reach on a wash, at every contrast level:
 *  WCAG AA for text (1.4.3). Controls need 3:1 there (1.4.11). */
export const WASH_TARGET = { text: 4.5, control: 3 } as const

function rgba(rgb: Rgb, alpha: number): string {
  if (alpha >= 1) return toHex(rgb)
  const [r, g, b] = rgb.map((c) => Math.round(clamp01(c) * 255))
  return `rgb(${r} ${g} ${b} / ${alpha})`
}

/**
 * Solve every token for one theme in one mode.
 *
 * Returns CSS values keyed by custom-property name.
 */
export function solveTokens(
  color: ThemeColors,
  mode: Mode,
  level: ContrastLevel = 'standard',
): Record<TokenName, string> {
  const target = TARGETS[level]
  const bgs = backgroundsFor(color, mode)
  const washes = washesFor(color, mode)
  const pole = mode === 'light' ? BLACK : WHITE

  // Each colour is solved twice: to its design target on the surfaces,
  // then — starting from that answer — to AA on the washes. The second
  // pass only ever moves a colour further toward the pole, so it cannot
  // undo the first.
  const bothText = (start: Rgb, surfaceTarget: number) =>
    solveMix(solveMix(start, pole, bgs, surfaceTarget), pole, washes, WASH_TARGET.text)
  const bothAlpha = (ink: Rgb, floor: number, surfaceTarget: number, washTarget: number) =>
    Math.max(solveAlpha(ink, floor, bgs, surfaceTarget), solveAlpha(ink, floor, washes, washTarget))

  const ink = bothText(color(mode === 'light' ? 'surface-900' : 'surface-50'), target.ink)
  const muted = bothAlpha(ink, DESIGN_ALPHA[mode].muted, target.muted, WASH_TARGET.text)
  const faint = bothAlpha(ink, DESIGN_ALPHA[mode].faint, target.faint, WASH_TARGET.text)
  const textFor = (role: string) => toHex(bothText(color(`${role}-500`), target.text))
  const fillFor = (role: string) => solveFill(color(`${role}-500`), target.fill)

  const accent = fillFor('primary')
  const danger = fillFor('error')
  const success = fillFor('success')
  const warning = fillFor('warning')
  const neutral = fillFor('surface')
  // The *signal* role — what is new (the unread mark, a badge, today) —
  // rides the secondary scale. Every theme gets it solved; whether a
  // theme's unread mark actually uses it is decided in `app.css`.
  const signal = fillFor('secondary')

  return {
    '--ui-ink': toHex(ink),
    '--ui-ink-muted': rgba(ink, muted),
    '--ui-ink-faint': rgba(ink, faint),
    '--ui-accent-ink': textFor('primary'),
    '--ui-danger-ink': textFor('error'),
    '--ui-warning-ink': textFor('warning'),
    '--ui-success-ink': textFor('success'),
    '--ui-tertiary-ink': textFor('tertiary'),
    '--ui-signal-ink': textFor('secondary'),
    '--ui-accent-fill': toHex(accent.fill),
    '--ui-on-accent': toHex(accent.on),
    '--ui-danger-fill': toHex(danger.fill),
    '--ui-on-danger': toHex(danger.on),
    '--ui-success-fill': toHex(success.fill),
    '--ui-on-success': toHex(success.on),
    '--ui-warning-fill': toHex(warning.fill),
    '--ui-on-warning': toHex(warning.on),
    '--ui-neutral-fill': toHex(neutral.fill),
    '--ui-on-neutral': toHex(neutral.on),
    '--ui-signal-fill': toHex(signal.fill),
    '--ui-on-signal': toHex(signal.on),
    // A control's outline (text field, switch track, selection ring):
    // the faintest ink that still reads as a boundary.
    '--ui-control': rgba(ink, bothAlpha(ink, 0.3, target.control, WASH_TARGET.control)),
  }
}

function block(selector: string, tokens: Record<string, string>, indent = ''): string {
  const body = Object.entries(tokens)
    .map(([name, value]) => `${indent}  ${name}: ${value};`)
    .join('\n')
  return `${indent}${selector} {\n${body}\n${indent}}`
}

/**
 * The stylesheet for one theme: light, dark, and both again under
 * `prefers-contrast: more` (what iOS "Increase Contrast" reports).
 *
 * The dark selectors match both the element carrying the theme and a
 * themed element *inside* a dark document, so the theme editor's scoped
 * preview gets its own theme's tokens without restating them.
 */
export function themeTokensCss(themeId: string, color: ThemeColors): string {
  const light = `[data-theme='${themeId}']`
  const dark = `[data-mode='dark'][data-theme='${themeId}'],\n[data-mode='dark'] [data-theme='${themeId}']`
  return [
    block(light, solveTokens(color, 'light')),
    block(dark, solveTokens(color, 'dark')),
    `@media (prefers-contrast: more) {\n${block(light, solveTokens(color, 'light', 'more'), '  ')}\n${block(
      dark.replace('\n', '\n  '),
      solveTokens(color, 'dark', 'more'),
      '  ',
    )}\n}`,
  ].join('\n\n')
}

/**
 * Build a `ThemeColors` lookup from `--color-*` declarations in CSS
 * text, following `var(--color-…)` indirections. Throws on a colour it
 * cannot read, so a theme that changes format fails loudly instead of
 * silently getting black text.
 */
export function colorsFromCss(css: string): ThemeColors {
  const raw = new Map<string, string>()
  for (const m of css.matchAll(/--color-([a-z]+-(?:contrast-)?[a-z0-9]+)\s*:\s*([^;]+);/g)) {
    raw.set(m[1], m[2].trim())
  }
  const cache = new Map<string, Rgb>()
  const resolve = (name: string, depth = 0): Rgb => {
    const hit = cache.get(name)
    if (hit) return hit
    const value = raw.get(name)
    if (value === undefined) throw new Error(`--color-${name} is not defined`)
    const ref = value.match(/^var\(--color-([^)]+)\)$/)
    if (ref && depth < 8) return resolve(ref[1], depth + 1)
    const parsed = parseColor(value)
    if (!parsed) throw new Error(`--color-${name}: cannot read "${value}"`)
    cache.set(name, parsed)
    return parsed
  }
  return (name) => resolve(name)
}
