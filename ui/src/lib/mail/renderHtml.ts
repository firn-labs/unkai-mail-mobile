/**
 * The message-body rendering pipeline.
 *
 * A received message's HTML is hostile input: it can carry scripts,
 * tracking pixels, layouts built for a 900px desktop window, and
 * colours chosen against a white page that vanish on a dark phone
 * screen. Everything that has to happen between "bytes from IMAP"
 * and "readable on a 390pt screen" lives here, in one ordered pass,
 * so the reader component only decides *when* to run it.
 *
 * Order matters and is load-bearing:
 *
 *   1. **Sanitise** (DOMPurify) — scripts, frames, forms, external
 *      stylesheets out.
 *   2. **Autolink** bare URLs, so plain-text links are tappable and
 *      visible to the URLhaus check below.
 *   3. **Annotate links** — external links get flagged for the tap
 *      handler (they leave the app), `cid:` anchors get their
 *      content-id parked in a data attribute.
 *   4. **Block remote images** unless the user opted in for this
 *      sender. Loading them is a read receipt.
 *   5. **Resolve `cid:` images** against the parts fetched with the
 *      message. These ride *inside* the message, so they need no
 *      consent and are never blocked.
 *   6. **Fold quoted history** into collapsed blocks — on a phone,
 *      an un-folded reply chain means scrolling past ten rounds of
 *      quoting to reach three new sentences.
 *   7. **Dark reading mode** — light inline backgrounds become dark
 *      ones in the theme's hue (only when the user chose it, and only
 *      in dark mode — see `darkenEmailBackgrounds`).
 *   8. **Fix contrast** (#472) — re-tint only the inline colours
 *      that fail WCAG against the actual background behind them.
 *
 * Steps 4–8 are conditional; 1–3 always run.
 */

import DOMPurify from 'dompurify'
import { applyInlineImages } from '../inlineImages'
import { darkenEmailBackgrounds, ensureReadableEmailText, type Rgb } from '../emailContrast'
import { m } from '../../paraglide/messages'

/** 1×1 transparent GIF standing in for a blocked remote image. */
const BLOCKED_IMG_PLACEHOLDER =
  'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7'

/** Tags DOMPurify drops outright. `style` goes too: an email's
 *  stylesheet can restyle our own chrome once the body is inlined
 *  into the page, and inline `style=` attributes (which survive)
 *  are what senders actually rely on. */
const FORBID_TAGS = [
  'script',
  'noscript',
  'object',
  'embed',
  'applet',
  'iframe',
  'frame',
  'frameset',
  'form',
  'input',
  'textarea',
  'select',
  'button',
  'base',
  'meta',
  'link',
  'style',
]

const ADD_ATTR = [
  'target',
  'data-unkai-cid',
  'data-unkai-blocked-src',
  'title',
  'data-attachment-ref',
  'data-cid',
  'data-filename',
  'data-label',
]

/** Sanitise without any of the later passes — used when quoting a
 *  message into a reply, where the result goes into the composer
 *  rather than onto the screen. */
export function sanitizeEmailHtml(html: string): string {
  return DOMPurify.sanitize(html, { FORBID_TAGS, ADD_ATTR, FORCE_BODY: true })
}

export interface ProcessOptions {
  /** Load remote images (the sender is trusted, or the user tapped
   *  "Show images" on this message). */
  showImages: boolean
  /** `cid:` → object URL, from `buildInlineImageUrls`. */
  inlineUrls: Record<string, string>
  /** Inline parts are still being fetched — placeholders instead of
   *  broken-image icons. */
  inlineLoading: boolean
  /** Re-tint failing colours. False in white-canvas mode, where the
   *  sender's own assumptions already hold. */
  adjustContrast: boolean
  /** Dark reading mode: re-colour light backgrounds before the contrast
   *  pass (step 7). Needs `adjustContrast` and `contrastColors`. */
  darkReading?: boolean
  /** Canvas + text colour the body is composited on, for steps 7–8. */
  contrastColors?: { background: Rgb; text: Rgb }
}

export interface ProcessResult {
  html: string
  /** At least one remote image was withheld — the reader shows the
   *  "Show images" bar off this. */
  hadBlocked: boolean
}

export function processEmailHtml(html: string, opts: ProcessOptions): ProcessResult {
  if (!html) return { html: '', hadBlocked: false }
  try {
    const clean = sanitizeEmailHtml(html)
    const doc = new DOMParser().parseFromString(clean, 'text/html')

    autolinkPlainTextUrls(doc)

    doc.querySelectorAll('a[href]').forEach((a) => {
      const href = a.getAttribute('href') ?? ''
      if (!href) return
      if (href.toLowerCase().startsWith('cid:')) {
        // `cid:` anchors point at a part of this same message. Park
        // the id and neutralise navigation — the reader's tap
        // handler opens the part instead.
        const cid = href.slice(4).trim().replace(/^<|>$/g, '')
        a.setAttribute('data-unkai-cid', cid)
        a.setAttribute('href', '#')
      } else {
        a.setAttribute('rel', 'noopener noreferrer')
      }
    })

    let hadBlocked = false
    if (!opts.showImages) {
      doc.querySelectorAll('img').forEach((img) => {
        const src = img.getAttribute('src') ?? ''
        const lower = src.toLowerCase()
        if (src && !lower.startsWith('data:') && !lower.startsWith('cid:')) {
          hadBlocked = true
          img.setAttribute('data-unkai-blocked-src', src)
          img.setAttribute('src', BLOCKED_IMG_PLACEHOLDER)
          img.removeAttribute('srcset')
          if (!img.getAttribute('alt')) img.setAttribute('alt', '(blocked image)')
        }
      })
    }

    applyInlineImages(doc, opts.inlineUrls, opts.inlineLoading)
    collapseQuotedBlocks(doc)
    makeResponsive(doc)

    if (opts.adjustContrast && opts.contrastColors) {
      if (opts.darkReading) {
        darkenEmailBackgrounds(doc.body, {
          canvas: opts.contrastColors.background,
          resolveColor: resolveCssColorViaCanvas,
        })
      }
      ensureReadableEmailText(doc.body, {
        background: opts.contrastColors.background,
        text: opts.contrastColors.text,
        resolveColor: resolveCssColorViaCanvas,
      })
    }

    return { html: doc.body.innerHTML, hadBlocked }
  } catch (e) {
    console.warn('processEmailHtml failed:', e)
    return { html: '', hadBlocked: false }
  }
}

/**
 * Wrap bare `http(s)://…` runs in text nodes with real anchors.
 *
 * Plenty of senders (CLI mailers, hand-written HTML, plain-text
 * parts we promoted) put a URL straight into the body. Without this
 * they render as unclickable text *and* bypass the link-safety
 * check, which only walks `<a href>`.
 */
export function autolinkPlainTextUrls(doc: Document): void {
  const URL_RE = /\bhttps?:\/\/[^\s<>"')\]]+/g
  const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT)
  const targets: Text[] = []
  let node = walker.nextNode() as Text | null
  while (node) {
    // Never rewrite inside an existing anchor: the text there is the
    // link's own label, and wrapping it would nest anchors.
    if (!node.parentElement?.closest('a') && URL_RE.test(node.nodeValue ?? '')) {
      targets.push(node)
    }
    URL_RE.lastIndex = 0
    node = walker.nextNode() as Text | null
  }

  for (const text of targets) {
    const value = text.nodeValue ?? ''
    const frag = doc.createDocumentFragment()
    let last = 0
    for (const match of value.matchAll(URL_RE)) {
      const start = match.index ?? 0
      // Trailing sentence punctuation is almost never part of a URL.
      const raw = match[0].replace(/[.,;:!?]+$/, '')
      if (start > last) frag.appendChild(doc.createTextNode(value.slice(last, start)))
      const a = doc.createElement('a')
      a.setAttribute('href', raw)
      a.setAttribute('rel', 'noopener noreferrer')
      a.textContent = raw
      frag.appendChild(a)
      last = start + raw.length
    }
    if (last < value.length) frag.appendChild(doc.createTextNode(value.slice(last)))
    text.parentNode?.replaceChild(frag, text)
  }
}

/**
 * Collapse quoted history and forwarded blocks behind a summary.
 *
 * Three shapes get folded, in priority order:
 *
 *   1. Our own `data-unkai-block` wrappers (a reply or forward sent
 *      from Unkai) — we know exactly where those start.
 *   2. A top-level `<blockquote>` following some new text — the
 *      convention every mail client renders replies with.
 *   3. A "On <date>, <someone> wrote:" attribution line and
 *      everything after it, which is what plain-text replies
 *      promoted to HTML look like.
 *
 * Folding is skipped when the quote *is* the whole message: a
 * forward with no added commentary would otherwise open as an empty
 * screen with one collapsed card on it.
 */
export function collapseQuotedBlocks(doc: Document): void {
  const body = doc.body
  if (!body) return

  const wrap = (nodes: Node[], label: string) => {
    if (nodes.length === 0) return
    const first = nodes[0]
    const details = doc.createElement('details')
    details.className = 'quoted-collapse'
    const summary = doc.createElement('summary')
    summary.className = 'quoted-collapse__summary'
    summary.textContent = label
    details.appendChild(summary)
    const content = doc.createElement('div')
    content.className = 'quoted-collapse__content'
    first.parentNode?.insertBefore(details, first)
    for (const n of nodes) content.appendChild(n)
    details.appendChild(content)
  }

  // 1 — our own wrappers.
  const ownBlocks = Array.from(
    body.querySelectorAll('div[data-unkai-block="quoted-history"], div[data-unkai-block="forwarded-mail"]'),
  )
  for (const block of ownBlocks) {
    if (block.closest('details.quoted-collapse')) continue
    const isForward = block.getAttribute('data-unkai-block') === 'forwarded-mail'
    // Only fold when there is something above it worth reading.
    if (!hasContentBefore(block)) continue
    wrap([block], isForward ? m.mobile_forwarded_message() : m.mobile_quoted_history())
  }

  // 2 — a top-level blockquote with new text above it.
  const quotes = Array.from(body.children).filter(
    (el) => el.tagName === 'BLOCKQUOTE' && !el.closest('details.quoted-collapse'),
  )
  for (const q of quotes) {
    if (!hasContentBefore(q)) continue
    // Take the attribution line directly above the quote with it —
    // "On Tuesday, Alex wrote:" belongs to the quote, not the reply.
    const nodes: Node[] = [q]
    const prev = q.previousElementSibling
    if (prev && isAttributionLine(prev.textContent ?? '')) nodes.unshift(prev)
    wrap(nodes, m.mobile_quoted_history())
  }

  // 3 — a bare attribution line and everything after it.
  const children = Array.from(body.children)
  const idx = children.findIndex(
    (el) =>
      !el.closest('details.quoted-collapse') &&
      el.tagName !== 'BLOCKQUOTE' &&
      isAttributionLine(el.textContent ?? ''),
  )
  if (idx > 0) {
    const rest = children.slice(idx)
    if (rest.length > 1) wrap(rest, m.mobile_quoted_history())
  }
}

function hasContentBefore(el: Element): boolean {
  let sibling = el.previousSibling
  while (sibling) {
    const text = sibling.textContent?.trim() ?? ''
    if (text.length > 0) return true
    if (sibling.nodeType === Node.ELEMENT_NODE && (sibling as Element).querySelector('img'))
      return true
    sibling = sibling.previousSibling
  }
  return false
}

/** "On 14 Apr 2026 at 09:41, Alex Morgan wrote:" and its German /
 *  French cousins. Deliberately conservative — a false positive
 *  hides real content behind a fold. */
function isAttributionLine(text: string): boolean {
  const t = text.trim()
  if (t.length === 0 || t.length > 300) return false
  return /(^|\s)(wrote|schrieb|a écrit|escribió|ha scritto)\s*:?\s*$/i.test(t) ||
    /^-{2,}\s*(original message|ursprüngliche nachricht|forwarded message)\s*-{2,}/i.test(t)
}

/**
 * Make a desktop-width body survive a phone screen.
 *
 * Senders hard-code table and container widths for a ~600px desktop
 * pane. Left alone those force a horizontally scrolling page, which
 * on a phone means every vertical swipe drifts sideways. Rather
 * than rewrite layouts we can't understand, we cap the *declared*
 * widths — the CSS in `app.css` then lets the remaining
 * over-wide content scroll inside its own container instead of the
 * page.
 */
function makeResponsive(doc: Document): void {
  doc.querySelectorAll<HTMLElement>('[width]').forEach((el) => {
    const raw = el.getAttribute('width') ?? ''
    const px = Number.parseInt(raw, 10)
    if (Number.isFinite(px) && px > 320) {
      el.removeAttribute('width')
      el.style.maxWidth = '100%'
      el.style.width = '100%'
    }
  })
  doc.querySelectorAll<HTMLElement>('table, td, div, img').forEach((el) => {
    const width = el.style.width
    const minWidth = el.style.minWidth
    if (width && /^\d+(px)?$/.test(width) && Number.parseInt(width, 10) > 320) {
      el.style.width = '100%'
      el.style.maxWidth = '100%'
    }
    if (minWidth && /^\d+(px)?$/.test(minWidth) && Number.parseInt(minWidth, 10) > 320) {
      el.style.minWidth = '0'
    }
  })
}

/**
 * Resolve any CSS colour string to RGB by painting it.
 *
 * Theme colours are `oklch(...)` and senders write everything from
 * `#abc` to `rgb(1 2 3 / 40%)` to `rebeccapurple`; hand-parsing that
 * is a losing game. A 1×1 canvas gives the browser's own answer.
 */
let probe: CanvasRenderingContext2D | null = null
/** Answers by colour string. A newsletter repeats the same few colours
 *  on hundreds of cells; each probe is a canvas read-back. */
const resolved = new Map<string, Rgb | null>()

export function resolveCssColorViaCanvas(raw: string): Rgb | null {
  if (!raw) return null
  const known = resolved.get(raw)
  if (known !== undefined) return known
  let result: Rgb | null = null
  try {
    // One 1×1 canvas for the whole session. A new canvas and context
    // per call cost a GPU-backed allocation each — for every colour of
    // every cell, on the thread that is trying to show the message.
    if (!probe) {
      const canvas = document.createElement('canvas')
      canvas.width = 1
      canvas.height = 1
      probe = canvas.getContext('2d', { willReadFrequently: true })
    }
    if (probe) {
      probe.clearRect(0, 0, 1, 1)
      probe.fillStyle = '#000'
      probe.fillStyle = raw
      // An unparseable value leaves fillStyle at the previous colour;
      // that's indistinguishable from a real black, so we accept it —
      // black on our canvas is a legitimate answer either way.
      probe.fillRect(0, 0, 1, 1)
      const [r, g, b, a] = probe.getImageData(0, 0, 1, 1).data
      result = { r, g, b, a: a / 255 }
    }
  } catch {
    result = null
  }
  if (resolved.size > 500) resolved.clear()
  resolved.set(raw, result)
  return result
}

/** The canvas + inherited text colour the body composites onto,
 *  read off the live document so a theme switch re-tints correctly. */
export function themeBaseColors(el: HTMLElement): { background: Rgb; text: Rgb } {
  const styles = getComputedStyle(el)
  return {
    background: resolveCssColorViaCanvas(styles.backgroundColor) ?? { r: 255, g: 255, b: 255, a: 1 },
    text: resolveCssColorViaCanvas(styles.color) ?? { r: 0, g: 0, b: 0, a: 1 },
  }
}

/** Wrap a plain-text body so the same pipeline can render it:
 *  escape markup, keep line breaks, let the autolinker find URLs. */
export function textToHtml(text: string): string {
  const escaped = text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
  return `<div style="white-space:pre-wrap">${escaped}</div>`
}

/** Does this string already look like HTML? Used when a draft or a
 *  quote could be either. */
export function looksLikeHtml(text: string): boolean {
  return /<[a-z][\s\S]*>/i.test(text)
}
