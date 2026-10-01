import { describe, expect, it } from 'vitest'
import {
  CLOUD_PROVIDERS,
  normaliseServer,
  planDavSources,
  providerById,
  type DavInput,
} from './cloudProviders'

const input = (over: Partial<DavInput> = {}): DavInput => ({
  server: '',
  username: 'alex@example.com',
  password: 'secret',
  useCalendars: true,
  useContacts: true,
  name: '',
  ...over,
})

describe('the catalogue', () => {
  it('has unique ids', () => {
    const ids = CLOUD_PROVIDERS.map((p) => p.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('gives every fixed DAV preset an https server for both services', () => {
    for (const p of CLOUD_PROVIDERS.filter((p) => p.route === 'dav')) {
      for (const server of [p.calendarServer, p.contactsServer]) {
        // null = the user types their own; otherwise it must be https.
        if (server !== null) expect(server, p.id).toMatch(/^https:\/\/[a-z0-9.-]+(:\d+)?$/)
      }
    }
  })

  it('routes the providers without standard DAV through the device', () => {
    // Google needs OAuth for CalDAV/CardDAV and Microsoft has none, so
    // offering them a password form would be a form that cannot work.
    for (const id of ['google', 'microsoft', 'icloud', 'yahoo']) {
      expect(providerById(id)?.route, id).toBe('device')
    }
  })
})

describe('normaliseServer', () => {
  it('adds https and drops trailing slashes', () => {
    expect(normaliseServer(' cloud.example.com/ ')).toBe('https://cloud.example.com')
    expect(normaliseServer('http://nas.local:5000//')).toBe('http://nas.local:5000')
    expect(normaliseServer('')).toBe('')
  })
})

describe('planDavSources', () => {
  it('makes one source when both services share a host', () => {
    const plans = planDavSources(providerById('posteo')!, input())
    expect(plans).toEqual([
      {
        displayName: 'Posteo',
        serverUrl: 'https://posteo.de:8443',
        username: 'alex@example.com',
        password: 'secret',
        useCalendars: true,
        useContacts: true,
        covers: 'both',
      },
    ])
  })

  it('splits a provider that serves calendars and contacts from different hosts', () => {
    const plans = planDavSources(providerById('fastmail')!, input())
    expect(plans.map((p) => [p.serverUrl, p.useCalendars, p.useContacts, p.covers])).toEqual([
      ['https://caldav.fastmail.com', true, false, 'calendars'],
      ['https://carddav.fastmail.com', false, true, 'contacts'],
    ])
    // Two sources, told apart by name.
    expect(plans[0].displayName).not.toBe(plans[1].displayName)
  })

  it('asks a split host only for the service that was chosen', () => {
    const plans = planDavSources(providerById('fastmail')!, input({ useCalendars: false }))
    expect(plans).toHaveLength(1)
    expect(plans[0]).toMatchObject({
      serverUrl: 'https://carddav.fastmail.com',
      useCalendars: false,
      useContacts: true,
      displayName: 'Fastmail',
    })
  })

  it('uses the typed server for a self-hosted preset, and refuses without one', () => {
    const owncloud = providerById('owncloud')!
    expect(planDavSources(owncloud, input({ server: 'cloud.example.com' }))[0].serverUrl).toBe(
      'https://cloud.example.com',
    )
    expect(planDavSources(owncloud, input({ server: '  ' }))).toEqual([])
  })

  it('refuses without a username or without any service', () => {
    const posteo = providerById('posteo')!
    expect(planDavSources(posteo, input({ username: ' ' }))).toEqual([])
    expect(planDavSources(posteo, input({ useCalendars: false, useContacts: false }))).toEqual([])
  })

  it('keeps a name the user chose', () => {
    expect(planDavSources(providerById('posteo')!, input({ name: ' Privat ' }))[0].displayName).toBe(
      'Privat',
    )
  })

  it('plans nothing for providers that are not reached over DAV', () => {
    expect(planDavSources(providerById('google')!, input())).toEqual([])
    expect(planDavSources(providerById('nextcloud')!, input())).toEqual([])
  })
})
