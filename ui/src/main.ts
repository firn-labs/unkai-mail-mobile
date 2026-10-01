// First: decides the language before anything formats a message.
import './lib/localeSetup'
import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { getLocale } from './paraglide/runtime'
import { installDynamicType } from './lib/dynamicType'
import { applyLastTheme } from './lib/theme'
import { installKeyboardViewport } from './lib/keyboardViewport'

// The document language is what VoiceOver picks its voice by. Left at
// `index.html`'s static "en", a German UI was read aloud by an English
// voice — every word mispronounced (WCAG 3.1.1). The locale comes from
// `lib/locale.ts`: the one picked in Settings, else the device's.
document.documentElement.lang = getLocale()

// Before mounting, so the first frame is already at the user's text size.
installDynamicType()

// …and in the user's theme, light or dark, rather than light until the
// settings have arrived.
applyLastTheme()

// The keyboard takes screen away instead of scrolling the shell.
installKeyboardViewport()

// One entry point, one window. (The desktop build mounts seven
// routes off a `?view=` query parameter — one per pop-out window —
// which a phone has no equivalent for: everything that was a
// separate window there is a screen or a sheet here.)
const app = mount(App, { target: document.getElementById('app')! })

export default app
