<script lang="ts">
  /**
   * Avatar — round photo or initials circle.
   *
   * v3: one colour for every initials circle — the accent wash with the
   * accent ink, initials in the display face. v2 hashed each sender
   * into one of six pastel pairs; the colour carried no meaning (two
   * people could share one, one person got a different one per address)
   * and six pastels on one screen were most of what made the lists look
   * like a template. The initials tell people apart; the photo, when
   * there is one, does it better. The wash pairing is also one the
   * contrast solver guarantees (`washesFor`), which the raw 700/300
   * shades of the old palette were not.
   *
   * Used by the contact list and detail, the reader's sender line, the
   * search results and the More hub's account card. Centralising the
   * markup keeps them identical without each repeating the
   * photo/initials branching.
   *
   * The size is driven via inline `width` / `height` style (not a
   * Tailwind `w-N h-N` class) so a numeric prop survives Tailwind's
   * just-in-time class scanner — dynamically composed utility class
   * names get purged out of the final CSS bundle.
   */
  import { nameInitials } from './fromHeader'

  interface Props {
    /** Pre-resolved photo URL (e.g. via `contactPhotoSrc`).  Pass
     *  null/undefined to render the initials fallback. */
    photo?: string | null
    /** String the initial letter is taken from.  Required so we
     *  always have *something* to render even when the photo URL is
     *  missing or 404s mid-flight. */
    displayName: string
    /** Pixel size — used for both width/height and the font-size of
     *  the initial letter (scaled to ~40% of the box).  32 matches
     *  the contact-row layout; mail rows use 36 for visual balance
     *  against the two-line text block. */
    size?: number
    /** Optional `<img alt>` text.  Defaults to empty (decorative) —
     *  the sender's name is already rendered next to the avatar, so
     *  duplicating it would just produce a screen-reader stutter. */
    alt?: string
  }

  const {
    photo,
    displayName,
    size = 32,
    alt = '',
  }: Props = $props()

  // Two-letter initials when the name has more than one word
  // ("Max Mustermann" → "MM"), single letter otherwise.  See
  // `nameInitials` in fromHeader.ts for the full rule table.
  const initials = $derived(nameInitials(displayName))

  const sizePx = $derived(`${size}px`)
  // Font scales down a notch when there are two glyphs to fit so the
  // pair doesn't crowd the circle.  Floor at 9 px so the smallest
  // (28 px) avatars stay legible.
  const fontPx = $derived(
    `${Math.max(9, Math.round(size * (initials.length > 1 ? 0.36 : 0.42)))}px`,
  )
</script>

{#if photo}
  <img
    src={photo}
    {alt}
    loading="lazy"
    class="rounded-full object-cover shrink-0"
    style="width: {sizePx}; height: {sizePx};"
  />
{:else}
  <span
    class="avatar"
    style="width: {sizePx}; height: {sizePx}; font-size: {fontPx};"
    aria-hidden="true"
  >
    {initials}
  </span>
{/if}

<style>
  .avatar {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    border-radius: var(--r-full);
    background: var(--ui-tint-accent);
    color: var(--ui-accent-ink);
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.01em;
  }
</style>
