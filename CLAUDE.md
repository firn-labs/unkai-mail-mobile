# Unkai Mail — Mobile

> **This is the stable app.** A sibling copy, `../Unkai Mobile Lab`,
> is where UI ideas are tried first. The two are kept strictly apart:
> a change goes into whichever one the request names, and features
> move from the Lab to here only when that is asked for explicitly.
>
> | | Stable (this folder) | Lab |
> |---|---|---|
> | Folder | `Unkai Mobile Try2` | `Unkai Mobile Lab` |
> | Bundle id | `com.unkai.mail` | `com.unkai.lab` |
> | Name on the phone | Unkai Mail | Unkai Lab |
> | Icon | navy (`storm`) | red (`lab-red`) |
>
> **When porting from the Lab, never bring its identity along.** That
> lives in `tauri.conf.json` (`productName`, `identifier`, window
> title), `src-tauri/Info.ios.plist` (`CFBundleDisplayName`),
> `gen/apple/unkai-mobile_iOS/de.lproj/InfoPlist.strings`
> (`CFBundleDisplayName` again — the German home-screen name),
> `gen/apple/project.yml` + `project.pbxproj` (bundle id, product
> name), the root `package.json` name, `app-icon.png` and the
> generated icon sets, `logos/unkai-logo-lab/` and its `lab-red` entry
> in `src-tauri/src/logo.rs`, and every `unkai-logo://lab-red` in the
> UI (here it is `unkai-logo://storm`). Miss one and the stable app
> ships with the Lab's name or icon.
>
> The Lab's features up to 2026-09-19 (test mode, introduction, cloud
> picker, design system, user-built theme, accessibility pass, privacy
> notice) were ported here on 2026-09-30. Its redesign — v2 (structure)
> and v3 "Wolkenmeer" (look) — followed on 2026-10-01, **without** the
> Lab's Undo for moves, which is a feature rather than design and had not
> run on a device yet (see `SwipeRow` below and `docs/design/`).

## Vision

Unkai Mail on a phone: a native iOS app (Android next) built on the
same Rust core as the desktop client, standing out through deep
**Nextcloud integration**. Same protocols, same encrypted local
store, same Nextcloud surfaces — a UI designed for one hand and a
390pt screen instead of a three-pane desktop window.

This repository is a **port of the desktop app**
([firn-labs/unkai-mail](https://github.com/firn-labs/unkai-mail)),
not a companion app: the `crates/` workspace is shared code that
compiles unchanged for iOS. What was rewritten is the Tauri shell
and the entire frontend.

## Key Differentiators

- **Nextcloud Talk integration** — browse and create Talk rooms, drop a join link into a message
- **Nextcloud Files integration** — attach a *share link* instead of a 40 MB file, browse and save files
- **Contact & Calendar sync** — CardDAV + CalDAV against Nextcloud (or any DAV server)
- **The phone's own calendars and contacts** — EventKit + Contacts as a first-class source, so whatever iCloud / Google / Exchange account iOS already holds shows up next to the Nextcloud ones
- **Encrypted local store** — SQLCipher cache, optionally gated behind a passphrase
- **End-to-end mail encryption** — OpenPGP and S/MIME, keys imported from the Files app

## Tech Stack

- **Language:** Rust (core logic, protocol handling, backend)
- **App framework:** Tauri 2 mobile (Rust static library inside a UIKit host + WKWebView)
- **Frontend:** Svelte 5 + TypeScript + Vite
- **Styling:** Tailwind CSS 4 + Skeleton theme variables (no component library — the mobile UI is hand-built)
- **Platform targets:** iOS 15+ today; Android via `tauri android` next

## Project Structure

```
unkai-mobile/
├── Cargo.toml              # Workspace root
├── package.json            # Tauri CLI + the ios:* / android:* scripts
├── crates/                 # UNCHANGED from the desktop app
│   ├── unkai-core/        # Shared types, models, error handling
│   ├── unkai-imap/        # IMAP mail retrieval
│   ├── unkai-smtp/        # SMTP mail sending
│   ├── unkai-jmap/        # JMAP modern mail access
│   ├── unkai-caldav/      # CalDAV calendar sync
│   ├── unkai-carddav/     # CardDAV contact sync
│   ├── unkai-nextcloud/   # Nextcloud API (Talk, Files, OCS)
│   ├── unkai-store/       # Local storage, caching, keychain
│   ├── unkai-discovery/   # Account autoconfiguration (SRV, autoconfig)
│   ├── unkai-crypto/      # OpenPGP + S/MIME primitives
│   ├── unkai-mcp/         # Local MCP server (desktop-only; parked here, never started)
│   ├── unkai-system/      # NEW, mobile-only: the device's own calendar/contacts/reminders (EventKit + Contacts)
│   └── unkai-commands/    # Transport-agnostic application layer
├── src-tauri/              # The MOBILE shell (command shims + iOS chrome)
│   ├── src/lib.rs         # Command shims + `run()` (the mobile entry point)
│   ├── src/state.rs       # The one app context
│   ├── src/notifier.rs    # `UiNotifier` impl → Tauri events
│   ├── src/logo.rs        # Embedded brand art for the `unkai-logo://` scheme
│   ├── Info.ios.plist     # iOS plist keys merged at build time
│   └── gen/apple/         # Generated Xcode project (committed)
└── ui/                     # The MOBILE frontend (Svelte 5 + TypeScript + Vite)
    └── src/
        ├── App.svelte      # Shell: boot, tabs, stacks, modals, events
        ├── app.css         # Themes + the mobile layout primitives
        └── lib/
            ├── api/        # Typed IPC layer (one wrapper per command)
            ├── ui/         # Mobile component vocabulary (NavBar, Sheet, SwipeRow, …)
            ├── screens/    # One file per screen
            ├── mail/       # Rendering pipeline, envelope loading, folders, replies
            └── icons/      # The icon family: the desktop's 129 + `Moon` (mobile only)
```

## Protocols & Integrations

| Protocol/API | Purpose | Crate |
|---|---|---|
| IMAP | Mail retrieval | `unkai-imap` |
| SMTP | Mail sending | `unkai-smtp` |
| JMAP | Modern mail access (where supported) | `unkai-jmap` |
| CalDAV | Calendar sync (Nextcloud + others) | `unkai-caldav` |
| CardDAV | Contact sync (Nextcloud + others) | `unkai-carddav` |
| Nextcloud OCS/API | Talk rooms, file sharing, app integrations | `unkai-nextcloud` |
| EventKit / Contacts | The device's own calendars, reminders and address book | `unkai-system` |

## What the mobile port dropped, and why

This is the ledger. When something from the desktop app seems
missing, it is missing **on purpose** — check here before adding it
back.

| Dropped | Why |
|---|---|
| System tray, tray badge, menus | No such surface on a phone. |
| Multi-window (pop-out compose / reader / event editor) | One window. Pop-outs became screens and sheets. |
| The multi-profile *runtime* (`ProfileRegistry`, profile windows, switch-in-place) | One window, one user. The storage layout still is profile-scoped (`profiles/<id>/`), so a future switcher means swapping a handle in `AppState`, not reintroducing a window registry. |
| In-app updater (`tauri-plugin-updater`) | The App Store / TestFlight / a sideload is the update channel. |
| `mailto:` deep links, `.eml` / `.ics` file associations, single-instance argv | iOS hands third-party apps neither the default-mail-handler role nor a second-launch argv. |
| Autostart | No login items on iOS. |
| The local MCP server | A localhost HTTP endpoint nothing on the phone can reach. The crate stays as a parked handle (`state.rs`) because `update_app_settings` takes one; `mcp_enabled` is pinned `false`. |
| Hardware security keys (WebAuthn PRF unlock) | No security-key port. The *passphrase* half of the same key envelope is kept and fully wired (`fido_*` commands). |
| System-font enumeration, printing, native "Save As" | No font catalogue, no printer dialog, no user-chosen save path. Saving goes to the app's Documents directory (visible in Files). |
| Settings export/import to a chosen file | Nextcloud is the transport (`ncProbe` / `ncRestore`), which is also how settings reach the phone from a desktop install. |
| Rich-text compose (Tiptap), the Markdown code editor (CodeMirror) | See "Compose is plain text" below. Removed from `package.json` too. |
| Imported custom themes | They ride a runtime `<link>` to a user-chosen CSS file; no such path here. The 22 stock themes stay. |

## Architecture Principles

- **Separation of concerns** — the Rust core handles all protocol/business logic; the UI is a thin presentation layer
- **Offline-first** — every screen paints from the local cache before the network is touched, and stays useful with no signal
- **Security-first** — TLS everywhere, credentials in the iOS keychain, SQLCipher at rest, no plaintext secrets
- **Modular design** — each protocol as its own crate, shared verbatim with the desktop app
- **One window, one context (`src-tauri/src/state.rs`)** — every `#[tauri::command]` shim that touches profile state takes `state: State<'_, AppState>` and resolves once through `app_ctx(&state)`; never an inline lookup. The desktop's window-label→profile map has no mobile counterpart.
- **Storage stays profile-scoped** — `unkai_store::ProfilePaths` still owns every path (`profiles/<id>/cache.db` + settings, one SQLCipher key per profile in the keychain). The root comes from Tauri's `app_config_dir()`, not `dirs::config_dir()`: on iOS only the sandbox is writable.
- **The shell never grows logic** — a shim extracts state, delegates to `unkai-commands`, returns. Anything longer belongs in the application layer.

## Frontend ↔ backend IPC: the `api/` layer

All backend IPC goes through the typed layer in
[`ui/src/lib/api/`](ui/src/lib/api/) — **never import `@tauri-apps/*`
directly in a component.** A vitest guard
(`ui/src/lib/api/noDirectIpc.test.ts`) fails the build on violations.

- **Commands**: one typed wrapper per `#[tauri::command]`, grouped into domain modules (`api/mail`, `api/compose`, `api/accounts`, `api/contacts`, `api/calendar`, `api/nextcloud`, `api/talk`, `api/notes`, `api/tasks`, `api/crypto`, `api/settings`, `api/system`), all funnelling through `call()` in `api/core.ts`. Components do `import * as api from '../api'`. **When you add/rename/change a Rust command, update its wrapper in the matching domain module in the same PR.**
- **Events**: every backend push channel is registered in `AppEventPayloads` in `api/events.ts`. Subscribe with `api.onAppEvent('new-mail', handler)`. Adding a new event = adding it to the registry first. (The desktop's popout→main handoff events are gone: with one window those are function calls.)
- **Platform affordances** (the file picker, local notifications, `convertFileSrc` asset URLs) live in `api/platform.ts` — this file IS the list of non-command platform surface.
- **DTO types**: `api/types.ts` holds placeholder `any` aliases for backend DTOs. Tightening them is **lazy** — replace an alias with a real interface whenever you touch code that consumes it.

## Backend command layer: the `unkai-commands` crate

The backend twin of the `api/` layer. Every `#[tauri::command]` body,
the background loops, and the crypto bridge live in
`crates/unkai-commands` — a crate with **no `tauri` dependency**.
`src-tauri/src/lib.rs` holds only thin shims plus the mobile chrome.

- **Domain modules mirror `ui/src/lib/api/` exactly.** A command and its typed frontend wrapper always share a name and a domain. **Adding a Rust command = body in the matching `unkai-commands` module + a shim in `lib.rs`'s alphabetical `generate_handler![]` + a wrapper in the matching `api/` module, in one PR.**
- **`UiNotifier` (`commands::notify`) is the only channel back to the UI.** The application layer never emits Tauri events; it calls trait methods. `src-tauri/src/notifier.rs` (`MobileNotifier`) is the mobile implementation — a plain broadcast, since there is one window. Adding a push channel = a trait method + a `MobileNotifier` impl + an `api/events.ts` registry entry.
- **A synchronous shim is `#[tauri::command(async)]`, never a bare `#[tauri::command]`.** Tauri runs a non-async command on the *main thread* — on iOS the thread that draws the webview and delivers its touches — so every cache read, keychain lookup and vCard parse behind a bare attribute froze the screen while it ran, on every navigation. `(async)` runs the same function on Tauri's thread pool. The only exceptions are `open_url` and `open_app_settings`, which hand off to UIKit (module header of `src-tauri/src/lib.rs`).
- **Keep `unkai-commands` Tauri-free.** Anything needing an `AppHandle` belongs in `src-tauri`. The payoff is that the whole application layer compiles and tests without a Tauri runtime — which is also why the iOS port was a shell rewrite and not a rewrite of the app.

## Mobile UI conventions

The desktop's glass-panel/IconRail/three-pane vocabulary does not
apply here. What follows does. The primitives live in
`ui/src/app.css` and `ui/src/lib/ui/`; **reach for them before
writing new markup.**

### The demo account ("Testmodus")

The **Testmodus** pill at the top left of the first-run screen seeds the
local cache with a mail account, folders, messages, contacts and a
calendar, so the whole app is usable with no server, no network and no
fixture process. `crates/unkai-commands/src/demo.rs`; the flag is
`Account::demo` (SQLite column + `#[serde(default)]`).

- **Leaving it is a first-class action.** Settings opens with a
  test-mode card whose **Testmodus verlassen** (behind a `Confirm`, since
  it deletes) runs `remove_demo_account` and lets `App.svelte` route: to
  the first-run screen when no account is left, into the app otherwise.
  Removing the demo from Settings → Accounts goes through the same
  cleanup (`accounts::remove_account` routes the demo id to
  `demo::remove_demo_data`).
- **Removal takes everything, and is idempotent.** It used to delete
  only the two account rows; messages, folders, contacts, calendars and
  task lists are keyed by the account id but *not* foreign-keyed to it,
  so the orphans kept the unread badge at the demo's count and the demo's
  people in the composer's suggestions. `demo.rs` has tests for the full
  wipe, for keeping real accounts, and for a half-removed demo.
- **No accounts left → first-run screen.** `App.svelte` watches the
  account list; before, removing the last account left the app on empty
  tabs.

- **The sample data follows the UI's language** (2026-10-02): German for a
  German UI, English for the other nine (`DemoLanguage::for_locale`; the
  UI passes `getLocale()` to `create_demo_account`). It was German only,
  so an English user started test mode into a German inbox. Each fixture
  holds both texts side by side (`lang.pick(en, de)`) rather than two
  parallel lists, and `both_languages_seed_the_same_states` holds them to
  the same states; `english_sample_data_has_no_german_left_in_it` catches
  a missed translation. **A new fixture needs both texts.** Sender
  addresses are the same in both languages (contact cards match by
  address); the English phone numbers are in the fictional 555-01xx range.
  Two fixes rode along: the vCards had a stray indent that vCard folding
  read as a continuation of `FN` (every contact's name swallowed its email
  line on an edit), and "today" in the calendar was 09:00 *UTC*, so after
  local midnight in Europe and every evening in the Americas the day's
  events sat on the wrong date — it is local time now (`today_at_nine`).
- **It is not a fake server.** Nothing speaks IMAP or SMTP — the data
  is written straight into the cache the app already reads from. That
  is what makes it cheap *and* honest: the app is offline-first, so
  every read path already goes through the cache and is exercised
  exactly as it would be against a real server.
- **Only the write-to-server paths are gated**, and `poll_folder` is
  the one gate that covers all of sync — both unified fetches and the
  background loop reach the server through it. Without that, a demo
  account would spend a connect timeout on `demo.invalid` (RFC 2606,
  so it can never resolve) and report a network error for a mailbox
  the user can see working.
- **"Already archived" is the backend's call.** `get_archive_folder` exposes the folder `archive_message` would use; `mail/archive.ts` caches it per account, and a message already in it is offered **Move to Inbox** instead of Archive (list swipe, action sheet, multi-select bar, reader). Guessing from folder names disagreed with the backend, and archive-to-self is a silent no-op there — the row vanished and came back on reopen. Batch results are honoured too: only UIDs the batch command returns are removed from the list.
- **Local edits really work.** Read/unread, flag, archive, delete and
  move all mutate the cache, so archive genuinely moves the message to
  Archive rather than just dropping the row — see
  `demo::move_cached_message`. A demo where the gesture only *looks*
  like it worked would test nothing. The batch commands (`move_messages`,
  `archive_messages`) have the same cache-only branch as the single ones.
- **Sending is refused, never silently swallowed.** "Sent" for a
  message that went nowhere is the one outcome a mail client must not
  report. The backend refuses before anything is queued (the real
  guard); `Compose` also pre-checks `account.demo` so the message is
  localised, because backend strings stay English by convention.
- **Attachments are metadata only.** The chips render, the download
  reports there is no file. Fabricating a PDF would be worse.
- **Fixtures cover states, not plausibility**: unread, flagged,
  pinned, high priority, attachments, HTML and plain text, a two-
  message thread, a 640px newsletter that exercises the renderer's
  width clamping, and one message in each of the other folders. The
  contacts deliberately overlap the mail senders, because the mail
  list resolves a sender's card and the composer autocompletes from
  them — an overlap-free demo would leave those paths untested.

Groupware rides a `DavSourceKind::Local` Nextcloud account, which
already means "no remote, sync is a no-op, writes stay in the cache".
Calendar, Contacts and Tasks needed **no** demo-specific code as a
result: they loop over Nextcloud accounts and this is simply one.

Two shapes worth knowing before adding fixtures: `ContactRow.vcard_raw`
and `CalendarEventRow.ics_raw` are **not** optional, and every write
path round-trips them. Filling either with a placeholder is a trap that
only surfaces later, as a corrupted record after an edit — so the
fixtures emit real vCard 3.0 and real VEVENTs.

### The design system

Every size, colour, radius and duration comes from a token in
`ui/src/app.css` under `DESIGN SYSTEM`. **Nothing hard-codes a
value.** That indirection is the redesign: the old UI chose its
numbers per component — a 15px here, a 0.75rem there, the same
`color-mix(… 18%)` written out in nine places — which is why it read
as a collection of screens rather than one product. Nothing was wrong
individually; nothing agreed either.

Tokens derive from the active Skeleton theme's colour scale, so all 23
themes (22 stock + the house theme) keep working and keep their own
character.

**v3 — "Wolkenmeer", the same day's second pass** ("modern, simple, and
not a template"). v2 below fixed the *structure*; v3 replaced the *look*,
which still read as a stock kit — a pastel initials circle on every mail
row, a rainbow of settings tiles, the platform blue, one identical card
with one soft shadow around everything. The critique, the palette, the
audits and before/after screenshots are in
`docs/design/2026-09-30-v3-wolkenmeer.md`. The rules, each carried by a
token or a primitive (header of `DESIGN SYSTEM` in `app.css`):

- **Two colours, two jobs.** The accent — **petrol** in the house theme —
  is what you can *do*. The **signal** — Morgenrot, the first light on a sea of clouds —
  is what is *new*: `--ui-unread`, the tab badge and folder counts
  (`--ui-now-fill` / `--ui-on-now`, `.count-pill--signal`), today in the
  calendar. `--ui-signal-ink/-fill` + `--ui-on-signal` are solved from
  the *secondary* scale for every theme, but only `[data-theme='unkai']`
  maps the roles onto them; a stock theme's secondary was never chosen to
  mean "unread", so stock themes keep unread = accent.
- **Rounded speaks, the text face reads.** `--font-display` is the
  theme's heading family — SF Pro Rounded (`ui-rounded`) in the house
  theme, `inherit` elsewhere. Screen titles, section titles, a message's
  subject, calendar numerals. Both are system fonts: nothing bundled.
- **Boxes only for groups.** Cards are flat (no shadow). The mail list,
  the contact list, the reader and compose sit on the page; settings and
  forms stay in cards.
- **One left edge**, `--gutter` (20pt): titles, list text, section
  titles, card edges; `.page-inset` for content placed directly on the
  page. Marks (the unread dot) hang in the margin before it.
- **Bars float.** `TabBar`, the reader's toolbar, the multi-select bar
  and bottom `Sheet`s are capsules above the home indicator on `--ui-bar`
  with `--e-float`. The tab bar overlays the screen and publishes its
  measured height as `--bar-clearance` on `<html>`; `.screen__body` and
  `.ptr__scroller` pad (and `scroll-padding`) by it, `Fab` and the toasts
  sit above it. **A new bottom-anchored element reads `--bar-clearance`.**
- **Row glyphs are muted ink.** `.icon-tile` is a bare glyph in
  `--ui-ink-muted`; only `--warning` / `--danger` colour it, and
  `--lg` gives it a quiet ground. `FeatureDef.tone` is gone.

**v2 — the 2026-09-30 redesign** ("customer-oriented, simply structured,
modern"). The critique it answers and the three principles behind it are
in `docs/design/2026-09-30-redesign.md`: *mail first*, *one labelled
primary action per screen*, *a calm surface with one accent*. What it
changed, in the order a reader of this file needs it:

- **A house theme, `unkai`** (`ui/src/themes/unkai.css`), the default for
  new installs (`DEFAULT_THEME_ID`, and `AppSettings::default` /
  `remove_custom_theme` in Rust). The system font, cool near-neutral
  surfaces, one blue accent. Shade 500 of accent, error and success is
  dark enough for a **white** label at 4.5:1 — the solver uses 500 as the
  fill of buttons and swipe actions and, correctly, picks black when white
  fails; a black label on a blue button read as dated. It is a local
  theme, so `accessibleColors.test.ts` reads `@import './themes/…'` from
  `app.css` as well as the Skeleton ones.
- **Type one step larger** (body 16, display 32), **radii one step
  rounder** (cards 20, sheets 28), **light-mode cards without borders**
  (`--ui-card-border` is transparent in light, a hairline in dark).
- **Tints are tokens**: `--ui-tint-accent/-success/-warning/-danger`,
  `--ui-press`, `--ui-fill-quiet`. Each replaced the same `color-mix()`
  written out in several components.
- **New shapes** (`app.css`): `.icon-tile` (+ `--success/--warning/
  --danger/--neutral/--lg`), `.tile-grid` + `.tile`, `.search-trigger`,
  `.section-title`, `.screen--plain`, `.wide-button--plain`,
  `.bar-button--filled`, `.bar-button--labelled`. `Fab` is an extended
  pill with its label.

| Group | Tokens | Notes |
|---|---|---|
| Type | `--t-display` … `--t-micro`, `--t-bar-*`, `--t-input`, `--t-control` | Each step carries its own line-height; all follow Dynamic Type (see Accessibility) |
| Space | `--s-1` … `--s-12`, `--gutter` | 4pt grid; `--gutter` (20pt) is the page's one left edge |
| Radius | `--r-xs` … `--r-full` | A hierarchy: 18px card, 28px sheet and floating bar, capsule for anything you tap to go |
| Face | `--font-display` | The theme's heading family — see v3 |
| Surface | `--ui-canvas`, `--ui-card`, `--ui-plain`, `--ui-card-border`, `--ui-hairline`, `--ui-border` | See below |
| Tint | `--ui-tint-*`, `--ui-press`, `--ui-fill-quiet` | Washes. Text on one is ink, never the same role's ink |
| Ink | `--ui-ink`, `--ui-ink-muted`, `--ui-ink-faint` | Three levels of emphasis, not ad-hoc greys — **generated**, contrast-guaranteed |
| Colour as text / fill | `--ui-accent-ink`, `--ui-danger-ink`, … · `--ui-accent-fill` + `--ui-on-accent`, … · `--ui-control` | **Generated** per theme; never use a raw `--color-*-500` for text or a label on a fill |
| Elevation | `--e-1` … `--e-3`, `--e-float` | Cards have none (v3); `--e-float` for what floats over content |
| Layout | `--ui-bar`, `--bar-clearance` | The floating bars' surface; the tab bar's measured footprint |
| Motion | `--m-fast/base/slow`, `--m-ease`, `--m-spring` | One easing curve for the whole app |
| Accent | `--ui-unread`, `--ui-flagged`, `--ui-urgent`, `--ui-secure`, `--ui-done`, `--ui-now-fill` / `--ui-on-now` | Roles, never "the amber one"; unread and "now" ride the signal in the house theme |

Four decisions are load-bearing:

- **The canvas is recessed and cards sit above it.** The old UI painted
  the page and its rows the same colour, so a "card" could only be
  suggested with a border. Giving the layout a z-axis is what makes
  grouping something the eye reads instead of something a divider
  asserts — and it is why dark mode drops the canvas to near-black:
  you cannot lift a card off an already-light-grey ground.
- **`.list-group` is a card.** Because every screen already used it,
  that one rule carried the redesign across all ~32 of them without
  touching each. Prefer changing a primitive over restyling a screen.
- **Hierarchy over density.** Sender at `--t-headline` (700 when
  unread), subject at `--t-callout`; rows are ≥52px and mail rows 72px,
  with **no avatar** (v3: 66pt of every row for two letters the name
  already spells).
  On a phone you can only read a handful of rows per glance regardless,
  so the win from cramming in one more is smaller than the cost to every
  glance. Mail rows are **two lines, always**: status glyphs trail the
  subject instead of owning a third line that stayed empty on most rows.
- **Unread has three signals, none of them alone:** a dot hanging in the
  margin (the signal colour), the sender's weight, and the subject in
  full ink; the date goes to full ink too. (v1 tinted the whole row and
  drew a rail — read as *selected*; v2 also coloured the date, a column
  of accent that repeated the dots.)
- **Lists that want width are plain.** `.screen--plain` re-points
  `--ui-canvas` (and `--ui-row`, which `SwipeRow` paints) at `--ui-plain`,
  so the mail list runs edge to edge: card margins cost every row ~32px
  of subject line. The contact list (sticky letter headings) and the
  reader are plain too. Grouped content (settings, folders) stays in cards.
- **Settings are scanned by their headings, not by colour** (v3). v2 led
  every row with a category-coloured tile; v3 draws the glyph alone in
  muted ink — see above.

### The user-built theme

Settings → Darstellung → *Eigenes Theme* lets the user generate a whole
theme from three choices: a **base colour** (surfaces), an **accent**
(primary) and a **typeface**. Stored as three fields on `AppSettings`
(`custom_theme_base` / `_accent` / `_font`) and compiled to CSS by
`ui/src/lib/customTheme.ts`.

- **Eleven shades are derived, not stored.** The user supplies one
  colour per role; `buildScale` produces the ramp. Keeping the settings
  file down to three strings also means the generator can improve
  without a migration.
- **The maths is OKLab, and that is not incidental.** HSL's "lightness"
  is not lightness — yellow and blue at `hsl(… 50%)` differ about
  sixfold in perceived brightness — so an HSL ramp is uneven in a
  different way for every hue, and text contrast becomes a lottery
  decided by which colour the user picked. In OKLab the lightness
  ladder is fixed and only hue and chroma come from the user, so shade
  500 always lands at the same brightness and the layout's contrast
  relationships hold whatever is chosen.
- **Out-of-gamut colours lose chroma, never hue.** Many OKLCH triples
  name colours sRGB cannot show. Converting one anyway and clamping the
  channels shifts hue *and* lightness — `#3b82f6` drifted 13° toward
  purple at the light end. `toGamut` binary-searches chroma down
  instead, giving up only the saturation that was never displayable.
- **The sliders are bounded, and there are only two.** `INTENSITY_MAX`
  caps the base far lower than the accent, because surfaces are 95% of
  the pixels and must stay near-neutral. There is no brightness slider:
  `buildScale` sets lightness per shade and ignores the input's, so
  such a control would move and change nothing.
- **Semantic colours are not derived from the picks.** Success, warning
  and error carry meaning in their hue. A user whose accent is red must
  not get green error states, so those three stay fixed and are only
  ramped.
- **The preview is scoped, not global.** `data-theme="custom"` on the
  preview element makes the generated variables cascade into that
  subtree while the rest of the app keeps the active theme — otherwise
  the preview shows the user someone else's colours. It also re-derives
  the handful of `--ui-*` tokens it uses, since those are computed at
  `:root` and do not re-resolve for a descendant that changed theme.

### Accessibility (EAA / WCAG 2.1 AA, Apple's accessibility labels)

The European Accessibility Act (applied since 2025-06-28) points at EN
301 549, which is WCAG 2.1 AA; the App Store's accessibility nutrition
labels ask about VoiceOver, Voice Control, Larger Text, Dark Interface,
Differentiate Without Color, Sufficient Contrast and Reduced Motion.
The app is built to answer all of those, and these are the rules that
keep it that way. **Breaking one is a compliance regression, not a
style nit.**

**Contrast is solved, not chosen.** `ui/src/lib/accessibleColors.ts`
takes each theme's colour scale and moves every text and control colour
the *least* distance toward black (light) / white (dark) that reaches
its WCAG target on every surface it sits on — card, canvas, raised card,
bottom bar, unread tint. A theme that already passed looks exactly as
before.

**Surfaces and washes have different targets.** `backgroundsFor` lists
the surfaces people read on, where the design targets apply (7:1 for the
primary ink). `washesFor` lists the translucent washes that text also
sits on — `--ui-fill-quiet` and `--ui-tint-accent`, mirrored from
`app.css` — where every token is guaranteed **AA** (`WASH_TARGET`,
4.5:1 text / 3:1 controls) in a second pass. The split is not a
convenience: on a theme whose dark card already only just gives pure
white 7:1 (Reign), *any* wash puts the ink under 7:1, and no ink can be
whiter than white. Before the washes were listed, muted ink on the quiet
fill measured 4.7:1 and faint ink 4.0:1 in the house theme. The role
tints (`--ui-tint-warning/-success/-danger`) only ever carry the primary
ink; a separate test holds that pairing at AA. The result is `ui/src/accessible-colors.css` (generated, one
block per theme × light/dark × `prefers-contrast: more`), and
`customTheme.ts` runs the same solver for the user-built theme.

- Measured before the solver: secondary text 1.8–2.8:1 in *every* stock
  theme, white labels on green/amber swipe actions 1.1–2:1, the Reign
  theme's accent as text 1.02:1. No fixed recipe fixes that, because some
  stock scales are not even monotonic.
- `accessibleColors.test.ts` re-measures the *emitted, rounded* values
  for all 23 themes and five extreme custom picks, and fails when the CSS
  is stale. After changing the solver or adding a theme:
  `npm --prefix ui test -- -u`.
- **Use the tokens, never the raw shades:** text in `--ui-*-ink` (or the
  `.ink-accent` / `.ink-danger` / `.ink-warning` / `.ink-success` /
  `.ink-muted` / `.ink-faint` classes), a filled control as the pair
  `--ui-*-fill` + `--ui-on-*`, an outline as `--ui-control`. Tailwind's
  `text-primary-500` & co. and a hard `color: #fff` on a fill are exactly
  what failed. A tinted background at ≤20% (`bg-primary-500/10`) is fine;
  put ink-coloured text on it, not accent-coloured text.
- Opacity is not a colour: `opacity-60` on text drops it under AA. Use a
  lighter ink.

**Dynamic Type.** `lib/dynamicType.ts` measures a hidden
`font: -apple-system-body` probe and sets `--dt-content` (content, capped
at 200%) and `--dt-chrome` (bars, capped at 130%) on `<html>`; every type
token and Tailwind's `text-*` sizes multiply by them, and a
`ResizeObserver` follows a change live. It does not touch `rem`, so
spacing stays on the grid while text grows. Consequences:

- **No px font sizes and no `text-[15px]`** — use a `--t-*` token or the
  `.t-title` / `.t-body` / `.t-footnote` / `.t-micro` classes.
- **No fixed heights around text.** `min-height`, not `height`; a label in
  a fixed-width box needs `text-overflow: ellipsis`.
- Text fields use `--t-input` (never below 16px, or iOS zooms on focus).

**VoiceOver, Switch Control, keyboard.**

- **Every action is a real `<button>`**; a gesture is never the only way.
  `SwipeRow` is a gesture layer: its content carries the primary button
  (a mail row *is* a `<button>`), and it renders visually hidden
  "gesture twins" for its actions (or one "More actions" button when
  there is a long-press sheet). `PullToRefresh` has a hidden Refresh
  button; the edge-swipe back gesture has the back button.
- **A tap that started on a control belongs to that control.** SwipeRow
  no longer calls `onclick` for it (a task's checkbox used to *also* open
  the editor), and it swallows the click that follows a swipe, a
  long-press or closing an open row.
- **State shown only by colour or an icon gets `sr-only` text** — unread,
  flagged, pinned, encrypted, priority, replied (WCAG 1.4.1).
- **Every form control has an accessible name.** A placeholder is not a
  label: add `aria-label` (or a `<label>`). A new input without one is a
  bug.
- **`Sheet` is a real dialog:** it moves focus in on open, traps Tab,
  closes only the topmost sheet on Escape, hands focus back on close, and
  gives every dismissible card a hidden Close button. `Confirm` is an
  `alertdialog` described by its body.
- **Announcements:** toasts sit in pre-mounted live regions (errors
  assertive, the rest polite) and pause their timer while touched or
  focused; `Spinner` and the pull indicator are `role="status"`.
- **`<html lang>` follows the locale** (`main.ts`) — at the static "en",
  VoiceOver read the German UI with an English voice.
- `:focus-visible` draws a ring in `--ui-accent-ink`; rows inside a
  clipped card draw it inset. Touch never shows it.
- Targets are ≥44pt even when the drawn control is smaller (`Toggle`,
  the search clear button, the task checkbox, toast buttons) — extend the
  hit area with a pseudo-element or padding, don't shrink the design.
- `prefers-reduced-motion`, `prefers-reduced-transparency` and
  `prefers-contrast: more` are all honoured in `app.css`.

What cannot be checked in the simulator from the command line and still
needs a human pass on a device: VoiceOver reading order screen by screen,
Voice Control ("Tap Archive"), Full Keyboard Access on iPad.

### Introduction

`lib/Intro.svelte` — five pages on what a new user cannot see (where
things are, the gestures, and per variant how to leave test mode or where
to connect calendars). Shown once per variant (`lib/introState.ts`,
`localStorage`, guarded) the first time the app is ready with that kind of
account — straight after test mode is started, or after the first real
account — and replayable from Settings → Introduction. Every sentence
names a real control by its on-screen label: **rename a control or change
a gesture → change the page in the same PR.** It is a `Sheet`, so it has
the dialog behaviour, and each page change moves focus to its heading.

One Svelte 5 trap found while building it: a prop is a *getter over the
parent's expression*, so a callback prop read after the child closed
itself can already see the parent's new state (`intro = null` turned
`onconnect` into `undefined` mid-click). Read callback props before
calling anything that changes the parent.

### Connecting clouds

Settings → **Clouds, Kalender & Kontakte** → + opens a provider picker
(`lib/cloudProviders.ts`, pure and tested). Two routes, chosen per
provider, and the module header says why:

| Route | Providers | How |
|---|---|---|
| Through the iPhone | iCloud, Google, Microsoft 365 / Outlook, Yahoo | a short guide, then the existing device source (EventKit + Contacts) |
| Directly (CalDAV/CardDAV) | mailbox.org (`dav.mailbox.org`), Posteo (`posteo.de:8443`), Fastmail (`caldav.` / `carddav.fastmail.com`), ownCloud, any other server | the generic DAV form with the server filled in |
| Nextcloud | Nextcloud | the existing Login Flow v2 screen |

- **Google and Microsoft cannot go direct.** Google's CalDAV/CardDAV need
  OAuth with an app registered and verified at Google; Microsoft has no
  CalDAV/CardDAV at all. A password form for them would be a form that
  cannot work. iOS already syncs both, so the device source reaches them.
- **A preset only fills in the server.** Discovery is the same RFC 6764
  ladder as for any server, so no provider gets special backend code.
  Only providers with published, stable DAV addresses are listed.
- **Split hosts become two sources.** The backend stores one server per
  source, so Fastmail (calendars and contacts on different hosts) is
  added as two, each asked only for its own service (`planDavSources`).
- **Verified end to end** against a local Radicale server in the
  simulator (2026-09-19): discovery through `/.well-known`, then a synced
  contact and a synced event with the right time zone. The named
  providers themselves have not been tried with real accounts.

### Edge swipe back

`App.svelte` pops the current stack when a touch that starts within 24pt
of the left edge is dragged right — the platform's own back gesture,
which a thumb reaches far more easily than the back button in the
top-left corner. The edge claims the touch in the capture phase, so a
mail row's own swipe never sees a gesture that starts there (taps still
work: only `pointerdown` is stopped). Off while a sheet or modal is up.

### Large titles

`NavBar` takes `large` for a **destination** — a screen you go to and
stay in: tab roots, Settings, Appearance. The title renders at
`--t-display`, then collapses into the bar as the content scrolls,
with the compact title fading in as it goes and a hairline appearing
under the bar.

Not for transient views (a message, a contact, an editor), where the
title is a breadcrumb for something already on screen. Not for a title
that is really a *control* — the calendar's month sits between
prev/next arrows and collapsing it would strand them around a gap.

The bar hears its own screen's scroller (`.screen__body` or
`.ptr__scroller`) through a capture-phase `scroll` listener on `.screen`
rather than taking a scroll offset as a prop: threading one through 32
screens would be plumbing nobody maintains, and a scroller looked up once
at mount misses lists that render a spinner first. The listener is
rAF-throttled for the same reason the gesture components are. `Fab`
listens the same way.

### Navigation

- **Up to five tabs, one stack each** (`lib/nav.svelte.ts`). Switching tabs preserves each stack's position; tapping the active tab pops it to its root. Screens are *data* (`{ kind, props }`), not components — `App.svelte` maps `kind` to a component.
- **Mail opens on the inbox.** The Mail tab's root is `mail-inbox`: `App.svelte` renders `MailList` with `root` for the one account's INBOX, or the unified inbox once there are several. The mailbox list (`mail-home`) is pushed from the inbox's leading **Mailboxes** button and goes Back to the inbox. v1 opened on the folder list, which charged a tap for the one thing nearly every session is for. Special-use folders are named by role in the user's language (`MailHome.roleName`), never by the server's name.
- **The inbox has an account switch** once there are two or more accounts: the line under the large title ("All accounts ⌄" / the account's name) is a control (`NavBar`'s `onsubtitle`) that opens an `ActionSheet` — all accounts, then each one, with its unread count and a check on the current choice. The choice is remembered per device (`lib/mail/inboxScope.svelte.ts`, `localStorage` `unkai.inboxAccount`, deliberately not in the synced settings); a removed account falls back to all (`inboxChoice.ts`, tested). With one account there is nothing to switch and the line is not shown.
- **The tab bar is configurable** (`lib/features.svelte.ts`, Settings → Features). Mail is always first and More always last; up to three slots between them hold whatever features the user picked. **The default is four tabs: Mail, Calendar, Contacts, More** (`DEFAULT_TABS`). Only what works out of the box earns a default slot — Tasks without a Nextcloud opened on a "connect a server" page, the first dead end a new user met. It lives under More. A feature can also be hidden outright, which removes it from the tab bar *and* from More. `nav.svelte.ts` keeps a stack per *possible* tab, so taking Files off the bar and putting it back returns to the folder the user was in. **A new feature means: an entry in `FEATURES`, a root `ScreenKind`, and nothing else** — the tab bar, the More list and the settings screen all derive from that array.
- **A feature screen shows Back exactly when there is somewhere to go back to** (`backLabel={nav.canGoBack ? … : null}`). A feature is a tab root *or* a page under More depending on the user's arrangement; v1 hard-wired one or the other, so Tasks under More had no Back button and Notes on a tab had one that did nothing.
- **More is a hub**: an account card, the features not on the bar as a tile grid, then Outbox and Settings. Search lives on the inbox only.
- **One way to ask for a refresh.** Lists are pulled to refresh; a sync button in the bar appears only while there is no list to pull (the empty state). `PullToRefresh` carries its own hidden Refresh button for VoiceOver and keyboards.
- **Modals are not screens.** Compose and the event editor present over everything and are tracked separately in `App.svelte`. Pushing them onto a tab stack would let a tab switch strand a half-written message.
- **A screen with its own bottom bar hides the tab bar** (`HIDES_TAB_BAR` in `App.svelte`). Two stacked toolbars eat a fifth of the screen.

### Layout primitives (`app.css`)

- **`.screen`** = nav bar + `.screen__body` (the only scrolling element). The page itself never scrolls; `overscroll-behavior: none` on `body` keeps iOS from rubber-banding the whole shell.
- **`.list-group` / `.list-row`** — the grouped-list shape used by every list in the app, with inset hairline separators. `.list-group--inset` for the rounded card variant (now a no-op: every group is inset). Rows are ≥52px.
- **`.form-group`** scopes the form-control styling: transparent fill, 1px border, 10px radius. **Every text control is 16px** — anything smaller makes iOS zoom the viewport on focus.
- **`.bar-button`** (44×44 icon button in a nav bar; `--text`, `--strong`, `--destructive`, `--labelled`, and `--filled` for the one commit action a bar may carry, e.g. Send), **`.wide-button`** (full-width capsule; `--quiet`, `--plain`, `--destructive`), **`.value-button`** (inline primary-coloured value/action).
- **`.icon-tile`**, **`.tile-grid` / `.tile`**, **`.search-trigger`**, **`.section-title`**, **`.screen--plain`** — see "The design system". A search trigger is a button that *looks* like the field and opens the search screen; a customer looks for a field, not for a magnifier in a corner.
- **Safe areas**: `env(safe-area-inset-*)` is applied in `NavBar`, `TabBar`, `Sheet`, and every bottom bar. Never hard-code a bottom padding instead.
- **The keyboard takes screen away; it never scrolls the shell** (`lib/keyboardViewport.ts`). iOS scrolls the *page* to reveal a focused field, and the shell slid up with it — the nav bar under the status bar on every form. The keyboard's height is published as `--keyboard-inset` on `<html>` (and `data-keyboard` once it is a real keyboard): `#app`, `.sheet-root`, `.setup-shell` and `.compose-shell` end above it, the page scroll is reset to 0, the focused field is scrolled into view inside its own scroller, and the tab bar hides while typing. **A new full-screen fixed layer sets `bottom: var(--keyboard-inset, 0px)`**, and a bottom padding of `env(safe-area-inset-bottom)` under a field subtracts `--keyboard-inset` (Compose's toolbar).

### Component vocabulary (`lib/ui/`)

- **`NavBar`** — back affordance + title + actions + optional `below` slot (search field, segmented control) + a `busy` hairline. Never hand-roll a header. The large title listens for scroll on its `.screen` in the **capture phase** (scroll does not bubble), so a list that mounts after a spinner — the inbox — still collapses it; a lookup at mount missed those. A back label that does not fit its slot (~96pt; "Einstellungen" does not) becomes plain **Back**, as on the platform — measured with a hidden copy and a ResizeObserver, not guessed from the length; the accessible name keeps the full "Back to …". An empty `title` renders no heading (the reader's subject is the content's own `<h1>`).
- **`Fab`** — an extended pill: icon *and* label ("New message", "New event"), above `--bar-clearance`. It tucks into a disc while the list scrolls down and comes back on scrolling up or near the top (24pt hysteresis, rubber-band clamped). The label stays in the DOM and is repeated as `aria-label`.
- **`Sheet`** — `kind="full"` for a page-sized modal, `kind="sheet"` for a bottom card with drag-to-dismiss; the card floats (inset, rounded all round) like the bars. Both close on backdrop tap and Escape, and manage focus like a native dialog (see Accessibility). `hasOpenSheet()` tells the shell one is up.
- **`ActionSheet`** — the mobile replacement for the desktop's `⋯` menu *and* right-click menu. Row actions beyond the primary tap go here, reached by long-press or an explicit `⋯` button. `selected: true` marks the current choice when the sheet is a picker (check + `aria-current`). Rows are keyed by position, not label: two accounts or folders can share a name, and a duplicate key is a runtime error.
- **`SwipeRow`** — swipe actions with direction locking (the first few px decide swipe vs scroll) and full-swipe commit. **Destructive actions apply immediately and offer Undo through a toast** — a phone that asks "are you sure?" on every swipe is one nobody triages mail on. *(As of 2026-09-30 the Undo half is not wired: `toasts.undo()` has no caller, because a moved message's new UID is not returned by the backend. Built in the Lab — `lib/mail/undoMove.ts` there, with the move commands returning the new UID — and not ported here yet.)* Put the row's primary action in a `<button>` inside it; the row adds hidden button twins for its swipe actions. A full swipe commits only past 65% of the row (and at least 88pt beyond the revealed buttons), and the outermost action visibly fills the row once it would — at the old 45% a brisk swipe meant to reveal Delete archived the message instead.
- **`Confirm`** is only for the genuinely irreversible: removing an account, deleting a folder, wiping the vault.
- **`PullToRefresh`** — owns its scroller (the pull may only start at `scrollTop === 0`) and provides the `onnearbottom` hook lists page from.
- **`Toasts`** — the mobile stand-in for a status bar. Errors linger, confirmations don't, one optional action. Drawn as the page's inverse (`--ui-ink` ground, `--ui-card` text): contrast is symmetric, so that is ≥7:1 in every theme.
- **`EmptyState`** — always distinguish an empty collection from an empty filter result. Two different sentences, always.
- **`SearchField`** — its clear button assigns `value` programmatically and fires no native `input` event, so callers react through `onchange` (or an `$effect` on the value), never `oninput` alone.

### Per-account folder visibility

`Account.hidden_folders` (full paths, `#[serde(default)]`, one JSON
column) is a **display filter on the mailbox list only** — the folders
keep syncing, keep turning up in search, and stay valid move targets.
Edited in Settings → Accounts → *the account* → Visible folders,
applied in `MailHome.visibleFolders`.

It stores what is *hidden*, never what is visible, and that direction
is the point: a folder created later — on the server, on the desktop,
by a filter rule — shows up on its own instead of staying invisible
until someone remembers to tick it.

### Staying responsive

The app runs on one thread that is also painting, behind an IPC
bridge, over a mobile connection. Four rules keep that from showing,
and each exists because its absence was visible as stutter or a hang.

- **Every network path has a deadline.** `crates/unkai-imap/src/timeout.rs`
  wraps the IMAP socket in an idle timeout, so any command fails after
  45s of silence rather than blocking forever, plus a 20s cap on
  connect and the TLS handshake. This is not optional politeness on a
  phone: iOS suspends the process on backgrounding and tears down the
  radio, so a socket is routinely a half-open zombie on resume — writes
  succeed, nothing ever comes back, and the command never returns. A
  hung command means a promise that never settles, which means a
  spinner that never stops. The deadline resets on *progress*, not
  completion, so a slow 30 MB attachment is unaffected.
  `ui/src/lib/schedule.ts`'s `withTimeout` is the UI-side counterpart
  for paths whose transport has no deadline of its own.
- **Coalesce bursts before they reach the webview.** A push channel
  fires once per *batch*, never once per item: `new-mail` carries a
  `count` for the whole folder poll. Per-item events meant a 40-message
  first sync pushed 40 events, each answered with an OS notification
  and a fan-out of unread-count IPC calls — the freeze users reported
  as "notifications hang". On the UI side, `debounce` and
  `singleFlight` (`ui/src/lib/schedule.ts`) are the standard wrappers:
  **any refresh reachable from more than one trigger gets
  `singleFlight`**, or two of them race to assign the same state and
  the slower, staler one wins.
- **Gestures write to the DOM, not through the template.** `SwipeRow`
  and `PullToRefresh` track their offset in a plain variable and write
  `transform` inside a `requestAnimationFrame`. Interpolating an offset
  into a `style` attribute makes every one of the ~120 pointer events
  in a drag re-parse a style declaration on the main thread, which is
  exactly the stutter-behind-the-finger it looks like. Keep `$state`
  for what changes the *markup* (a handful of times per gesture), not
  for what changes per frame. A drag also takes `setPointerCapture`, or
  a finger leaving the element strands the gesture half-open.
- **Long lists skip what is off screen.** `content-visibility: auto`
  with `contain-intrinsic-size` — `MailList`'s own `li` rule, or the
  shared `.list-lazy` in `app.css`. No windowing library, and
  find-in-page and screen readers keep working. Related: a row's
  incidental DOM is mounted on demand, which is why `SwipeRow` only
  builds its action panels once a swipe starts — eagerly, a
  200-message inbox built 600 invisible buttons.

Four more, from the 2026-10-01 pass. "The app hangs on opening a mail,
going back, archiving and at start" did not reproduce against the
localhost fixture (everything answered in 20–40 ms); with a simulated
phone connection (200 ms per IMAP command, ~1.3 s to connect and log
in) it did, and each cause below was measured that way — see Project
Status for the numbers.

- **No triage waits for the server.** The reader's Archive, Delete, Move
  and "Mark unread" go back at once and tell the lists underneath
  through `lib/mail/listBus.ts` (`hide` → `settle` on success / `unhide`
  on refusal, `patch` for read/flag); the toast confirms when the server
  has done it. The flag flips immediately and flips back on failure.
  Multi-select removes its rows at once and puts back only what the
  backend did not report as gone; Delete is one `delete_messages` batch
  per mailbox (was a connection and login per message, in turn). A list
  keeps a hidden row filtered out of fresh loads until it settles, so a
  sync landing mid-move cannot bring it back.
- **Mail lists stay mounted under the reader** (`KEEP_ALIVE` in
  `App.svelte`: inbox, mail lists, search). Back is instant and lands on
  the row the user left, scroll position included. Every mounted screen
  sits in its own `.screen-host` in a fixed document order — moving a
  scrolled node resets its scroll — and `data-current` decides which is
  shown; the rest are `visibility: hidden` and `inert`. No z-index on the
  hosts: it would cap the sheets a screen opens beneath the tab bar. A
  kept list stays current by itself: `active` (back on top → the 30 s
  throttled refresh), `new-mail` / `mail-flags-updated` → a cache
  re-read, and the list bus. **Only screens that keep themselves current
  may join `KEEP_ALIVE`**; the others read their data on mount.
- **A cached message is not fetched again.** A message under a UID never
  changes on the server; the reader used to open an IMAP connection and
  re-download the whole message on every open, then render the body a
  second time. It also renders once: the body is a derived *string*
  (a flag toggle does not re-run sanitise/parse/re-tint), and the
  contrast colours come from the screen root, bound before the message
  arrives — not from a body element whose binding re-ran the pipeline.
  The canvas colour probe is one shared 1×1 canvas with a memo.
- **The first frame is in the user's theme.** `applyLastTheme()` in
  `main.ts` paints from what `applyTheme` last stored, before the
  settings IPC answers; a dark-mode start used to flash light grey. A
  faster boot also exposed the tab bar measuring `--bar-clearance`
  before the viewport settled; it now measures its own box
  (`offsetHeight` + resolved `bottom`) and observes `<html>`.

Two consequences worth knowing before "fixing" them:

- **`.glass-panel` is opaque on mobile.** Its users here (NavBar, the
  compose toolbar) are `flex-shrink: 0` siblings of the scroller, not
  layers above it, so the blur resolved to a flat colour while costing a
  full-width backdrop re-sample every frame of every scroll. The blur
  stays on `.glass-float`. v3's floating bars (tab bar, reader toolbar,
  multi-select) *do* overlay the list, and are still opaque on purpose:
  a live blur under the one element on every screen is that same
  per-frame cost, on every scroll of the app.
- **`MailList` throttles its refresh on mount and on return** (30s,
  matching the sync loop's own floor). Before lists were kept alive the
  shell rebuilt the list on every Back, and without the throttle each
  one paid a full IMAP round trip. A pull, a Retry, and the post-empty
  recovery are never throttled — those are the user asking.

### Copy and i18n

- **Localise every user-visible string via `paraglide-js`.** `import { m } from '../../paraglide/messages'`, call `m.key()`. Mobile keys are prefixed `mobile_`. Backend Rust strings stay English and are wrapped on the frontend.
- **Ten languages** (2026-10-01): en, de, es, fr, it, pt (Brazilian), ja, zh (Simplified), ko, ru — the main app-store markets, all left-to-right. **A new key goes into all ten `ui/messages/*.json` in the same change.** `src/lib/i18nCoverage.test.ts` fails the build when a key the app uses (`m.key` in a file importing the messages) is missing in any of them or a `{placeholder}` differs from English — Paraglide would otherwise fall back to English silently. The ~900 desktop-era keys in `en.json`/`de.json` that no mobile screen uses are deliberately not translated further. Write counts so grammar cannot break ("Удалено писем: {count}"), keep in-app paths in the app's own labels ("Más → Ajustes → …") and iOS paths in iOS's ("Ajustes → Privacidad y seguridad").
- **The language follows the iPhone unless pinned** (`lib/locale.ts`, strategy `custom-unkai`, registered by `localeSetup.ts`, which `main.ts` imports first). Paraglide's `localStorage` strategy *stored* the language it resolved on the first start, so a later change of the iPhone's language never reached the app. Settings → Language (`LanguageSettings.svelte`) pins one under `unkai.locale` (synced in the settings bundle; the old `PARAGLIDE_LOCALE` is removed on start) and reloads. Traditional Chinese (`zh-Hant`, TW/HK/MO) is passed over rather than shown Simplified. **Dates, times and sorting use `formatLocale()`**, never `undefined`: the pinned language, else the device's own regional formats. Keep the strategy list in `vite.config.ts` and the `paraglide:compile` script in step.
- **No other-mail-client references** in code, comments, or commit messages. Describe the behaviour ("the standard triage gesture"), name the RFC, not the product. Hostnames in string literals are factual data and stay.
- **No team-member names anywhere user-visible** — placeholders and fixtures use `Alex Morgan`, `you@example.com`, and friends. The exceptions are the project-context documents (`CLAUDE.md`, `README.md`, `SBOM.md`, `License.md`) and commit metadata.
- **No hard-coded strings in `.ts` modules either** — `theme.ts` once shipped "Eigenes Theme" to English users. A string in a module-level object is a getter calling `m.*()`.
- Screen-reader-only strings are `mobile_sr_*`.

### Privacy and store compliance

**The privacy notice is a claim about the code.** `lib/PrivacyNotice.svelte`
(Settings → Privacy, and a link on the first-run screen before any address
is typed — App Store 5.1.1, GDPR Art. 13) states what the app does with
data, and each statement was checked against the code; the component's
header lists where. **Change a data flow → change the notice in the same
PR.** The flows it describes:

- No analytics, advertising, crash reporting or developer endpoint.
- Account setup sends the *full address* to the provider's own autoconfig
  host (`autoconfig.<domain>`, `<domain>/.well-known/…`) and only the
  *domain* to `autoconfig.thunderbird.net`.
- Link check (default on) downloads the URLhaus list from abuse.ch and
  matches locally.
- Mail is always TLS (IMAP only via `tls_connect`; SMTP implicit or
  required STARTTLS). DAV/Nextcloud follow the URL the user enters.
- Removing an account deletes its keychain entries (password, PGP *and*
  S/MIME key + passphrase — the S/MIME half was missing until 2026-09-15)
  and its cached rows. iOS keeps keychain items across an uninstall,
  which the notice says.

**Apple privacy manifest** —
`src-tauri/gen/apple/unkai-mobile_iOS/PrivacyInfo.xcprivacy`. Declares no
tracking, no collected data, and the two "required reason" API
categories found in a **release** binary: file timestamps (`stat`… from
Rust std/SQLCipher, C617.1; `NSFileModificationDate` from the dialog
plugin's file picker, 3B52.1) and disk space (`statfs` from SQLCipher and
rustix, E174.1). Debug builds over-report (`NSUserDefaults`,
`systemUptime`, `activeInputModes` appear only there, from unused objc2
bindings) — **re-check against a release build** after adding a
dependency or native code; the commands are in the file's header.

**Localised permission prompts** — `CFBundleLocalizations` (en, de) in
`Info.ios.plist`, German purpose strings in
`gen/apple/unkai-mobile_iOS/de.lproj/InfoPlist.strings`. A new purpose
string goes into both files.

**Open iOS Settings** — `open_app_settings` (shell-only command, like
`open_url`, since it needs the opener plugin). Opens `app-settings:`. In
the iOS 26.5 simulator it lands on the Settings root rather than the
app's page; verify on a device.

**Still the team's decision / input — not something code can settle:**

- `ITSAppUsesNonExemptEncryption` is deliberately **not** set. The app
  ships its own crypto (SQLCipher, OpenPGP, S/MIME), so "false" would be
  an untrue export-compliance statement; answer it in App Store Connect
  (usually the mass-market exemption plus the annual BIS self-
  classification report) and then add the key.
- App Store Connect: App Privacy = "Data Not Collected" (must match the
  manifest), a hosted privacy policy URL with the same content as the
  in-app notice, the accessibility nutrition labels, and DSA trader
  status for EU storefronts.
- A German *Impressum* / provider identification (DDG §5) and a contact
  for privacy and vulnerability reports (the EU Cyber Resilience Act's
  reporting duties apply from 2026-09-11; full obligations from
  2027-12-11). These need Firn Labs' legal name and address, which are
  not in this repository and must not be invented.

## Calendar / contact / task sources

Groupware data has four possible origins, and the whole app is built
so that a screen never has to know which one it's looking at. They are
the `DavSourceKind` variants on `NextcloudAccount`:

| Kind | What it is | Sync means | Writes go to |
|---|---|---|---|
| `Nextcloud` | A full Login-Flow-v2 connection | CalDAV / CardDAV `sync-collection` | the server |
| `Dav` | Any standards-compliant CalDAV/CardDAV server | the same | the server |
| `System` | **The device's own Calendar, Reminders and Contacts** | a full EventKit / Contacts read | the framework |
| `Local` | No remote at all | a no-op | the cache alone |

Every calendar, addressbook and task list in the cache is keyed by a
source id, which is why a fourth *kind* was far less invasive than a
second kind of key: `CalendarScreen`, `ContactsScreen` and
`TasksScreen` already loop over `getNextcloudAccounts()` and needed no
change at all.

### The System source (`crates/unkai-system`)

The only place in the workspace that talks Objective-C. It exposes
plain Rust — `String`, `DateTime<Utc>`, **vCard text** — so nothing
`Retained<…>` ever escapes and callers need no `unsafe`, no
autorelease pool and no thread affinity. On a non-Apple target every
entry point compiles to the same signature and answers
`SystemError::Unsupported`, which is what will let the Android port
build before it has its own bridge.

`crates/unkai-commands/src/system_source.rs` is the adapter onto the
cache. Four decisions there are load-bearing:

- **Snapshots, not deltas.** EventKit and Contacts can only answer
  "here is everything". So each sync reads the whole collection, diffs
  it against the hrefs already cached (`list_calendar_event_hrefs` /
  `list_contact_hrefs`), and hands `apply_*_delta` both halves — one
  atomic write, every downstream reader unchanged.
- **Events are stored flattened.** EventKit returns *occurrences*, not
  a master plus an `RRULE`, so that's what the cache gets: one row per
  occurrence, `rrule` empty. It costs rows over the sync window
  (`SYNC_PAST` / `SYNC_FUTURE`, −1y…+3y — EventKit itself caps a query
  at four years) and buys exactness: an occurrence the user edited or
  cancelled in the Calendar app looks here exactly as it does there,
  with no re-implementation of Apple's recurrence engine.
- **Writes take ICS / vCard, not domain structs.** Every write path in
  the app already holds the serialised form at the point it would
  `PUT`, so the system branch drops into `support.rs`'s
  `dav_*_for` helpers without a single signature changing — and an
  accepted invite or an imported `.ics` works on a system calendar for
  free. Contacts go the other way too: `CNContactVCardSerialization`
  hands us vCard text, which `unkai-carddav`'s existing parser reads.
- **Identity is adopted from the framework.** A created event or
  contact comes back with EventKit's / Contacts' own identifier, and
  the cached row keys itself by *that* — otherwise the next sync
  reports the same item under its real id and the user sees a twin.

Two things the System source deliberately refuses: creating, renaming
or deleting a *calendar* (the calendar list is shared with every other
app on the phone and belongs in iOS Settings), and running twice — one
device source only, or every event shows up doubled.

Permissions are separate per database (calendars, reminders, contacts),
iOS only ever prompts **once** per install, and a refusal is reported
as a permission state rather than an error so the UI can point at
Settings instead of offering a pointless retry.

## Mail-rendering conventions

The pipeline lives in `ui/src/lib/mail/renderHtml.ts` and runs in one
ordered pass. The order is load-bearing:

1. **Sanitise** (DOMPurify) — scripts, frames, forms, `<style>` out.
2. **Autolink** bare URLs so plain-text links are tappable *and* visible to the link-safety check.
3. **Annotate links** — external links get the tap handler that leaves the app; `cid:` anchors park their content-id.
4. **Block remote images** unless the sender is trusted or the user tapped "Show images". Loading them is a read receipt.
5. **Resolve `cid:` images** from the parts fetched with the message. These ride *inside* the message: no consent needed, never blocked.
6. **Fold quoted history** behind a `<details>` — on a phone an unfolded reply chain is ten screens of scrolling to reach three new sentences.
7. **Dark reading mode** (`darkenEmailBackgrounds`) — only when chosen *and* the app is dark: light inline backgrounds map to dark ones in the theme's hue (white becomes the page, greys a panel just above it, pale colours keep their hue), mid and dark ones (buttons, header bars) stay, and each text colour keeps the contrast the sender gave it — so a near-black headline becomes near-white and grey body text light grey, rather than everything meeting at the 4.5:1 floor. Images are untouched.
8. **Fix contrast** (`emailContrast.ts`) — re-tint *only* the inline colours that fail WCAG against the background actually behind them. Runs in the dark reading mode only; on white paper the sender's own assumptions hold.

**Reading mode.** By default a message is shown on white paper, as its sender laid it out; in light mode always. In dark mode the user can choose the dark reading mode — Settings → Appearance → *Mail in dark mode*, or the reader's ⋯ sheet ("Show in dark reading mode" / "Show original"), which flips the same setting. The stored field is still `mail_html_white_background` (inverted), so nothing changed in Rust or the settings bundle. The white-paper rules for the quoted-history fold carry `.email-html-body` twice so their specificity *equals* the `[data-mode='dark']` rules they answer; with one class less they lost, and the fold was a dark box on the white sheet.

Plus one mobile-specific pass: **`makeResponsive`** caps declared
widths over 320px, and `app.css` lets what's left scroll inside its
own container. Senders lay out for a 600px desktop pane; left alone,
every vertical swipe drifts sideways.

**Inline images (`cid:`)** come from one `fetch_inline_images` call
per message — never a `download_email_attachment` loop, which opens
an IMAP connection and re-FETCHes the whole message per part. The
frontend matching rules live in `inlineImages.ts` (unit-tested), and
the object URLs are revoked when the reader unmounts.

### Compose is plain text with an HTML tail

Deliberate, and worth not "fixing" without a reason:

- A `contenteditable` under an iOS software keyboard fights selection handles, autocorrect replacements and scroll-into-view. What people type on a phone is prose, not layout.
- The quoted history in a reply must survive **byte for byte** — the sender's tables, inline styles and images. Keeping it out of the editor (rendered below it, spliced in at send time) is the only way to guarantee that. The desktop build does the same thing for the same reason.

So an outgoing message is: the typed text converted to HTML with its
line breaks, then the account signature, then the untouched quoted
block (`lib/mail/reply.ts` builds the latter two).

## Development Guidelines

- Write clear, well-documented code — the team is learning as they build
- Prefer existing, well-maintained Rust crates over reimplementing protocols
- Write tests for protocol handling and pure frontend logic (the `lib/*.test.ts` modules are node-only by design — no DOM)
- Use `clippy` and `rustfmt` on all Rust code; `npm run check` must be clean before a PR
- Commit messages should explain *why*, not just *what*
- Keep the UI responsive — heavy work belongs in the Rust async loops, never in a click handler
- **Maintain `SBOM.md` AND `License.md` on every dependency change** — package, licence, licence category, and the reconciliation date in `SBOM.md`; attribution section in `License.md`. Introducing a stronger copyleft licence than what's already in the tree is a project-level decision — surface it to Nick / Jannik, don't slip it into a routine PR.

## Build & Run

### Prerequisites

- **Xcode** (16+) with the iOS SDK and at least one simulator runtime, plus the command line tools (`xcode-select --install`).
- **Rust** with the iOS targets:
  ```bash
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
  ```
- **Node 22+**.
- No Perl / OpenSSL install is needed: `openssl-src` and SQLCipher are
  compiled from source by the build for each iOS target, and cached in
  `target/`. The first iOS build is therefore slow; later ones are not.

### Commands

Build through these two rather than the Tauri CLI directly:

```bash
npm run ios:sim      # clean + build for the simulator, reliably
npm run ios:clean    # drop gen/apple/build
```

`tauri ios build` exports through `gen/apple/build` and fails with
*"Directory not empty"* whenever a previous export is still sitting
there — which, in a UI project, is every single rebuild. The whole
directory is generated, so dropping it first costs only the re-link
and makes the loop reliable. Use `npm run ios:sim` rather than calling
the Tauri CLI directly.

Everything runs from the **repository root** (that's where the Tauri
CLI and the `tauri` npm script live — the Xcode "Build Rust Code"
phase shells back into `npm run -- tauri ios xcode-script`, so don't
move them):

```bash
npm install                 # root: Tauri CLI
npm --prefix ui install     # frontend deps

npm run ios:dev             # build + run on a simulator, with live reload
npm run ios:build           # build the .app / .ipa
npm run ios:open            # open the generated project in Xcode
npm run ios:init            # regenerate gen/apple (after changing bundle.iOS)

npm run dev                 # run the same app in a desktop window (fast UI iteration)

npm run ui:check            # svelte-check + tsc
npm run ui:test             # vitest
cargo test --workspace
cargo clippy --workspace --target aarch64-apple-ios-sim
```

Simulator build + install + launch by hand, when you want a specific
device:

```bash
npx tauri ios build --debug --target aarch64-sim
xcrun simctl boot "iPhone 17 Pro"
xcrun simctl install booted "src-tauri/gen/apple/build/arm64-sim/Unkai Mail.app"
xcrun simctl launch booted com.unkai.mail
xcrun simctl io booted screenshot shot.png
```

### iOS project specifics

- **`src-tauri/gen/apple/` is generated but committed.** Regenerate with `npm run ios:init` after changing `bundle.iOS` in `tauri.conf.json`; the generator writes `project.yml` and the plists, so **never hand-edit inside `gen/apple/`** — put plist keys in `src-tauri/Info.ios.plist` and frameworks in `bundle.iOS.frameworks`.
  **Two exceptions, both bundle resources Tauri has no config for:** `unkai-mobile_iOS/PrivacyInfo.xcprivacy` and the permission texts in `unkai-mobile_iOS/<lang>.lproj/InfoPlist.strings` (de, es, fr, it, pt, ja, zh-Hans, ko, ru). The eight added on 2026-10-01 were registered in `project.pbxproj` by hand, entry for entry like `de` (file reference, build file, group child, Resources phase, `knownRegions`) rather than through xcodegen, which would also have dropped `DEVELOPMENT_TEAM` and the Info.plist; xcodegen picks them up from the folder if it is ever re-run. `CFBundleLocalizations` in `Info.ios.plist` lists all ten — without it iOS tells the webview the user prefers English and the UI never switches.
  - **`tauri ios build` does not re-run xcodegen.** It only patches build settings in the existing `project.pbxproj` (the timestamp changes, the file list does not), so a new file in `unkai-mobile_iOS/` is silently left out of the app. Adding one means `xcodegen generate --spec project.yml` in `gen/apple/` — **with `Externals/` emptied first** (move its `arm64/`, `x86_64/` aside and back), or xcodegen adds the built `libapp.a` archives as app resources. It also drops `DEVELOPMENT_TEAM` from the two build configurations and rewrites `unkai-mobile_iOS/Info.plist`; restore both, diff `project.pbxproj`, and then **check the built `.app`** actually contains the file.
  Two things about that regeneration are worth knowing before you fight them:
  - **`ios:init` does not overwrite an existing `project.yml`.** A new entry in `bundle.iOS.frameworks` therefore appears to do nothing. Delete `src-tauri/gen/apple/project.yml` and run `npm run ios:init` again; the file comes back with the new framework and nothing else changed.
  - **It *does* rewrite `gen/apple/<app>_iOS/Info.plist` back to the bare default**, dropping every key from `Info.ios.plist`. That is not damage: the next `tauri ios build` merges `Info.ios.plist` back in before it calls `xcodebuild`, so run a build before judging the diff.
- **Frameworks are needed for framework *constants*, not for classes.** `objc2` looks classes up by name at runtime, so `unkai-system` called EventKit and Contacts for a long time with neither linked. The moment one `extern` constant is used (`CNContactImageDataKey`, for the contact-photo fix) the link fails with an undefined symbol — which is why `Contacts` and `EventKit` are now in `bundle.iOS.frameworks` alongside `SystemConfiguration`. `cargo check` cannot catch this; only a real build links.
- **`SystemConfiguration.framework`** is linked through `bundle.iOS.frameworks`. `hickory-resolver` reads the system DNS configuration through `system_configuration`; without the framework the final Xcode link fails on `SCDynamicStore*` symbols.
- **The app icon** is generated, not hand-placed. `app-icon.png` at the repo root is the single 1024×1024 source (the `storm` brand mark on its own gradient, the same style the boot screen serves as `unkai-logo://storm`), and `npx tauri icon app-icon.png --ios-color "#1E2A40"` regenerates `src-tauri/icons/` *and* `gen/apple/Assets.xcassets/AppIcon.appiconset` from it. Two things the source has to be, which the art in `logos/` is not: **square-cornered and fully opaque** — iOS applies its own squircle mask, so a pre-rounded PNG gets its corners cut twice and shows dark wedges, and an icon with real transparency renders those pixels black. `app-icon.png` is therefore the rounded 1024 art composited onto its own gradient, corners included.
- **`Info.ios.plist`** carries `UIFileSharingEnabled` + `LSSupportsOpeningDocumentsInPlace` (so saved attachments are reachable in the Files app), the photo-library and camera purpose strings (the compose `<input type="file">` shows those options only when they're declared), the five calendar / reminders / contacts purpose strings the System source needs, and the portrait-only orientation set for iPhone. **A permission requested without its purpose string terminates the process** — that is why both the modern (`NS*FullAccessUsageDescription`) and the legacy (`NSCalendarsUsageDescription`, `NSRemindersUsageDescription`) keys are present.
- **Signing**: a simulator build needs no identity. For a device, pass the team as an env var — nothing is committed to `tauri.conf.json` **or to `project.pbxproj`**, so a fresh clone still builds for the simulator without an Apple account. Choosing a team in Xcode's *Signing & Capabilities* writes `DEVELOPMENT_TEAM = "…";` back into `gen/apple/unkai-mobile.xcodeproj/project.pbxproj`; **don't commit that line** (it was removed before the first push, 2026-10-02) — check `git diff` on that file before every commit:

  ```bash
  APPLE_DEVELOPMENT_TEAM=<team id> npm run -- tauri ios build --debug --target aarch64
  xcrun devicectl device install app --device <device udid> \
      "src-tauri/gen/apple/build/arm64/Unkai Mail.ipa"
  ```

  The team id is the `OU` field of the signing certificate
  (`security find-identity -v -p codesigning`, then
  `security find-certificate -c "<name>" -p | openssl x509 -noout -subject`).

  Three things bite the first time:

  - **The device has to be registered with the team** or xcodebuild
    fails with *"Your team has no devices"*. Registering it is a one-off:
    build once through Xcode, or run `xcodebuild … -allowProvisioningUpdates
    -allowProvisioningDeviceRegistration` directly. Note a bare `xcodebuild`
    then fails in the *Build Rust Code* phase — `tauri ios xcode-script`
    talks back to the `tauri ios build` process over a local RPC socket that
    only exists when Tauri drives the build. So: register with `xcodebuild`,
    then build with `tauri ios build`.
  - **The first launch needs the profile trusted on the phone**
    (Settings → General → VPN & Device Management → the developer app →
    Trust), otherwise `devicectl … process launch` is refused with
    *"profile has not been explicitly trusted by the user"*. Nothing on the
    Mac can do this step.
  - **A free (personal) Apple ID team gets a 7-day profile.** The app stops
    launching when it expires and has to be rebuilt and reinstalled. A paid
    membership gets a year.

  There is no release pipeline yet — see below.

### Developing without a mail account

`devtools/fake_mail_server.py` is an IMAP + SMTP server that speaks
exactly the slice of the protocols this client uses, over a
self-signed certificate it generates on first run:

```bash
python3 devtools/fake_mail_server.py
# IMAP TLS      → localhost:9993
# SMTP STARTTLS → localhost:9465
# any username / password is accepted
```

Add it in the app through *Enter server details manually*; the
certificate prompt appears, and trusting it is the same path a
self-hosted server takes. The mailbox is **mutable** (flags, moves,
deletes and appends stick) and SMTP **delivers back into the IMAP
store**, so composing a message and pulling to refresh shows the mail
you just sent — the whole loop is exercisable offline.

Three details worth knowing:

- **`UNKAI_FIXTURE_TRACE=1` logs every IMAP and SMTP command.** The
  fastest way to answer "where did the client conversation stop?" and,
  just as usefully, "how many round trips does this screen actually
  cost?" — the trace is how the per-folder `STATUS` loop and the
  reconcile's old `UID SEARCH ALL` were spotted.

- **Submission is STARTTLS, not implicit TLS**, because
  `unkai-smtp` picks its TLS mode from the port number (`465` ⇒
  implicit, anything else ⇒ STARTTLS) and binding 465 needs root.
  That heuristic is a real limitation for users too: a server
  offering *implicit* TLS on a non-standard port cannot currently be
  configured. Fixing it means an explicit TLS-mode choice in the
  account model and the setup UI — a shared-crate change, so it
  belongs in its own issue.
- **`crates/unkai-imap/tests/fixture_server.rs`** drives the real
  IMAP and SMTP clients against the fixture. It's `#[ignore]`d (it
  needs the server running) and is what catches the fixture drifting
  away from what the client actually sends:

  ```bash
  cargo test -p unkai-imap --test fixture_server -- --ignored
  ```

  Drift runs in both directions, and the fixture is the side that is
  usually wrong. It answered `UID SEARCH UID 1:1` with the whole
  mailbox until the sync path started sending bounded searches — a
  permissive fixture that would have hidden a real client bug. When a
  test against it fails, check which side is actually misbehaving
  before changing the client.

### Background sync — the honest limitation

iOS suspends the process shortly after the app leaves the foreground.
Mail is therefore checked while the app is on screen and for the grace
period after backgrounding — there is no push delivery and no
background fetch yet. `NotificationSettings` and `AboutScreen` both
say so to the user rather than implying otherwise. Adding real
background delivery means either BGTaskScheduler (limited, best-effort)
or a push path (which needs a server component and an APNs key).

### CI

Two tiers, same shape as the desktop repo:

| Workflow | When | What |
|---|---|---|
| `smoke.yml` | Every PR + push to `main` | `cargo fmt --check`, `cargo check` **for `aarch64-apple-ios-sim`** on macOS, frontend typecheck + build + tests |
| `ci.yml` (*Quality gate*) | `workflow_call` + manual | clippy (iOS target), tests, cargo-audit, cargo-deny, npm lockfile-lint + audit, frontend checks |
| `codeql.yml` / `osv-scanner.yml` / `semgrep.yml` | `workflow_call` + crons | Static / supply-chain scanners |
| `weekly-security.yml` | Sunday cron | The gate + the scanners on `main` |

The Rust jobs check against the **iOS simulator target** on macOS
runners on purpose: this app ships to a phone, so "it compiles" has
to mean "it compiles for iOS".

**There is no release workflow.** The desktop repo's installer matrix,
updater signing, and tag pipeline were removed — they describe a
distribution model this app doesn't have yet. When the app is stable
and moves to its own Firn Labs repository, the release story is
TestFlight / App Store (or an ad-hoc `.ipa`), and that pipeline gets
written then, against a real signing certificate.

## Project Status

**Phase: mobile port complete, unverified against a live server**

- The Rust workspace, the application layer and the encrypted store compile and run on iOS unchanged
- The mobile shell (`src-tauri`) is a rewrite: 221 command shims, one app context, no window/tray/profile machinery
- The frontend is a rewrite: 5 tabs (three of them user-assignable), ~32 screens, a hand-built mobile component vocabulary, bilingual (en/de) from day one and in ten languages since 2026-10-01
- Verified in the simulator against a live IMAP/SMTP server (`devtools/fake_mail_server.py`): account setup incl. certificate trust, folder discovery with special-use detection, message list, reader (HTML, attachments, quote folding), swipe actions, archive (confirmed server-side), compose → SMTP → outbox retry → delivery → sync, search, dark mode
- **Not yet verified: Nextcloud** — Files, Talk, Notes, Tasks, Calendar, Contacts and Shares have never run against a real server. They're written against the same command layer the desktop uses, but treat them as unproven.
- **The System source reads correctly in the simulator** (2026-09-03): the device source was added, all three permissions granted, and the simulator's own Contacts and Calendar databases show up in the app — contacts with their **profile photos**, calendar events tinted with the **calendar's own EventKit colour**. Still unproven: every *write* path (create / update / delete of an event, reminder or contact), the permission prompts on a first install, and all of it on real hardware.
- **A responsiveness pass landed** (2026-09-03) — see "Staying responsive". The three reported symptoms had three distinct causes: the IMAP client had **no timeouts at any layer**, so a stalled socket hung a refresh forever; `new-mail` fired **once per message**, so a burst of mail meant a burst of OS notifications and unread-count IPC; and the gesture components re-rendered an interpolated `style` attribute on every pointer event. Re-verified in the simulator against the fixture server afterwards: pull-to-refresh (one connection per pull, retracts cleanly), swipe-to-archive with its toast, tab and stack navigation, and the reconcile now issuing a bounded `UID SEARCH UID low:high` instead of `UID SEARCH ALL`.
- Also unverified: a real provider (Gmail/Fastmail/Mailbox.org — app passwords, larger mailboxes, IDLE), OpenPGP/S-MIME with real keys, notifications on a device. **The timeout values (45s idle / 20s connect) have only met a healthy localhost server** — a genuinely flaky mobile connection is what would tell us whether they are tuned right.
- **An accessibility and compliance pass landed** (2026-09-15) — see "Accessibility" and "Privacy and store compliance". Contrast guaranteed per theme (tested), Dynamic Type, VoiceOver/keyboard access to every gesture, dialog focus management, labelled form controls, edge swipe back, in-app privacy notice, Apple privacy manifest, German permission prompts, and the S/MIME key now deleted with its account. Verified in the simulator (iOS 26.5, German): live text-size change up to the largest accessibility size, row tap / swipe / tap-to-close / long-press sheet, edge swipe back, Settings → Privacy, manifest and `de.lproj` present in the built app. **Not verified:** VoiceOver and Voice Control on a device, and whether `open_app_settings` reaches the app's own page on a device.
- **Redesign v2 + v3 "Wolkenmeer" ported from the Lab** (2026-10-01) — see "The design system", "Navigation" and `docs/design/` (rationale, audits and the Lab's before/after screenshots). Design only: the Lab's Undo was left out. The identity edits of the first port were re-applied (`unkai-logo://storm`, "Unkai Mail" on About, no Lab wording), and Morgenrot is argued from the name here, not from the icon, which is navy. Verified 2026-10-01: `npm run check` (0 errors, 0 warnings), 177 UI tests incl. the contrast snapshot, the Rust tests of `unkai-core` and `unkai-commands`, a simulator build ("Unkai Mail.app"), and on screen (iOS 26.5, German, dark, the fixture account): inbox, More, Settings, Appearance with the house theme, About with the `storm` mark — screenshots in `docs/design/2026-10-01-port/`. Not looked at here: light mode, the reader, first run; the Lab's checks cover the same code. **An existing install keeps its saved theme** (this simulator had Cerberus, the old default): choose Unkai under Settings → Darstellung; the new default only applies to fresh installs.
- **Test mode, introduction and cloud picker landed** (2026-09-19) — see "The demo account", "Introduction", "Connecting clouds". Verified in the simulator: the test-mode pill, the demo introduction on first start, Settings → Leave test mode (to first-run with no other account, staying in the app with one), a real account (fixture server) triggering the second introduction and its "Connect now", the provider picker, the device guide, the Fastmail form, and a full CalDAV/CardDAV connection to a local Radicale server.
- **Account switch, login screen, ten languages, reading mode, responsiveness** (2026-10-01) — see "Navigation", "Layout primitives", "Staying responsive", "Copy and i18n" and "Mail-rendering conventions". Pre-change backup: `~/Documents/Unkai Mobile Try2 Backup 2026-10-01 vor Konto-Sprachen-Lesemodus.tar.gz`. Measured in the simulator against `devtools/fake_mail_server.py` with 300 extra messages and 200 ms injected per IMAP command (a temporary probe logging tap → content, frame gaps and IPC timings; removed again): archive from the reader 2 186 ms → 41 ms to the list (server finishes behind), flag 1 718 ms → immediate, opening a cached message no longer re-fetches it (1.7 s of IMAP and a second render), Back 25–36 ms with the scroll position kept. The login screen lost its card-around-outlined-fields (three surfaces, two border colours) and WebKit's yellow/blue AutoFill paint (`-webkit-autofill` is overridden with an inset shadow in the surface colour, `--field-surface`), got visible labels and a Go key that submits, and lost the duplicate "Next" in the bar. Verified on screen (iOS 26.5, German, dark): cold start without the light flash, the new add-account form incl. manual servers and certificate trust (fixture server, second account "Arbeit"), the inbox switch (all / one account, counts, remembered), dark reading mode and back to original with the light quote fold, Settings → Language → Español (UI and dates) and back, compose and the setup form with the keyboard up. 208 UI tests (incl. i18n coverage for all ten languages), `npm run check` clean, `cargo fmt`/`clippy` clean, 53 `unkai-commands` tests. **Not verified:** the eight new languages beyond Spanish on screen, a software keyboard (the simulator had a hardware keyboard, so only the accessory bar showed), light mode of the new screens, a real device and a real server.
- Next: the System source's *write* paths in the simulator, then a real mail account, then Nextcloud, then Android (`npm run android:init` — note `unkai-system` needs an Android bridge before the device source works there), then a signing certificate and TestFlight

## Development Workflow

The team follows a simple loop for every issue:

1. **Pick an issue** — choose an open issue to work on
2. **Ask Claude** — Claude uses this `CLAUDE.md` as project context, so keep it up to date
3. **Understand & revise** — review the output, make sure you understand the code, adjust
4. **Push** — commit and push when the work is solid

This means Claude should:
- Always explain *what* the code does and *why* it's written that way
- Not just produce code — teach the team as you go
- Keep `CLAUDE.md` updated when the project evolves

## Git Branching Strategy

Short-lived feature branches, one per issue, off an up-to-date `main`;
merge via PR; delete the branch after merge.

```bash
git checkout main && git pull origin main
git checkout -b feature/<issue-number>-<short-slug>
# …work…
git push -u origin feature/<issue-number>-<short-slug>
```

**Never push directly to `main`.** Merge early, merge small; if you
add a shared type other work needs, split it into its own tiny PR
first.

### Claude reminder obligations

**Before opening a PR:**
> "Ready to open a PR? Double-check: you're on a feature branch named `feature/<issue-number>-<slug>`, branched from an up-to-date `main`, and this branch covers exactly one issue."

**After an issue is merged to `main`:**
> "This is now merged to main. Delete the feature branch locally and on the remote, then remind the other developer (Nick/Jannik) to pull main before starting their next branch."

## Team Context

- **Nick** and **Jannik** — two-person team, new to building a project of this scale
- AI assistance (Claude) is a core part of the development workflow
- Expect frequent questions about Rust idioms, protocol details, and design patterns — answer thoroughly with explanations
- This repository is [firn-labs/unkai-mail-mobile](https://github.com/firn-labs/unkai-mail-mobile) (created 2026-10-02, private until the team makes it public). Nothing here should be pushed to the desktop app's repo, and the Lab (`../Unkai Mobile Lab`) is not part of it.
- **There are no prebuilt binaries.** The README tells testers to build the app themselves (simulator, test mode, own signing team for a device). Keep that section true when the build steps change, and replace it once a TestFlight / App Store channel exists.
