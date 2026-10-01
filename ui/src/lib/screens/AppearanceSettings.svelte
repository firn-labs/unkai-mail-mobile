<script lang="ts">
  /** Theme + light/dark mode. The Skeleton theme set carries over
   *  from the desktop unchanged, so a user's chosen palette follows
   *  them between devices through the settings bundle. */
  import NavBar from '../ui/NavBar.svelte'
  import Icon from '../Icon.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import { nav } from '../nav.svelte'
  import { settingsStore } from '../settingsStore.svelte'
  import { toasts } from '../toast.svelte'
  import { STOCK_THEMES, applyTheme, type ThemeMode } from '../theme'
  import { CUSTOM_THEME_ID, normaliseHex } from '../customTheme'
  import { m } from '../../paraglide/messages'

  const settings = $derived(settingsStore.value)

  /** The user's accent, for the swatch on the editor row. */
  const customAccent = $derived(normaliseHex(settings?.custom_theme_accent ?? '#3b82f6'))

  const MODES: { value: ThemeMode; label: string; icon: 'sun' | 'moon' | 'design-palette' }[] =
    $derived([
      { value: 'system', label: m.mobile_theme_system(), icon: 'design-palette' },
      { value: 'light', label: m.mobile_theme_light(), icon: 'sun' },
      { value: 'dark', label: m.mobile_theme_dark(), icon: 'moon' },
    ])

  async function setMode(mode: ThemeMode) {
    if (!settings) return
    applyTheme(settings.theme_name, mode)
    try {
      await settingsStore.patch({ theme_mode: mode })
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  /**
   * How a message is shown while the app is dark: on white paper as the
   * sender designed it (the default), or in the dark reading mode. The
   * stored field is the older "white background" switch, kept so the
   * choice still travels with the settings bundle.
   */
  async function setDarkReading(dark: boolean) {
    try {
      await settingsStore.patch({ mail_html_white_background: !dark })
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  async function setTheme(name: string) {
    if (!settings) return
    applyTheme(name, settings.theme_mode)
    try {
      await settingsStore.patch({ theme_name: name })
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_settings_appearance()}
    backLabel={m.mobile_settings()}
    onback={() => nav.pop()}
    large
  />
  <div class="screen__body">
    {#if !settings}
      <Spinner />
    {:else}
      <div class="list-header">{m.mobile_appearance_mode()}</div>
      <div class="list-group">
        {#each MODES as mode (mode.value)}
          <button
            type="button"
            class="list-row"
            aria-current={settings.theme_mode === mode.value ? 'true' : undefined}
            onclick={() => setMode(mode.value)}
          >
            <Icon name={mode.icon} size={20} />
            <span class="flex-1 list-row__title">{mode.label}</span>
            {#if settings.theme_mode === mode.value}
              <span class="ink-accent"><Icon name="check" size={18} /></span>
            {/if}
          </button>
        {/each}
      </div>

      <!-- Next to light/dark because it only means something in dark
           mode — the footer says so, rather than greying it out. -->
      <div class="list-header">{m.mobile_reading_mode_header()}</div>
      <div class="list-group">
        {#each [false, true] as dark (dark)}
          <button
            type="button"
            class="list-row"
            aria-current={(settings.mail_html_white_background === false) === dark ? 'true' : undefined}
            onclick={() => setDarkReading(dark)}
          >
            <Icon name={dark ? 'moon' : 'sun'} size={20} />
            <span class="min-w-0 flex-1">
              <span class="block list-row__title">
                {dark ? m.mobile_reading_dark() : m.mobile_reading_original()}
              </span>
              <span class="block list-row__detail">
                {dark ? m.mobile_reading_dark_hint() : m.mobile_reading_original_hint()}
              </span>
            </span>
            {#if (settings.mail_html_white_background === false) === dark}
              <span class="ink-accent"><Icon name="check" size={18} /></span>
            {/if}
          </button>
        {/each}
      </div>
      <p class="list-footer">{m.mobile_reading_mode_footer()}</p>

      <!-- The user-built theme gets its own group above the stock
           list: it is the only entry that leads somewhere rather than
           just being selected, so mixing it into the picker would make
           one row behave unlike its twenty-two neighbours. -->
      <div class="list-header">{m.mobile_theme_custom_name()}</div>
      <div class="list-group">
        <button class="list-row" onclick={() => nav.push('settings-theme-editor')}>
          <span class="theme-dot" style="background: {customAccent}"></span>
          <span class="min-w-0 flex-1">
            <span class="block list-row__title">{m.mobile_theme_editor_open()}</span>
            <span class="block list-row__detail">{m.mobile_theme_custom_desc()}</span>
          </span>
          {#if settings.theme_name === CUSTOM_THEME_ID}
            <span class="ink-accent"><Icon name="check" size={18} /></span>
          {/if}
          <Icon name="nav-forward" size={16} />
        </button>
      </div>

      <div class="list-header">{m.mobile_appearance_theme()}</div>
      <div class="list-group">
        {#each STOCK_THEMES as theme (theme.id)}
          <button class="list-row" onclick={() => setTheme(theme.id)}>
            <span class="min-w-0 flex-1">
              <span class="block list-row__title">{theme.label}</span>
              <span class="block list-row__detail">{theme.description}</span>
            </span>
            {#if settings.theme_name === theme.id}
              <span class="ink-accent"><Icon name="check" size={18} /></span>
            {/if}
          </button>
        {/each}
      </div>
      <div class="h-16"></div>
    {/if}
  </div>
</div>

<style>
  /* A dot in the user's own accent, so the row shows what it opens. */
  .theme-dot {
    width: 22px;
    height: 22px;
    border-radius: var(--r-full);
    flex-shrink: 0;
    border: 1px solid color-mix(in oklab, var(--color-surface-950) 15%, transparent);
  }
</style>
