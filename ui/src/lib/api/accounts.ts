/**
 * Mail-account management: setup wizard discovery, connection tests,
 * certificate trust probing, and CRUD on the account list.
 *
 * Generated wrappers over the backend commands (#473) — one typed
 * function per `#[tauri::command]`. Argument keys mirror the Rust
 * parameter names (camelCased, as Tauri expects on the wire).
 */

import { call } from './core'
import type {
  Account,
  DiscoveredAccount,
  ProbedCert,
  ProviderPreset,
  TrustedCert,
} from './types'

export function addAccount(args: { account: Account; password: string }): Promise<void> {
  return call('add_account', args)
}

export function updateAccount(args: { account: Account }): Promise<void> {
  return call('update_account', args)
}

export function removeAccount(args: { id: string }): Promise<void> {
  return call('remove_account', args)
}

export function getAccounts(): Promise<Account[]> {
  return call('get_accounts')
}

export function setAccountPassword(args: { id: string; password: string }): Promise<void> {
  return call('set_account_password', args)
}

export function discoverAccountSettings(args: {
  email: string
}): Promise<DiscoveredAccount | null> {
  return call('discover_account_settings', args)
}

export function listProviderPresets(): Promise<ProviderPreset[]> {
  return call('list_provider_presets')
}

export function probeServerCertificate(args: { host: string; port: number }): Promise<ProbedCert> {
  return call('probe_server_certificate', args)
}

export function testConnection(args: {
  host: string
  port: number
  username: string
  password: string
  trustedCerts?: TrustedCert[] | null
}): Promise<string> {
  return call('test_connection', args)
}

export function testJmapConnection(args: {
  jmapUrl: string
  username: string
  password: string
}): Promise<string> {
  return call('test_jmap_connection', args)
}

export function detectJmap(args: { host: string }): Promise<string | null> {
  return call('detect_jmap', args)
}

/* ── Demo account ───────────────────────────────────────────────── */
//
// Seeds the local cache with a mail account, folders, messages,
// contacts and a calendar so the whole app is exercisable without a
// server. Backed by `unkai_commands::demo`; see that module for what
// works and what deliberately doesn't (sending is refused, not queued).

export interface DemoSummary {
  accountId: string
  messages: number
  contacts: number
  events: number
}

/**
 * Create — or re-create — the demo account and its data. `locale` is the
 * UI's language: the sample data is German for `de`, English otherwise.
 */
export function createDemoAccount(locale?: string): Promise<DemoSummary> {
  return call('create_demo_account', { locale })
}

/** Remove the demo account. Real accounts are untouched. */
export function removeDemoAccount(): Promise<void> {
  return call('remove_demo_account')
}

/** Whether the demo account is already present. */
export function demoAccountExists(): Promise<boolean> {
  return call('demo_account_exists')
}
