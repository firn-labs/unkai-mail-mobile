import { describe, expect, it, vi } from 'vitest'
import { debounce, singleFlight, withTimeout } from './schedule'

describe('debounce', () => {
  it('collapses a burst into one trailing run', async () => {
    vi.useFakeTimers()
    const run = vi.fn()
    const wrapped = debounce(run, 100)

    for (let i = 0; i < 40; i++) wrapped()
    expect(run).not.toHaveBeenCalled()

    vi.advanceTimersByTime(100)
    expect(run).toHaveBeenCalledTimes(1)
    vi.useRealTimers()
  })

  it('passes the arguments of the last call', () => {
    vi.useFakeTimers()
    const run = vi.fn()
    const wrapped = debounce(run, 50)

    wrapped('first')
    wrapped('last')
    vi.advanceTimersByTime(50)

    expect(run).toHaveBeenCalledExactlyOnceWith('last')
    vi.useRealTimers()
  })

  it('cancel drops a pending run', () => {
    vi.useFakeTimers()
    const run = vi.fn()
    const wrapped = debounce(run, 50)

    wrapped()
    wrapped.cancel()
    vi.advanceTimersByTime(500)

    expect(run).not.toHaveBeenCalled()
    vi.useRealTimers()
  })
})

describe('singleFlight', () => {
  it('does not start a second run while one is in flight', async () => {
    const inner = vi.fn(() => new Promise<number>(() => {}))
    const guarded = singleFlight(inner)

    const a = guarded()
    const b = guarded()

    expect(inner).toHaveBeenCalledTimes(1)
    // Both callers ride the same run rather than each getting one.
    expect(b).toBe(a)
  })

  it('schedules exactly one re-run however many calls arrive during a run', async () => {
    let pending: Array<(v: number) => void> = []
    const inner = vi.fn(() => new Promise<number>((r) => pending.push(r)))
    const guarded = singleFlight(inner)

    const first = guarded()
    for (let i = 0; i < 10; i++) guarded()
    expect(inner).toHaveBeenCalledTimes(1)

    pending.shift()!(1) // finish run 1 → triggers the single re-run
    await Promise.resolve()
    await Promise.resolve()
    expect(inner).toHaveBeenCalledTimes(2)

    pending.shift()!(2) // finish run 2 → nothing further was queued
    await first
    expect(inner).toHaveBeenCalledTimes(2)
  })

  it('reopens the gate after a rejection', async () => {
    const inner = vi
      .fn<() => Promise<string>>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce('ok')
    const guarded = singleFlight(inner)

    await expect(guarded()).rejects.toThrow('offline')
    await expect(guarded()).resolves.toBe('ok')
    expect(inner).toHaveBeenCalledTimes(2)
  })
})

describe('withTimeout', () => {
  it('passes a value through when it beats the deadline', async () => {
    await expect(withTimeout(Promise.resolve('fast'), 1000)).resolves.toBe('fast')
  })

  it('rejects when the promise never settles', async () => {
    vi.useFakeTimers()
    const stuck = new Promise<never>(() => {})
    const raced = withTimeout(stuck, 5000, 'gave up')
    const assertion = expect(raced).rejects.toThrow('gave up')
    await vi.advanceTimersByTimeAsync(5000)
    await assertion
    vi.useRealTimers()
  })

  it('propagates the original rejection rather than the timeout', async () => {
    await expect(withTimeout(Promise.reject(new Error('real')), 1000)).rejects.toThrow('real')
  })
})
