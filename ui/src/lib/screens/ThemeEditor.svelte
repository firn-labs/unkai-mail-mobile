<script lang="ts">
  /**
   * Build your own theme: a base colour, an accent, and a typeface.
   *
   * # Why sliders and not a colour picker
   *
   * `<input type="color">` opens the OS picker, which sounds like the
   * obvious answer and is the wrong one here. It is a modal that covers
   * the thing you are trying to judge, it offers a full RGB gamut when
   * only a slice of it makes a usable UI, and it hands back a colour
   * with no notion of *why* — so a user who picks a beautiful mid-grey
   * base gets a theme with no character, and one who picks vivid
   * magenta gets a screen nobody can read.
   *
   * Two sliders — hue and intensity — solve all three: the preview stays
   * visible and updates under the finger, the ranges are bounded to what
   * actually works (see `INTENSITY_MAX`), and each control maps onto a
   * question the user can answer — "warmer or cooler?", "how colourful?"
   * — rather than a coordinate they have to guess at. There is no
   * lightness slider on purpose; see `CANONICAL_L`.
   *
   * The hex value stays visible and editable underneath, so anyone who
   * *does* have a specific brand colour can just type it.
   */
  import NavBar from '../ui/NavBar.svelte'
  import Icon from '../Icon.svelte'
  import { nav } from '../nav.svelte'
  import { settingsStore } from '../settingsStore.svelte'
  import { toasts } from '../toast.svelte'
  import {
    CUSTOM_THEME_ID,
    FONT_STACKS,
    type FontStyleId,
    buildScale,
    fontStack,
    hexToOklch,
    normaliseHex,
    oklchToHex,
  } from '../customTheme'
  import { applyCustomTheme, applyTheme } from '../theme'
  import { m } from '../../paraglide/messages'

  /**
   * Ceiling on how colourful a pick can be.
   *
   * sRGB can express roughly 0.37 chroma; letting the slider reach it
   * would let someone build a theme they cannot read their mail in.
   * Capping the *base* much lower than the *accent* encodes the real
   * relationship between them: surfaces are 95% of the pixels and must
   * stay near-neutral, accents are a few percent and should sing.
   */
  const INTENSITY_MAX = { base: 0.09, accent: 0.3 }

  /**
   * The lightness a stored colour is canonicalised to.
   *
   * There is deliberately no brightness control. `buildScale` assigns
   * each shade its own fixed lightness — that is what keeps contrast
   * predictable whatever colour is picked — so an input colour's
   * lightness is read and then thrown away. A slider for it would move
   * and change nothing, which is worse than not offering it.
   *
   * What the user actually controls is *which* colour (hue) and *how
   * much* of it (chroma). The stored hex is those two at shade 500's
   * lightness, so the value in the field is the colour the theme is
   * really built from rather than an arbitrary point on its ramp.
   */
  const CANONICAL_L = 0.652

  interface Channel {
    hue: number
    intensity: number
  }

  /** Split a stored hex into slider positions, discarding lightness. */
  function toChannel(hex: string): Channel {
    const { c, h } = hexToOklch(normaliseHex(hex))
    return { hue: h, intensity: c }
  }

  function toHex(ch: Channel): string {
    return oklchToHex({ l: CANONICAL_L, c: ch.intensity, h: ch.hue })
  }

  const settings = $derived(settingsStore.value)

  let base = $state<Channel>({ hue: 250, intensity: 0.03 })
  let accent = $state<Channel>({ hue: 250, intensity: 0.18 })
  let font = $state<FontStyleId>('system')
  /** Set once the stored settings have seeded the sliders, so the
   *  seeding effect never fights the user's dragging. */
  let seeded = $state(false)

  $effect(() => {
    if (seeded || !settings) return
    base = toChannel(settings.custom_theme_base)
    accent = toChannel(settings.custom_theme_accent)
    font = (settings.custom_theme_font ?? 'system') as FontStyleId
    seeded = true
  })

  const baseHex = $derived(toHex(base))
  const accentHex = $derived(toHex(accent))

  /**
   * Recompile the stylesheet as the sliders move.
   *
   * Live, not on release: a colour choice is a judgement about how the
   * result looks, and making someone lift their finger to find out
   * turns one decision into a dozen guesses. The compile is pure maths
   * over ~150 values and the write is a single `textContent`
   * assignment, so it comfortably keeps up with a drag.
   */
  $effect(() => {
    applyCustomTheme({ base: baseHex, accent: accentHex, font })
  })

  const isActive = $derived(settings?.theme_name === CUSTOM_THEME_ID)

  /** Persist, and switch to the theme if it isn't already showing. */
  async function save(activate: boolean) {
    try {
      await settingsStore.patch({
        custom_theme_base: baseHex,
        custom_theme_accent: accentHex,
        custom_theme_font: font,
        ...(activate ? { theme_name: CUSTOM_THEME_ID } : {}),
      })
      if (activate && settings) applyTheme(CUSTOM_THEME_ID, settings.theme_mode)
    } catch (e) {
      toasts.error(m.mobile_settings_save_failed(), e)
    }
  }

  /**
   * Save on the way out rather than behind a Save button.
   *
   * The preview *is* the feedback — by the time someone leaves, they
   * have already seen the result and decided. A confirmation step here
   * would ask them to agree to something they have been looking at for
   * a minute.
   */
  function leave() {
    void save(false)
    nav.pop()
  }

  const PRESETS: Array<{ id: string; base: string; accent: string }> = [
    { id: 'midnight', base: '#1e2a40', accent: '#3b82f6' },
    { id: 'forest', base: '#22312a', accent: '#10b981' },
    { id: 'ember', base: '#33241f', accent: '#f97316' },
    { id: 'plum', base: '#2e2233', accent: '#a855f7' },
    { id: 'slate', base: '#2b2f33', accent: '#64748b' },
    { id: 'rose', base: '#33232a', accent: '#f43f5e' },
  ]

  function applyPreset(p: { base: string; accent: string }) {
    base = toChannel(p.base)
    accent = toChannel(p.accent)
  }

  const FONTS: Array<{ id: FontStyleId; label: () => string; desc: () => string }> = [
    { id: 'system', label: m.mobile_theme_font_system, desc: m.mobile_theme_font_system_desc },
    { id: 'rounded', label: m.mobile_theme_font_rounded, desc: m.mobile_theme_font_rounded_desc },
    { id: 'serif', label: m.mobile_theme_font_serif, desc: m.mobile_theme_font_serif_desc },
    { id: 'mono', label: m.mobile_theme_font_mono, desc: m.mobile_theme_font_mono_desc },
  ]

  /** A hue strip for the slider track, so the control shows what it
   *  does instead of relying on the reader to remember a colour wheel. */
  function hueTrack(ch: Channel, max: number): string {
    const stops = []
    for (let h = 0; h <= 360; h += 30) {
      const swatch = oklchToHex({
        l: CANONICAL_L,
        c: Math.min(ch.intensity || max * 0.6, max),
        h,
      })
      stops.push(`${swatch} ${(h / 360) * 100}%`)
    }
    return `linear-gradient(to right, ${stops.join(', ')})`
  }

  function rampFor(hex: string, chromaScale: number): string[] {
    const scale = buildScale(hex, chromaScale)
    return [scale[100], scale[300], scale[500], scale[700], scale[900]]
  }
</script>

<div class="screen">
  <NavBar
    title={m.mobile_theme_editor_title()}
    backLabel={m.mobile_settings_appearance()}
    onback={leave}
    large
  />

  <div class="screen__body">
    <!-- Preview first. It is the reason the screen exists, and putting
         it above the controls means it never scrolls out of sight while
         a slider is being dragged. -->
    <div class="list-header">{m.mobile_theme_preview()}</div>
    <!--
      `data-theme="custom"` on the preview itself, not on the document.

      The generated stylesheet defines its variables under
      `[data-theme='custom']`, so putting the attribute here makes those
      values cascade into this subtree *only*. Without it the preview
      inherited whichever theme was currently active and cheerfully
      showed the user someone else's colours — the one thing a preview
      must never do. Now it shows the edit even while a stock theme is
      still the one in use.
    -->
    <div
      class="preview"
      data-theme={CUSTOM_THEME_ID}
      style="font-family: {fontStack(font)}"
    >
      <div class="preview__row preview__row--unread">
        <span class="preview__rail"></span>
        <span class="preview__avatar">AM</span>
        <span class="min-w-0 flex-1">
          <span class="preview__from">{m.mobile_theme_preview_sender()}</span>
          <span class="preview__subject">{m.mobile_theme_preview_subject()}</span>
        </span>
        <span class="preview__date">09:24</span>
      </div>
      <div class="preview__actions">
        <button type="button" class="preview__button">{m.mobile_theme_preview_button()}</button>
        <span class="preview__chip">{m.mobile_theme_active()}</span>
      </div>
    </div>

    <!-- ── Base ────────────────────────────────────────────────── -->
    <div class="list-header">{m.mobile_theme_base()}</div>
    <div class="list-group">
      <div class="swatch-row">
        {#each rampFor(baseHex, 0.28) as shade (shade)}
          <span class="swatch" style="background: {shade}"></span>
        {/each}
      </div>
      <label class="slider">
        <span class="slider__label">{m.mobile_theme_hue()}</span>
        <input
          type="range"
          min="0"
          max="360"
          step="1"
          bind:value={base.hue}
          style="--track: {hueTrack(base, INTENSITY_MAX.base)}"
        />
      </label>
      <label class="slider">
        <span class="slider__label">{m.mobile_theme_intensity()}</span>
        <input type="range" min="0" max={INTENSITY_MAX.base} step="0.002" bind:value={base.intensity} />
      </label>
      <div class="hex-row">
        <span class="hex-row__label">{m.mobile_theme_hex()}</span>
        <input
          class="hex-row__input"
          aria-label="{m.mobile_theme_base()}: {m.mobile_theme_hex()}"
          value={baseHex}
          onchange={(e) => (base = toChannel(e.currentTarget.value))}
          autocapitalize="none"
          autocorrect="off"
          spellcheck="false"
        />
      </div>
    </div>
    <p class="list-footer">{m.mobile_theme_base_hint()}</p>

    <!-- ── Accent ──────────────────────────────────────────────── -->
    <div class="list-header">{m.mobile_theme_accent()}</div>
    <div class="list-group">
      <div class="swatch-row">
        {#each rampFor(accentHex, 1) as shade (shade)}
          <span class="swatch" style="background: {shade}"></span>
        {/each}
      </div>
      <label class="slider">
        <span class="slider__label">{m.mobile_theme_hue()}</span>
        <input
          type="range"
          min="0"
          max="360"
          step="1"
          bind:value={accent.hue}
          style="--track: {hueTrack(accent, INTENSITY_MAX.accent)}"
        />
      </label>
      <label class="slider">
        <span class="slider__label">{m.mobile_theme_intensity()}</span>
        <input
          type="range"
          min="0"
          max={INTENSITY_MAX.accent}
          step="0.005"
          bind:value={accent.intensity}
        />
      </label>
      <div class="hex-row">
        <span class="hex-row__label">{m.mobile_theme_hex()}</span>
        <input
          class="hex-row__input"
          aria-label="{m.mobile_theme_accent()}: {m.mobile_theme_hex()}"
          value={accentHex}
          onchange={(e) => (accent = toChannel(e.currentTarget.value))}
          autocapitalize="none"
          autocorrect="off"
          spellcheck="false"
        />
      </div>
    </div>
    <p class="list-footer">{m.mobile_theme_accent_hint()}</p>

    <!-- ── Presets ─────────────────────────────────────────────── -->
    <div class="list-header">{m.mobile_theme_presets()}</div>
    <div class="list-group">
      <div class="preset-row">
        {#each PRESETS as preset (preset.id)}
          <button
            type="button"
            class="preset"
            aria-label={preset.id}
            onclick={() => applyPreset(preset)}
          >
            <span class="preset__base" style="background: {buildScale(preset.base, 0.28)[800]}">
              <span class="preset__accent" style="background: {preset.accent}"></span>
            </span>
          </button>
        {/each}
      </div>
    </div>

    <!-- ── Font ────────────────────────────────────────────────── -->
    <div class="list-header">{m.mobile_theme_font()}</div>
    <div class="list-group">
      {#each FONTS as option (option.id)}
        <button type="button" class="list-row" onclick={() => (font = option.id)}>
          <span class="min-w-0 flex-1">
            <span
              class="block list-row__title"
              style="font-family: {FONT_STACKS[option.id]}"
            >
              {option.label()}
            </span>
            <span class="block list-row__detail">{option.desc()}</span>
          </span>
          {#if font === option.id}
            <Icon name="check" size={18} />
          {/if}
        </button>
      {/each}
    </div>
    <p class="list-footer">{m.mobile_theme_font_hint()}</p>

    <div class="px-3 pt-3">
      {#if isActive}
        <button type="button" class="wide-button wide-button--quiet" onclick={() => void save(false)}>
          {m.mobile_theme_active()}
        </button>
      {:else}
        <button type="button" class="wide-button" onclick={() => void save(true)}>
          {m.mobile_theme_activate()}
        </button>
      {/if}
    </div>

    <div class="h-24"></div>
  </div>
</div>

<style>
  /* ── Preview ─────────────────────────────────────────────────── */
  /*
     The preview re-derives the `--ui-*` tokens it uses.

     Those tokens live on `:root`, so they resolve against whichever
     theme is *active* — which for a preview is precisely the wrong
     answer. `data-theme="custom"` on the element scopes the underlying
     `--color-*` values, but tokens already computed at `:root` do not
     re-resolve just because a descendant changed theme; they have to be
     redefined here to pick the scoped colours up.

     It duplicates five declarations from `app.css`, which is a real (if
     small) cost. The alternative is a preview that lies, and the whole
     screen exists to show the user the truth about their colours before
     they commit to them. Mirrors the light/dark split exactly.
  */
  .preview {
    --ui-card: var(--color-surface-50);
    --ui-hairline: color-mix(in oklab, var(--color-surface-500) 14%, transparent);
    /* The inks and fills are *not* restated: `accessible-colors` rules
       match any element carrying `data-theme`, this one included, so
       the preview already has the custom theme's solved values. Only
       the alias has to be re-pointed, since `:root` resolved it. */
    --ui-unread: var(--ui-accent-ink);

    background: var(--ui-card);
    margin: 0 var(--s-3) var(--s-3);
    border: 1px solid var(--ui-hairline);
    border-radius: var(--r-lg);
    box-shadow: var(--e-1);
    overflow: hidden;
  }

  :global([data-mode='dark']) .preview {
    --ui-card: var(--color-surface-900);
    --ui-hairline: color-mix(in oklab, var(--color-surface-50) 10%, transparent);
  }

  .preview__row {
    display: flex;
    align-items: flex-start;
    gap: var(--s-3);
    padding: var(--s-3) var(--s-4) var(--s-3) var(--s-3);
    position: relative;
    background: color-mix(in oklab, var(--color-primary-500) 5%, transparent);
  }

  .preview__rail {
    position: absolute;
    left: 0;
    top: var(--s-2);
    bottom: var(--s-2);
    width: 3px;
    border-radius: 0 var(--r-full) var(--r-full) 0;
    background: var(--ui-unread);
  }

  .preview__avatar {
    width: 38px;
    height: 38px;
    border-radius: var(--r-full);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    font-size: var(--t-caption);
    font-weight: 600;
    background: color-mix(in oklab, var(--color-primary-500) 22%, transparent);
    color: var(--color-primary-700);
  }

  .preview__from {
    display: block;
    font-size: var(--t-headline);
    line-height: var(--t-headline-lh);
    font-weight: 680;
    color: var(--ui-ink);
  }

  .preview__subject {
    display: block;
    font-size: var(--t-body);
    line-height: var(--t-body-lh);
    color: var(--ui-ink);
  }

  .preview__date {
    font-size: var(--t-caption);
    color: var(--ui-accent-ink);
    font-weight: 600;
    flex-shrink: 0;
  }

  .preview__actions {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-3) var(--s-4);
    border-top: 1px solid var(--ui-hairline);
  }

  .preview__button {
    flex: 1;
    min-height: 40px;
    border-radius: var(--r-md);
    background: var(--ui-accent-fill);
    color: var(--ui-on-accent);
    font-size: var(--t-body);
    font-weight: 600;
  }

  .preview__chip {
    padding: 4px 10px;
    border-radius: var(--r-full);
    font-size: var(--t-caption);
    font-weight: 600;
    color: var(--ui-ink-muted);
    background: color-mix(in oklab, var(--color-surface-500) 16%, transparent);
  }

  /* ── Ramp swatches ───────────────────────────────────────────── */
  .swatch-row {
    display: flex;
    gap: 0;
    padding: var(--s-3) var(--s-4) 0;
  }

  .swatch {
    flex: 1;
    height: 26px;
  }

  .swatch:first-child {
    border-radius: var(--r-sm) 0 0 var(--r-sm);
  }

  .swatch:last-child {
    border-radius: 0 var(--r-sm) var(--r-sm) 0;
  }

  /* ── Sliders ─────────────────────────────────────────────────── */
  .slider {
    display: block;
    padding: var(--s-2) var(--s-4);
  }

  .slider__label {
    display: block;
    font-size: var(--t-caption);
    line-height: var(--t-caption-lh);
    color: var(--ui-ink-faint);
    margin-bottom: 2px;
  }

  .slider input[type='range'] {
    width: 100%;
    -webkit-appearance: none;
    appearance: none;
    height: 28px;
    background: transparent;
  }

  /* The track carries the hue gradient where one was supplied, so the
     control previews its own outcome. */
  .slider input[type='range']::-webkit-slider-runnable-track {
    height: 6px;
    border-radius: var(--r-full);
    background: var(--track, color-mix(in oklab, var(--color-surface-500) 25%, transparent));
  }

  .slider input[type='range']::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 26px;
    height: 26px;
    margin-top: -10px;
    border-radius: var(--r-full);
    background: #fff;
    border: 1px solid color-mix(in oklab, var(--color-surface-950) 18%, transparent);
    box-shadow: var(--e-2);
  }

  /* ── Hex field ───────────────────────────────────────────────── */
  .hex-row {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    padding: var(--s-2) var(--s-4) var(--s-3);
  }

  .hex-row__label {
    font-size: var(--t-caption);
    color: var(--ui-ink-faint);
  }

  .hex-row__input {
    flex: 1;
    min-height: 40px;
    padding-inline: var(--s-3);
    border: 1px solid var(--ui-border);
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--ui-ink);
    /* 16px or iOS zooms the viewport when the field takes focus. */
    font-size: var(--t-input);
    font-family: ui-monospace, monospace;
    text-align: right;
  }

  /* ── Presets ─────────────────────────────────────────────────── */
  .preset-row {
    display: flex;
    gap: var(--s-3);
    padding: var(--s-3) var(--s-4);
  }

  .preset {
    flex: 1;
    aspect-ratio: 1;
    border-radius: var(--r-md);
    overflow: hidden;
    transition: transform var(--m-fast) var(--m-ease);
  }

  .preset:active {
    transform: scale(0.92);
  }

  .preset__base {
    display: flex;
    align-items: flex-end;
    justify-content: flex-end;
    width: 100%;
    height: 100%;
    padding: 5px;
  }

  .preset__accent {
    width: 42%;
    height: 42%;
    border-radius: var(--r-full);
  }
</style>
