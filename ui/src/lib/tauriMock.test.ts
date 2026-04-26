// mockIPC 注入完整性测：每个 KNOWN_TAURI_COMMANDS 调用都 SHALL 不抛 undefined。
// 对应 spec frontend-test-pyramid §"mockIPC 必须覆盖所有 Tauri command 与 listen event"。

import { invoke } from '@tauri-apps/api/core'
import { emit, listen } from '@tauri-apps/api/event'
import { clearMocks } from '@tauri-apps/api/mocks'
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import { KNOWN_TAURI_COMMANDS, setupMockIPC } from './tauriMock'

const ARGS_BY_CMD: Record<string, Record<string, unknown>> = {
  list_sessions: { projectId: 'mock-rich-rust', pageSize: 10, cursor: null },
  search_sessions: { projectId: 'mock-rich-rust', query: 'foo' },
  get_session_detail: { projectId: 'mock-rich-rust', sessionId: 'sess-rust-active' },
  get_subagent_trace: { rootSessionId: 'r', subagentSessionId: 's' },
  get_image_asset: { rootSessionId: 'r', sessionId: 's', blockId: 'b' },
  get_tool_output: { rootSessionId: 'r', sessionId: 's', toolUseId: 'tu' },
  update_config: { section: 'notifications', configData: { enabled: false } },
  get_notifications: { limit: 50, offset: 0 },
  mark_notification_read: { notificationId: 'notif-1' },
  delete_notification: { notificationId: 'notif-x' },
  clear_notifications: { triggerId: null },
  add_trigger: {
    trigger: { id: 't1', name: 'n', enabled: true, contentType: 'tool_result', mode: 'error_status' },
  },
  remove_trigger: { triggerId: 't1' },
  pin_session: { projectId: 'mock-rich-rust', sessionId: 'sess-rust-2' },
  unpin_session: { projectId: 'mock-rich-rust', sessionId: 'sess-rust-active' },
  hide_session: { projectId: 'mock-rich-rust', sessionId: 'sess-rust-2' },
  unhide_session: { projectId: 'mock-rich-rust', sessionId: 'sess-rust-3' },
  get_project_session_prefs: { projectId: 'mock-rich-rust' },
}

beforeEach(() => {
  setupMockIPC('multi-project-rich')
})

afterEach(() => {
  clearMocks()
})

describe('mockIPC coverage', () => {
  test.each(KNOWN_TAURI_COMMANDS as readonly string[])(
    'command "%s" returns non-undefined or rejects clearly',
    async (cmd) => {
      const args = ARGS_BY_CMD[cmd] ?? {}
      const result = await invoke(cmd, args).catch((e) => ({ __mockError: String(e) }))
      // 不允许 undefined：要么有值，要么是显式错误
      expect(result).not.toBeUndefined()
    },
  )

  test('未知 command 走兜底 reject 含 not implemented', async () => {
    await expect(invoke('totally_made_up_command')).rejects.toThrow('not implemented')
  })

  test('listen() 不抛 transformCallback 错', async () => {
    const unlisten = await listen('notification-update', () => {})
    expect(typeof unlisten).toBe('function')
    unlisten()
  })

  test('listen() 4 个真实 event 都能挂载', async () => {
    const events = [
      'notification-update',
      'notification-added',
      'file-change',
      'session-metadata-update',
    ]
    for (const ev of events) {
      const u = await listen(ev, () => {})
      expect(typeof u).toBe('function')
      u()
    }
  })

  // P1.2 (codex review)：listen 不只是挂载，还要确认 emit 能触达 handler。
  // 这层守护的是 mockIPC 的 event 链路完整——前端 Sidebar / fileChangeStore /
  // App.svelte 的 listen 回调依赖此链路在 dev/test 下能跑通；UI 渲染由 Playwright 覆盖。
  describe('emit → listen handler 联调', () => {
    test.each([
      'notification-update',
      'notification-added',
      'file-change',
      'session-metadata-update',
    ] as const)('emit("%s") → handler 收到 payload', async (eventName) => {
      const handler = vi.fn()
      const unlisten = await listen(eventName, handler)
      const payload = { test: 'data', eventName }
      await emit(eventName, payload)
      // emit/listen 走 mock 链是同步的（mockIPC shouldMockEvents 内部 sync dispatch）
      expect(handler).toHaveBeenCalledTimes(1)
      const callArg = handler.mock.calls[0][0] as { event: string; payload: unknown }
      expect(callArg.event).toBe(eventName)
      expect(callArg.payload).toEqual(payload)
      unlisten()
    })

    test('unlisten 后 emit 不再触达', async () => {
      const handler = vi.fn()
      const unlisten = await listen('file-change', handler)
      unlisten()
      await emit('file-change', { sessionId: 's1' })
      expect(handler).not.toHaveBeenCalled()
    })
  })
})
