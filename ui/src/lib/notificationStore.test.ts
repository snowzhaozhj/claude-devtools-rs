import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, test, vi } from 'vitest'

import { refreshUnreadCount } from './notificationStore.svelte'
import { getUnreadCount } from './tabStore.svelte'

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
})
