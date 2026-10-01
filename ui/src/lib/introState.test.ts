import { describe, expect, it } from 'vitest'
import { introSeen, introVariantFor, markIntroSeen } from './introState'

function memoryStore() {
  const data = new Map<string, string>()
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
  }
}

describe('introVariantFor', () => {
  it('shows nothing without accounts', () => {
    expect(introVariantFor([])).toBeNull()
  })

  it('shows the test-mode tour for the demo alone', () => {
    expect(introVariantFor([{ demo: true }])).toBe('demo')
  })

  it('prefers the real-account tour as soon as one real account exists', () => {
    expect(introVariantFor([{ demo: true }, { demo: false }])).toBe('normal')
    expect(introVariantFor([{}])).toBe('normal')
  })
})

describe('introSeen / markIntroSeen', () => {
  it('remembers each variant on its own', () => {
    const store = memoryStore()
    expect(introSeen('demo', store)).toBe(false)
    markIntroSeen('demo', store)
    expect(introSeen('demo', store)).toBe(true)
    expect(introSeen('normal', store)).toBe(false)
  })

  it('never throws when storage does', () => {
    const broken = {
      getItem: () => {
        throw new Error('denied')
      },
      setItem: () => {
        throw new Error('denied')
      },
    }
    // Unreadable storage counts as "seen", so a broken store cannot
    // make the introduction appear on every launch.
    expect(introSeen('demo', broken)).toBe(true)
    expect(() => markIntroSeen('demo', broken)).not.toThrow()
    expect(introSeen('demo', null)).toBe(false)
  })
})
