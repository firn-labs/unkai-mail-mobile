/**
 * settingsStore — the app's `AppSettings` row, loaded once and
 * shared reactively.
 *
 * The desktop build threads `appPrefs` down from `App.svelte` as a
 * prop; on mobile the settings screens are several stack levels
 * away from the shell, so prop-drilling would mean every screen in
 * between re-declaring a prop it doesn't use. A store is the
 * cheaper shape here.
 *
 * Writes go through `patch()`, which updates the local copy
 * optimistically and persists in the background: settings toggles
 * must feel instant under a finger, and the backend write is a
 * local file plus a debounced Nextcloud push.
 */

import * as api from './api'
import { formatError } from './errors'
import type { AppSettings } from './api'

class SettingsStore {
  /** `null` until the first load settles. Screens render a spinner
   *  off this rather than guessing defaults that would flash. */
  value = $state<AppSettings | null>(null)
  error = $state('')

  async load(): Promise<AppSettings | null> {
    try {
      this.value = await api.settings.getAppSettings()
      this.error = ''
    } catch (e) {
      this.error = formatError(e)
    }
    return this.value
  }

  /**
   * Merge a partial change in and persist it.
   *
   * Optimistic: the local copy updates before the IPC resolves, so
   * a toggle animates immediately. On failure the error is exposed
   * and the caller can reload — we deliberately don't roll back
   * silently, which would make a toggle flip back with no
   * explanation.
   */
  async patch(change: Partial<AppSettings>): Promise<void> {
    if (!this.value) return
    const next = { ...this.value, ...change }
    this.value = next
    try {
      await api.settings.updateAppSettings({ newSettings: next })
      this.error = ''
    } catch (e) {
      this.error = formatError(e)
      throw e
    }
  }
}

export const settingsStore = new SettingsStore()
