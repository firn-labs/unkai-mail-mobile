/**
 * Which folder is "the archive" for an account — asked of the backend,
 * never guessed.
 *
 * Archiving a message that already sits in that folder is a silent
 * no-op in `archive_message`, so a screen that offers Archive there
 * makes the message disappear from the list and come back on the next
 * open. Screens use this to offer "Move to Inbox" instead.
 */

import * as api from '../api'
import { sameFolder } from './folders'

/** One lookup per account per session. The folder list changes rarely,
 *  and a stale answer only swaps which button is shown. */
const lookups = new Map<string, Promise<string | null>>()

export function archiveFolderFor(accountId: string): Promise<string | null> {
  let pending = lookups.get(accountId)
  if (!pending) {
    pending = api.mail.getArchiveFolder({ accountId }).catch(() => {
      // Don't remember a failure: the next screen should ask again.
      lookups.delete(accountId)
      return null
    })
    lookups.set(accountId, pending)
  }
  return pending
}

/** Whether `folder` is `archive`. False while the archive is unknown. */
export function isArchiveFolder(folder: string, archive: string | null | undefined): boolean {
  return !!archive && sameFolder(folder, archive)
}
