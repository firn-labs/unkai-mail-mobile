/**
 * Folder classification: which mailbox is the Inbox, which is
 * Trash, and what order they belong in.
 *
 * IMAP servers answer this two ways and neither is universal:
 * RFC 6154 special-use attributes (`\Sent`, `\Junk`, …) when the
 * server bothers, and otherwise a name that could be "Sent",
 * "Gesendete Elemente", or "[Gmail]/Sent Mail". We take the
 * attribute when it's there and fall back to names — the same
 * two-tier rule the backend's own folder pickers use.
 */

import type { IconName } from '../Icon.svelte'
import { formatLocale } from '../locale'

export interface FolderLike {
  name: string
  delimiter?: string | null
  attributes?: string[]
  unread_count?: number | null
}

export type SpecialUse = 'inbox' | 'drafts' | 'sent' | 'junk' | 'trash' | 'archive'

/** Name hints per special-use kind, lower-cased and matched as a
 *  whole path segment so a user folder called "Sent invoices"
 *  doesn't get promoted into the special tier. */
const NAME_HINTS: Record<Exclude<SpecialUse, 'inbox'>, string[]> = {
  drafts: ['drafts', 'draft', 'entwürfe', 'entwurf', 'brouillons', 'brouillon', 'bozze'],
  sent: [
    'sent',
    'sent items',
    'sent mail',
    'gesendet',
    'gesendete elemente',
    'gesendete objekte',
    'envoyés',
    'envoyes',
    'inviati',
    'enviados',
  ],
  junk: ['junk', 'spam', 'bulk mail', 'unerwünscht', 'werbung', 'pourriel', 'indesiderata'],
  trash: ['trash', 'deleted', 'deleted items', 'papierkorb', 'gelöscht', 'corbeille', 'cestino'],
  archive: ['archive', 'archives', 'archiv', 'archivio', 'archivo'],
}

const ATTR_MAP: Record<string, SpecialUse> = {
  '\\drafts': 'drafts',
  '\\sent': 'sent',
  '\\junk': 'junk',
  '\\trash': 'trash',
  '\\archive': 'archive',
  '\\all': 'archive',
  '\\inbox': 'inbox',
}

export function specialUse(folder: FolderLike): SpecialUse | null {
  if (folder.name.toUpperCase() === 'INBOX') return 'inbox'

  for (const attr of folder.attributes ?? []) {
    const kind = ATTR_MAP[attr.toLowerCase()]
    if (kind) return kind
  }

  const delim = folder.delimiter || '/'
  const leaf = folder.name.split(delim === '.' ? '.' : delim).pop()?.toLowerCase().trim() ?? ''
  if (!leaf) return null
  for (const [kind, hints] of Object.entries(NAME_HINTS)) {
    if (hints.includes(leaf)) return kind as SpecialUse
  }
  return null
}

/** Canonical order for the special tier — the order these appear in
 *  every mail client, which is worth matching exactly. */
const SPECIAL_ORDER: SpecialUse[] = ['inbox', 'drafts', 'sent', 'archive', 'junk', 'trash']

export function iconFor(kind: SpecialUse | null): IconName {
  switch (kind) {
    case 'inbox':
      return 'global-inbox'
    case 'drafts':
      return 'drafts'
    case 'sent':
      return 'sent'
    case 'junk':
      return 'spam'
    case 'trash':
      return 'trash'
    case 'archive':
      return 'archive'
    default:
      // A plain folder. `move-to-folder` is the *action* glyph (a
      // tray with an arrow) and reads as "download" in a list.
      return 'files'
  }
}

export interface SortedFolders<F extends FolderLike> {
  /** Special-use folders in canonical order. */
  special: { folder: F; kind: SpecialUse }[]
  /** Everything else, alphabetically by full path so subfolders sit
   *  under their parent without needing an explicit tree on a phone
   *  (indentation is enough at this width). */
  custom: { folder: F; depth: number }[]
}

export function sortFolders<F extends FolderLike>(folders: F[]): SortedFolders<F> {
  const special: { folder: F; kind: SpecialUse }[] = []
  const seen = new Set<SpecialUse>()
  const custom: F[] = []

  for (const f of folders) {
    const kind = specialUse(f)
    // Only the first match per kind is promoted: a server that
    // advertises two Archive folders would otherwise show both in
    // the special tier and neither in its own hierarchy.
    if (kind && !seen.has(kind)) {
      seen.add(kind)
      special.push({ folder: f, kind })
    } else {
      custom.push(f)
    }
  }

  special.sort((a, b) => SPECIAL_ORDER.indexOf(a.kind) - SPECIAL_ORDER.indexOf(b.kind))

  const collator = new Intl.Collator(formatLocale(), { sensitivity: 'base' })
  custom.sort((a, b) => collator.compare(a.name, b.name))

  return {
    special,
    custom: custom.map((folder) => {
      const delim = folder.delimiter || '/'
      const depth = folder.name.split(delim === '.' ? '.' : delim).length - 1
      return { folder, depth: Math.min(depth, 3) }
    }),
  }
}

/** Leaf name for display: "INBOX/Projects/2026" → "2026". */
export function leafName(folder: FolderLike): string {
  const delim = folder.delimiter || '/'
  const parts = folder.name.split(delim === '.' ? '.' : delim)
  return parts[parts.length - 1] || folder.name
}

/** The mailbox every IMAP account has, under exactly this name (RFC 3501
 *  §5.1: `INBOX` is reserved and case-insensitive on every server). */
export const INBOX = 'INBOX'

/**
 * Whether two folder names denote the same mailbox, by the rule the
 * backend compares them with: Rust's `eq_ignore_ascii_case`.
 *
 * ASCII-only on purpose. `toLowerCase()` would also fold "É" to "é",
 * and a name the UI then considered "the archive folder" would still
 * be a *different* folder to the backend — the exact disagreement that
 * made a message vanish on archive and reappear on reopen.
 */
export function sameFolder(a: string, b: string): boolean {
  if (a.length !== b.length) return false
  const fold = (s: string) => s.replace(/[A-Z]/g, (c) => c.toLowerCase())
  return fold(a) === fold(b)
}
