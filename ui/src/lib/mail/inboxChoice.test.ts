import { describe, expect, it } from 'vitest'
import { resolveInboxAccount } from './inboxChoice'

describe('resolveInboxAccount', () => {
  it('shows the only account when there is one, whatever was chosen', () => {
    expect(resolveInboxAccount(['a'], null)).toBe('a')
    expect(resolveInboxAccount(['a'], 'gone')).toBe('a')
  })

  it('shows all accounts by default when there are several', () => {
    expect(resolveInboxAccount(['a', 'b'], null)).toBeNull()
  })

  it('shows the chosen account', () => {
    expect(resolveInboxAccount(['a', 'b'], 'b')).toBe('b')
  })

  it('falls back to all accounts when the chosen one was removed', () => {
    expect(resolveInboxAccount(['a', 'b'], 'c')).toBeNull()
  })
})
