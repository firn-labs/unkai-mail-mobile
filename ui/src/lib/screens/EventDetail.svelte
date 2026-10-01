<script lang="ts">
  /**
   * One event, read-only, with the actions that belong to it:
   * edit, delete, join a meeting link, and RSVP when the user is an
   * attendee rather than the organiser.
   */
  import * as api from '../api'
  import type { CalendarEvent } from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import Confirm from '../ui/Confirm.svelte'
  import { nav } from '../nav.svelte'
  import { toasts } from '../toast.svelte'
  import { fullDate, timeOnly, bareEmail } from '../format'
  import { m } from '../../paraglide/messages'

  interface Props {
    event: CalendarEvent
    onedit: (event: CalendarEvent) => void
  }

  let { event, onedit }: Props = $props()

  let confirmDelete = $state(false)
  let rsvpBusy = $state(false)

  const attendees = $derived(event.attendees ?? [])
  const sameDay = $derived(
    new Date(event.start).toDateString() === new Date(event.end).toDateString(),
  )

  async function remove() {
    confirmDelete = false
    try {
      await api.calendar.deleteCalendarEvent({ eventId: event.id })
      toasts.show(m.mobile_event_deleted())
      nav.pop()
    } catch (e) {
      toasts.error(m.mobile_event_delete_failed(), e)
    }
  }

  async function rsvp(partstat: 'ACCEPTED' | 'TENTATIVE' | 'DECLINED') {
    rsvpBusy = true
    try {
      await api.calendar.rsvpExistingEvent({ eventId: event.id, partstat })
      toasts.success(m.mobile_rsvp_sent())
    } catch (e) {
      toasts.error(m.mobile_rsvp_failed(), e)
    } finally {
      rsvpBusy = false
    }
  }

  function openLink(url: string) {
    void api.system.openUrl({ url }).catch((e) => toasts.error(m.mobile_open_failed(), e))
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_event()}
    backLabel={m.mobile_back()}
    onback={() => nav.pop()}
  >
    {#snippet actions()}
      <button
        type="button"
        class="bar-button bar-button--text"
        onclick={() => onedit(event)}
      >
        {m.mobile_edit()}
      </button>
    {/snippet}
  </NavBar>

  <div class="screen__body">
    <div class="page-inset pt-4">
      <h1 class="text-xl font-semibold selectable">{event.summary}</h1>
      <p class="mt-1 text-sm ink-muted">
        {#if sameDay}
          {fullDate(event.start)} – {timeOnly(event.end)}
        {:else}
          {fullDate(event.start)} → {fullDate(event.end)}
        {/if}
      </p>
    </div>

    <div class="list-group list-group--inset">
      {#if event.location}
        <div class="list-row">
          <Icon name="location" size={18} />
          <span class="flex-1 selectable">{event.location}</span>
        </div>
      {/if}
      {#if event.url}
        <button class="list-row" onclick={() => openLink(event.url)}>
          <Icon name="meetings" size={18} />
          <span class="flex-1 truncate ink-accent">{m.mobile_join_meeting()}</span>
          <Icon name="open-in-browser" size={16} />
        </button>
      {/if}
      {#if event.rrule}
        <div class="list-row">
          <Icon name="refresh" size={18} />
          <span class="flex-1 list-row__detail">{m.mobile_recurring_event()}</span>
        </div>
      {/if}
      {#if (event.reminders ?? []).length > 0}
        <div class="list-row">
          <Icon name="notification" size={18} />
          <span class="flex-1 list-row__detail">
            {m.mobile_reminder_before({ minutes: event.reminders[0].trigger_minutes_before })}
          </span>
        </div>
      {/if}
    </div>

    {#if event.description}
      <div class="list-header">{m.mobile_notes()}</div>
      <div class="list-group">
        <p class="whitespace-pre-wrap px-4 py-3 text-sm selectable">{event.description}</p>
      </div>
    {/if}

    {#if attendees.length > 0}
      <div class="list-header">{m.mobile_attendees({ count: attendees.length })}</div>
      <div class="list-group">
        {#each attendees as attendee (attendee.email)}
          <div class="list-row">
            <Icon
              name={attendee.status === 'ACCEPTED'
                ? 'rsvp-accept'
                : attendee.status === 'DECLINED'
                  ? 'rsvp-decline'
                  : attendee.status === 'TENTATIVE'
                    ? 'rsvp-tentative'
                    : 'contacts'}
              size={18}
            />
            <span class="min-w-0 flex-1">
              <span class="block truncate list-row__title">
                {attendee.common_name || bareEmail(attendee.email)}
              </span>
              <span class="block truncate list-row__detail">{bareEmail(attendee.email)}</span>
            </span>
          </div>
        {/each}
      </div>

      <div class="list-header">{m.mobile_your_response()}</div>
      <div class="flex gap-2 px-3">
        <button
          type="button"
          class="wide-button"
          disabled={rsvpBusy}
          onclick={() => rsvp('ACCEPTED')}>{m.mobile_rsvp_accept()}</button
        >
        <button
          type="button"
          class="wide-button wide-button--quiet"
          disabled={rsvpBusy}
          onclick={() => rsvp('TENTATIVE')}>{m.mobile_rsvp_maybe()}</button
        >
        <button
          type="button"
          class="wide-button wide-button--quiet"
          disabled={rsvpBusy}
          onclick={() => rsvp('DECLINED')}>{m.mobile_rsvp_decline()}</button
        >
      </div>
    {/if}

    <div class="px-3 pb-10 pt-6">
      <button
        type="button"
        class="wide-button wide-button--destructive"
        onclick={() => (confirmDelete = true)}
      >
        {m.mobile_delete_event()}
      </button>
    </div>
  </div>
</div>

{#if confirmDelete}
  <Confirm
    title={m.mobile_delete_event()}
    body={m.mobile_delete_event_body({ summary: event.summary })}
    confirmLabel={m.mobile_delete()}
    onconfirm={remove}
    oncancel={() => (confirmDelete = false)}
  />
{/if}
