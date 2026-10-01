<script lang="ts">
  /**
   * The shell.
   *
   * Four jobs, and deliberately nothing else — screens own their
   * own data:
   *
   *   1. **Boot** — settings, theme, vault state, accounts. Decides
   *      between the lock screen, the first-run setup, and the app.
   *   2. **Chrome** — the tab bar, the per-tab navigation stacks,
   *      and the slide transition between screens.
   *   3. **Modals** — the composer and the event editor present over
   *      everything. They're not stack entries: a tab switch must
   *      never strand a half-written message.
   *   4. **Backend events** — new mail, reminders, unread counts.
   *      This is the one place that turns a backend push into a
   *      local notification, so the rules live together.
   */
  import * as api from './lib/api'
  import type { DatabaseStatusView, Email } from './lib/api'
  import type { UnlistenFn } from '@tauri-apps/api/event'

  import { nav, type Screen, type ScreenKind } from './lib/nav.svelte'
  import { features } from './lib/features.svelte'
  import { accountsStore } from './lib/accountsStore.svelte'
  import { contactsStore } from './lib/contactsStore.svelte'
  import { settingsStore } from './lib/settingsStore.svelte'
  import { toasts } from './lib/toast.svelte'
  import { applyCustomTheme, applyTheme, installSystemModeListener } from './lib/theme'
  import { debounce, singleFlight } from './lib/schedule'
  import { buildReplyInitial } from './lib/mail/reply'
  import { inboxScope, resolveInboxAccount } from './lib/mail/inboxScope.svelte'
  import { displayName } from './lib/format'

  import TabBar from './lib/ui/TabBar.svelte'
  import Toasts from './lib/ui/Toasts.svelte'
  import Spinner from './lib/ui/Spinner.svelte'
  import { hasOpenSheet } from './lib/ui/Sheet.svelte'

  import LockScreen from './lib/screens/LockScreen.svelte'
  import AccountSetup from './lib/screens/AccountSetup.svelte'
  import Compose, { type ComposeInitial } from './lib/screens/Compose.svelte'
  import EventEdit from './lib/screens/EventEdit.svelte'

  import MailHome from './lib/screens/MailHome.svelte'
  import MailList from './lib/screens/MailList.svelte'
  import MailMessage from './lib/screens/MailMessage.svelte'
  import MailSearch from './lib/screens/MailSearch.svelte'
  import Outbox from './lib/screens/Outbox.svelte'
  import CalendarScreen from './lib/screens/CalendarScreen.svelte'
  import EventDetail from './lib/screens/EventDetail.svelte'
  import TasksScreen from './lib/screens/TasksScreen.svelte'
  import ContactsScreen from './lib/screens/ContactsScreen.svelte'
  import ContactDetail from './lib/screens/ContactDetail.svelte'
  import MoreScreen from './lib/screens/MoreScreen.svelte'
  import FilesScreen from './lib/screens/FilesScreen.svelte'
  import TalkScreen from './lib/screens/TalkScreen.svelte'
  import NotesScreen from './lib/screens/NotesScreen.svelte'
  import NoteDetail from './lib/screens/NoteDetail.svelte'
  import SharesScreen from './lib/screens/SharesScreen.svelte'
  import SettingsScreen from './lib/screens/SettingsScreen.svelte'
  import AccountsSettings from './lib/screens/AccountsSettings.svelte'
  import AccountEdit from './lib/screens/AccountEdit.svelte'
  import AccountFolders from './lib/screens/AccountFolders.svelte'
  import AppearanceSettings from './lib/screens/AppearanceSettings.svelte'
  import LanguageSettings from './lib/screens/LanguageSettings.svelte'
  import ThemeEditor from './lib/screens/ThemeEditor.svelte'
  import NotificationSettings from './lib/screens/NotificationSettings.svelte'
  import MailSettings from './lib/screens/MailSettings.svelte'
  import SecuritySettings from './lib/screens/SecuritySettings.svelte'
  import EncryptionSettings from './lib/screens/EncryptionSettings.svelte'
  import NextcloudSettings from './lib/screens/NextcloudSettings.svelte'
  import SourcesSettings from './lib/screens/SourcesSettings.svelte'
  import FeatureSettings from './lib/screens/FeatureSettings.svelte'
  import AboutScreen from './lib/screens/AboutScreen.svelte'
  import PrivacyScreen from './lib/screens/PrivacyScreen.svelte'
  import Intro from './lib/Intro.svelte'
  import { introSeen, introVariantFor, type IntroVariant } from './lib/introState'
  import { m } from './paraglide/messages'

  type Phase = 'booting' | 'locked' | 'setup' | 'ready'

  let phase = $state<Phase>('booting')
  let dbStatus = $state<DatabaseStatusView | null>(null)
  let unreadTotal = $state(0)
  let unreadByAccount = $state<Record<string, number>>({})
  let outboxCount = $state(0)
  let ncConnected = $state(false)

  /** The composer, when open. Null closes it. */
  let compose = $state<ComposeInitial | null>(null)
  /** The event editor, when open. */
  let eventEdit = $state<{ event: unknown | null; day?: string } | null>(null)
  /** The add-account sheet, opened from Settings → Accounts. */
  let addingAccount = $state(false)
  /** The introduction, when open, and which one. */
  let intro = $state<IntroVariant | null>(null)

  /* ── Boot ────────────────────────────────────────────────────── */

  async function boot() {
    // Settings first: the theme should be right on the first frame
    // the user sees, not one repaint later.
    const settings = await settingsStore.load()
    if (settings) {
      applyCustomTheme({
        base: settings.custom_theme_base,
        accent: settings.custom_theme_accent,
        font: settings.custom_theme_font,
      })
      applyTheme(settings.theme_name, settings.theme_mode)
    }

    try {
      dbStatus = await api.settings.databaseStatus()
      if (dbStatus?.locked) {
        phase = 'locked'
        return
      }
    } catch {
      // No envelope yet (first run) — treat as unlocked and let the
      // cache open itself.
    }

    await afterUnlock()
  }

  async function afterUnlock() {
    const accounts = await accountsStore.load()
    phase = accounts.length === 0 ? 'setup' : 'ready'
    void contactsStore.load()
    void refreshCounts()
    void checkNextcloud()
  }

  /**
   * Re-read the three badge numbers.
   *
   * Three points about the shape, all of them about not making a
   * sync tick feel like a stall:
   *
   *   * The two unread reads run **concurrently**. They were
   *     sequential, which paid two full IPC round trips for two
   *     independent cache queries.
   *   * `singleFlight` keeps overlapping calls to one. Several push
   *     channels land on this function, and a tick that reports new
   *     mail on three accounts used to start three identical passes
   *     that raced each other to assign the same three variables.
   *   * Failures leave the previous number on screen rather than
   *     zeroing it — a badge that blinks to 0 on a flaky read is
   *     worse than a badge that is briefly stale.
   */
  const refreshCounts = singleFlight(async () => {
    const [total, byAccount, outbox] = await Promise.allSettled([
      api.mail.getTotalUnread(),
      api.mail.getUnreadCountsByAccount(),
      api.compose.countOutbox(),
    ])
    if (total.status === 'fulfilled') unreadTotal = total.value
    if (byAccount.status === 'fulfilled') unreadByAccount = byAccount.value
    if (outbox.status === 'fulfilled') outboxCount = outbox.value
  })

  /**
   * The entry point every *push-driven* caller uses.
   *
   * A single sync tick can emit new-mail, unread-count and
   * outbox events together; without the debounce each one starts its
   * own pass and the burst costs three times what it should. 250ms is
   * below the threshold where a badge update reads as laggy and well
   * above the width of an event burst.
   */
  const scheduleCountRefresh = debounce(() => void refreshCounts(), 250)

  async function checkNextcloud() {
    try {
      ncConnected = (await api.nextcloud.getNextcloudAccounts()).length > 0
    } catch {
      ncConnected = false
    }
  }

  $effect(() => {
    void boot()
  })

  /** Keep `system` theme mode following the OS. */
  $effect(() => {
    const settings = settingsStore.value
    if (!settings) return
    // The generated stylesheet is installed whatever theme is active:
    // it is scoped to `[data-theme="custom"]`, so it costs nothing when
    // a stock theme is selected and makes switching to the custom one
    // instant rather than a compile-then-paint.
    applyCustomTheme({
      base: settings.custom_theme_base,
      accent: settings.custom_theme_accent,
      font: settings.custom_theme_font,
    })
    applyTheme(settings.theme_name, settings.theme_mode)
    return installSystemModeListener(settings.theme_mode, settings.theme_name)
  })

  /* ── Backend events ──────────────────────────────────────────── */

  /**
   * Ceiling on OS notifications raised in one burst window.
   *
   * The backend already collapses a poll into one event per folder,
   * so reaching this needs several accounts reporting at once. It is
   * a backstop, not the primary mechanism: each `showNotification`
   * is an IPC call into a plugin that then talks to
   * `UNUserNotificationCenter`, and enough of them in one tick is
   * what makes the UI thread visibly stop.
   */
  const MAX_NOTIFICATIONS_PER_BURST = 3

  let notificationsThisBurst = 0
  const resetNotificationBurst = debounce(() => (notificationsThisBurst = 0), 3000)

  /**
   * Raise one local notification, subject to the burst ceiling.
   *
   * Beyond the ceiling the notification is dropped rather than
   * queued: it is a "you have new mail" nudge, and a nudge that
   * arrives after the user has already opened the app is noise. The
   * unread badge carries the information either way.
   */
  function notify(options: { title: string; body: string }) {
    if (notificationsThisBurst >= MAX_NOTIFICATIONS_PER_BURST) return
    notificationsThisBurst++
    resetNotificationBurst()
    try {
      api.platform.showNotification(options)
    } catch (e) {
      // A denied permission must not take the badge update with it.
      console.warn('notification failed', e)
    }
  }

  /**
   * New mail arrives as a push from the sync loop. Two things
   * happen: the badge updates, and — if the user asked for it — a
   * local notification is raised.
   *
   * Bursts are collapsed in the backend: one event per folder per
   * poll, carrying the batch size. Anything above a couple of
   * messages becomes one summary rather than one notification each.
   */
  function notifyNewMail(payload: {
    count: number
    from?: string
    subject?: string
    folder: string
  }) {
    if (!settingsStore.value?.notifications_enabled) return
    // `count` is absent on nothing the backend sends, but a missing
    // one must not silently mean "no new mail" — that is precisely
    // the bug this branch had before the field was wired up.
    const count = payload.count ?? 1
    if (count > 2) {
      notify({
        title: m.mobile_notify_burst_title({ count }),
        body: m.mobile_notify_burst_body({ folder: payload.folder }),
      })
    } else if (payload.from) {
      notify({
        title: displayName(payload.from),
        body: payload.subject || m.mobile_no_subject(),
      })
    }
  }

  $effect(() => {
    if (phase !== 'ready') return
    const unlisteners: Promise<UnlistenFn>[] = []

    unlisteners.push(
      api.onAppEvent('new-mail', (event) => {
        notifyNewMail(event.payload)
        scheduleCountRefresh()
      }),
      api.onAppEvent('unread-count-updated', (event) => {
        unreadTotal = event.payload
      }),
      api.onAppEvent('unread-count-by-account-updated', (event) => {
        unreadByAccount = event.payload
      }),
      api.onAppEvent('outbox-updated', () => {
        scheduleCountRefresh()
      }),
      api.onAppEvent('event-reminder', (event) => {
        const payload = event.payload
        if (!settingsStore.value?.calendar_reminders_enabled) return
        notify({
          title: `📅 ${payload.summary}`,
          body: m.mobile_notify_event_body({
            minutes: payload.minutesBefore,
            location: payload.location ?? '',
          }),
        })
      }),
      api.onAppEvent('message-reminder', (event) => {
        const payload = event.payload
        notify({
          title: m.mobile_notify_reminder_title(),
          body: payload.subject || displayName(payload.from),
        })
        toasts.show(payload.subject || m.mobile_no_subject(), {
          action: {
            label: m.mobile_open(),
            run: () =>
              nav.go('mail', 'mail-message', {
                accountId: payload.accountId,
                folder: payload.folder,
                uid: payload.uid,
                subject: payload.subject,
              }),
          },
        })
      }),
    )

    return () => {
      for (const pending of unlisteners) void pending.then((un) => un())
      scheduleCountRefresh.cancel()
      resetNotificationBurst.cancel()
    }
  })

  /* ── Composer plumbing ───────────────────────────────────────── */

  function openCompose(initial: ComposeInitial = {}) {
    compose = initial
  }

  async function openReply(mail: Email, kind: 'reply' | 'reply-all' | 'forward', uid: number) {
    const account = accountsStore.list.find((a) => a.id === mail.account_id)
    const initial = buildReplyInitial(mail, kind, account?.email ?? '', uid)

    if (kind === 'forward' && (mail.attachments ?? []).length > 0) {
      // Carrying the attachments is what people mean by "forward";
      // fetching them here (rather than in the composer) keeps the
      // composer free of message-fetch logic.
      try {
        initial.attachments = await Promise.all(
          mail.attachments.map(
            async (att: { filename: string; content_type: string; part_id: number }) => ({
              filename: att.filename,
              content_type: att.content_type,
              data: await api.mail.downloadEmailAttachment({
                accountId: mail.account_id,
                folder: mail.folder,
                uid,
                partId: att.part_id,
              }),
            }),
          ),
        )
      } catch (e) {
        toasts.error(m.mobile_forward_attachments_failed(), e)
      }
    }

    compose = initial
  }

  /** Open the composer for a draft the user tapped in the Drafts
   *  folder: pull the message, its body and its attachments back in. */
  async function openDraft(ref: { accountId: string; folder: string; uid: number }) {
    try {
      const draft = await api.mail.fetchMessage(ref)
      const attachments = await Promise.all(
        (draft.attachments ?? []).map(
          async (att: { filename: string; content_type: string; part_id: number }) => ({
            filename: att.filename,
            content_type: att.content_type,
            data: await api.mail.downloadEmailAttachment({ ...ref, partId: att.part_id }),
          }),
        ),
      )
      compose = {
        accountId: ref.accountId,
        to: draft.to.join(', '),
        cc: draft.cc.join(', '),
        subject: draft.subject,
        body: draft.body_text ?? '',
        attachments,
        draftRef: ref,
        // The stored body already carries whatever signature was
        // appended when it was saved; re-stamping would stack them.
        skipSignature: true,
      }
    } catch (e) {
      toasts.error(m.mobile_draft_open_failed(), e)
    }
  }

  function handleCompose(initial?: Record<string, unknown>) {
    if (initial?.draftRef) {
      void openDraft(initial.draftRef as { accountId: string; folder: string; uid: number })
      return
    }
    openCompose((initial ?? {}) as ComposeInitial)
  }

  /* ── Screen rendering ────────────────────────────────────────── */

  const current = $derived(nav.current)

  /**
   * The inbox the Mail tab opens on: the one account's own; with
   * several, all of them merged — or the one picked with the switch
   * under the inbox title (`mail/inboxScope`). With a single account
   * the unified view would only add an account label to every row.
   *
   * `key` remounts the list when that choice flips (an account was
   * picked or added), since the list loads its mailbox once on mount.
   */
  const inbox = $derived.by(() => {
    const id = resolveInboxAccount(
      accountsStore.list.map((a) => a.id as string),
      inboxScope.accountId,
    )
    return { accountId: id, key: id ?? '*' }
  })

  /**
   * Screens that stay mounted while another screen is pushed over them.
   *
   * The shell used to render only the screen on top, so opening a
   * message destroyed the list and going back built it again: a cache
   * read over IPC, every row re-created, the scroll position lost — and
   * after 30 seconds a full server round trip on top. Going back is the
   * most frequent navigation in a mail app, so the lists now stay where
   * they were, hidden under the reader. Back is instant and lands on
   * the row the user left from.
   *
   * Only the mail lists: they are what people return to, and they keep
   * themselves current (`MailList`'s `active` prop, its event listeners,
   * `mail/listBus.ts`). Other screens still remount, because they read
   * their data once on mount and would show it stale.
   */
  const KEEP_ALIVE = new Set<ScreenKind>(['mail-inbox', 'mail-list', 'mail-search'])

  /**
   * Everything mounted: each tab's kept-alive screens plus the screen on
   * top. A fixed order (tab by tab, bottom to top) so a screen's DOM
   * node is never moved — moving a scrolled element resets its scroll
   * position, which is half of what keeping it alive is for. Which one
   * is visible is decided by `data-current`, not by position.
   */
  const mounted = $derived.by(() => {
    const out: Screen[] = []
    for (const stack of Object.values(nav.stacks)) {
      for (const s of stack) {
        if (s.id === current.id || KEEP_ALIVE.has(s.kind)) out.push(s)
      }
    }
    return out
  })

  /** The screen under the top one, revealed while an edge swipe drags
   *  the top one away — when it is mounted (`KEEP_ALIVE`). */
  let peekId = $state<number | null>(null)

  /**
   * Screens that bring their own bottom bar hide the tab bar while
   * they're up. Two stacked toolbars eat a fifth of a phone screen
   * and read as one confused control strip; the reader's Reply /
   * Archive / Delete row is the thing a thumb should find there.
   */
  const HIDES_TAB_BAR = new Set(['mail-message'])
  const showTabBar = $derived(!HIDES_TAB_BAR.has(current.kind))

  /**
   * Hiding the feature you're standing on has to land somewhere.
   *
   * The settings screen can take the active tab off the bar (or hide
   * its feature outright) while that tab is the one behind the
   * settings stack. Without this the app would keep rendering a
   * screen with no tab to return to. Mail is the fallback for the
   * same reason it's the first tab: it's the one that always exists.
   */
  $effect(() => {
    const visible: string[] = ['mail', 'more', ...features.tabs]
    if (!visible.includes(nav.tab)) nav.selectTab('mail')
  })

  /* ── Introduction and test mode ──────────────────────────────── */

  /** Which introduction fits the accounts that exist right now. */
  const introVariant = $derived(introVariantFor(accountsStore.list))

  /**
   * Show the introduction once per variant, the first time the app is
   * ready with that kind of account — right after "Test mode" on the
   * first-run screen, or after the first real account is added.
   *
   * Waits for any modal to close: the add-account sheet especially,
   * which is still on screen in the moment its account appears.
   */
  $effect(() => {
    if (phase !== 'ready' || intro || compose || eventEdit || addingAccount) return
    const variant = introVariant
    if (variant && !introSeen(variant)) intro = variant
  })

  /**
   * No accounts left → back to the first-run screen. Covers removing the
   * last account from Settings → Accounts, the demo included, which used
   * to leave the app standing on empty tabs with nothing to set up from.
   */
  $effect(() => {
    if (phase === 'ready' && !accountsStore.loading && accountsStore.list.length === 0) {
      nav.resetAll()
      phase = 'setup'
    }
  })

  function showIntro() {
    intro = introVariant ?? 'normal'
  }

  /** "Connect now" at the end of the real-account introduction. */
  function openCloudSources() {
    nav.go('more', 'settings')
    nav.push('settings-sources')
  }

  /**
   * Leave test mode: delete the demo and everything it seeded, then land
   * wherever the remaining accounts put the user — back on the first-run
   * screen when the demo was the only one, in the app otherwise.
   *
   * Every stack is reset first: screens hold account ids and folder names
   * that no longer exist.
   */
  async function leaveDemo(): Promise<void> {
    try {
      await api.accounts.removeDemoAccount()
    } catch (e) {
      toasts.error(m.mobile_testmode_leave_failed(), e)
      return
    }
    nav.resetAll()
    let remaining = 0
    try {
      remaining = (await accountsStore.load()).length
    } catch {
      // The demo is gone either way; with no readable list, the
      // first-run screen is the safe place to be.
    }
    void contactsStore.load()
    void refreshCounts()
    void checkNextcloud()
    if (remaining === 0) phase = 'setup'
    toasts.show(m.mobile_testmode_left())
  }

  /* ── Edge swipe back ─────────────────────────────────────────── */

  /**
   * Swiping in from the left edge goes back — the gesture every iOS
   * navigation stack has, and the first one people try. Without it the
   * only way back was the button in the top-left corner, the hardest
   * point on the screen for a thumb to reach.
   *
   * Four decisions:
   *
   *   * **The edge wins.** A touch that starts within `EDGE_PX` of the
   *     edge is claimed in the capture phase, before a mail row's own
   *     swipe sees it — the same priority the platform gives its own
   *     back gesture. Taps are unaffected: only `pointerdown` is
   *     stopped, and a button's `click` still fires.
   *   * **The screen follows the finger**, written straight to the DOM
   *     in a frame callback, for the same reasons `SwipeRow` does it.
   *   * **Far enough or fast enough commits**, like a sheet's drag.
   *   * **It is never the only way.** The back button stays, and the
   *     gesture is off while a sheet or modal is up.
   */
  const EDGE_PX = 24
  const EDGE_LOCK_PX = 10
  const EDGE_COMMIT_RATIO = 0.35
  const EDGE_COMMIT_VELOCITY = 0.45 // px per ms

  let appBody: HTMLElement | undefined = $state()
  /** The host of the screen on top, looked up when a swipe starts. */
  let screenHost: HTMLElement | null = null

  /** The screen a committed edge swipe landed on. It arrives without the
   *  slide-in animation — the finger already did the sliding. */
  let arrivedByGesture = $state<number | null>(null)

  let edge: { pointerId: number; x0: number; y0: number; t0: number; axis: 'none' | 'x'; dx: number } | null =
    null
  let edgeFrame = 0

  function edgeGestureAllowed(): boolean {
    return phase === 'ready' && nav.canGoBack && !compose && !eventEdit && !addingAccount && !hasOpenSheet()
  }

  function paintEdge() {
    edgeFrame = 0
    if (!screenHost || !edge) return
    screenHost.style.transition = 'none'
    screenHost.style.transform = `translate3d(${edge.dx}px, 0, 0)`
  }

  function settleEdge(toX: number, then?: () => void) {
    const node = screenHost
    if (edgeFrame) cancelAnimationFrame(edgeFrame)
    edgeFrame = 0
    if (!node) return then?.()
    node.style.transition = 'transform var(--m-base) var(--m-ease)'
    node.style.transform = `translate3d(${toX}px, 0, 0)`
    // A fixed wait rather than `transitionend`: with Reduce Motion the
    // duration is ~1ms and the event is not reliably delivered at all.
    const ms = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--m-base')) || 220
    setTimeout(() => {
      if (!then) {
        node.style.transition = ''
        node.style.transform = ''
      }
      then?.()
    }, ms)
  }

  function onEdgeDown(e: PointerEvent) {
    if (e.pointerType === 'mouse' || e.clientX > EDGE_PX || !edgeGestureAllowed()) return
    e.stopPropagation()
    screenHost = appBody?.querySelector<HTMLElement>('.screen-host[data-current]') ?? null
    edge = { pointerId: e.pointerId, x0: e.clientX, y0: e.clientY, t0: performance.now(), axis: 'none', dx: 0 }
  }

  function onEdgeMove(e: PointerEvent) {
    if (!edge || edge.pointerId !== e.pointerId) return
    const dx = e.clientX - edge.x0
    const dy = e.clientY - edge.y0
    if (edge.axis === 'none') {
      if (Math.abs(dx) < EDGE_LOCK_PX && Math.abs(dy) < EDGE_LOCK_PX) return
      if (dx <= Math.abs(dy)) {
        // Vertical, or leftward: not a back swipe. Let it go.
        edge = null
        return
      }
      edge.axis = 'x'
      const stack = nav.stack
      peekId = stack.length > 1 ? stack[stack.length - 2].id : null
      try {
        appBody?.setPointerCapture(e.pointerId)
      } catch {
        // The pointer may already be gone; the gesture still tracks
        // while the finger stays over the screen.
      }
    }
    e.stopPropagation()
    edge.dx = Math.max(0, dx)
    if (!edgeFrame) edgeFrame = requestAnimationFrame(paintEdge)
  }

  function onEdgeEnd(e: PointerEvent) {
    if (!edge || edge.pointerId !== e.pointerId) return
    const gesture = edge
    edge = null
    if (gesture.axis !== 'x') return
    const width = appBody?.clientWidth ?? window.innerWidth
    const velocity = gesture.dx / Math.max(1, performance.now() - gesture.t0)
    const commit =
      e.type === 'pointerup' &&
      (gesture.dx > width * EDGE_COMMIT_RATIO || velocity > EDGE_COMMIT_VELOCITY)
    if (!commit) {
      settleEdge(0, () => {
        peekId = null
        if (screenHost) {
          screenHost.style.transition = ''
          screenHost.style.transform = ''
        }
      })
      return
    }
    settleEdge(width, () => {
      const stack = nav.stack
      arrivedByGesture = stack.length > 1 ? stack[stack.length - 2].id : null
      peekId = null
      nav.pop()
    })
  }

  /* Any navigation that did not come from the gesture animates normally. */
  $effect(() => {
    if (current.id !== arrivedByGesture) arrivedByGesture = null
  })
</script>

{#if phase === 'booting'}
  <div class="boot">
    <img src="unkai-logo://storm" alt="" class="h-16 w-16" />
    <Spinner full={false} />
  </div>
{:else if phase === 'locked' && dbStatus}
  <LockScreen
    status={dbStatus}
    onunlocked={() => {
      phase = 'booting'
      void afterUnlock()
    }}
  />
{:else if phase === 'setup'}
  <AccountSetup
    firstRun
    onclose={(added) => {
      if (added) void afterUnlock()
    }}
  />
{:else}
  <div class="app">
    <main
      class="app__body"
      bind:this={appBody}
      onpointerdowncapture={onEdgeDown}
      onpointermovecapture={onEdgeMove}
      onpointerupcapture={onEdgeEnd}
      onpointercancelcapture={onEdgeEnd}
    >
      {#each mounted as s (s.id)}
        {@const isCurrent = s.id === current.id}
        <div
          class="screen-host"
          data-current={isCurrent ? 'true' : undefined}
          data-peek={s.id === peekId ? 'true' : undefined}
          data-direction={nav.direction}
          data-instant={s.id === arrivedByGesture ? 'true' : undefined}
          inert={!isCurrent}
          aria-hidden={isCurrent ? undefined : 'true'}
        >
          {@render screen(s, isCurrent)}
        </div>
      {/each}
    </main>

    {#if showTabBar}
      <TabBar unread={unreadTotal} outbox={outboxCount} />
    {/if}
  </div>
{/if}

{#snippet screen(current: Screen, active: boolean)}
          {#if current.kind === 'mail-inbox'}
            {#key inbox.key}
              <MailList
                accountId={inbox.accountId}
                folder="INBOX"
                title={m.folder_name_inbox()}
                root
                {active}
                {unreadByAccount}
                oncompose={handleCompose}
              />
            {/key}
          {:else if current.kind === 'mail-home'}
            <MailHome {unreadByAccount} {outboxCount} />
          {:else if current.kind === 'mail-list'}
            <MailList
              accountId={current.props.accountId as string | null}
              folder={current.props.folder as string}
              title={current.props.title as string}
              {active}
              oncompose={handleCompose}
            />
          {:else if current.kind === 'mail-message'}
            <MailMessage
              accountId={current.props.accountId as string}
              folder={current.props.folder as string}
              uid={current.props.uid as number}
              subject={current.props.subject as string}
              onreply={openReply}
              onmailto={(fields) => openCompose(fields)}
            />
          {:else if current.kind === 'mail-search'}
            <MailSearch
              accountId={(current.props.accountId as string | null) ?? null}
              folder={(current.props.folder as string | null) ?? null}
            />
          {:else if current.kind === 'outbox'}
            <Outbox />
          {:else if current.kind === 'calendar-home'}
            <CalendarScreen
              onedit={(event, day) => (eventEdit = { event, day })}
            />
          {:else if current.kind === 'event-detail'}
            <EventDetail
              event={current.props.event as never}
              onedit={(event) => (eventEdit = { event })}
            />
          {:else if current.kind === 'tasks-home'}
            <TasksScreen />
          {:else if current.kind === 'contacts-home'}
            <ContactsScreen />
          {:else if current.kind === 'contact-detail'}
            <ContactDetail
              contact={current.props.contact as never}
              oncompose={(initial) => openCompose(initial)}
            />
          {:else if current.kind === 'more-home'}
            <MoreScreen {outboxCount} {ncConnected} />
          {:else if current.kind === 'files'}
            <FilesScreen />
          {:else if current.kind === 'talk'}
            <TalkScreen oncompose={(initial) => openCompose(initial)} />
          {:else if current.kind === 'notes'}
            <NotesScreen />
          {:else if current.kind === 'note-detail'}
            <NoteDetail
              note={current.props.note as never}
              ncId={current.props.ncId as string}
            />
          {:else if current.kind === 'shares'}
            <SharesScreen oncompose={(initial) => openCompose(initial)} />
          {:else if current.kind === 'settings'}
            <SettingsScreen onleavedemo={leaveDemo} onshowintro={showIntro} />
          {:else if current.kind === 'settings-accounts'}
            <AccountsSettings onaddaccount={() => (addingAccount = true)} />
          {:else if current.kind === 'settings-account'}
            <AccountEdit accountId={current.props.accountId as string} />
          {:else if current.kind === 'settings-account-folders'}
            <AccountFolders accountId={current.props.accountId as string} />
          {:else if current.kind === 'settings-appearance'}
            <AppearanceSettings />
          {:else if current.kind === 'settings-language'}
            <LanguageSettings />
          {:else if current.kind === 'settings-theme-editor'}
            <ThemeEditor />
          {:else if current.kind === 'settings-notifications'}
            <NotificationSettings />
          {:else if current.kind === 'settings-mail'}
            <MailSettings />
          {:else if current.kind === 'settings-security'}
            <SecuritySettings />
          {:else if current.kind === 'settings-encryption'}
            <EncryptionSettings />
          {:else if current.kind === 'settings-nextcloud'}
            <NextcloudSettings />
          {:else if current.kind === 'settings-sources'}
            <SourcesSettings />
          {:else if current.kind === 'settings-features'}
            <FeatureSettings />
          {:else if current.kind === 'settings-privacy'}
            <PrivacyScreen />
          {:else if current.kind === 'settings-about'}
            <AboutScreen />
          {/if}
{/snippet}

{#if compose}
  <Compose initial={compose} onclose={() => (compose = null)} />
{/if}

{#if eventEdit}
  <EventEdit
    event={eventEdit.event as never}
    day={eventEdit.day}
    onclose={() => (eventEdit = null)}
  />
{/if}

{#if intro && phase === 'ready'}
  <Intro
    variant={intro}
    onclose={() => (intro = null)}
    onconnect={openCloudSources}
  />
{/if}

{#if addingAccount}
  <AccountSetup
    onclose={(added) => {
      addingAccount = false
      if (added) void refreshCounts()
    }}
  />
{/if}

<Toasts />

<style>
  /* Positioned, because the tab bar floats over the bottom of the body
     rather than taking a strip below it (see `TabBar`). */
  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .app__body {
    flex: 1;
    min-height: 0;
    position: relative;
    overflow: hidden;
  }

  .screen-host {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--ui-canvas);
  }

  /* A kept-alive screen under the top one: laid out, so it keeps its
     scroll position, but not painted and not reachable (`inert`). */
  .screen-host:not([data-current]):not([data-peek]) {
    visibility: hidden;
  }

  /* No z-index on the hosts: one would make every host a stacking
     context and cap the sheets a screen opens beneath the tab bar.
     Document order already puts the top screen above the one an edge
     swipe uncovers (both are in stack order), and the rest are hidden. */
  .screen-host[data-current] {
    /* The animation belongs to *becoming* the top screen, so it runs
       both for a screen just mounted and for a kept-alive one that a
       Back brings up again. */
    animation: screen-in var(--m-slow) var(--m-ease);
  }

  /* Forward pushes come from the right, Back from the left — the
     direction the stack actually moved, so the gesture and the
     animation agree. */
  .screen-host[data-current][data-direction='back'] {
    animation-name: screen-in-back;
  }

  /* Arrived by an edge swipe: the finger already moved the screen. */
  .screen-host[data-current][data-instant] {
    animation: none;
  }

  /* While dragged, the screen's leading edge casts a shadow onto the
     canvas it uncovers, so it reads as a sheet of paper being moved. */
  .screen-host {
    box-shadow: -8px 0 24px -12px rgb(0 0 0 / 0.35);
  }

  @keyframes screen-in {
    from {
      transform: translateX(18%);
      opacity: 0.4;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }

  @keyframes screen-in-back {
    from {
      transform: translateX(-14%);
      opacity: 0.4;
    }
    to {
      transform: translateX(0);
      opacity: 1;
    }
  }

  .boot {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
  }
</style>
