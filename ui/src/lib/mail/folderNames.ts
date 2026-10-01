/**
 * Folder names as a person reads them.
 *
 * A special-use folder is named by its *role* in the app's language
 * ("Entwürfe", "Papierkorb"), whatever the server calls it; every other
 * folder by its leaf name ("INBOX/Projekte/2026" → "2026"). The mailbox
 * list did this first; the move picker and its "Moved to …" toast did
 * not, and offered "INBOX", "Drafts" and "Junk" in a German interface.
 * One function, so the three can no longer disagree.
 *
 * Separate from `folders.ts` because it reads the message catalogue,
 * and `folders.ts` stays a pure module its node tests can load.
 */
import { m } from '../../paraglide/messages'
import { leafName, specialUse, type FolderLike, type SpecialUse } from './folders'

/** A special-use folder's name in the user's language. */
export function roleName(kind: SpecialUse): string {
  switch (kind) {
    case 'inbox':
      return m.folder_name_inbox()
    case 'drafts':
      return m.folder_name_drafts()
    case 'sent':
      return m.folder_name_sent()
    case 'archive':
      return m.folder_name_archive()
    case 'junk':
      return m.folder_name_junk()
    case 'trash':
      return m.folder_name_trash()
  }
}

/** What to call a folder on screen. */
export function folderLabel(folder: FolderLike): string {
  const kind = specialUse(folder)
  return kind ? roleName(kind) : leafName(folder)
}
