/** The pure half of `inboxScope.svelte.ts`, kept apart so it is tested
 *  without touching storage. */

/**
 * The inbox to show for these accounts and this choice.
 *
 * A chosen account that no longer exists (it was removed) falls back to
 * all accounts rather than to an empty list. Pure, so it is tested
 * without a DOM.
 */
export function resolveInboxAccount(
  accountIds: string[],
  chosen: string | null,
): string | null {
  if (accountIds.length === 1) return accountIds[0]
  if (chosen !== null && accountIds.includes(chosen)) return chosen
  return null
}
