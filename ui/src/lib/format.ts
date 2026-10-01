/**
 * Display formatting shared by every list and header in the app.
 *
 * Dates in particular are worth centralising: a mail list wants
 * "09:41" for today and "Tue" for this week, a message header wants
 * the full date, and a calendar agenda wants something in between.
 * Getting that wrong is the difference between a list you can scan
 * and one you have to read.
 */

import { m } from '../paraglide/messages'
import { formatLocale } from './locale'

/** `"Alex Morgan <alex@example.com>"` → `"alex@example.com"`. */
export function bareEmail(addr: string): string {
  const match = addr.match(/<([^>]+)>/)
  return (match ? match[1] : addr).trim()
}

/** `"Alex Morgan <alex@example.com>"` → `"Alex Morgan"`, falling
 *  back to the address when the header carries no display name. */
export function displayName(addr: string): string {
  if (!addr) return ''
  const match = addr.match(/^\s*"?([^"<]*?)"?\s*<[^>]+>\s*$/)
  const name = match?.[1]?.trim()
  if (name) return name
  const bare = bareEmail(addr)
  return bare.split('@')[0] || bare
}

/** Up to two initials for an avatar bubble. */
export function initials(addr: string): string {
  const name = displayName(addr).trim()
  if (!name) return '?'
  const parts = name.split(/[\s._-]+/).filter(Boolean)
  if (parts.length === 0) return '?'
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase()
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase()
}

/**
 * A stable hue per address, so the same sender always gets the same
 * avatar colour. Cheap FNV-ish hash — this is decoration, not
 * identity, and a collision costs nothing.
 */
export function colorFor(seed: string): string {
  let h = 0
  for (let i = 0; i < seed.length; i++) {
    h = (h * 31 + seed.charCodeAt(i)) >>> 0
  }
  return `oklch(0.62 0.13 ${h % 360})`
}

function startOfDay(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
}

/**
 * The mail-list timestamp: time for today, weekday inside the last
 * week, `dd.MM.` inside the year, `dd.MM.yy` beyond it. Locale-aware
 * throughout — `toLocaleTimeString` gives a German user 24-hour
 * clock and an American one AM/PM without a branch here.
 */
export function listDate(iso: string | Date): string {
  const d = iso instanceof Date ? iso : new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  const now = new Date()
  const dayDelta = (startOfDay(now) - startOfDay(d)) / 86_400_000
  if (dayDelta <= 0) return d.toLocaleTimeString(formatLocale(), { hour: '2-digit', minute: '2-digit' })
  if (dayDelta < 7) return d.toLocaleDateString(formatLocale(), { weekday: 'short' })
  if (d.getFullYear() === now.getFullYear())
    return d.toLocaleDateString(formatLocale(), { day: '2-digit', month: '2-digit' })
  return d.toLocaleDateString(formatLocale(), {
    day: '2-digit',
    month: '2-digit',
    year: '2-digit',
  })
}

/** Full date + time, for a message header or an event detail. */
export function fullDate(iso: string | Date): string {
  const d = iso instanceof Date ? iso : new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  return d.toLocaleString(formatLocale(), {
    weekday: 'short',
    day: '2-digit',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

export function timeOnly(iso: string | Date): string {
  const d = iso instanceof Date ? iso : new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  return d.toLocaleTimeString(formatLocale(), { hour: '2-digit', minute: '2-digit' })
}

/** "Today" / "Tomorrow" / "Mon, 14 Apr" for agenda section headers
 *  and task due dates. */
export function dayHeading(d: Date): string {
  const today = startOfDay(new Date())
  const delta = (startOfDay(d) - today) / 86_400_000
  if (delta === 0) return m.mobile_today()
  if (delta === 1) return m.mobile_tomorrow()
  if (delta === -1) return m.mobile_yesterday()
  return d.toLocaleDateString(formatLocale(), {
    weekday: 'short',
    day: 'numeric',
    month: 'short',
  })
}

/**
 * Parse a `YYYY-MM-DD` string in the *local* zone.
 *
 * `new Date("2026-04-14")` is UTC midnight, which renders as the
 * 13th anywhere west of Greenwich — the classic off-by-one-day bug
 * in birthday and due-date rendering.
 */
export function parseLocalDate(value: string): Date | null {
  const match = value.match(/^(\d{4})-(\d{2})-(\d{2})/)
  if (!match) return null
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]))
}

/** `YYYY-MM-DD` in the local zone — the value an `<input type="date">` wants. */
export function toDateInput(d: Date): string {
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

/** `HH:MM` in the local zone — the value an `<input type="time">` wants. */
export function toTimeInput(d: Date): string {
  const p = (n: number) => String(n).padStart(2, '0')
  return `${p(d.getHours())}:${p(d.getMinutes())}`
}

export function fileSize(bytes: number | null | undefined): string {
  if (bytes == null) return ''
  if (bytes < 1024) return `${bytes} B`
  const units = ['KB', 'MB', 'GB']
  let v = bytes / 1024
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`
}

/** Trim a folder path to its leaf: `INBOX/Projects/2026` → `2026`. */
export function folderLeaf(path: string, delimiter = '/'): string {
  const parts = path.split(delimiter === '.' ? '.' : delimiter)
  return parts[parts.length - 1] || path
}

/** First line of a body, for a list-row preview. */
export function snippet(text: string | null | undefined, max = 120): string {
  if (!text) return ''
  const flat = text
    .replace(/\r/g, '')
    .replace(/^>.*$/gm, '')
    .replace(/\s+/g, ' ')
    .trim()
  return flat.length > max ? `${flat.slice(0, max)}…` : flat
}

/** Strip tags from an HTML body cheaply, for the same purpose.
 *  Uses the DOM parser rather than a regex so entities decode and
 *  `<style>` contents don't leak into the preview. */
export function htmlSnippet(html: string | null | undefined, max = 120): string {
  if (!html) return ''
  const doc = new DOMParser().parseFromString(html, 'text/html')
  doc.querySelectorAll('style, script, head').forEach((el) => el.remove())
  return snippet(doc.body.textContent ?? '', max)
}
