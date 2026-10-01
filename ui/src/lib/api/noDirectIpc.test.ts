/**
 * Guard for the api-layer boundary (#473): components must not import
 * `@tauri-apps/*` directly — all backend IPC goes through
 * `ui/src/lib/api/`. This test is what keeps the rule true after the
 * initial migration; a new direct import anywhere in `ui/src` fails CI
 * with a pointer to the module that should be used instead.
 *
 * Sources are gathered via Vite's `import.meta.glob` (raw, eager) so
 * the test needs no Node fs types and stays in sync with the module
 * graph the bundler sees.
 *
 * The desktop build allowed a handful of exceptions for pop-out
 * window plumbing. The mobile app has one window, so the only
 * exceptions left are:
 *   - the api layer itself (it owns the Tauri imports)
 *   - type-only imports from `@tauri-apps/api/event` (`Event`,
 *     `UnlistenFn`), which carry no runtime coupling
 */

import { describe, expect, test } from 'vitest'

const sources = import.meta.glob(
  [
    '../../**/*.svelte',
    '../../**/*.ts',
    '!../../**/*.test.ts',
    '!../../paraglide/**',
    '!../../lib/api/**',
  ],
  { query: '?raw', import: 'default', eager: true },
) as Record<string, string>

/* No file outside the api layer may touch @tauri-apps at runtime. */
const ALLOWED_FILES: RegExp[] = []

/* Import forms allowed anywhere. */
const TYPE_ONLY_EVENT_IMPORT =
  /import\s+type\s+\{[^}]*\}\s+from\s+['"]@tauri-apps\/api\/event['"]/
const WINDOW_MODULES = ['@tauri-apps/api/window', '@tauri-apps/api/webviewWindow']

describe('api-layer boundary', () => {
  test('no direct @tauri-apps usage outside the api layer', () => {
    const offenders: string[] = []
    for (const [file, src] of Object.entries(sources)) {
      if (ALLOWED_FILES.some((re) => re.test(file))) continue
      for (const line of src.split('\n')) {
        if (!line.includes('@tauri-apps')) continue
        if (TYPE_ONLY_EVENT_IMPORT.test(line)) continue
        if (WINDOW_MODULES.some((m) => line.includes(m))) continue
        offenders.push(`${file.replace('../../', 'src/')}: ${line.trim()}`)
      }
    }
    expect(
      offenders,
      'direct @tauri-apps usage found — route it through ui/src/lib/api/ instead (see #473)',
    ).toEqual([])
  })

  /* The mobile app is single-window by design: screens live in the
   * navigation stacks and modals in `App.svelte`. A `new
   * WebviewWindow(...)` anywhere would be a desktop habit that the
   * shell has no plumbing for — no capability entry, no lifecycle,
   * nothing listening. */
  test('no WebviewWindow construction anywhere', () => {
    const offenders: string[] = []
    for (const [file, src] of Object.entries(sources)) {
      for (const line of src.split('\n')) {
        if (line.includes('new WebviewWindow(')) {
          offenders.push(`${file.replace('../../', 'src/')}: ${line.trim()}`)
        }
      }
    }
    expect(
      offenders,
      'this app has one window — use a screen (nav.push) or a Sheet instead',
    ).toEqual([])
  })
})
