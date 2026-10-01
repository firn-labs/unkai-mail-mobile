/**
 * Rate-limiting helpers for work driven by backend pushes.
 *
 * The app's push channels are bursty by nature: one sync tick can
 * report new mail on three accounts, flip flags on two folders and
 * drain the outbox, all inside a single frame. Answering each of
 * those literally means a fan-out of IPC calls landing at once —
 * which on a phone is exactly the "the app froze for a second"
 * complaint, because the webview's main thread is also the one
 * painting.
 *
 * These two wrappers are the standard answers, kept here rather than
 * re-implemented per screen so the timing behaviour is consistent
 * (and testable without a DOM).
 */

/**
 * Collapse a burst of calls into one trailing run.
 *
 * Calling the returned function schedules `fn`; calling it again
 * before the delay elapses restarts the timer rather than queueing a
 * second run. The *trailing* edge is deliberate: these callbacks are
 * "re-read the current state", so the run that matters is the one
 * after the burst has settled — a leading-edge call would read a
 * half-applied state and then need a second pass anyway.
 *
 * `cancel()` drops a pending run, for component teardown.
 */
export function debounce<A extends unknown[]>(
  fn: (...args: A) => void,
  delayMs: number,
): ((...args: A) => void) & { cancel: () => void } {
  let timer: ReturnType<typeof setTimeout> | null = null

  const wrapped = (...args: A) => {
    if (timer !== null) clearTimeout(timer)
    timer = setTimeout(() => {
      timer = null
      fn(...args)
    }, delayMs)
  }

  wrapped.cancel = () => {
    if (timer !== null) clearTimeout(timer)
    timer = null
  }

  return wrapped
}

/**
 * Stop an async function from running twice at once.
 *
 * While a run is in flight, further calls return that same promise
 * instead of starting a second one. If any call arrives *during* a
 * run, exactly one more run is scheduled after it finishes — so a
 * request that came in against stale state is never silently dropped,
 * but ten of them still cost one extra pass rather than ten.
 *
 * This is what makes a refresh safe to trigger from several places at
 * once (a pull gesture, an empty-state button, a backend push) without
 * any of them having to know about the others. Without it, two
 * overlapping refreshes race to assign the list and the slower one
 * wins — which is how a refresh can appear to "undo" itself.
 *
 * Rejections propagate to every caller waiting on that run, and leave
 * the gate closed-and-reset so the next call starts cleanly.
 */
export function singleFlight<T>(fn: () => Promise<T>): () => Promise<T> {
  let inFlight: Promise<T> | null = null
  let rerunRequested = false

  const run = async (): Promise<T> => {
    try {
      const result = await fn()
      if (rerunRequested) {
        rerunRequested = false
        return await run()
      }
      return result
    } finally {
      // Only the outermost run clears the gate; a re-run above
      // resolves through this same promise.
      inFlight = null
      rerunRequested = false
    }
  }

  return () => {
    if (inFlight) {
      rerunRequested = true
      return inFlight
    }
    inFlight = run()
    return inFlight
  }
}

/**
 * Race a promise against a deadline.
 *
 * The UI-side counterpart to the socket deadlines in
 * `crates/unkai-imap/src/timeout.rs`: those cover the mail protocols,
 * but a command can also stall in a layer that has none (a DAV
 * request, a wedged IPC bridge after the app is resumed). Anything a
 * spinner is blocked on gets one of these, so a stuck call surfaces
 * as an error the user can retry instead of a control that never
 * comes back.
 *
 * Rejects with `reason` on expiry. The underlying promise is *not*
 * cancelled — nothing in the IPC layer can be — it is simply no
 * longer awaited, so a late answer resolves into nothing.
 */
export function withTimeout<T>(
  promise: Promise<T>,
  ms: number,
  reason = 'The request took too long. Check your connection and try again.',
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(reason)), ms)
    promise.then(
      (value) => {
        clearTimeout(timer)
        resolve(value)
      },
      (error) => {
        clearTimeout(timer)
        reject(error)
      },
    )
  })
}
