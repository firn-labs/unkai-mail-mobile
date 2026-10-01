/**
 * Turning a message you're reading into one you're writing.
 *
 * Reply, Reply-all and Forward each have a few rules that are easy
 * to get subtly wrong, so they live here rather than inside the
 * reader:
 *
 *   * **Subject prefixes** are added once, not stacked — "Re: Re:
 *     Re:" is the classic tell of a client that re-prefixes blindly.
 *   * **Reply-all drops you.** Comparison is on the bare address,
 *     because To/Cc entries carry display names and a raw string
 *     compare would leave the user CC'd on their own reply.
 *   * **The quote is HTML and untouched.** The original body keeps
 *     its tables, styles and images; it is sanitised (a sender's
 *     scripts must not ride into the outgoing message) but never
 *     reflowed. The composer renders it below the editor and
 *     splices it in at send time.
 *   * **Threading anchors** ride along so the recipient's client
 *     groups the reply with its parent.
 */

import { forwardedMailHtml, quotedHistoryHtml } from '../inviteHtml'
import { sanitizeEmailHtml, looksLikeHtml } from './renderHtml'
import { bareEmail } from '../format'
import type { ComposeInitial } from '../screens/Compose.svelte'
import { formatLocale } from '../locale'

export interface ReplyableMail {
  account_id: string
  folder: string
  id?: string
  from: string
  to: string[]
  cc: string[]
  subject: string
  body_text: string | null
  body_html?: string | null
  date: string
  message_id?: string | null
  references_ids?: string[]
  protection?: string | null
  attachments?: { filename: string; content_type: string; part_id: number }[]
}

export type ReplyKind = 'reply' | 'reply-all' | 'forward'

function replySubject(subject: string): string {
  return /^re:/i.test(subject.trim()) ? subject : `Re: ${subject}`
}

function forwardSubject(subject: string): string {
  return /^fwd?:/i.test(subject.trim()) ? subject : `Fwd: ${subject}`
}

/** Plain text → HTML with its line breaks intact; HTML passes
 *  through. Used for senders who wrote plain text. */
function bodyAsHtml(mail: ReplyableMail): string {
  if (mail.body_html) return sanitizeEmailHtml(mail.body_html)
  const text = mail.body_text ?? ''
  if (looksLikeHtml(text)) return sanitizeEmailHtml(text)
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/\n/g, '<br>')
}

/**
 * Build what the composer should open with.
 *
 * `selfAddress` is the account's own address, needed only for
 * reply-all; pass the account the message was received on. `uid`
 * is the source message's IMAP uid — the `Email` DTO doesn't carry
 * it, and the backend needs it to stamp the answered flag.
 */
export function buildReplyInitial(
  mail: ReplyableMail,
  kind: ReplyKind,
  selfAddress: string,
  uid: number,
): ComposeInitial {
  const when = new Date(mail.date).toLocaleString(formatLocale())

  if (kind === 'forward') {
    return {
      accountId: mail.account_id,
      subject: forwardSubject(mail.subject),
      quotedHtml: forwardedMailHtml({
        fromHeader: mail.from,
        whenText: when,
        subjectText: mail.subject,
        toHeader: mail.to.join(', '),
        bodyHtml: bodyAsHtml(mail),
      }),
      // Deliberately no `encrypt`: forwarding changes the
      // recipients, and the new ones may have no key.
      repliedTo: { accountId: mail.account_id, folder: mail.folder, uid, kind: 'forward' },
    }
  }

  const self = selfAddress.toLowerCase()
  const others =
    kind === 'reply-all'
      ? [...mail.to, ...mail.cc].filter((addr) => {
          const bare = bareEmail(addr).toLowerCase()
          return bare && bare !== self && bare !== bareEmail(mail.from).toLowerCase()
        })
      : []

  return {
    accountId: mail.account_id,
    to: mail.from,
    cc: others.join(', '),
    subject: replySubject(mail.subject),
    quotedHtml: quotedHistoryHtml({
      fromHeader: mail.from,
      whenText: when,
      bodyHtml: bodyAsHtml(mail),
    }),
    inReplyTo: mail.message_id ?? null,
    references: [...(mail.references_ids ?? []), ...(mail.message_id ? [mail.message_id] : [])],
    // Replying to an encrypted thread keeps it encrypted by
    // default; the user can still switch it off in the composer.
    encrypt: !!mail.protection,
    repliedTo: { accountId: mail.account_id, folder: mail.folder, uid, kind },
  }
}
