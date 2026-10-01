import { describe, expect, it } from 'vitest'
import { negotiateLocale } from './locale'

const SHIPPED = ['en', 'de', 'es', 'fr', 'it', 'pt', 'ja', 'zh', 'ko', 'ru']

describe('negotiateLocale', () => {
  it('takes the first preferred language the app ships', () => {
    expect(negotiateLocale(['nl-NL', 'fr-FR', 'en-US'], SHIPPED)).toBe('fr')
  })

  it('matches by language, not region', () => {
    expect(negotiateLocale(['de-AT'], SHIPPED)).toBe('de')
    expect(negotiateLocale(['pt-PT'], SHIPPED)).toBe('pt')
  })

  it('gives Simplified Chinese to Simplified readers', () => {
    expect(negotiateLocale(['zh-Hans-CN'], SHIPPED)).toBe('zh')
    expect(negotiateLocale(['zh-CN'], SHIPPED)).toBe('zh')
    expect(negotiateLocale(['zh'], SHIPPED)).toBe('zh')
  })

  it('passes over Traditional Chinese for the next preference', () => {
    expect(negotiateLocale(['zh-Hant-TW', 'en-US'], SHIPPED)).toBe('en')
    expect(negotiateLocale(['zh-HK', 'ja-JP'], SHIPPED)).toBe('ja')
    expect(negotiateLocale(['zh-TW'], SHIPPED)).toBeUndefined()
  })

  it('answers undefined when nothing matches', () => {
    expect(negotiateLocale(['nl', 'sv'], SHIPPED)).toBeUndefined()
  })
})
