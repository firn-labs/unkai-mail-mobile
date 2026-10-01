<script lang="ts">
  /** Version, project links, and the honest list of what this
   *  build can't do yet. */
  import * as api from '../api'
  import Icon from '../Icon.svelte'
  import NavBar from '../ui/NavBar.svelte'
  import { nav } from '../nav.svelte'
  import { m } from '../../paraglide/messages'

  let version = $state('')

  $effect(() => {
    void api.system
      .getAppVersion()
      .then((v) => (version = v))
      .catch(() => (version = '—'))
  })

  function open(url: string) {
    void api.system.openUrl({ url }).catch(() => {})
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_settings_about()}
    backLabel={m.mobile_settings()}
    onback={() => nav.pop()}
  />
  <div class="screen__body">
    <div class="flex flex-col items-center gap-2 px-6 pb-4 pt-8 text-center">
      <img src="unkai-logo://storm" alt="" class="h-16 w-16" />
      <h1 class="text-lg font-semibold">Unkai Mail</h1>
      <p class="text-sm ink-muted">{m.mobile_about_version({ version })}</p>
    </div>

    <div class="list-group">
      <button class="list-row" onclick={() => nav.push('settings-privacy')}>
        <Icon name="eye-off" size={20} />
        <span class="flex-1 list-row__title">{m.mobile_settings_privacy()}</span>
        <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
      </button>
      <button class="list-row" onclick={() => open('https://github.com/firn-labs/unkai-mail-mobile')}>
        <Icon name="open-in-browser" size={20} />
        <span class="flex-1 list-row__title">{m.mobile_about_project()}</span>
        <span class="list-row__chevron"><Icon name="nav-forward" size={16} /></span>
      </button>
    </div>

    <div class="list-header">{m.mobile_about_limitations()}</div>
    <div class="list-group">
      <p class="px-4 py-3 text-sm leading-relaxed">{m.mobile_about_limitations_body()}</p>
    </div>

    <p class="list-footer pb-10">© 2026 Firn Labs · GPL-3.0</p>
  </div>
</div>
