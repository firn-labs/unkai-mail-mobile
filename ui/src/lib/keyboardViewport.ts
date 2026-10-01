/**
 * Keep the app inside the part of the screen the keyboard leaves free.
 *
 * The shell never scrolls (`html, body { overflow: hidden }`): every
 * screen is a bar plus its own scroller. But iOS knows nothing of that.
 * When a field takes focus it scrolls the *page* to bring the field
 * into view, and the whole shell slid up with it — the navigation bar
 * under the status bar and the Dynamic Island, the title cut in half,
 * Cancel out of reach. Every form had it; the account setup and the
 * composer most visibly.
 *
 * So the keyboard is treated as what it is, a part of the screen taken
 * away: its height is published as `--keyboard-inset` on `<html>`, the
 * shell and the full-screen layers end above it (`app.css`), the page's
 * own scroll is put back to zero, and the focused field is brought into
 * view inside its own scroller. `data-keyboard` on `<html>` lets the
 * tab bar step aside while typing, as it does in native apps.
 */

/** Below this the "keyboard" is just the input accessory bar of a
 *  hardware keyboard — still worth clearing, but not worth hiding the
 *  tab bar for. */
const KEYBOARD_MIN_PX = 120

export function installKeyboardViewport(): void {
  const vv = window.visualViewport
  if (!vv) return
  const root = document.documentElement
  let frame = 0

  const update = () => {
    frame = 0
    // The layout viewport does not shrink for the keyboard; the visual
    // one does. What lies between them at the bottom is the keyboard.
    const layout = Math.max(window.innerHeight, root.clientHeight)
    const inset = Math.max(0, Math.round(layout - vv.height - vv.offsetTop))
    root.style.setProperty('--keyboard-inset', `${inset}px`)
    root.toggleAttribute('data-keyboard', inset >= KEYBOARD_MIN_PX)
    if (window.scrollY !== 0 || window.scrollX !== 0) window.scrollTo(0, 0)
  }
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(update)
  }

  vv.addEventListener('resize', schedule)
  vv.addEventListener('scroll', schedule)
  window.addEventListener('scroll', schedule, { passive: true })

  document.addEventListener('focusin', (event) => {
    const field = event.target
    if (!(field instanceof HTMLElement) || !field.matches('input, textarea, select, [contenteditable]')) {
      return
    }
    schedule()
    // Once the keyboard is in and the layout has shrunk, show the field
    // inside its own scroller rather than letting the page move.
    setTimeout(() => {
      field.scrollIntoView({ block: 'nearest' })
      schedule()
    }, 320)
  })
}
