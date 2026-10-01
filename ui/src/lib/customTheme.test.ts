import { describe, expect, it } from 'vitest'
import {
  CUSTOM_THEME_ID,
  SHADES,
  buildScale,
  buildThemeCss,
  contrastInk,
  fontStack,
  hexToOklch,
  normaliseHex,
  oklchToHex,
} from './customTheme'

describe('normaliseHex', () => {
  it('accepts the shapes a text field actually produces', () => {
    expect(normaliseHex('#1E2A40')).toBe('#1e2a40')
    expect(normaliseHex('1e2a40')).toBe('#1e2a40')
    expect(normaliseHex('  #ABC  ')).toBe('#aabbcc')
    expect(normaliseHex('abc')).toBe('#aabbcc')
  })

  it('falls back rather than throwing on half-typed input', () => {
    // The editor calls this on every keystroke, so "#1e2a4" has to be
    // survivable — an exception here would take the settings screen down.
    expect(normaliseHex('#1e2a4')).toBe('#808080')
    expect(normaliseHex('')).toBe('#808080')
    expect(normaliseHex('nonsense', '#ff0000')).toBe('#ff0000')
  })
})

describe('OKLCH round trip', () => {
  it('returns the colour it was given', () => {
    for (const hex of ['#1e2a40', '#dc2626', '#16a34a', '#ffffff', '#000000', '#7c3aed']) {
      expect(oklchToHex(hexToOklch(hex))).toBe(hex)
    }
  })

  it('places greys at zero chroma', () => {
    expect(hexToOklch('#808080').c).toBeCloseTo(0, 3)
  })

  it('orders lightness the way the eye does', () => {
    // The whole point of OKLab over HSL: yellow must not claim the same
    // lightness as blue. In HSL both are 50%.
    const yellow = hexToOklch('#ffff00').l
    const blue = hexToOklch('#0000ff').l
    expect(yellow).toBeGreaterThan(blue + 0.3)
  })
})

describe('buildScale', () => {
  it('produces every shade the theme needs', () => {
    const scale = buildScale('#3b82f6')
    for (const shade of SHADES) {
      expect(scale[shade]).toMatch(/^#[0-9a-f]{6}$/)
    }
  })

  it('runs light to dark, monotonically', () => {
    const scale = buildScale('#3b82f6')
    const lightness = SHADES.map((s) => hexToOklch(scale[s]).l)
    for (let i = 1; i < lightness.length; i++) {
      expect(lightness[i]).toBeLessThan(lightness[i - 1])
    }
  })

  it('lands shade 500 at the same lightness whatever the hue', () => {
    // This is what makes an arbitrary user colour safe to build on: the
    // contrast relationships in the layout do not move when the colour
    // does.
    const shades = ['#ff0000', '#00ff00', '#0000ff', '#ffff00'].map(
      (hex) => hexToOklch(buildScale(hex)[500]).l,
    )
    for (const l of shades) expect(l).toBeCloseTo(shades[0], 2)
  })

  it('keeps the hue it was given', () => {
    const hue = hexToOklch('#3b82f6').h
    for (const shade of [300, 500, 700] as const) {
      expect(hexToOklch(buildScale('#3b82f6')[shade]).h).toBeCloseTo(hue, 0)
    }
  })

  it('tapers chroma at both ends so the ramp stays in gamut', () => {
    const scale = buildScale('#ff0000')
    const mid = hexToOklch(scale[500]).c
    expect(hexToOklch(scale[50]).c).toBeLessThan(mid)
    expect(hexToOklch(scale[950]).c).toBeLessThan(mid)
  })

  it('flattens toward neutral when asked, for surfaces', () => {
    const vivid = hexToOklch(buildScale('#ff0000', 1)[500]).c
    const surface = hexToOklch(buildScale('#ff0000', 0.28)[500]).c
    expect(surface).toBeLessThan(vivid * 0.4)
  })
})

describe('contrastInk', () => {
  it('picks ink that can actually be read', () => {
    expect(contrastInk('#ffffff')).toBe('#000000')
    expect(contrastInk('#000000')).toBe('#ffffff')
    expect(contrastInk('#1e2a40')).toBe('#ffffff')
    expect(contrastInk('#fef3c7')).toBe('#000000')
  })
})

describe('fontStack', () => {
  it('resolves the known styles', () => {
    expect(fontStack('rounded')).toContain('ui-rounded')
    expect(fontStack('serif')).toContain('ui-serif')
    expect(fontStack('mono')).toContain('ui-monospace')
  })

  it('falls back to system for anything unknown', () => {
    expect(fontStack('nope')).toBe(fontStack('system'))
    expect(fontStack('')).toBe(fontStack('system'))
  })
})

describe('buildThemeCss', () => {
  const css = buildThemeCss({ base: '#1e2a40', accent: '#3b82f6', font: 'rounded' })

  it('scopes to the custom theme so stock themes survive', () => {
    expect(css).toContain(`[data-theme='${CUSTOM_THEME_ID}']`)
  })

  it('emits every role the app reads', () => {
    for (const role of ['surface', 'primary', 'secondary', 'tertiary', 'success', 'warning', 'error']) {
      expect(css).toContain(`--color-${role}-500:`)
      expect(css).toContain(`--color-${role}-contrast-500:`)
    }
  })

  it('emits all eleven shades per role', () => {
    for (const shade of SHADES) expect(css).toContain(`--color-surface-${shade}:`)
  })

  it('keeps semantic colours out of the user\'s hands', () => {
    // An accent of red must not make "success" red too — the meaning
    // lives in the hue and has to survive any theme.
    const redAccent = buildThemeCss({ base: '#1e2a40', accent: '#dc2626', font: 'system' })
    const greenAccent = buildThemeCss({ base: '#1e2a40', accent: '#16a34a', font: 'system' })
    const successOf = (s: string) => s.match(/--color-success-500: (#[0-9a-f]{6})/)![1]
    expect(successOf(redAccent)).toBe(successOf(greenAccent))
  })

  it('applies the chosen font', () => {
    expect(css).toContain('ui-rounded')
  })
})
