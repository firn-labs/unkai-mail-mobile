/**
 * In-app toasts: the mobile stand-in for the desktop status bar.
 *
 * A phone has no persistent chrome to put "Moved to Archive" or
 * "Couldn't reach the server" in, and a modal for every outcome
 * would be exhausting. Toasts sit above the tab bar, auto-dismiss,
 * and can carry one action — which is what makes destructive
 * gestures safe: Archive and Delete apply immediately and offer
 * Undo, instead of asking first.
 */

import { formatError } from './errors'
import { m } from '../paraglide/messages'

export type ToastKind = 'info' | 'success' | 'error'

export interface Toast {
  id: number
  kind: ToastKind
  text: string
  /** Optional single action, rendered as a button on the right. */
  action?: { label: string; run: () => void }
  /** Milliseconds on screen. Errors linger; confirmations don't. */
  timeout: number
}

let nextId = 1

/**
 * How many toasts may be on screen at once.
 *
 * There is room for about three above the tab bar, and a stack taller
 * than that stops being a status readout and becomes a wall. It is
 * also a safety limit: anything that fails per-item in a loop (a bulk
 * archive over a dead connection, a sync erroring per account) calls
 * `toasts.error` once per item, and an unbounded stack meant dozens
 * of animating, shadowed elements mounted at once — which is what
 * made in-app notifications lock the UI up rather than just look
 * untidy.
 *
 * Oldest are dropped first: the newest failure is the one still
 * relevant to what the user just did.
 */
const MAX_VISIBLE = 3

/**
 * Suppression window for an identical message.
 *
 * Repeats within this are folded into the toast already showing
 * rather than stacked behind it. The same loop that overflows the
 * stack usually produces the *same* sentence every time ("Couldn't
 * reach the server"), and telling someone that five times says
 * nothing the first one didn't.
 */
const DEDUPE_WINDOW_MS = 4000

class Toasts {
  list = $state<Toast[]>([])

  /** Live dismissal timers, so a toast evicted early doesn't leave one
   *  behind to fire against a recycled id. */
  #timers = new Map<number, ReturnType<typeof setTimeout>>()

  /** Last time each distinct message was shown, for `DEDUPE_WINDOW_MS`. */
  #lastShown = new Map<string, number>()

  show(text: string, opts: Partial<Omit<Toast, 'id' | 'text'>> = {}): number {
    const now = Date.now()

    // Fold a repeat into the toast that is already saying it. An
    // actionable toast is exempt: two "Undo" offers are two different
    // undos, and collapsing them would strand one of the actions.
    const previous = this.#lastShown.get(text)
    if (!opts.action && previous !== undefined && now - previous < DEDUPE_WINDOW_MS) {
      const showing = this.list.find((t) => t.text === text)
      if (showing) {
        this.#arm(showing.id, showing.timeout)
        return showing.id
      }
    }
    this.#lastShown.set(text, now)

    const id = nextId++
    const toast: Toast = {
      id,
      text,
      kind: opts.kind ?? 'info',
      action: opts.action,
      timeout: opts.timeout ?? (opts.kind === 'error' ? 6000 : opts.action ? 5000 : 2600),
    }

    const next = [...this.list, toast]
    // Evict from the front, cancelling each dropped toast's timer.
    while (next.length > MAX_VISIBLE) {
      const dropped = next.shift()
      if (dropped) this.#clear(dropped.id)
    }
    this.list = next

    this.#arm(id, toast.timeout)
    return id
  }

  /** (Re-)start a toast's dismissal countdown. */
  #arm(id: number, timeout: number): void {
    this.#clear(id)
    if (timeout > 0) {
      this.#timers.set(
        id,
        setTimeout(() => this.dismiss(id), timeout),
      )
    }
  }

  #clear(id: number): void {
    const timer = this.#timers.get(id)
    if (timer !== undefined) {
      clearTimeout(timer)
      this.#timers.delete(id)
    }
  }

  success(text: string): number {
    return this.show(text, { kind: 'success' })
  }

  /**
   * Surface a failure. Takes the raw thrown value so call sites can
   * hand over whatever they caught — `UnkaiError` arrives as a
   * string over IPC, but a JS `Error` can slip through too.
   */
  error(text: string, cause?: unknown): number {
    const detail = cause === undefined ? '' : formatError(cause)
    return this.show(detail ? `${text}: ${detail}` : text, { kind: 'error' })
  }

  /** Apply-now-with-Undo, the shape every destructive swipe uses. */
  undo(text: string, undo: () => void): number {
    return this.show(text, { action: { label: m.mobile_undo(), run: undo } })
  }

  /**
   * Stop a toast's countdown while the user is on it — a finger resting
   * on it, or keyboard / VoiceOver focus inside it. Pairs with
   * `release`. An Undo that times out while someone is reaching for it
   * is an Undo they did not get (WCAG 2.2.1).
   */
  hold(id: number): void {
    if (this.list.some((t) => t.id === id)) this.#clear(id)
  }

  /** Restart a held toast's full countdown. */
  release(id: number): void {
    const toast = this.list.find((t) => t.id === id)
    if (toast) this.#arm(id, toast.timeout)
  }

  dismiss(id: number): void {
    this.#clear(id)
    const next = this.list.filter((t) => t.id !== id)
    // Assigning an identical-length array would re-render the stack
    // for nothing — a double dismissal (the timer and a tap racing)
    // is routine.
    if (next.length !== this.list.length) this.list = next
  }
}

export const toasts = new Toasts()
