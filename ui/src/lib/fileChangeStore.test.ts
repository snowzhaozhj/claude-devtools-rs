// fileChangeStore.dedupeRefresh / scheduleRefresh 行为契约测：
// - dedupeRefresh：同 key 并发合并 / 串行触发
// - scheduleRefresh：leading + trailing 250ms 节流（高频 file-change 下合并）

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import {
  _resetScheduleRefreshForTest,
  dedupeRefresh,
  scheduleRefresh,
} from './fileChangeStore.svelte'

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

describe('scheduleRefresh', () => {
  beforeEach(() => {
    _resetScheduleRefreshForTest()
    vi.useFakeTimers()
  })

  test('窗口外首次调用立即触发（leading）', async () => {
    const fn = vi.fn().mockResolvedValue(undefined)
    scheduleRefresh('a', fn)
    // microtask 排干 dedupeRefresh 内 async IIFE
    await Promise.resolve()
    expect(fn).toHaveBeenCalledTimes(1)
  })

  test('窗口内多次调用合并为一次 trailing', async () => {
    const fn = vi.fn().mockResolvedValue(undefined)
    scheduleRefresh('b', fn)
    await Promise.resolve()
    expect(fn).toHaveBeenCalledTimes(1)

    // 紧接着的 5 次调用应该被 trailing 合并
    scheduleRefresh('b', fn)
    scheduleRefresh('b', fn)
    scheduleRefresh('b', fn)
    scheduleRefresh('b', fn)
    scheduleRefresh('b', fn)
    expect(fn).toHaveBeenCalledTimes(1)

    // 推进到 trailing timer 触发
    await vi.advanceTimersByTimeAsync(260)
    expect(fn).toHaveBeenCalledTimes(2)
  })

  test('trailing 跑的是最后一次传入的 fn（保留最新闭包）', async () => {
    const first = vi.fn().mockResolvedValue(undefined)
    const second = vi.fn().mockResolvedValue(undefined)
    const third = vi.fn().mockResolvedValue(undefined)

    scheduleRefresh('c', first)
    await Promise.resolve()
    expect(first).toHaveBeenCalledTimes(1)

    scheduleRefresh('c', second)
    scheduleRefresh('c', third)
    await vi.advanceTimersByTimeAsync(260)

    expect(second).not.toHaveBeenCalled()
    expect(third).toHaveBeenCalledTimes(1)
  })

  test('窗口结束后再调直接触发，无需 trailing', async () => {
    const fn = vi.fn().mockResolvedValue(undefined)
    scheduleRefresh('d', fn)
    await Promise.resolve()
    expect(fn).toHaveBeenCalledTimes(1)

    await vi.advanceTimersByTimeAsync(260)
    scheduleRefresh('d', fn)
    await Promise.resolve()
    expect(fn).toHaveBeenCalledTimes(2)
  })

  test('不同 key 各自独立节流', async () => {
    const fn = vi.fn().mockResolvedValue(undefined)
    scheduleRefresh('e', fn)
    scheduleRefresh('f', fn)
    scheduleRefresh('g', fn)
    await Promise.resolve()
    expect(fn).toHaveBeenCalledTimes(3)
  })
})
