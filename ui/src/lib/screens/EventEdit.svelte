<script lang="ts">
  /**
   * Create or edit an event, presented as a full sheet.
   *
   * Dates and times use the platform's own `<input type="date">` /
   * `type="time"` controls: iOS renders its wheel picker for them,
   * which is both familiar and free. A hand-rolled picker would be
   * worse in every dimension that matters here.
   *
   * Local ↔ UTC conversion is the one thing to get right: the
   * backend takes RFC 3339 UTC, the inputs speak local wall-clock
   * time, and `new Date(y, m, d, h, min)` is the conversion that
   * respects the device's zone (string parsing does not).
   */
  import * as api from '../api'
  import type { CalendarEvent, CalendarSummary, NextcloudAccount } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Sheet from '../ui/Sheet.svelte'
  import Toggle from '../Toggle.svelte'
  import { toasts } from '../toast.svelte'
  import { settingsStore } from '../settingsStore.svelte'
  import { toDateInput, toTimeInput, parseLocalDate } from '../format'
  import { m } from '../../paraglide/messages'

  interface Props {
    /** `null` creates a new event. */
    event?: CalendarEvent | null
    /** `YYYY-MM-DD` the new event should start on. */
    day?: string
    onclose: (saved: boolean) => void
  }

  let { event = null, day, onclose }: Props = $props()

  /* Read once, deliberately: the event editor is mounted fresh
     for each open and unmounted on close, so the form owns its
     fields from here on. */
  // svelte-ignore state_referenced_locally
  const seed = event

  const editing = !!seed

  let calendars = $state<CalendarSummary[]>([])
  let calendarId = $state('')
  let summary = $state(seed?.summary ?? '')
  let description = $state(seed?.description ?? '')
  let location = $state(seed?.location ?? '')
  let url = $state(seed?.url ?? '')
  let allDay = $state(false)
  let saving = $state(false)

  // svelte-ignore state_referenced_locally
  const startDate = new Date(seed?.start ?? (day ? `${day}T09:00` : Date.now()))
  const endDate = new Date(seed?.end ?? startDate.getTime() + 3_600_000)

  let startDay = $state(toDateInput(startDate))
  let startTime = $state(toTimeInput(startDate))
  let endDay = $state(toDateInput(endDate))
  let endTime = $state(toTimeInput(endDate))
  let reminderMinutes = $state<number | null>(
    seed?.reminders?.[0]?.trigger_minutes_before ?? 15,
  )
  let attendees = $state((seed?.attendees ?? []).map((a: { email: string }) => a.email).join(', '))

  const REMINDER_CHOICES = [null, 0, 5, 15, 30, 60, 1440]

  async function loadCalendars() {
    try {
      const accounts: NextcloudAccount[] = await api.nextcloud.getNextcloudAccounts()
      const all: CalendarSummary[] = []
      for (const acc of accounts) {
        all.push(...(await api.calendar.getCachedCalendars({ ncId: acc.id })))
      }
      calendars = all.filter((c) => !c.read_only)
      // Prefer the event's own calendar, then the configured default,
      // then whatever is first.
      const preferred =
        (editing && calendars.find((c) => seed!.id.startsWith(c.id))?.id) ||
        settingsStore.value?.default_calendar_id ||
        calendars[0]?.id ||
        ''
      calendarId = preferred
    } catch (e) {
      toasts.error(m.mobile_calendars_load_failed(), e)
    }
  }

  $effect(() => {
    void loadCalendars()
  })

  /** Combine a `YYYY-MM-DD` + `HH:MM` pair into a local Date. */
  function combine(dayValue: string, timeValue: string): Date | null {
    const base = parseLocalDate(dayValue)
    if (!base) return null
    if (allDay) return base
    const [h, min] = timeValue.split(':').map(Number)
    base.setHours(h || 0, min || 0, 0, 0)
    return base
  }

  async function save() {
    const start = combine(startDay, startTime)
    const end = combine(endDay, endTime)
    if (!summary.trim()) {
      toasts.error(m.mobile_event_needs_title())
      return
    }
    if (!start || !end) {
      toasts.error(m.mobile_event_needs_dates())
      return
    }
    if (end <= start && !allDay) {
      toasts.error(m.mobile_event_end_before_start())
      return
    }
    if (!calendarId) {
      toasts.error(m.mobile_event_needs_calendar())
      return
    }

    saving = true
    const input = {
      summary: summary.trim(),
      description: description.trim() || null,
      location: location.trim() || null,
      url: url.trim() || null,
      start: start.toISOString(),
      end: (allDay ? new Date(end.getTime() + 86_399_000) : end).toISOString(),
      all_day: allDay,
      attendees: attendees
        .split(',')
        .map((a: string) => a.trim())
        .filter(Boolean)
        .map((email: string) => ({ email, common_name: null, status: null, role: null })),
      reminders:
        reminderMinutes === null
          ? []
          : [{ trigger_minutes_before: reminderMinutes, action: 'DISPLAY' }],
    }

    try {
      if (editing) {
        await api.calendar.updateCalendarEvent({ eventId: seed!.id, input })
        toasts.success(m.mobile_event_updated())
      } else {
        await api.calendar.createCalendarEvent({ calendarId, input })
        toasts.success(m.mobile_event_created())
      }
      onclose(true)
    } catch (e) {
      toasts.error(m.mobile_event_save_failed(), e)
    } finally {
      saving = false
    }
  }

  function reminderLabel(value: number | null): string {
    if (value === null) return m.mobile_reminder_none()
    if (value === 0) return m.mobile_reminder_at_start()
    if (value < 60) return m.mobile_reminder_minutes({ count: value })
    if (value < 1440) return m.mobile_reminder_hours({ count: value / 60 })
    return m.mobile_reminder_days({ count: value / 1440 })
  }
</script>

<Sheet kind="full" label={editing ? m.mobile_edit_event() : m.mobile_new_event()} dismissible={false}>
  <div class="screen">
    <NavBar title={editing ? m.mobile_edit_event() : m.mobile_new_event()}>
      {#snippet leading()}
        <button type="button" class="bar-button bar-button--text" onclick={() => onclose(false)}>
          {m.mobile_cancel()}
        </button>
      {/snippet}
      {#snippet actions()}
        <button
          type="button"
          class="bar-button bar-button--text bar-button--strong"
          disabled={saving}
          onclick={save}
        >
          {saving ? m.mobile_saving() : m.mobile_save()}
        </button>
      {/snippet}
    </NavBar>

    <div class="screen__body form-group">
      <div class="list-group">
        <div class="px-4 py-2">
          <input
            type="text"
            bind:value={summary}
            aria-label={m.mobile_event_title_placeholder()}
            placeholder={m.mobile_event_title_placeholder()}
            class="text-lg"
          />
        </div>
        <div class="px-4 py-2">
          <input
            type="text"
            bind:value={location}
            aria-label={m.mobile_event_location_placeholder()}
            placeholder={m.mobile_event_location_placeholder()}
          />
        </div>
      </div>

      <div class="list-group">
        <div class="list-row">
          <span class="flex-1">{m.mobile_all_day()}</span>
          <Toggle bind:checked={allDay} label={m.mobile_all_day()} />
        </div>
        <div class="list-row">
          <span class="w-16 shrink-0 text-sm">{m.mobile_starts()}</span>
          <input
            type="date"
            bind:value={startDay}
            class="flex-1"
            aria-label={m.mobile_sr_start_date()}
          />
          {#if !allDay}
            <input
              type="time"
              bind:value={startTime}
              class="w-28"
              aria-label={m.mobile_sr_start_time()}
            />
          {/if}
        </div>
        <div class="list-row">
          <span class="w-16 shrink-0 text-sm">{m.mobile_ends()}</span>
          <input type="date" bind:value={endDay} class="flex-1" aria-label={m.mobile_sr_end_date()} />
          {#if !allDay}
            <input
              type="time"
              bind:value={endTime}
              class="w-28"
              aria-label={m.mobile_sr_end_time()}
            />
          {/if}
        </div>
      </div>

      <div class="list-group">
        <div class="list-row">
          <Icon name="calendar" size={18} />
          <span class="flex-1">{m.mobile_calendar()}</span>
          <select bind:value={calendarId} class="max-w-[55%]" aria-label={m.mobile_calendar()}>
            {#each calendars as cal (cal.id)}
              <option value={cal.id}>{cal.display_name}</option>
            {/each}
          </select>
        </div>
        <div class="list-row">
          <Icon name="notification" size={18} />
          <span class="flex-1">{m.mobile_reminder()}</span>
          <select bind:value={reminderMinutes} class="max-w-[55%]" aria-label={m.mobile_reminder()}>
            {#each REMINDER_CHOICES as choice (String(choice))}
              <option value={choice}>{reminderLabel(choice)}</option>
            {/each}
          </select>
        </div>
      </div>

      <div class="list-header">{m.mobile_attendees_label()}</div>
      <div class="list-group">
        <div class="px-4 py-2">
          <input
            type="text"
            bind:value={attendees}
            inputmode="email"
            autocapitalize="none"
            autocorrect="off"
            aria-label={m.mobile_attendees_placeholder()}
            placeholder={m.mobile_attendees_placeholder()}
          />
        </div>
        <div class="px-4 py-2">
          <input type="url" bind:value={url} aria-label={m.mobile_event_url_placeholder()} placeholder={m.mobile_event_url_placeholder()} />
        </div>
      </div>

      <div class="list-header">{m.mobile_notes()}</div>
      <div class="list-group">
        <div class="px-4 py-2">
          <textarea rows="5" bind:value={description} aria-label={m.mobile_notes()}></textarea>
        </div>
      </div>

      <div class="h-10"></div>
    </div>
  </div>
</Sheet>
