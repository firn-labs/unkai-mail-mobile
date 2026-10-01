import { describe, expect, it } from 'vitest'
import { CHROME_MAX, CONTENT_MAX, DEFAULT_BODY_PX, SCALE_MIN, scalesFor } from './dynamicType'

describe('scalesFor', () => {
  it('is exactly 1 at the default text size', () => {
    expect(scalesFor(DEFAULT_BODY_PX)).toEqual({ content: 1, chrome: 1 })
  })

  it('scales content and chrome together while both are under their caps', () => {
    // iOS "xxxLarge": 23px body.
    expect(scalesFor(23)).toEqual({ content: 1.353, chrome: 1.3 })
    // "xLarge": 19px.
    expect(scalesFor(19)).toEqual({ content: 1.118, chrome: 1.118 })
  })

  it('caps content at 200% and chrome lower, at the accessibility sizes', () => {
    // AX5, the largest: 53px body.
    expect(scalesFor(53)).toEqual({ content: CONTENT_MAX, chrome: CHROME_MAX })
  })

  it('follows the smallest size down, and no further', () => {
    // "xSmall" is 14px, which is the floor itself.
    expect(scalesFor(14).content).toBeCloseTo(SCALE_MIN, 3)
    expect(scalesFor(8).content).toBeCloseTo(SCALE_MIN, 3)
  })

  it('treats a failed measurement as the default rather than collapsing the text', () => {
    expect(scalesFor(Number.NaN)).toEqual({ content: 1, chrome: 1 })
    expect(scalesFor(0)).toEqual({ content: 1, chrome: 1 })
  })
})
