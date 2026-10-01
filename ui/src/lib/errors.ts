/**
 * Turning whatever a failed IPC call threw into a sentence.
 *
 * The backend's `UnkaiError` is a serde-tagged Rust enum, so it
 * arrives as an object — `{ Network: "connection refused" }`,
 * `{ Auth: "…" }` — or, for unit variants, as the bare string
 * `"MessageGone"`. `String(e)` on the object form produces
 * `[object Object]`, which is how a real error message turns into
 * nothing at all in front of a user.
 *
 * Everything that surfaces an error to a person goes through here.
 */

/** Variant names that carry no payload — the name IS the message. */
const UNIT_VARIANT_TEXT: Record<string, string> = {
  MessageGone: 'The message no longer exists on the server.',
}

export function formatError(cause: unknown): string {
  if (cause == null) return ''
  if (typeof cause === 'string') return UNIT_VARIANT_TEXT[cause] ?? cause
  if (cause instanceof Error) return cause.message

  if (typeof cause === 'object') {
    const entries = Object.entries(cause as Record<string, unknown>)
    // The tagged-enum shape: exactly one key, the variant name.
    if (entries.length === 1) {
      const [variant, payload] = entries[0]
      if (typeof payload === 'string' && payload.trim()) return payload
      return UNIT_VARIANT_TEXT[variant] ?? variant
    }
    // Some plugin errors arrive as `{ message: "…" }`.
    const message = (cause as { message?: unknown }).message
    if (typeof message === 'string') return message
  }

  return String(cause)
}

/**
 * Does this failure look like "the server's certificate isn't
 * trusted" rather than a wrong password or an unreachable host?
 *
 * Matching on the message text is crude, but the alternative — a
 * typed error variant threaded through the shared command layer —
 * is a change to code the desktop app also ships, for one screen's
 * benefit. Being wrong here is cheap in both directions: a false
 * positive shows a certificate the user can decline, a false
 * negative shows the raw error.
 */
export function isCertificateError(cause: unknown): boolean {
  const text = formatError(cause).toLowerCase()
  return (
    text.includes('certificate') ||
    text.includes('unknownissuer') ||
    text.includes('unknown issuer') ||
    text.includes('invalidcertificate') ||
    text.includes('self-signed') ||
    text.includes('selfsigned') ||
    text.includes('bad certificate') ||
    text.includes('tls handshake') ||
    text.includes('invalid peer certificate')
  )
}
