import { describe, expect, it } from 'vitest'
import { INBOX, sameFolder, specialUse } from './folders'

describe('sameFolder', () => {
  it('ignores ASCII case, as the backend does', () => {
    expect(sameFolder('Archive', 'ARCHIVE')).toBe(true)
    expect(sameFolder('INBOX/Archiv', 'inbox/archiv')).toBe(true)
  })

  it('does not fold non-ASCII letters the backend would treat as different', () => {
    // Rust's eq_ignore_ascii_case says these differ; so must we.
    expect(sameFolder('ARCHIVÉ', 'archivé')).toBe(false)
  })

  it('never matches different names', () => {
    expect(sameFolder('Archive', 'Archives')).toBe(false)
    expect(sameFolder('Archive', 'INBOX')).toBe(false)
  })
})

describe('INBOX', () => {
  it('is what specialUse recognises as the inbox', () => {
    expect(specialUse({ name: INBOX })).toBe('inbox')
  })
})
