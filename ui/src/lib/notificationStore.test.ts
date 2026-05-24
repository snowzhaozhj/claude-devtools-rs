import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

// vi.mock 必须在 import notificationStore.svelte 之前——用 hoisted factory 暴露
// 测试可控的 setBadgeCount spy 和"使其 throw"切换函数。整个 `getCurrentWindow()`
// chain 替代为 stub 避免 mockWindows 多实例化问题（每次调用返回新 Window 对象，
// 直接 spy 不命中实际被消费的实例）。
const { setBadgeCountSpy, makeSetBadgeCountThrow } = vi.hoisted(() => {
  const spy = vi.fn().mockResolvedValue(undefined)
  let shouldThrow = false
  return {
    setBadgeCountSpy: spy,
    makeSetBadgeCountThrow: (on: boolean) => {
      shouldThrow = on
      spy.mockImplementation((count?: number) => {
        if (shouldThrow) return Promise.reject(new Error('platform not supported'))
        // 记录入参——`toHaveBeenLastCalledWith` 直接读 mock.calls。
        return Promise.resolve(count)
      })
    },
  }
})

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ setBadgeCount: setBadgeCountSpy }),
}))

import { refreshUnreadCount } from './notificationStore.svelte'
import { getUnreadCount } from './tabStore.svelte'

beforeEach(() => {
  setBadgeCountSpy.mockClear()
  makeSetBadgeCountThrow(false)
})

afterEach(() => {
  clearMocks()
})

describe('notificationStore', () => {
  test('并发刷新复用同一次 getNotifications 请求，并在完成后补跑 dirty refresh', async () => {
    const calls: string[] = []
    const responses = [
      { notifications: [], total: 0, totalCount: 0, unreadCount: 7, hasMore: false },
      { notifications: [], total: 0, totalCount: 0, unreadCount: 8, hasMore: false },
    ]
    mockIPC(vi.fn((cmd) => {
      calls.push(cmd)
      if (cmd === 'get_notifications') return responses.shift()
      throw new Error(`unexpected command: ${cmd}`)
    }))

    const [first, second] = await Promise.all([
      refreshUnreadCount(),
      refreshUnreadCount(),
    ])
    await vi.waitFor(() => expect(getUnreadCount()).toBe(8))

    expect(first).toBe(7)
    expect(second).toBe(7)
    expect(calls).toEqual(['get_notifications', 'get_notifications'])
  })

  // issue #258：把 30 s setInterval 轮询换成 push event 主路径，前端 listener
  // wired 上的前提是 `refreshUnreadCount` 真的会同步写 Dock badge——即"有未读
  // 用 setBadgeCount(N)，无未读用 setBadgeCount(undefined) 清掉"。listener
  // wiring 本身在 `App.svelte::onMount` 已存在，本测试只锁住"trigger →
  // refreshUnreadCount → setBadgeCount" 这一段不会因后续重构断掉。
  test('refreshUnreadCount 同步 Dock badge：>0 时写 N，==0 时清掉', async () => {
    const responses = [
      { notifications: [], total: 0, totalCount: 0, unreadCount: 5, hasMore: false },
      { notifications: [], total: 0, totalCount: 0, unreadCount: 0, hasMore: false },
    ]
    mockIPC(vi.fn((cmd) => {
      if (cmd === 'get_notifications') return responses.shift()
      throw new Error(`unexpected command: ${cmd}`)
    }))

    await refreshUnreadCount()
    expect(setBadgeCountSpy).toHaveBeenLastCalledWith(5)

    await refreshUnreadCount()
    // unreadCount=0 → setBadgeCount(undefined) 清掉 macOS Dock badge
    expect(setBadgeCountSpy).toHaveBeenLastCalledWith(undefined)
  })

  // issue #258：非 macOS 平台 setBadgeCount throw（plugin 不支持），
  // refreshUnreadCount 必须 swallow 让后续 push event 主路径继续工作；
  // 否则首次 throw 会传染 promise，下一次 inflightUnreadRefresh 永停在 reject 态。
  test('setBadgeCount throw 时 refreshUnreadCount 仍返回 unreadCount 不抛错', async () => {
    makeSetBadgeCountThrow(true)
    mockIPC(vi.fn((cmd) => {
      if (cmd === 'get_notifications') {
        return { notifications: [], total: 0, totalCount: 0, unreadCount: 3, hasMore: false }
      }
      throw new Error(`unexpected command: ${cmd}`)
    }))

    const result = await refreshUnreadCount()
    expect(result).toBe(3)
    await vi.waitFor(() => expect(getUnreadCount()).toBe(3))
  })
})
