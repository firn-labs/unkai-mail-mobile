// `defineConfig` from `vitest/config` is a thin superset of vite's
// — it adds the `test` field's type so the tsc step in `npm run
// check` accepts our test config block.  Behaves identically to
// vite's `defineConfig` for the build / dev paths.
import { defineConfig } from 'vitest/config'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'
import { paraglideVitePlugin } from '@inlang/paraglide-js'

// Paraglide compiles messages from `messages/{locale}.json` into
// `src/paraglide/` on every dev tick + at build time.  See #190.
//   * `outdir` is the generated module the rest of the app imports
//     via `import * as m from './paraglide/messages'`.
//   * `strategy` decides where the active locale comes from at
//     runtime. `custom-unkai` is `src/lib/locale.ts`: the language
//     pinned in Settings → Language, else the first of the device's
//     languages the app ships; `baseLocale` (English) after that.
//     Paraglide's own `localStorage` strategy stored the language it
//     resolved on the first start, so the app never followed a later
//     change of the iPhone's language. Keep this list in step with
//     the `--strategy` of `paraglide:compile` in package.json.
export default defineConfig({
  plugins: [
    paraglideVitePlugin({
      project: './project.inlang',
      outdir: './src/paraglide',
      strategy: ['custom-unkai', 'baseLocale'],
    }),
    svelte(),
    tailwindcss(),
  ],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: 'dist',
  },
  // Vitest config — pure-function tests only.  Stays on node
  // (no jsdom) and stubs `globalThis.localStorage` per test;
  // adding component tests later would need a DOM environment
  // opted in via `// @vitest-environment` comments.
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
})
