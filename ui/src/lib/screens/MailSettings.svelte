<script lang="ts">
  /** Reading and syncing preferences. Every toggle writes through
   *  immediately — a Save button on a settings screen is a way to
   *  lose changes. */
  import NavBar from '../ui/NavBar.svelte'
  import Toggle from '../Toggle.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import { nav } from '../nav.svelte'
  import { settingsStore } from '../settingsStore.svelte'
  import { toasts } from '../toast.svelte'
  import { m } from '../../paraglide/messages'

  const settings = $derived(settingsStore.value)

  const INTERVALS = [60, 120, 300, 600, 1800]

  async function patch(change: Record<string, unknown>) {
    try {
      await settingsStore.patch(change)
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  function intervalLabel(seconds: number): string {
    return seconds < 3600
      ? m.mobile_interval_minutes({ count: Math.round(seconds / 60) })
      : m.mobile_interval_hours({ count: Math.round(seconds / 3600) })
  }
</script>

<div class="screen">
  <NavBar title={m.mobile_settings_mail()} backLabel={m.mobile_settings()} onback={() => nav.pop()} />
  <div class="screen__body form-group">
    {#if !settings}
      <Spinner />
    {:else}
      <div class="list-header">{m.mobile_settings_reading()}</div>
      <div class="list-group">
        <div class="list-row">
          <span class="flex-1">
            <span class="block list-row__title">{m.mobile_conversation_view()}</span>
            <span class="block list-row__detail">{m.mobile_conversation_view_hint()}</span>
          </span>
          <Toggle
            checked={settings.conversation_view_enabled}
            label={m.mobile_conversation_view()}
            onchange={(v) => patch({ conversation_view_enabled: v })}
          />
        </div>
        <!-- How a message looks in dark mode lives in Appearance, next
             to light/dark — the one place someone looks for it. -->
        <div class="list-row">
          <span class="flex-1">
            <span class="block list-row__title">{m.mobile_remote_images()}</span>
            <span class="block list-row__detail">{m.mobile_remote_images_hint()}</span>
          </span>
          <Toggle
            checked={settings.auto_load_remote_images}
            label={m.mobile_remote_images()}
            onchange={(v) => patch({ auto_load_remote_images: v })}
          />
        </div>
        <div class="list-row">
          <span class="flex-1">
            <span class="block list-row__title">{m.mobile_link_check()}</span>
            <span class="block list-row__detail">{m.mobile_link_check_hint()}</span>
          </span>
          <Toggle
            checked={settings.link_check_enabled}
            label={m.mobile_link_check()}
            onchange={(v) => patch({ link_check_enabled: v })}
          />
        </div>
      </div>

      <div class="list-header">{m.mobile_settings_sync()}</div>
      <div class="list-group">
        <div class="list-row">
          <span class="flex-1">
            <span class="block list-row__title">{m.mobile_background_sync()}</span>
            <span class="block list-row__detail">{m.mobile_background_sync_hint()}</span>
          </span>
          <Toggle
            checked={settings.background_sync_enabled}
            label={m.mobile_background_sync()}
            onchange={(v) => patch({ background_sync_enabled: v })}
          />
        </div>
        {#if settings.background_sync_enabled}
          <div class="list-row">
            <span class="flex-1 list-row__title">{m.mobile_sync_interval()}</span>
            <select
              aria-label={m.mobile_sync_interval()}
              value={settings.background_sync_interval_secs}
              onchange={(e) =>
                patch({ background_sync_interval_secs: Number(e.currentTarget.value) })}
              class="max-w-[50%]"
            >
              {#each INTERVALS as seconds (seconds)}
                <option value={seconds}>{intervalLabel(seconds)}</option>
              {/each}
            </select>
          </div>
        {/if}
      </div>
      <p class="list-footer pb-8">{m.mobile_background_sync_ios_note()}</p>
    {/if}
  </div>
</div>
