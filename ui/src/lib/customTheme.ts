/**
 * Build a complete theme from three choices.
 *
 * The stock themes each ship eleven shades per colour role, hand-picked
 * by a designer. A user picking two colours in a settings screen is not
 * going to supply eleven of anything, so this module derives them —
 * and the derivation has to be good, because every surface, border and
 * piece of text in the app is one of these values.
 *
 * # Why OKLab and not HSL
 *
 * The obvious approach is HSL: keep hue and saturation, walk lightness.
 * It falls apart immediately, because HSL's "lightness" is not
 * lightness — `hsl(60 100% 50%)` (yellow) and `hsl(240 100% 50%)`
 * (blue) claim the same L and differ by a factor of about six in
 * perceived brightness. A ramp built that way is uneven in a different
 * way for every hue: yellows blow out at the light end, blues turn to
 * mud at the dark end, and text contrast becomes a lottery decided by
 * which colour the user happened to pick.
 *
 * OKLab is built so that equal steps look like equal steps. Fixing the
 * lightness ladder there and varying only hue and chroma gives every
 * colour the same ramp shape, which is what makes an arbitrary user
 * colour safe to build an entire UI on.
 *
 * # What it produces
 *
 * Skeleton's CSS variables, as a stylesheet scoped to
 * `[data-theme="custom"]`. The rest of the app — including the app's
 * own `--ui-*` tokens, which read from the Skeleton scale — then works
 * unchanged, because as far as it can tell this is just another theme.
 */

import { parseColor, themeTokensCss } from './accessibleColors'

/** The theme id a generated theme is published under. */
export const CUSTOM_THEME_ID = 'custom'

/* ── Colour space conversions ─────────────────────────────────────── */

interface Oklch {
  /** Perceptual lightness, 0 (black) … 1 (white). */
  l: number
  /** Chroma — distance from grey. Unbounded in principle; ~0.37 is
   *  about as saturated as sRGB can actually represent. */
  c: number
  /** Hue angle in degrees. */
  h: number
}

/** sRGB channel (0…1) → linear light. */
function linearize(v: number): number {
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4)
}

/** Linear light → sRGB channel (0…1). */
function delinearize(v: number): number {
  return v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055
}

export function hexToOklch(hex: string): Oklch {
  const { r, g, b } = hexToRgb(hex)
  const lr = linearize(r / 255)
  const lg = linearize(g / 255)
  const lb = linearize(b / 255)

  // Linear sRGB → LMS → cube root → OKLab (Björn Ottosson's matrices).
  const l_ = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb)
  const m_ = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb)
  const s_ = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb)

  const L = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_
  const a = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_
  const bb = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_

  const c = Math.sqrt(a * a + bb * bb)
  let h = (Math.atan2(bb, a) * 180) / Math.PI
  if (h < 0) h += 360
  return { l: L, c, h }
}

/** OKLCH → linear sRGB, unclamped, so callers can test for gamut. */
function oklchToLinearRgb({ l, c, h }: Oklch): [number, number, number] {
  const rad = (h * Math.PI) / 180
  const a = c * Math.cos(rad)
  const b = c * Math.sin(rad)

  const l_ = l + 0.3963377774 * a + 0.2158037573 * b
  const m_ = l - 0.1055613458 * a - 0.0638541728 * b
  const s_ = l - 0.0894841775 * a - 1.291485548 * b

  const lc = l_ * l_ * l_
  const mc = m_ * m_ * m_
  const sc = s_ * s_ * s_

  return [
    4.0767416621 * lc - 3.3077115913 * mc + 0.2309699292 * sc,
    -1.2684380046 * lc + 2.6097574011 * mc - 0.3413193965 * sc,
    -0.0041960863 * lc - 0.7034186147 * mc + 1.707614701 * sc,
  ]
}

/** Does this colour exist in sRGB at all? */
function inGamut(colour: Oklch): boolean {
  const eps = 1e-4
  return oklchToLinearRgb(colour).every((v) => v >= -eps && v <= 1 + eps)
}

/**
 * Pull a colour back into sRGB by reducing chroma, never by clamping
 * channels.
 *
 * This is the difference between a generated palette that looks
 * hand-made and one that looks broken. Plenty of OKLCH triples name
 * colours sRGB cannot show — a light vivid blue especially, since
 * sRGB's blue primary is very dark. Converting one anyway produces
 * out-of-range channels, and clamping them changes *hue and lightness*,
 * not just saturation: `#3b82f6`'s ramp drifted 13° toward purple at
 * the light end and the shades no longer matched their target
 * brightness.
 *
 * Reducing chroma instead keeps hue and lightness exactly, and gives up
 * only the saturation that was never displayable. A binary search over
 * ~16 steps lands well inside a single 8-bit step, and the ramp stays
 * an even ladder for every hue the user can pick.
 */
function toGamut(colour: Oklch): Oklch {
  if (inGamut(colour)) return colour
  let lo = 0
  let hi = colour.c
  for (let i = 0; i < 16; i++) {
    const mid = (lo + hi) / 2
    if (inGamut({ ...colour, c: mid })) lo = mid
    else hi = mid
  }
  return { ...colour, c: lo }
}

export function oklchToHex(colour: Oklch): string {
  const [r, g, b] = oklchToLinearRgb(toGamut(colour))
  return rgbToHex(
    clamp255(delinearize(r) * 255),
    clamp255(delinearize(g) * 255),
    clamp255(delinearize(b) * 255),
  )
}

function clamp255(v: number): number {
  return Math.max(0, Math.min(255, Math.round(v)))
}

export function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const clean = normaliseHex(hex).slice(1)
  return {
    r: parseInt(clean.slice(0, 2), 16),
    g: parseInt(clean.slice(2, 4), 16),
    b: parseInt(clean.slice(4, 6), 16),
  }
}

function rgbToHex(r: number, g: number, b: number): string {
  const to = (v: number) => v.toString(16).padStart(2, '0')
  return `#${to(r)}${to(g)}${to(b)}`
}

/**
 * Coerce user input into `#rrggbb`.
 *
 * The editor's text field lets people type or paste, so this accepts
 * the shapes they actually produce — `#abc`, `abc`, `ABCDEF` — and
 * falls back to mid-grey rather than throwing. A settings screen that
 * blows up on a half-typed colour is worse than one that shows grey
 * for a keystroke.
 */
export function normaliseHex(input: string, fallback = '#808080'): string {
  const raw = (input ?? '').trim().replace(/^#/, '')
  if (/^[0-9a-fA-F]{3}$/.test(raw)) {
    return `#${raw[0]}${raw[0]}${raw[1]}${raw[1]}${raw[2]}${raw[2]}`.toLowerCase()
  }
  if (/^[0-9a-fA-F]{6}$/.test(raw)) return `#${raw.toLowerCase()}`
  return fallback
}

/* ── The lightness ladder ─────────────────────────────────────────── */

export const SHADES = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950] as const
export type Shade = (typeof SHADES)[number]

/**
 * Target OKLab lightness per shade.
 *
 * Fixed rather than derived from the input colour, and that is the
 * point: shade 500 always lands at the same perceived brightness
 * whichever colour the user picked, so a layout built on 100/500/900
 * keeps its contrast relationships no matter what. Only hue and chroma
 * come from the user.
 *
 * The curve is deliberately not linear — it is denser at the light end,
 * where the eye discriminates more finely, which is where the app puts
 * its surfaces.
 */
const LIGHTNESS: Record<Shade, number> = {
  50: 0.977,
  100: 0.949,
  200: 0.897,
  300: 0.828,
  400: 0.744,
  500: 0.652,
  600: 0.567,
  700: 0.478,
  800: 0.39,
  900: 0.305,
  950: 0.235,
}

/**
 * How much of the base chroma each shade keeps.
 *
 * Chroma has to taper at both ends or the ramp leaves the sRGB gamut
 * and clips: there is no such thing as a vivid near-white or a vivid
 * near-black, and asking for one produces a flat, wrong colour after
 * clamping. Peaking around 500-600 also matches how a hand-built
 * palette behaves — the mid tones carry the identity, the extremes are
 * nearly neutral.
 */
const CHROMA_FACTOR: Record<Shade, number> = {
  50: 0.22,
  100: 0.36,
  200: 0.6,
  300: 0.82,
  400: 0.95,
  500: 1,
  600: 0.98,
  700: 0.88,
  800: 0.74,
  900: 0.58,
  950: 0.46,
}

/**
 * Build an eleven-shade ramp from one colour.
 *
 * `chromaScale` lets the caller flatten the result: surfaces want the
 * user's hue as a faint tint rather than a colour, so they pass a small
 * number and get near-neutrals that still read as *their* grey. The
 * accent passes 1 and keeps its full character.
 */
export function buildScale(hex: string, chromaScale = 1): Record<Shade, string> {
  const base = hexToOklch(normaliseHex(hex))
  const out = {} as Record<Shade, string>
  for (const shade of SHADES) {
    out[shade] = oklchToHex({
      l: LIGHTNESS[shade],
      c: base.c * CHROMA_FACTOR[shade] * chromaScale,
      h: base.h,
    })
  }
  return out
}

/**
 * Pick black or white text for a background, by measured contrast.
 *
 * Skeleton expects a `--color-*-contrast-*` variable per shade, and
 * getting it wrong is not cosmetic — it is the difference between a
 * legible button and an unreadable one. The threshold is OKLab
 * lightness rather than a WCAG luminance ratio because the whole ramp
 * is defined in OKLab; 0.62 is where white-on-colour stops beating
 * black-on-colour for these ramps.
 */
export function contrastInk(hex: string): string {
  return hexToOklch(hex).l > 0.62 ? '#000000' : '#ffffff'
}

/* ── Font choices ─────────────────────────────────────────────────── */

export type FontStyleId = 'system' | 'rounded' | 'serif' | 'mono'

/**
 * The four families, all of them already on the device.
 *
 * Nothing here is downloaded: `ui-rounded`, `ui-serif` and
 * `ui-monospace` are generic families the platform maps to its own
 * faces (SF Pro Rounded, New York, SF Mono on iOS), so a font choice
 * costs no bytes, no layout shift and no licence. Each falls back
 * through a concrete stack for engines that do not know the generic.
 */
export const FONT_STACKS: Record<FontStyleId, string> = {
  system:
    '-apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui, "Segoe UI", Roboto, sans-serif',
  rounded:
    'ui-rounded, "SF Pro Rounded", "Hiragino Maru Gothic ProN", "Varela Round", system-ui, sans-serif',
  serif: 'ui-serif, "New York", Georgia, "Times New Roman", serif',
  mono: 'ui-monospace, "SF Mono", "Menlo", "Cascadia Mono", "Roboto Mono", monospace',
}

export function fontStack(id: string): string {
  return FONT_STACKS[(id as FontStyleId) ?? 'system'] ?? FONT_STACKS.system
}

/* ── Stylesheet generation ────────────────────────────────────────── */

export interface CustomThemeInput {
  /** Drives the surface scale — every background, border and hairline. */
  base: string
  /** Drives the primary scale — every accent, link and active state. */
  accent: string
  /** One of `FONT_STACKS`. */
  font: string
}

/**
 * How much of the base hue survives into the surface ramp.
 *
 * Surfaces are 95% of the pixels on screen, so they must not be
 * *coloured* — they must be neutral with a whisper of the user's hue,
 * enough that a warm theme and a cool theme feel different without
 * either fighting the content. 0.28 is low enough that even a fully
 * saturated pick stays a usable background.
 */
const SURFACE_CHROMA = 0.28

/**
 * Semantic colours are **not** derived from the user's picks.
 *
 * Success, warning and error mean something, and that meaning is
 * carried by hue: a user whose accent is red must not end up with green
 * error states, and one who picks green must not get an inbox where
 * "delete" and "done" are the same colour. These stay fixed and are
 * only ramped, so they keep their meaning in every theme.
 */
const SEMANTIC = {
  success: '#16a34a',
  warning: '#d97706',
  error: '#dc2626',
}

/** Emit `--color-<role>-<shade>` plus its contrast pair. */
function emitScale(role: string, scale: Record<Shade, string>): string {
  return SHADES.map(
    (shade) =>
      `  --color-${role}-${shade}: ${scale[shade]};\n` +
      `  --color-${role}-contrast-${shade}: ${contrastInk(scale[shade])};`,
  ).join('\n')
}

/**
 * Render the generated theme as a stylesheet.
 *
 * Scoped to `[data-theme="custom"]` so it sits alongside the stock
 * themes rather than replacing them — switching back to Cerberus is
 * just a change of attribute, and the user's colours are still there
 * when they switch back.
 */
export function buildThemeCss(input: CustomThemeInput): string {
  const surface = buildScale(input.base, SURFACE_CHROMA)
  const primary = buildScale(input.accent)
  // Secondary and tertiary are the accent rotated around the hue
  // circle. The app uses them for avatar tints and minor accents, so
  // they need to be *different* from primary but clearly related —
  // a complement would clash, these stay in family.
  const accentOklch = hexToOklch(normaliseHex(input.accent))
  const secondary = buildScale(oklchToHex({ ...accentOklch, h: (accentOklch.h + 35) % 360 }))
  const tertiary = buildScale(oklchToHex({ ...accentOklch, h: (accentOklch.h + 320) % 360 }))
  const success = buildScale(SEMANTIC.success)
  const warning = buildScale(SEMANTIC.warning)
  const error = buildScale(SEMANTIC.error)

  // The contrast-guaranteed text and control colours, solved from the
  // ramps above exactly as `accessible-colors.css` is for the stock
  // themes. Without this a user who picks a pale accent would get the
  // same illegible white-on-yellow buttons the solver exists to prevent.
  const scales: Record<string, Record<Shade, string>> = {
    surface,
    primary,
    secondary,
    tertiary,
    success,
    warning,
    error,
  }
  const colors = (name: string) => {
    const [role, shade] = name.split('-')
    const hex = scales[role]?.[Number(shade) as Shade]
    const rgb = hex ? parseColor(hex) : null
    if (!rgb) throw new Error(`custom theme has no --color-${name}`)
    return rgb
  }

  return `[data-theme='${CUSTOM_THEME_ID}'] {
  --unkai-custom-font: ${fontStack(input.font)};

${emitScale('surface', surface)}

${emitScale('primary', primary)}

${emitScale('secondary', secondary)}

${emitScale('tertiary', tertiary)}

${emitScale('success', success)}

${emitScale('warning', warning)}

${emitScale('error', error)}
}

[data-theme='${CUSTOM_THEME_ID}'] body {
  font-family: var(--unkai-custom-font);
}

${themeTokensCss(CUSTOM_THEME_ID, colors)}
`
}
