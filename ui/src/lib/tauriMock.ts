// dev/test 环境下用 Tauri 官方 mockIPC 注入假后端，让浏览器 / vitest /
// Playwright 能在没有真 Tauri runtime 的情况下跑通主路径。
//
// 触发条件由 main.ts 决定：仅当 import.meta.env.DEV && (URL ?mock=1 ||
// !window.__TAURI_INTERNALS__) 时调用 setupMockIPC()，真桌面窗口完全旁路。
//
// 契约见 openspec/specs/frontend-test-pyramid/spec.md。

import type { InvokeArgs } from '@tauri-apps/api/core'
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'

import { selectFixture, type Fixture } from './__fixtures__'

type ArgsRecord = Record<string, unknown> | undefined

/** 当前 mock 持有的 fixture 名（用于 console 提示）。 */
let activeFixtureName: string | null = null

/**
 * 已知 Tauri command 列表。MUST 与 src-tauri/src/lib.rs::invoke_handler!
 * 列表 + crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS 同步。
 *
 * 任何不在此列表的 command 调用 SHALL 走 unknownCommand 兜底。
 */
const KNOWN_TAURI_COMMANDS: readonly string[] = [
  'list_projects',
  'list_sessions',
  'get_session_detail',
  'get_subagent_trace',
  'get_image_asset',
  'get_tool_output',
  'search_sessions',
  'get_config',
  'update_config',
  'get_notifications',
  'mark_notification_read',
  'delete_notification',
  'mark_all_notifications_read',
  'clear_notifications',
  'add_trigger',
  'remove_trigger',
  'read_agent_configs',
  'pin_session',
  'unpin_session',
  'hide_session',
  'unhide_session',
  'get_project_session_prefs',
] as const

export { KNOWN_TAURI_COMMANDS }

class UnknownCommandError extends Error {
  constructor(cmd: string) {
    super(`[mockIPC] command "${cmd}" not implemented`)
    this.name = 'UnknownCommandError'
  }
}

function getArg<T>(payload: ArgsRecord, key: string): T | undefined {
  if (!payload) return undefined
  return payload[key] as T | undefined
}

function buildHandler(fx: Fixture) {
  return (cmd: string, rawPayload?: InvokeArgs): unknown => {
    const payload: ArgsRecord =
      rawPayload && typeof rawPayload === 'object' && !Array.isArray(rawPayload)
        ? (rawPayload as Record<string, unknown>)
        : undefined
    switch (cmd) {
      case 'list_projects':
        return fx.projects

      case 'list_sessions': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        const items = fx.sessions[projectId] ?? []
        return { items, nextCursor: null, total: items.length }
      }

      case 'search_sessions':
        return { results: fx.searchResults }

      case 'get_session_detail': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        const sessionId = getArg<string>(payload, 'sessionId') ?? ''
        const detail = fx.sessionDetails[`${projectId}:${sessionId}`]
        if (!detail) {
          return Promise.reject(
            new Error(`[mockIPC] no SessionDetail fixture for ${projectId}:${sessionId}`),
          )
        }
        return detail
      }

      case 'get_subagent_trace':
        return []

      case 'get_image_asset':
        return ''

      case 'get_tool_output':
        return { kind: 'missing' }

      case 'get_config':
        return fx.config

      case 'update_config': {
        const section = getArg<string>(payload, 'section')
        const data = getArg<Record<string, unknown>>(payload, 'configData')
        if (section === 'notifications' && data) {
          fx.config.notifications = { ...fx.config.notifications, ...(data as object) }
        } else if (section === 'general' && data) {
          fx.config.general = { ...fx.config.general, ...(data as object) }
        }
        return fx.config
      }

      case 'get_notifications':
        return fx.notifications

      case 'mark_notification_read': {
        const id = getArg<string>(payload, 'notificationId')
        const n = fx.notifications.notifications.find((x) => x.id === id)
        if (!n) return false
        if (!n.isRead) {
          n.isRead = true
          fx.notifications.unreadCount = Math.max(0, fx.notifications.unreadCount - 1)
        }
        return true
      }

      case 'delete_notification': {
        const id = getArg<string>(payload, 'notificationId')
        const before = fx.notifications.notifications.length
        fx.notifications.notifications = fx.notifications.notifications.filter(
          (x) => x.id !== id,
        )
        const removed = before - fx.notifications.notifications.length
        if (removed > 0) {
          fx.notifications.totalCount = fx.notifications.notifications.length
          fx.notifications.total = fx.notifications.totalCount
          fx.notifications.unreadCount = fx.notifications.notifications.filter(
            (x) => !x.isRead,
          ).length
        }
        return removed > 0
      }

      case 'mark_all_notifications_read': {
        for (const n of fx.notifications.notifications) n.isRead = true
        fx.notifications.unreadCount = 0
        return null
      }

      case 'clear_notifications': {
        const triggerId = getArg<string | null>(payload, 'triggerId')
        const before = fx.notifications.notifications.length
        if (triggerId) {
          fx.notifications.notifications = fx.notifications.notifications.filter(
            (x) => x.triggerName !== triggerId,
          )
        } else {
          fx.notifications.notifications = []
        }
        const removed = before - fx.notifications.notifications.length
        fx.notifications.totalCount = fx.notifications.notifications.length
        fx.notifications.total = fx.notifications.totalCount
        fx.notifications.unreadCount = fx.notifications.notifications.filter(
          (x) => !x.isRead,
        ).length
        return removed
      }

      case 'add_trigger': {
        const trigger = getArg<Record<string, unknown>>(payload, 'trigger')
        if (trigger) {
          // 简化 mock：直接 push 到 config.notifications.triggers
          fx.config.notifications.triggers.push({
            id: String(trigger.id ?? `mock-trig-${Date.now()}`),
            name: String(trigger.name ?? ''),
            enabled: Boolean(trigger.enabled),
            contentType: String(trigger.contentType ?? 'tool_result'),
            mode: String(trigger.mode ?? 'error_status'),
            color: trigger.color as string | undefined,
          })
        }
        return fx.config
      }

      case 'remove_trigger': {
        const id = getArg<string>(payload, 'triggerId')
        fx.config.notifications.triggers = fx.config.notifications.triggers.filter(
          (t) => t.id !== id,
        )
        return fx.config
      }

      case 'read_agent_configs':
        return fx.agentConfigs

      case 'get_project_session_prefs': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        return fx.prefs[projectId] ?? { pinned: [], hidden: [] }
      }

      case 'pin_session': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        const sessionId = getArg<string>(payload, 'sessionId') ?? ''
        const prefs = fx.prefs[projectId] ?? { pinned: [], hidden: [] }
        if (!prefs.pinned.includes(sessionId)) prefs.pinned.unshift(sessionId)
        fx.prefs[projectId] = prefs
        return null
      }

      case 'unpin_session': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        const sessionId = getArg<string>(payload, 'sessionId') ?? ''
        const prefs = fx.prefs[projectId]
        if (prefs) prefs.pinned = prefs.pinned.filter((s) => s !== sessionId)
        return null
      }

      case 'hide_session': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        const sessionId = getArg<string>(payload, 'sessionId') ?? ''
        const prefs = fx.prefs[projectId] ?? { pinned: [], hidden: [] }
        if (!prefs.hidden.includes(sessionId)) prefs.hidden.unshift(sessionId)
        fx.prefs[projectId] = prefs
        return null
      }

      case 'unhide_session': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        const sessionId = getArg<string>(payload, 'sessionId') ?? ''
        const prefs = fx.prefs[projectId]
        if (prefs) prefs.hidden = prefs.hidden.filter((s) => s !== sessionId)
        return null
      }

      default:
        // 兜底：未实现的 Tauri command。Tauri 内部 plugin 命令（plugin:event|*
        // 等）由 mockIPC 自身的 shouldMockEvents 处理，不会走到这里。
        if (cmd.startsWith('plugin:')) {
          // mockIPC 内部已处理，但 shouldMockEvents 关掉时会落到这里——
          // 返回空避免 listen() 抛错
          return undefined
        }
        console.warn(new UnknownCommandError(cmd).message)
        return Promise.reject(new UnknownCommandError(cmd))
    }
  }
}

/**
 * 注入 mockIPC 与 mockWindows。MUST 在 mount(App) 之前调用。
 *
 * 多次调用安全：每次都 clearMocks 后重新注入；fixture 切换走这条路径。
 */
export function setupMockIPC(fixtureName?: string | null): void {
  const fx = selectFixture(fixtureName)
  activeFixtureName = fx.name

  // mockWindows 必须在 mockIPC 之前 / 同时——否则 getCurrentWindow() 等会失败
  mockWindows('main')
  // shouldMockEvents: true 让 listen / emit 走 mock 链，不抛 transformCallback 错
  mockIPC(buildHandler(fx), { shouldMockEvents: true })

  console.info(
    `[mockIPC] setup with fixture "${fx.name}" — ` +
      `${fx.projects.length} projects, ` +
      `${Object.values(fx.sessions).reduce((acc, s) => acc + s.length, 0)} sessions`,
  )
}

export function getActiveFixtureName(): string | null {
  return activeFixtureName
}
