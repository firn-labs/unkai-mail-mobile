<script lang="ts">
  /**
   * Which features the app shows, and which of them get a tab.
   *
   * Two separate decisions, deliberately not merged into one control:
   *
   *   * **Show / hide** — a feature the user has no use for
   *     (Nextcloud Talk without a Nextcloud, say) disappears from the
   *     tab bar *and* from More.
   *   * **On the tab bar** — of what's left, at most three sit
   *     between Mail and More. Everything else stays in More, so
   *     taking a feature off the bar never hides it.
   *
   * A single three-state control would have been shorter and worse:
   * "hidden / in More / on a tab" reads as a ranking, and users would
   * have to work out that the middle state is the safe one.
   */
  import NavBar from '../ui/NavBar.svelte'
  import Icon from '../Icon.svelte'
  import Toggle from '../Toggle.svelte'
  import Spinner from '../ui/Spinner.svelte'
  import { nav } from '../nav.svelte'
  import { settingsStore } from '../settingsStore.svelte'
  import { features, FEATURES, MAX_TABS, type FeatureId } from '../features.svelte'
  import { toasts } from '../toast.svelte'
  import { m } from '../../paraglide/messages'

  const settings = $derived(settingsStore.value)

  async function toggleVisible(id: FeatureId, visible: boolean) {
    try {
      await features.setHidden(id, !visible)
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  async function toggleTab(id: FeatureId, onBar: boolean) {
    try {
      const ok = await features.setOnTabBar(id, onBar)
      // Refusing beats silently evicting whichever tab the user set
      // first — the bar is theirs, and the app shouldn't rearrange it
      // behind their back.
      if (!ok) toasts.show(m.mobile_features_tabs_full({ count: MAX_TABS }))
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  async function move(id: FeatureId, delta: -1 | 1) {
    try {
      await features.moveTab(id, delta)
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  async function reset() {
    try {
      await features.reset()
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_settings_features()}
    backLabel={m.mobile_settings()}
    onback={() => nav.pop()}
  />
  <div class="screen__body">
    {#if !settings}
      <Spinner />
    {:else}
      <div class="list-header">{m.mobile_features_tab_bar()}</div>
      <div class="list-group">
        <!-- Mail and More are shown but not togglable: the bar's two
             fixed ends are what keep every other arrangement safe. -->
        <div class="list-row">
          <span class="icon-tile" aria-hidden="true"><Icon name="email-envelope" size={18} /></span>
          <span class="flex-1 list-row__title">{m.mobile_tab_mail()}</span>
          <span class="list-row__detail">{m.mobile_features_fixed()}</span>
        </div>
        {#each features.tabs as id, index (id)}
          {@const feature = FEATURES.find((f) => f.id === id)}
          {#if feature}
            <div class="list-row">
              <span class="icon-tile" aria-hidden="true">
                <Icon name={feature.icon} size={18} />
              </span>
              <span class="flex-1 list-row__title">{feature.label()}</span>
              <button
                type="button"
                class="bar-button bar-button--text"
                disabled={index === 0}
                aria-label={m.mobile_features_move_up()}
                onclick={() => move(id, -1)}
              >
                <Icon name="nav-backward" size={16} />
              </button>
              <button
                type="button"
                class="bar-button bar-button--text"
                disabled={index === features.tabs.length - 1}
                aria-label={m.mobile_features_move_down()}
                onclick={() => move(id, 1)}
              >
                <Icon name="nav-forward" size={16} />
              </button>
            </div>
          {/if}
        {/each}
        <div class="list-row">
          <span class="icon-tile" aria-hidden="true"><Icon name="more" size={18} /></span>
          <span class="flex-1 list-row__title">{m.mobile_tab_more()}</span>
          <span class="list-row__detail">{m.mobile_features_fixed()}</span>
        </div>
      </div>
      <p class="list-footer">
        {m.mobile_features_tab_bar_hint({ count: MAX_TABS })}
      </p>

      <div class="list-header">{m.mobile_features_all()}</div>
      <div class="list-group">
        {#each FEATURES as feature (feature.id)}
          {@const visible = !features.isHidden(feature.id)}
          <div class="list-row">
            <span class="icon-tile" aria-hidden="true">
              <Icon name={feature.icon} size={18} />
            </span>
            <span class="flex-1">
              <span class="block list-row__title">{feature.label()}</span>
              <span class="block list-row__detail">
                {#if !visible}
                  {m.mobile_features_state_hidden()}
                {:else if features.isOnTabBar(feature.id)}
                  {m.mobile_features_state_tab()}
                {:else}
                  {m.mobile_features_state_more()}
                {/if}
              </span>
            </span>
            {#if visible}
              <button
                type="button"
                class="value-button"
                onclick={() => toggleTab(feature.id, !features.isOnTabBar(feature.id))}
              >
                {features.isOnTabBar(feature.id)
                  ? m.mobile_features_remove_tab()
                  : m.mobile_features_add_tab()}
              </button>
            {/if}
            <Toggle
              checked={visible}
              label={feature.label()}
              onchange={(v) => toggleVisible(feature.id, v)}
            />
          </div>
        {/each}
      </div>
      <p class="list-footer">{m.mobile_features_all_hint()}</p>

      <div class="page-inset pb-8 pt-2">
        <button type="button" class="wide-button wide-button--quiet" onclick={reset}>
          {m.mobile_features_reset()}
        </button>
      </div>
    {/if}
  </div>
</div>
