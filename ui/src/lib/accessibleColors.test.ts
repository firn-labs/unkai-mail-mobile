/// <reference types="node" />
// Node types for this file only. The theme sources are CSS, and vitest
// hands CSS modules — `?raw` imports included — to tests as empty
// strings, so `import.meta.glob` (as `api/noDirectIpc.test.ts` uses for
// source files) cannot read them. Plain `fs` can.
import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import {
  TOKEN_NAMES,
  backgroundsFor,
  colorsFromCss,
  contrast,
  over,
  parseColor,
  solveTokens,
  themeTokensCss,
  toHex,
  washesFor,
  type ContrastLevel,
  type Mode,
  type Rgb,
  type ThemeColors,
} from './accessibleColors'
import { buildThemeCss } from './customTheme'

/* ── Fixtures: the themes the build actually ships ────────────────── */

const appCss = readFileSync(new URL('../app.css', import.meta.url), 'utf8')

/** Read from `app.css` rather than listed here, so adding a theme there
 *  without regenerating the tokens fails this test instead of shipping
 *  that theme unchecked. */
const STOCK_IDS = [...appCss.matchAll(/@import '@skeletonlabs\/skeleton\/themes\/([a-z-]+)'/g)].map(
  (m) => m[1],
)
/** Themes that live in this repository (`src/themes/<id>.css`). */
const LOCAL_IDS = [...appCss.matchAll(/@import '\.\/themes\/([a-z-]+)\.css'/g)].map((m) => m[1])
const THEME_IDS = [...STOCK_IDS, ...LOCAL_IDS]

/** The file an `@import` of that theme resolves to: the package's
 *  `exports` map for a stock theme, `src/themes/` for a local one. */
function themeColors(id: string): ThemeColors {
  const url = LOCAL_IDS.includes(id)
    ? new URL(`../themes/${id}.css`, import.meta.url)
    : new URL(`../../node_modules/@skeletonlabs/skeleton/src/themes/${id}.css`, import.meta.url)
  return colorsFromCss(readFileSync(url, 'utf8'))
}

/** Parse what the solver emitted — hex or `rgb(r g b / a)` — so the
 *  checks below measure the *rounded* values the browser will see. */
function parseEmitted(value: string): { rgb: Rgb; alpha: number } {
  const hex = parseColor(value)
  if (hex) return { rgb: hex, alpha: 1 }
  const m = value.match(/^rgb\((\d+) (\d+) (\d+) \/ ([\d.]+)\)$/)
  if (!m) throw new Error(`unexpected token value ${value}`)
  return { rgb: [Number(m[1]) / 255, Number(m[2]) / 255, Number(m[3]) / 255], alpha: Number(m[4]) }
}

/** The WCAG floors, restated rather than imported: a test that read its
 *  thresholds from the module under test would pass however far they
 *  were lowered. */
const TARGETS: Record<ContrastLevel, { ink: number; muted: number; faint: number; text: number; fill: number; control: number }> = {
  standard: { ink: 7, muted: 5.5, faint: 4.5, text: 4.5, fill: 4.5, control: 3 },
  more: { ink: 7, muted: 7, faint: 7, text: 7, fill: 7, control: 4.5 },
}

/** On a translucent wash (quiet fill, accent tint) every text token owes
 *  AA and a control 3:1, at every contrast level — see `washesFor`. */
const WASH = { text: 4.5, control: 3 }

/** The worst ratio of a token across every background of the mode. */
function worst(value: string, bgs: Rgb[]): number {
  const { rgb, alpha } = parseEmitted(value)
  return Math.min(...bgs.map((bg) => contrast(over(rgb, alpha, bg), bg)))
}

function assertGuarantees(label: string, color: ThemeColors, mode: Mode, level: ContrastLevel) {
  const t = solveTokens(color, mode, level)
  const bgs = backgroundsFor(color, mode)
  const target = TARGETS[level]
  const failures: string[] = []
  const check = (token: string, ratio: number, min: number) => {
    if (!(ratio >= min)) failures.push(`${token} ${ratio.toFixed(2)} < ${min}`)
  }

  check('--ui-ink', worst(t['--ui-ink'], bgs), target.ink)
  check('--ui-ink-muted', worst(t['--ui-ink-muted'], bgs), target.muted)
  check('--ui-ink-faint', worst(t['--ui-ink-faint'], bgs), target.faint)
  for (const token of ['--ui-accent-ink', '--ui-danger-ink', '--ui-warning-ink', '--ui-success-ink', '--ui-tertiary-ink', '--ui-signal-ink'] as const) {
    check(token, worst(t[token], bgs), target.text)
  }
  for (const role of ['accent', 'danger', 'success', 'warning', 'neutral', 'signal'] as const) {
    const fill = parseEmitted(t[`--ui-${role}-fill`]).rgb
    const on = parseEmitted(t[`--ui-on-${role}`]).rgb
    check(`--ui-on-${role} on fill`, contrast(on, fill), target.fill)
  }
  check('--ui-control', worst(t['--ui-control'], bgs), target.control)

  const washes = washesFor(color, mode)
  for (const token of ['--ui-ink', '--ui-ink-muted', '--ui-ink-faint', '--ui-accent-ink', '--ui-danger-ink', '--ui-warning-ink', '--ui-success-ink', '--ui-tertiary-ink', '--ui-signal-ink'] as const) {
    check(`${token} on a wash`, worst(t[token], washes), WASH.text)
  }
  check('--ui-control on a wash', worst(t['--ui-control'], washes), WASH.control)

  expect(failures, `${label} ${mode} ${level}`).toEqual([])
}

/* ── Measuring ────────────────────────────────────────────────────── */

describe('contrast maths', () => {
  it('matches the WCAG reference values', () => {
    expect(contrast([0, 0, 0], [1, 1, 1])).toBeCloseTo(21, 5)
    expect(contrast([1, 1, 1], [1, 1, 1])).toBeCloseTo(1, 5)
    // #767676 is the canonical "just passes AA on white" grey.
    expect(contrast(parseColor('#767676')!, [1, 1, 1])).toBeCloseTo(4.54, 2)
  })

  it('reads every oklch spelling the stock themes use', () => {
    expect(toHex(parseColor('oklch(100% 0 none)')!)).toBe('#ffffff')
    expect(toHex(parseColor('oklch(0 0 0)')!)).toBe('#000000')
    expect(parseColor('oklch(57% 0.21 258.29deg)')).not.toBeNull()
    expect(parseColor('oklch(0.57 0.21 258.29)')).not.toBeNull()
    expect(parseColor('#abc')).toEqual(parseColor('#aabbcc'))
    expect(parseColor('rebeccapurple')).toBeNull()
  })

  it('follows var() indirections and refuses what it cannot read', () => {
    const colors = colorsFromCss('--color-a-1: #ffffff; --color-b-2: var(--color-a-1); --color-c-3: hsl(1 2 3);')
    expect(toHex(colors('b-2'))).toBe('#ffffff')
    expect(() => colors('c-3')).toThrow()
    expect(() => colors('missing-9')).toThrow()
  })
})

/* ── The guarantee ────────────────────────────────────────────────── */

describe('stock themes', () => {
  it('ships every theme app.css imports', () => {
    // A regex that silently matched nothing would make every loop below
    // vacuously pass.
    expect(STOCK_IDS.length).toBeGreaterThanOrEqual(22)
    // The house theme is the default; it must be checked like the rest.
    expect(LOCAL_IDS).toContain('unkai')
  })

  for (const id of THEME_IDS) {
    it(`${id}: every token meets its target in both modes and both contrast levels`, () => {
      const colors = themeColors(id)
      for (const mode of ['light', 'dark'] as const) {
        for (const level of ['standard', 'more'] as const) assertGuarantees(id, colors, mode, level)
      }
    })
  }

  it('leaves a colour alone wherever the theme already passes', () => {
    // The solver fixes what fails and nothing else: a theme whose accent
    // already clears the target keeps its designer's exact colour.
    let untouched = 0
    for (const id of THEME_IDS) {
      const colors = themeColors(id)
      for (const mode of ['light', 'dark'] as const) {
        const original = colors('primary-500')
        const passes = [...backgroundsFor(colors, mode), ...washesFor(colors, mode)].every(
          (bg) => contrast(original, bg) >= 4.5 + 0.08,
        )
        if (!passes) continue
        untouched++
        expect(solveTokens(colors, mode)['--ui-accent-ink'], `${id} ${mode}`).toBe(toHex(original))
      }
    }
    // Several dark themes have accents that pass as-is; if none did, this
    // test would be checking nothing.
    expect(untouched).toBeGreaterThan(3)
  })

  it('keeps the primary ink at AA on the role tints (badges, chips, notices)', () => {
    // `--ui-tint-warning` / `-success` / `-danger` in app.css. Only the
    // primary ink is ever set on them, so they are checked here rather
    // than handed to the solver as washes every token must clear.
    const TINTS: [string, number][] = [
      ['warning-500', 0.18],
      ['success-500', 0.14],
      ['error-500', 0.12],
    ]
    for (const id of THEME_IDS) {
      const colors = themeColors(id)
      for (const mode of ['light', 'dark'] as const) {
        const ink = parseEmitted(solveTokens(colors, mode)['--ui-ink']).rgb
        const surfaces = mode === 'light' ? [colors('surface-50'), colors('surface-100')] : [colors('surface-900')]
        for (const [shade, alpha] of TINTS) {
          for (const surface of surfaces) {
            const bg = over(colors(shade), alpha, surface)
            expect(contrast(ink, bg), `${id} ${mode} ink on ${shade} tint`).toBeGreaterThanOrEqual(4.5)
          }
        }
      }
    }
  })

  it('never makes secondary ink fainter than the design specified', () => {
    for (const id of THEME_IDS) {
      const t = solveTokens(themeColors(id), 'light')
      expect(parseEmitted(t['--ui-ink-muted']).alpha).toBeGreaterThanOrEqual(0.62)
      expect(parseEmitted(t['--ui-ink-faint']).alpha).toBeGreaterThanOrEqual(0.42)
    }
  })

  it('emits exactly the declared token set', () => {
    expect(Object.keys(solveTokens(themeColors('cerberus'), 'light'))).toEqual([...TOKEN_NAMES])
  })
})

describe('the user-built theme', () => {
  /** Picks chosen to hit the edges: near-white and near-black bases, a
   *  pale yellow accent (the case white labels fail on), a dark navy,
   *  and a mid grey where neither black nor white wins by much. */
  const PICKS = [
    { base: '#ffffff', accent: '#ffe600' },
    { base: '#000000', accent: '#1e2a40' },
    { base: '#1e2a40', accent: '#3b82f6' },
    { base: '#c0301c', accent: '#808080' },
    { base: '#16a34a', accent: '#ff00ff' },
  ]

  for (const pick of PICKS) {
    it(`base ${pick.base} / accent ${pick.accent} meets every target`, () => {
      const css = buildThemeCss({ ...pick, font: 'system' })
      const colors = colorsFromCss(css)
      for (const mode of ['light', 'dark'] as const) {
        for (const level of ['standard', 'more'] as const) assertGuarantees('custom', colors, mode, level)
      }
      // And the generated sheet really carries them.
      expect(css).toContain(`--ui-accent-ink: ${solveTokens(colors, 'light')['--ui-accent-ink']};`)
    })
  }
})

/* ── The generated stylesheet ─────────────────────────────────────── */

describe('accessible-colors.css', () => {
  it('is up to date with the solver and the installed themes', async () => {
    const css = [
      '/*',
      ' * GENERATED by src/lib/accessibleColors.test.ts — do not edit.',
      ' *',
      ' * Contrast-guaranteed text and control colours for every stock theme',
      ' * (WCAG 2.1 AA; AAA-level under prefers-contrast: more). Regenerate',
      ' * after changing the solver or a theme with:',
      ' *',
      ' *   npm --prefix ui test -- -u',
      ' */',
      '',
      ...THEME_IDS.map((id) => `/* ── ${id} ── */\n${themeTokensCss(id, themeColors(id))}\n`),
    ].join('\n')
    await expect(css).toMatchFileSnapshot('../accessible-colors.css')
  })
})
