<script lang="ts">
  /**
   * The calendar: a month grid with the selected day's agenda
   * underneath.
   *
   * That split is the one that works on a phone. A week view needs
   * horizontal room the screen doesn't have, and a pure agenda list
   * loses the shape of the month — which is what people actually
   * look at a calendar for. The grid answers "how busy is the
   * 14th?" with a dot; the list under it answers "what exactly?".
   */
  import { formatLocale } from '../locale'
  import * as api from '../api'
  import type { CalendarEvent, CalendarSummary, NextcloudAccount } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import EmptyState from '../ui/EmptyState.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import Fab from '../ui/Fab.svelte'
  import PullToRefresh from '../ui/PullToRefresh.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { dayHeading, timeOnly, toDateInput } from '../format'
  import { m } from '../../paraglide/messages'

  interface Props {
    onedit: (event: CalendarEvent | null, dayIso?: string) => void
  }

  let { onedit }: Props = $props()

  let ncAccounts = $state<NextcloudAccount[]>([])
  let calendars = $state<CalendarSummary[]>([])
  let events = $state<CalendarEvent[]>([])
  let loading = $state(true)
  let syncing = $state(false)

  /** First day of the displayed month. */
  let month = $state(new Date(new Date().getFullYear(), new Date().getMonth(), 1))
  let selected = $state(new Date())

  const visibleCalendars = $derived(calendars.filter((c) => !c.hidden))

  async function loadCalendars() {
    try {
      ncAccounts = await api.nextcloud.getNextcloudAccounts()
      const all: CalendarSummary[] = []
      for (const acc of ncAccounts) {
        try {
          all.push(...(await api.calendar.getCachedCalendars({ ncId: acc.id })))
        } catch {
          // A server that's unreachable right now still has cached
          // events; skip its calendar list and carry on.
        }
      }
      calendars = all
    } catch (e) {
      toasts.error(m.mobile_calendars_load_failed(), e)
    }
  }

  /** Load a month plus a week of padding, so the grid's leading and
   *  trailing days aren't empty when they belong to a busy week. */
  async function loadEvents() {
    if (visibleCalendars.length === 0) {
      events = []
      loading = false
      return
    }
    const start = new Date(month.getFullYear(), month.getMonth(), -7)
    const end = new Date(month.getFullYear(), month.getMonth() + 1, 7)
    try {
      events = await api.calendar.getCachedEvents({
        calendarIds: visibleCalendars.map((c) => c.id),
        rangeStart: start.toISOString(),
        rangeEnd: end.toISOString(),
      })
    } catch (e) {
      toasts.error(m.mobile_events_load_failed(), e)
    } finally {
      loading = false
    }
  }

  async function sync() {
    syncing = true
    try {
      for (const acc of ncAccounts) {
        await api.calendar.syncNextcloudCalendars({ ncId: acc.id })
      }
      await loadCalendars()
      await loadEvents()
      toasts.success(m.mobile_calendar_synced())
    } catch (e) {
      toasts.error(m.mobile_calendar_sync_failed(), e)
    } finally {
      syncing = false
    }
  }

  $effect(() => {
    void loadCalendars().then(loadEvents)
  })

  $effect(() => {
    void month
    void calendars.length
    void loadEvents()
  })

  /* ── Grid maths ──────────────────────────────────────────────── */

  /** Monday-first weeks: matches the locale of everyone this is
   *  being built for, and `Intl` gives no portable first-day API. */
  const grid = $derived.by(() => {
    const first = new Date(month.getFullYear(), month.getMonth(), 1)
    const offset = (first.getDay() + 6) % 7
    const days: Date[] = []
    for (let i = 0; i < 42; i++) {
      days.push(new Date(month.getFullYear(), month.getMonth(), 1 - offset + i))
    }
    return days
  })

  const weekdayLabels = $derived.by(() => {
    const base = new Date(2024, 0, 1) // a Monday
    return Array.from({ length: 7 }, (_, i) =>
      new Date(2024, 0, 1 + i).toLocaleDateString(formatLocale(), { weekday: 'narrow' }),
    ).map((label, i) => ({ label, key: i, base }))
  })

  function sameDay(a: Date, b: Date): boolean {
    return (
      a.getFullYear() === b.getFullYear() &&
      a.getMonth() === b.getMonth() &&
      a.getDate() === b.getDate()
    )
  }

  function eventsOn(day: Date): CalendarEvent[] {
    return events
      .filter((e: CalendarEvent) => {
        const start = new Date(e.start)
        const end = new Date(e.end)
        const dayStart = new Date(day.getFullYear(), day.getMonth(), day.getDate())
        const dayEnd = new Date(dayStart.getTime() + 86_400_000)
        // Overlap, not equality — a multi-day event belongs on every
        // day it covers.
        return start < dayEnd && end > dayStart
      })
      .sort((a, b) => new Date(a.start).getTime() - new Date(b.start).getTime())
  }

  const selectedEvents = $derived(eventsOn(selected))

  function calendarColor(event: CalendarEvent): string {
    const cal = calendars.find((c) => event.id.startsWith(c.id))
    return cal?.color || 'var(--color-primary-500)'
  }

  function shiftMonth(delta: number) {
    month = new Date(month.getFullYear(), month.getMonth() + delta, 1)
  }

  function today() {
    const now = new Date()
    month = new Date(now.getFullYear(), now.getMonth(), 1)
    selected = now
  }
</script>

<div class="screen">
  <!-- "Today" as a word on the left, the two month arrows together on
       the right. The old bar had five controls, three of them icons a
       new user had to guess at; syncing is a pull on the agenda now,
       the same gesture as in Mail, and the background loop does it
       anyway. -->
  <NavBar
    title={month.toLocaleDateString(formatLocale(), { month: 'long', year: 'numeric' })}
    backLabel={nav.canGoBack ? m.mobile_more() : null}
    onback={() => nav.pop()}
    busy={syncing}
  >
    {#snippet leading()}
      <button type="button" class="bar-button bar-button--text" onclick={today}>
        {m.mobile_today()}
      </button>
    {/snippet}
    {#snippet actions()}
      <!-- Pushed from More, the leading slot holds Back, so Today moves
           in with the arrows. -->
      {#if nav.canGoBack}
        <button type="button" class="bar-button bar-button--text" onclick={today}>
          {m.mobile_today()}
        </button>
      {/if}
      <!-- Only while there is no agenda to pull (see Contacts). -->
      {#if !loading && calendars.length === 0 && ncAccounts.length > 0}
        <button
          type="button"
          class="bar-button"
          aria-label={m.mobile_sync()}
          onclick={sync}
          disabled={syncing}
        >
          <Icon name={syncing ? 'loading' : 'sync'} size={20} />
        </button>
      {/if}
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_previous_month()}
        onclick={() => shiftMonth(-1)}
      >
        <Icon name="nav-backward" size={20} />
      </button>
      <button
        type="button"
        class="bar-button"
        aria-label={m.mobile_next_month()}
        onclick={() => shiftMonth(1)}
      >
        <Icon name="nav-forward" size={20} />
      </button>
    {/snippet}
  </NavBar>

  {#if loading}
    <Spinner />
  {:else if calendars.length === 0}
    <EmptyState
      icon="calendar"
      title={m.mobile_no_calendars_title()}
      body={m.mobile_no_calendars_body()}
      action={{ label: m.mobile_connect_nextcloud(), run: () => nav.go('more', 'settings-nextcloud') }}
    />
  {:else}
    <div class="month">
      <div class="month__weekdays">
        {#each weekdayLabels as day (day.key)}
          <span>{day.label}</span>
        {/each}
      </div>
      <div class="month__grid">
        {#each grid as day (day.getTime())}
          {@const dayEvents = eventsOn(day)}
          {@const inMonth = day.getMonth() === month.getMonth()}
          <button
            type="button"
            class="month__day"
            class:month__day--muted={!inMonth}
            class:month__day--today={sameDay(day, new Date())}
            class:month__day--selected={sameDay(day, selected)}
            aria-current={sameDay(day, new Date()) ? 'date' : undefined}
            aria-pressed={sameDay(day, selected)}
            onclick={() => (selected = day)}
          >
            <span class="month__num">{day.getDate()}</span>
            <span class="month__dots">
              {#each dayEvents.slice(0, 3) as event (event.id)}
                <span class="month__dot" style="background: {calendarColor(event)}"></span>
              {/each}
            </span>
          </button>
        {/each}
      </div>
    </div>

    <PullToRefresh onrefresh={sync} enabled={ncAccounts.length > 0}>
      <h2 class="section-title">{dayHeading(selected)}</h2>
      {#if selectedEvents.length === 0}
        <p class="agenda__empty">{m.mobile_no_events()}</p>
      {:else}
        <ul class="list-group">
          {#each selectedEvents as event (event.id)}
            <li>
              <button
                class="list-row"
                onclick={() => nav.push('event-detail', { event })}
              >
                <span class="agenda__bar" style="background: {calendarColor(event)}"></span>
                <span class="min-w-0 flex-1">
                  <span class="block truncate list-row__title">{event.summary}</span>
                  <span class="block list-row__detail">
                    {timeOnly(event.start)} – {timeOnly(event.end)}
                  </span>
                  {#if event.location}
                    <span class="agenda__place list-row__detail">
                      <Icon name="location" size={14} />
                      <span class="truncate">{event.location}</span>
                    </span>
                  {/if}
                </span>
                <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
      <div class="h-24"></div>
    </PullToRefresh>

    <Fab
      label={m.mobile_new_event()}
      onclick={() => onedit(null, toDateInput(selected))}
    />
  {/if}
</div>

<style>
  /* The month is a card on the canvas, like every other group; the
     agenda below scrolls on its own. */
  .month {
    flex-shrink: 0;
    margin: var(--s-1) var(--gutter) 0;
    padding: var(--s-2) var(--s-1) var(--s-2);
    border-radius: var(--r-lg);
    border: 1px solid var(--ui-card-border);
    background: var(--ui-card);
  }

  .month__weekdays,
  .month__grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
  }

  .month__weekdays {
    padding-bottom: var(--s-1);
  }

  .month__weekdays span {
    text-align: center;
    font-size: var(--t-micro);
    font-weight: 600;
    color: var(--ui-ink-muted);
  }

  .month__day {
    aspect-ratio: 1 / 0.85;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    border-radius: var(--r-sm);
  }

  .month__day:active {
    background: var(--ui-press);
  }

  /* Days of the neighbouring months. A faint ink rather than opacity:
     35% opacity put these dates at about 1.6:1, and they are still
     dates a user can tap. */
  .month__day--muted {
    color: var(--ui-ink-faint);
  }

  .month__num {
    font-family: var(--font-display);
    font-variant-numeric: tabular-nums;
    font-size: var(--t-callout);
    min-width: calc(26px * var(--dt-chrome, 1));
    height: calc(28px * var(--dt-chrome, 1));
    line-height: calc(28px * var(--dt-chrome, 1));
    border-radius: var(--r-full);
    text-align: center;
  }

  /* Today is *now*, so it takes the signal colour (Morgenrot in the
     house theme) — the same colour as an unread mark. Bold *and*
     a ring: a shape, so it is findable by anyone who does not separate
     the colour from the ink (WCAG 1.4.1). The signal ink is solved
     against every surface, so the ring clears 3:1. */
  .month__day--today .month__num {
    color: var(--ui-unread);
    font-weight: 750;
    box-shadow: inset 0 0 0 1.5px var(--ui-unread);
  }

  /* The day you picked is a choice, so it takes the action colour. */
  .month__day--selected .month__num {
    background: var(--ui-accent-fill);
    color: var(--ui-on-accent);
    font-weight: 650;
  }

  /* Picked *and* today: the filled disc in the signal colour. */
  .month__day--selected.month__day--today .month__num {
    background: var(--ui-now-fill);
    color: var(--ui-on-now);
    box-shadow: none;
  }

  .month__dots {
    display: flex;
    gap: 2px;
    height: 5px;
  }

  .month__dot {
    width: 5px;
    height: 5px;
    border-radius: var(--r-full);
  }

  .agenda__bar {
    width: 4px;
    align-self: stretch;
    border-radius: var(--r-full);
    flex-shrink: 0;
  }

  /* The place, after a glyph from the app's own set: the emoji pin drew
     in the platform's colour-emoji style, the one full-colour picture in
     an otherwise monochrome line. */
  .agenda__place {
    display: flex;
    align-items: center;
    gap: var(--s-1);
    min-width: 0;
  }

  /* A quiet sentence on the page. v2 drew a dashed box around it, the
     shape of a drop zone — as if the empty day were waiting for a file. */
  .agenda__empty {
    margin: var(--s-2) var(--gutter);
    padding: var(--s-6) 0;
    text-align: center;
    font-size: var(--t-callout);
    color: var(--ui-ink-muted);
  }
</style>
