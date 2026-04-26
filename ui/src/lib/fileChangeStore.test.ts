// fileChangeStore.dedupeRefresh 行为契约测：同 key 并发合并 / 串行触发。

import { afterEach, describe, expect, test, vi } from 'vitest'

import { dedupeRefresh } from './fileChangeStore.svelte'

afterEach(() => {
  vi.useRealTimers()
})

function defer(): { promise: Promise<void>; resolve: () => void } {
  let resolve!: () => void
  const promise = new Promise<void>((r) => {
    resolve = r
  })
  return { promise, resolve }
}

describe('dedupeRefresh', () => {
  test('同 key 并发只跑一次 fn', async () => {
    const fn = vi.fn().mockResolvedValue(undefined)
    const p1 = dedupeRefresh('key-a', fn)
    const p2 = dedupeRefresh('key-a', fn)
    const p3 = dedupeRefresh('key-a', fn)
    await Promise.all([p1, p2, p3])
    expect(fn).toHaveBeenCalledTimes(1)
  })

  test('同 key 并发返回同一个 Promise', () => {
    const { promise, resolve } = defer()
    const fn = vi.fn(() => promise)
    const p1 = dedupeRefresh('key-b', fn)
    const p2 = dedupeRefresh('key-b', fn)
    expect(p1).toBe(p2)
    resolve()
  })

  test('不同 key 并发各跑一次', async () => {
    const fn = vi.fn().mockResolvedValue(undefined)
    await Promise.all([
      dedupeRefresh('key-c', fn),
      dedupeRefresh('key-d', fn),
      dedupeRefresh('key-e', fn),
    ])
    expect(fn).toHaveBeenCalledTimes(3)
  })

  test('resolve 后再调同 key 重新触发', async () => {
    const fn = vi.fn().mockResolvedValue(undefined)
    await dedupeRefresh('key-f', fn)
    await dedupeRefresh('key-f', fn)
    expect(fn).toHaveBeenCalledTimes(2)
  })

  test('fn reject 不影响 inFlight 清理', async () => {
    const fn = vi
      .fn()
      .mockRejectedValueOnce(new Error('boom'))
      .mockResolvedValueOnce(undefined)
    await expect(dedupeRefresh('key-g', fn)).rejects.toThrow('boom')
    // 第二次调用应该重新跑 fn（inFlight 已清理）
    await dedupeRefresh('key-g', fn)
    expect(fn).toHaveBeenCalledTimes(2)
  })
})
