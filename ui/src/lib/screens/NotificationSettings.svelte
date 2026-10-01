<script lang="ts">
  /**
   * Notification preferences.
   *
   * The OS permission and the app's own switches are two different
   * things, and conflating them is how people end up with silent
   * mail. The first row shows (and can request) the system
   * permission; the rest are the app's own filters.
   */
  import * as api from '../api'
  import NavBar from '../ui/NavBar.svelte'
  import Icon from '../Icon.svelte'
  import Toggle from '../Toggle.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import { nav } from '../nav.svelte'
  import { settingsStore } from '../settingsStore.svelte'
  import { toasts } from '../toast.svelte'
  import { m } from '../../paraglide/messages'

  const settings = $derived(settingsStore.value)
  let permission = $state<'granted' | 'denied' | 'unknown'>('unknown')

  async function checkPermission() {
    try {
      permission = (await api.platform.notificationsPermissionGranted()) ? 'granted' : 'denied'
    } catch {
      permission = 'unknown'
    }
  }

  async function request() {
    try {
      const result = await api.platform.requestNotificationsPermission()
      permission = result === 'granted' ? 'granted' : 'denied'
      if (permission !== 'granted') toasts.show(m.mobile_notifications_denied())
    } catch (e) {
      toasts.error(m.mobile_notifications_request_failed(), e)
    }
  }

  $effect(() => {
    void checkPermission()
  })

  async function patch(change: Record<string, unknown>) {
    try {
      await settingsStore.patch(change)
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_settings_notifications()}
    backLabel={m.mobile_settings()}
    onback={() => nav.pop()}
  />
  <div class="screen__body">
    {#if !settings}
      <Spinner />
    {:else}
      <div class="list-group">
        <div class="list-row">
          <Icon name={permission === 'granted' ? 'success' : 'warning'} size={20} />
          <span class="flex-1">
            <span class="block list-row__title">{m.mobile_system_permission()}</span>
            <span class="block list-row__detail">
              {permission === 'granted'
                ? m.mobile_permission_granted()
                : m.mobile_permission_missing()}
            </span>
          </span>
          {#if permission !== 'granted'}
            <button type="button" class="value-button" onclick={request}>
              {m.mobile_allow()}
            </button>
          {/if}
        </div>
      </div>

      <div class="list-group">
        <div class="list-row">
          <span class="flex-1">
            <span class="block list-row__title">{m.mobile_notify_new_mail()}</span>
            <span class="block list-row__detail">{m.mobile_notify_new_mail_hint()}</span>
          </span>
          <Toggle
            checked={settings.notifications_enabled}
            label={m.mobile_notify_new_mail()}
            onchange={(v) => patch({ notifications_enabled: v })}
          />
        </div>
        <div class="list-row">
          <span class="flex-1">
            <span class="block list-row__title">{m.mobile_notify_events()}</span>
            <span class="block list-row__detail">{m.mobile_notify_events_hint()}</span>
          </span>
          <Toggle
            checked={settings.calendar_reminders_enabled}
            label={m.mobile_notify_events()}
            onchange={(v) => patch({ calendar_reminders_enabled: v })}
          />
        </div>
        <div class="list-row">
          <span class="flex-1">
            <span class="block list-row__title">{m.mobile_notify_meetings()}</span>
            <span class="block list-row__detail">{m.mobile_notify_meetings_hint()}</span>
          </span>
          <Toggle
            checked={settings.meeting_reminders_enabled}
            label={m.mobile_notify_meetings()}
            onchange={(v) => patch({ meeting_reminders_enabled: v })}
          />
        </div>
      </div>
      <p class="list-footer pb-8">{m.mobile_notifications_foreground_note()}</p>
    {/if}
  </div>
</div>
