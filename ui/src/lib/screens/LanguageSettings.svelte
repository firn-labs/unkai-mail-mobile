<script lang="ts">
  /**
   * Settings → Language.
   *
   * "Same as iPhone" first, and selected unless the user picked
   * something: the app follows the device by default (`lib/locale.ts`).
   * Each language is listed in its own name, with its name in the
   * current language beneath — someone who switched to a language they
   * cannot read must still find their own.
   *
   * Picking reloads the interface (`chooseLocale`), so the rows say
   * nothing about it: the new language on screen is the confirmation.
   */
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import { nav } from '../nav.svelte'
  import { locales } from '../../paraglide/runtime'
  import { LANGUAGE_NAMES, chooseLocale, deviceLocale, pinnedLocale } from '../locale'
  import { m } from '../../paraglide/messages'

  const pinned = pinnedLocale()
  const device = deviceLocale() ?? 'en'

  /** A language's name in the language the app is in now. */
  function localName(code: string): string {
    try {
      return new Intl.DisplayNames([document.documentElement.lang || 'en'], { type: 'language' }).of(
        code === 'zh' ? 'zh-Hans' : code,
      ) ?? code
    } catch {
      return code
    }
  }
</script>

<div class="screen">
  <NavBar title={m.mobile_settings_language()} backLabel={m.mobile_settings()} onback={() => nav.pop()} />
  <div class="screen__body">
    <div class="list-group">
      <button
        type="button"
        class="list-row"
        aria-current={pinned === null ? 'true' : undefined}
        onclick={() => pinned !== null && chooseLocale(null)}
      >
        <Icon name="translate" size={20} />
        <span class="min-w-0 flex-1">
          <span class="block list-row__title">{m.mobile_language_system()}</span>
          <span class="block list-row__detail">{LANGUAGE_NAMES[device]}</span>
        </span>
        {#if pinned === null}
          <span class="ink-accent"><Icon name="check" size={18} /></span>
        {/if}
      </button>
    </div>

    <div class="list-group">
      {#each locales as code (code)}
        <!-- `lang` on the name, so VoiceOver reads "Français" in French. -->
        <button
          type="button"
          class="list-row"
          aria-current={pinned === code ? 'true' : undefined}
          onclick={() => pinned !== code && chooseLocale(code)}
        >
          <span class="min-w-0 flex-1">
            <span class="block list-row__title" lang={code === 'zh' ? 'zh-Hans' : code}>
              {LANGUAGE_NAMES[code] ?? code}
            </span>
            <span class="block list-row__detail">{localName(code)}</span>
          </span>
          {#if pinned === code}
            <span class="ink-accent"><Icon name="check" size={18} /></span>
          {/if}
        </button>
      {/each}
    </div>
    <p class="list-footer pb-8">{m.mobile_language_footer()}</p>
  </div>
</div>
