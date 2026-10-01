# `ui/` — the mobile frontend

Svelte 5 + TypeScript + Vite, built as a phone app: five tabs, one
navigation stack per tab, bottom sheets, swipe triage, safe-area-aware
chrome. It talks to the Rust side exclusively through the typed IPC
layer in [`src/lib/api/`](src/lib/api/).

Read [`../CLAUDE.md`](../CLAUDE.md) for the conventions — navigation
model, layout primitives, the component vocabulary, the mail-rendering
pipeline, and the i18n rule.

## Layout

```
src/
├── App.svelte        # Shell: boot, tabs, stacks, modals, backend events
├── app.css           # Skeleton themes + the mobile layout primitives
├── main.ts           # One entry point, one window
└── lib/
    ├── api/          # Typed IPC — one wrapper per #[tauri::command]
    ├── ui/           # NavBar, TabBar, Sheet, ActionSheet, SwipeRow, …
    ├── screens/      # One file per screen
    ├── mail/         # Render pipeline, envelope loading, folders, replies
    ├── icons/        # The 129-icon family (shared with the desktop app)
    └── *.ts          # Pure logic, unit-tested (node, no DOM)
```

## Commands

Run these from the repo root (`npm run ios:dev` etc.) or here:

```bash
npm run dev      # Vite dev server — the Tauri CLI starts this for you
npm run build    # production bundle into dist/
npm run check    # paraglide compile + svelte-check + tsc
npm test         # vitest (pure-function tests only)
```

## Two rules worth repeating

1. **Never import `@tauri-apps/*` in a component.** Everything goes
   through `lib/api/`. `lib/api/noDirectIpc.test.ts` enforces it.
2. **Every user-visible string is a paraglide message**, added to both
   `messages/en.json` and `messages/de.json` in the same change.
