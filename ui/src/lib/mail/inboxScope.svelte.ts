/**
 * Which account's inbox the Mail tab opens on.
 *
 * With one account there is nothing to choose: the inbox is that
 * account's. With several, the inbox shows all of them merged by
 * default, and the switcher under the inbox title narrows it to one
 * (`MailList`, `root`). The choice is remembered, because someone who
 * keeps a work and a private account usually wants the same one every
 * time they open the app.
 *
 * Kept in `localStorage`, not in the backend settings: it is a view
 * preference of this device, like the last tab, and must not travel to
 * a desktop install through the settings bundle. Every access is
 * guarded, since storage can be unavailable.
 */

export { resolveInboxAccount } from './inboxChoice'

const STORAGE_KEY = 'unkai.inboxAccount'

function read(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY)
  } catch {
    return null
  }
}

class InboxScope {
  /** The chosen account id, or `null` for all accounts. */
  accountId = $state<string | null>(read())

  set(accountId: string | null): void {
    this.accountId = accountId
    try {
      if (accountId === null) localStorage.removeItem(STORAGE_KEY)
      else localStorage.setItem(STORAGE_KEY, accountId)
    } catch {
      // Not remembered across launches; the choice still applies now.
    }
  }
}

export const inboxScope = new InboxScope()
