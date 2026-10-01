/**
 * Every message the app shows exists in every language it ships.
 *
 * Paraglide falls back to English for a key a locale lacks, silently:
 * a string added to `en.json` and `de.json` alone would show up in
 * English for a Japanese or Russian user, and nothing would say so.
 * This test is what makes "add it to every language" a rule rather
 * than a hope (CLAUDE.md → Copy and i18n).
 *
 * "Shown" means referenced as `m.key` somewhere in `ui/src`. The
 * desktop-era keys still in `en.json` / `de.json` that no mobile screen
 * uses are not required elsewhere — translating dead strings into eight
 * languages would be work nobody sees.
 *
 * Placeholders must match English exactly: a `{count}` lost in
 * translation renders as nothing, and an invented `{name}` renders as
 * the literal text.
 */
import { describe, expect, test } from 'vitest'
import settings from '../../project.inlang/settings.json'

const sources = import.meta.glob(
  ['../**/*.svelte', '../**/*.ts', '!../**/*.test.ts', '!../paraglide/**'],
  { query: '?raw', import: 'default', eager: true },
) as Record<string, string>

const catalogs = import.meta.glob('../../messages/*.json', {
  import: 'default',
  eager: true,
}) as Record<string, Record<string, string>>

function catalog(locale: string): Record<string, string> {
  const entry = Object.entries(catalogs).find(([path]) => path.endsWith(`/${locale}.json`))
  if (!entry) throw new Error(`messages/${locale}.json is missing`)
  return entry[1]
}

// Only files that import the messages: elsewhere `m` is just a name
// (a Map's `m.set(…)`).
const used = new Set<string>()
for (const text of Object.values(sources)) {
  if (!text.includes('paraglide/messages')) continue
  for (const match of text.matchAll(/\bm\.([A-Za-z0-9_]+)\b/g)) used.add(match[1])
}

const english = catalog('en')
const placeholders = (s: string) => [...s.matchAll(/\{[A-Za-z_]+\}/g)].map((x) => x[0]).sort()

describe('message catalogs', () => {
  test('every m.key the app uses exists in English', () => {
    expect([...used].filter((key) => !(key in english))).toEqual([])
  })

  for (const locale of settings.locales) {
    test(`${locale}: every used message is translated, placeholders intact`, () => {
      const messages = catalog(locale)
      const missing = [...used].filter((key) => key in english && !messages[key]?.trim())
      expect(missing).toEqual([])
      const broken = [...used].filter(
        (key) =>
          messages[key] &&
          placeholders(messages[key]).join() !== placeholders(english[key] ?? '').join(),
      )
      expect(broken).toEqual([])
    })
  }
})
