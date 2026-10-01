/**
 * The clouds a user can connect calendars and contacts from.
 *
 * # Two routes, chosen per provider — and why
 *
 * **Through the iPhone** — iCloud, Google, Microsoft, Yahoo. iOS already
 * syncs these accounts into the device's own calendar and contact
 * databases, and the app reads those through the device source
 * (`unkai-system`, EventKit + Contacts). It is the only route that
 * works for Google and Microsoft at all: Google's CalDAV/CardDAV need
 * OAuth with an app registered and verified at Google, and Microsoft
 * offers no CalDAV/CardDAV, only its Graph API. It is also the route
 * the app has actually been verified on (see CLAUDE.md → Project
 * Status). One device source covers every account on the phone.
 *
 * **Directly over CalDAV/CardDAV** — providers that speak the standard
 * with a normal (or app-specific) password. These presets only fill in
 * the server the generic DAV form would otherwise ask for; discovery is
 * the same RFC 6764 ladder (`/.well-known`, principal, home-set) used
 * for any server. Only providers whose DAV addresses are published and
 * stable are listed, and nothing here invents a URL: a provider whose
 * address we could not state with confidence is simply not offered.
 *
 * Some providers serve calendars and contacts from different hosts
 * (Fastmail). The backend stores one server per source, so those become
 * two sources with the same credentials — see `planDavSources`.
 *
 * Pure data and pure functions: no DOM, no IPC, unit-tested in node.
 * Display strings live in the message catalogue; this module holds
 * keys and facts only.
 */

import { m } from '../paraglide/messages'

export type ProviderRoute = 'device' | 'nextcloud' | 'dav' | 'local'

export interface CloudProvider {
  id: string
  /** Brand names are proper nouns and are not translated. */
  name: string
  route: ProviderRoute
  /** Two-letter monogram for the tile. No logos: they are trademarks,
   *  and a bundled image of one is a licensing question, not a UI one. */
  monogram: string
  /** Tile colour, a hue in degrees; lightness comes from the theme. */
  hue: number
  /** One line under the name. */
  detail: () => string
  /** DAV route only: where calendars and contacts live. `null` means
   *  the user types their own server (self-hosted). */
  calendarServer?: string | null
  contactsServer?: string | null
  /** DAV route: what the username is, in the provider's words. */
  usernameHint?: () => string
  /** DAV route: what kind of password the provider expects. */
  passwordHint?: () => string
}

export const CLOUD_PROVIDERS: readonly CloudProvider[] = [
  // ── Through the iPhone ────────────────────────────────────────────
  {
    id: 'icloud',
    name: 'iCloud',
    route: 'device',
    monogram: 'iC',
    hue: 210,
    detail: () => m.mobile_cloud_detail_device(),
  },
  {
    id: 'google',
    name: 'Google',
    route: 'device',
    monogram: 'G',
    hue: 140,
    detail: () => m.mobile_cloud_detail_device(),
  },
  {
    id: 'microsoft',
    name: 'Microsoft 365 / Outlook',
    route: 'device',
    monogram: 'MS',
    hue: 200,
    detail: () => m.mobile_cloud_detail_device(),
  },
  {
    id: 'yahoo',
    name: 'Yahoo',
    route: 'device',
    monogram: 'Y!',
    hue: 280,
    detail: () => m.mobile_cloud_detail_device(),
  },

  // ── Directly ──────────────────────────────────────────────────────
  {
    id: 'nextcloud',
    name: 'Nextcloud',
    route: 'nextcloud',
    monogram: 'NC',
    hue: 205,
    detail: () => m.mobile_cloud_detail_nextcloud(),
  },
  {
    id: 'mailbox-org',
    name: 'mailbox.org',
    route: 'dav',
    monogram: 'mb',
    hue: 180,
    detail: () => m.mobile_cloud_detail_dav(),
    calendarServer: 'https://dav.mailbox.org',
    contactsServer: 'https://dav.mailbox.org',
    usernameHint: () => m.mobile_cloud_user_email({ provider: 'mailbox.org' }),
    passwordHint: () => m.mobile_cloud_password_account(),
  },
  {
    id: 'posteo',
    name: 'Posteo',
    route: 'dav',
    monogram: 'Po',
    hue: 110,
    detail: () => m.mobile_cloud_detail_dav(),
    calendarServer: 'https://posteo.de:8443',
    contactsServer: 'https://posteo.de:8443',
    usernameHint: () => m.mobile_cloud_user_email({ provider: 'Posteo' }),
    passwordHint: () => m.mobile_cloud_password_account(),
  },
  {
    id: 'fastmail',
    name: 'Fastmail',
    route: 'dav',
    monogram: 'Fm',
    hue: 220,
    detail: () => m.mobile_cloud_detail_dav(),
    calendarServer: 'https://caldav.fastmail.com',
    contactsServer: 'https://carddav.fastmail.com',
    usernameHint: () => m.mobile_cloud_user_email({ provider: 'Fastmail' }),
    passwordHint: () => m.mobile_cloud_password_app({ provider: 'Fastmail' }),
  },
  {
    id: 'owncloud',
    name: 'ownCloud',
    route: 'dav',
    monogram: 'oC',
    hue: 230,
    detail: () => m.mobile_cloud_detail_selfhosted(),
    calendarServer: null,
    contactsServer: null,
    usernameHint: () => m.mobile_username(),
    passwordHint: () => m.mobile_cloud_password_app({ provider: 'ownCloud' }),
  },
  {
    id: 'other-dav',
    name: 'CalDAV / CardDAV',
    route: 'dav',
    monogram: 'DAV',
    hue: 30,
    detail: () => m.mobile_cloud_detail_other(),
    calendarServer: null,
    contactsServer: null,
    usernameHint: () => m.mobile_username(),
  },
  {
    id: 'local',
    // Not a brand: labelled through `providerLabel`, in the user's
    // language.
    name: '',
    route: 'local',
    monogram: '⌂',
    hue: 30,
    detail: () => m.mobile_source_add_local_hint(),
  },
]

/** The name to show for a provider. */
export function providerLabel(provider: CloudProvider): string {
  return provider.route === 'local' ? m.mobile_source_add_local() : provider.name
}

export function providerById(id: string): CloudProvider | undefined {
  return CLOUD_PROVIDERS.find((p) => p.id === id)
}

/** What the user entered in the DAV form. */
export interface DavInput {
  /** The server they typed; only used when the preset has none. */
  server: string
  username: string
  password: string
  useCalendars: boolean
  useContacts: boolean
  /** Optional name; the provider's name otherwise. */
  name: string
}

/** One `addDavAccount` call. */
export interface DavSourcePlan {
  displayName: string
  serverUrl: string
  username: string
  password: string
  useCalendars: boolean
  useContacts: boolean
  /** Which service this call covers, so a partial failure can say
   *  which half failed. */
  covers: 'both' | 'calendars' | 'contacts'
}

/** `dav.example.com` → `https://dav.example.com`; trims a trailing
 *  slash. The backend tolerates both, but the preview and the source's
 *  stored id should agree with what is sent. */
export function normaliseServer(raw: string): string {
  const trimmed = raw.trim().replace(/\/+$/, '')
  if (!trimmed) return ''
  return /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`
}

/**
 * Turn a preset plus the form into the backend calls to make.
 *
 * One call when calendars and contacts share a server (or only one of
 * them is wanted); two when a provider splits them across hosts. Each
 * call gets only the service its host serves — asking the CalDAV host
 * for contacts would fail discovery and take the calendar down with it,
 * since the backend resolves both before it stores anything.
 *
 * Returns `[]` when there is nothing valid to do (no service selected,
 * no server for a self-hosted preset, no username).
 */
export function planDavSources(provider: CloudProvider, input: DavInput): DavSourcePlan[] {
  if (provider.route !== 'dav') return []
  const username = input.username.trim()
  if (!username || (!input.useCalendars && !input.useContacts)) return []

  const typed = normaliseServer(input.server)
  const calendarServer = provider.calendarServer ?? typed
  const contactsServer = provider.contactsServer ?? typed
  const baseName = input.name.trim() || provider.name

  const common = { username, password: input.password }

  if (input.useCalendars && input.useContacts && calendarServer === contactsServer) {
    if (!calendarServer) return []
    return [
      {
        ...common,
        displayName: baseName,
        serverUrl: calendarServer,
        useCalendars: true,
        useContacts: true,
        covers: 'both',
      },
    ]
  }

  const plans: DavSourcePlan[] = []
  const split = input.useCalendars && input.useContacts
  if (input.useCalendars) {
    if (!calendarServer) return []
    plans.push({
      ...common,
      displayName: split ? `${baseName} · ${m.mobile_tab_calendar()}` : baseName,
      serverUrl: calendarServer,
      useCalendars: true,
      useContacts: false,
      covers: 'calendars',
    })
  }
  if (input.useContacts) {
    if (!contactsServer) return []
    plans.push({
      ...common,
      displayName: split ? `${baseName} · ${m.mobile_tab_contacts()}` : baseName,
      serverUrl: contactsServer,
      useCalendars: false,
      useContacts: true,
      covers: 'contacts',
    })
  }
  return plans
}
