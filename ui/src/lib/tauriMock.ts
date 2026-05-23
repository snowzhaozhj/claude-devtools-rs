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
  'get_session_summaries_by_ids',
  'get_session_detail',
  'get_project_memory',
  'read_memory_file',
  'add_memory',
  'delete_memory',
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
  'ssh_connect',
  'ssh_disconnect',
  'ssh_test_connection',
  'ssh_get_state',
  'ssh_get_config_hosts',
  'ssh_resolve_host',
  'ssh_save_last_connection',
  'ssh_get_last_connection',
  'list_contexts',
  'switch_context',
  'get_active_context',
  'pin_session',
  'unpin_session',
  'hide_session',
  'unhide_session',
  'get_project_session_prefs',
  'check_for_update',
  'is_running_under_rosetta',
  'list_repository_groups',
  'get_worktree_sessions',
  'list_group_sessions',
  'list_wsl_distros',
  'http_server_start',
  'http_server_stop',
  'http_server_status',
  'get_telemetry_snapshot',
  'record_correctness_events',
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
        const pageSize = getArg<number>(payload, 'pageSize') ?? 20
        const offset = Number.parseInt(getArg<string>(payload, 'cursor') ?? '0', 10) || 0
        const allItems = fx.sessions[projectId] ?? []
        const items = allItems.slice(offset, offset + pageSize)
        const nextOffset = offset + items.length
        const nextCursor = nextOffset < allItems.length ? String(nextOffset) : null
        return { items, nextCursor, total: allItems.length }
      }

      case 'get_session_summaries_by_ids': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        const sessionIds = getArg<string[]>(payload, 'sessionIds') ?? []
        const byId = new Map((fx.sessions[projectId] ?? []).map((s) => [s.sessionId, s]))
        return sessionIds.flatMap((id) => {
          const summary = byId.get(id)
          return summary ? [summary] : []
        })
      }

      case 'list_repository_groups': {
        if (fx.repositoryGroups) return fx.repositoryGroups
        // fallback：每个 project 退化为单成员 group。
        return fx.projects.map((p) => ({
          id: p.id,
          identity: null,
          name: p.displayName,
          worktrees: [
            {
              id: p.id,
              path: p.path,
              name: p.displayName,
              gitBranch: null,
              isMainWorktree: true,
              sessions: (fx.sessions[p.id] ?? []).map((s) => s.sessionId),
              createdAt: null,
              mostRecentSession:
                (fx.sessions[p.id] ?? []).reduce(
                  (m, s) => (s.timestamp > m ? s.timestamp : m),
                  0,
                ) || null,
            },
          ],
          mostRecentSession:
            (fx.sessions[p.id] ?? []).reduce(
              (m, s) => (s.timestamp > m ? s.timestamp : m),
              0,
            ) || null,
          totalSessions: p.sessionCount,
        }))
      }

      case 'list_group_sessions': {
        // change `simplify-repository-as-project::D3`：k-way merge cursor 分页。
        // 后端 cursor wire 形态：base64(JSON.stringify({ perWorktree: { <wt-id>:
        // { kind: 'not_started' | 'after_mtime' { mtimeMs, sid } | 'exhausted' } } }))
        // mock 解码 cursor 后按 per-worktree offset 计算可参与的 sessions →
        // 合并 → timestamp desc → 取 pageSize → 编码新 cursor。这样 worktree
        // filter / Exhausted / loadMore 三个核心 cursor 语义都能被 vitest /
        // playwright e2e 真实覆盖（codex 二审 round 3 测试覆盖洞）。
        //
        // 损坏 cursor 仍 fallback 视为首页（与后端 `parse_group_cursor` 对齐）。
        type WtOffset =
          | { kind: 'not_started' }
          | { kind: 'after_mtime'; mtimeMs: number; sid: string }
          | { kind: 'exhausted' }
        interface CursorWire {
          perWorktree: Record<string, WtOffset>
        }

        const decodeCursor = (raw: string | undefined): CursorWire | null => {
          if (!raw) return null
          try {
            const bin = typeof atob === 'function'
              ? atob(raw)
              : Buffer.from(raw, 'base64').toString('binary')
            const bytes = new Uint8Array(bin.length)
            for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
            const json = new TextDecoder().decode(bytes)
            const parsed = JSON.parse(json) as unknown
            if (
              parsed && typeof parsed === 'object' &&
              'perWorktree' in parsed &&
              parsed.perWorktree && typeof parsed.perWorktree === 'object'
            ) {
              return parsed as CursorWire
            }
            return null
          } catch {
            return null
          }
        }

        const groupId = getArg<string>(payload, 'groupId') ?? ''
        const pageSize = getArg<number>(payload, 'pageSize') ?? 20
        const rawCursor = getArg<string>(payload, 'cursor')
        const cursor = decodeCursor(rawCursor)

        const groups =
          fx.repositoryGroups ??
          fx.projects.map((p) => ({
            id: p.id,
            worktrees: [{ id: p.id, name: p.displayName }],
          }))
        const group = groups.find((g) => g.id === groupId)
        if (!group) {
          return { sessions: [], nextCursor: null }
        }

        // per-worktree 过滤：cursor 缺省视为 NotStarted（全量）。
        const wtOffsets: Record<string, WtOffset> = cursor?.perWorktree ?? {}
        const eligible = group.worktrees.flatMap((wt) => {
          const off = wtOffsets[wt.id] ?? { kind: 'not_started' }
          if (off.kind === 'exhausted') return []
          const all = (fx.sessions[wt.id] ?? [])
            .slice()
            .sort((a, b) => b.timestamp - a.timestamp)
          const cutoff =
            off.kind === 'after_mtime'
              ? all.findIndex(
                  (s) =>
                    s.timestamp < off.mtimeMs ||
                    (s.timestamp === off.mtimeMs && s.sessionId > off.sid),
                )
              : 0
          const start = cutoff < 0 ? all.length : cutoff
          return all.slice(start).map((s) => ({
            ...s,
            worktreeId: wt.id,
            worktreeName: wt.name,
            groupId,
          }))
        })
        eligible.sort((a, b) => b.timestamp - a.timestamp)
        const items = eligible.slice(0, pageSize)

        // 跨页 next cursor：每个 wt 按本页消费末尾的 (mtime, sid) 编 after_mtime；
        // exhausted 保持；本页未消费的 worktree 保持原 offset。
        const consumedByWt: Record<string, { mtimeMs: number; sid: string }> = {}
        for (const s of items) {
          consumedByWt[s.worktreeId] = { mtimeMs: s.timestamp, sid: s.sessionId }
        }
        const newPerWorktree: Record<string, WtOffset> = {}
        let allExhausted = true
        for (const wt of group.worktrees) {
          const prev = wtOffsets[wt.id] ?? { kind: 'not_started' }
          if (prev.kind === 'exhausted') {
            newPerWorktree[wt.id] = { kind: 'exhausted' }
            continue
          }
          const consumed = consumedByWt[wt.id]
          if (!consumed) {
            // 本页没消费过该 wt：保持原 offset（NotStarted 或 AfterMtime）。
            newPerWorktree[wt.id] = prev
            allExhausted = false
            continue
          }
          const all = (fx.sessions[wt.id] ?? [])
            .slice()
            .sort((a, b) => b.timestamp - a.timestamp)
          const idxAfter = all.findIndex(
            (s) =>
              s.timestamp < consumed.mtimeMs ||
              (s.timestamp === consumed.mtimeMs && s.sessionId > consumed.sid),
          )
          if (idxAfter < 0) {
            newPerWorktree[wt.id] = { kind: 'exhausted' }
          } else {
            newPerWorktree[wt.id] = {
              kind: 'after_mtime',
              mtimeMs: consumed.mtimeMs,
              sid: consumed.sid,
            }
            allExhausted = false
          }
        }

        const encodeCursor = (obj: CursorWire): string => {
          const json = JSON.stringify(obj)
          if (typeof btoa === 'function') {
            const bytes = new TextEncoder().encode(json)
            let bin = ''
            for (const b of bytes) bin += String.fromCharCode(b)
            return btoa(bin)
          }
          return Buffer.from(json, 'utf8').toString('base64')
        }
        const nextCursor = allExhausted
          ? null
          : encodeCursor({ perWorktree: newPerWorktree })
        return { sessions: items, nextCursor }
      }

      case 'get_worktree_sessions': {
        const groupId = getArg<string>(payload, 'groupId') ?? ''
        const groups =
          fx.repositoryGroups ??
          fx.projects.map((p) => ({
            id: p.id,
            identity: null,
            name: p.displayName,
            worktrees: [
              {
                id: p.id,
                path: p.path,
                name: p.displayName,
                gitBranch: null,
                isMainWorktree: true,
                sessions: [],
                createdAt: null,
                mostRecentSession: null,
              },
            ],
            mostRecentSession: null,
            totalSessions: p.sessionCount,
          }))
        const group = groups.find((g) => g.id === groupId)
        if (!group) {
          return Promise.reject(
            new Error(`[mockIPC] no RepositoryGroup fixture for ${groupId}`),
          )
        }
        // 合并所有 worktree 的 sessions 并按 timestamp 倒序。
        const merged = group.worktrees.flatMap((wt) =>
          (fx.sessions[wt.id] ?? []).map((s) => ({
            ...s,
            worktreeId: wt.id,
            worktreeName: wt.name,
          })),
        )
        merged.sort((a, b) => b.timestamp - a.timestamp)
        return { items: merged, nextCursor: null, total: merged.length }
      }

      case 'search_sessions': {
        const query = (getArg<string>(payload, 'query') ?? '').toLowerCase()
        const results = fx.searchResults
          .map((r) => {
            const summary = (fx.sessions[r.projectId] ?? []).find((s) => s.sessionId === r.sessionId)
            return {
              sessionId: r.sessionId,
              projectId: r.projectId,
              sessionTitle: summary?.title ?? r.sessionId,
              hits: [],
              totalMatches: r.matches,
            }
          })
          .filter((r) => r.sessionTitle.toLowerCase().includes(query) || r.sessionId.toLowerCase().includes(query))
        return {
          results,
          totalMatches: results.reduce((sum, r) => sum + r.totalMatches, 0),
          sessionsSearched: 0,
          query,
          isPartial: false,
        }
      }

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

      case 'get_project_memory': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        return fx.memories?.[projectId] ?? {
          projectId,
          hasMemory: false,
          count: 0,
          defaultFile: null,
          layers: [],
        }
      }

      case 'read_memory_file': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        const file = getArg<string>(payload, 'file') ?? ''
        const content = fx.memoryFiles?.[`${projectId}:${file}`]
        if (content === undefined) {
          return Promise.reject(
            new Error(`[mockIPC] no memory file fixture for ${projectId}:${file}`),
          )
        }
        return { projectId, file, filePath: `/mock/${projectId}/memory/${file}`, content }
      }

      case 'add_memory': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        const file = getArg<string>(payload, 'file') ?? ''
        const content = getArg<string>(payload, 'content') ?? ''
        // mock 直接更新 fixture 内存态，返新的 ProjectMemory
        if (!fx.memoryFiles) fx.memoryFiles = {}
        fx.memoryFiles[`${projectId}:${file}`] = content
        const existing = fx.memories?.[projectId] ?? {
          projectId,
          hasMemory: false,
          count: 0,
          defaultFile: null,
          layers: [],
        }
        const layers = [...existing.layers]
        if (!layers.some((l: { file: string }) => l.file === file)) {
          layers.push({ file, title: file, hook: null, kind: 'orphan' })
        }
        const updated = {
          projectId,
          hasMemory: true,
          count: layers.length,
          defaultFile: existing.defaultFile ?? layers[0]?.file ?? null,
          layers,
        }
        if (!fx.memories) fx.memories = {}
        fx.memories[projectId] = updated
        return updated
      }

      case 'delete_memory': {
        const projectId = getArg<string>(payload, 'projectId') ?? ''
        const file = getArg<string>(payload, 'file') ?? ''
        if (!fx.memoryFiles?.[`${projectId}:${file}`]) {
          return Promise.reject(new Error(`memory file ${file} not found`))
        }
        delete fx.memoryFiles[`${projectId}:${file}`]
        const existing = fx.memories?.[projectId]
        const layers = (existing?.layers ?? []).filter(
          (l: { file: string }) => l.file !== file,
        )
        const updated = {
          projectId,
          hasMemory: layers.length > 0,
          count: layers.length,
          defaultFile: layers[0]?.file ?? null,
          layers,
        }
        if (!fx.memories) fx.memories = {}
        fx.memories[projectId] = updated
        return updated
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
        } else if (section === 'ssh' && data) {
          fx.config.ssh = {
            ...(fx.config.ssh ?? { profiles: [], lastConnection: null, autoReconnect: false }),
            ...(data as object),
          }
        } else if (section === 'httpServer' && data) {
          fx.config.httpServer = { ...(fx.config.httpServer ?? { enabled: false, port: 3456 }), ...(data as object) }
        }
        return fx.config
      }

      // ---------------------------------------------------------------------
      // server-mode：模拟 ServerState 行为（详 ipc-data-api / server-mode spec）
      // ---------------------------------------------------------------------
      case 'http_server_start': {
        const port = getArg<number>(payload, 'port')
        if (port == null || port < 1024 || port > 65535) {
          throw new Error('port must be in 1024..=65535')
        }
        fx.mockHttpServer = { running: true, port, lastError: null }
        fx.config.httpServer = { enabled: true, port }
        return null
      }

      case 'http_server_stop': {
        if (fx.mockHttpServer) {
          fx.mockHttpServer.running = false
          fx.mockHttpServer.lastError = null
        } else {
          fx.mockHttpServer = { running: false, port: 3456, lastError: null }
        }
        if (fx.config.httpServer) {
          fx.config.httpServer.enabled = false
        } else {
          fx.config.httpServer = { enabled: false, port: 3456 }
        }
        return null
      }

      case 'http_server_status': {
        if (fx.mockHttpServer) return { ...fx.mockHttpServer }
        const persisted = fx.config.httpServer ?? { enabled: false, port: 3456 }
        return { running: false, port: persisted.port, lastError: null }
      }

      case 'get_telemetry_snapshot': {
        return {
          schemaVersion: 1,
          uptimeSecs: 42,
          capturedAt: Date.now(),
          counters: {
            'metadata.cache.hit': 1234,
            'metadata.cache.miss': 12,
            'panic.recovered': 0,
            'cdt_ssh.error': 0,
            'cdt_api.error': 0,
            'stale_update.triggered': 0,
          },
          histograms: {
            'ipc.list_sessions.duration_ns': {
              count: 100,
              buckets: Array.from({ length: 32 }, (_, i) => (i === 27 ? 100 : 0)),
              p50Ns: 1 << 28,
              p95Ns: 1 << 28,
              p99Ns: 1 << 28,
              maxBucket: 27,
            },
            'ipc.get_session_detail.duration_ns': {
              count: 50,
              buckets: Array.from({ length: 32 }, (_, i) => (i === 26 ? 50 : 0)),
              p50Ns: 1 << 27,
              p95Ns: 1 << 27,
              p99Ns: 1 << 27,
              maxBucket: 26,
            },
            'ipc.list_repository_groups.duration_ns': {
              count: 0,
              buckets: Array.from({ length: 32 }, () => 0),
              p50Ns: null,
              p95Ns: null,
              p99Ns: null,
              maxBucket: null,
            },
            'ipc.list_projects.duration_ns': {
              count: 0,
              buckets: Array.from({ length: 32 }, () => 0),
              p50Ns: null,
              p95Ns: null,
              p99Ns: null,
              maxBucket: null,
            },
          },
          recentEvents: [],
        }
      }

      case 'record_correctness_events': {
        return { ok: true }
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

      case 'ssh_connect': {
        const request = getArg<Record<string, unknown>>(payload, 'request') ?? {}
        return {
          contextId: String(request.contextId ?? request.host ?? 'mock-ssh'),
          status: 'connected',
          authChain: [],
        }
      }

      case 'ssh_disconnect':
        return null

      case 'ssh_test_connection': {
        const request = getArg<Record<string, unknown>>(payload, 'request') ?? {}
        return {
          contextId: `test-${String(request.host ?? 'mock-ssh')}`,
          status: 'connected',
          authChain: [{ source: { type: 'envAgent' }, outcome: { type: 'success' }, elapsedMs: 3 }],
        }
      }

      case 'ssh_get_state':
        return { activeContextId: 'local', contexts: [] }

      case 'ssh_get_config_hosts':
        return ['mock-prod', 'mock-staging', ...(fx.config.ssh?.profiles ?? []).map((profile) => profile.host)]

      case 'ssh_resolve_host': {
        const alias = getArg<string>(payload, 'alias') ?? ''
        return { host: alias, port: 22, user: null, identityFiles: [], identityAgent: null, degraded: true }
      }

      case 'ssh_save_last_connection': {
        const request = getArg<Record<string, unknown>>(payload, 'request') ?? {}
        const port = typeof request.port === 'number' ? request.port : null
        const username = typeof request.username === 'string' ? request.username : null
        const authMethod = request.authMethod === 'password' ? 'password' : 'sshConfig'
        const contextId = typeof request.contextId === 'string' ? request.contextId : null
        fx.config.ssh = fx.config.ssh ?? { profiles: [], lastConnection: null, autoReconnect: false }
        fx.config.ssh.lastConnection = {
          host: String(request.host ?? ''),
          port,
          username,
          authMethod,
          contextId,
        }
        return fx.config.ssh.lastConnection
      }

      case 'ssh_get_last_connection':
        return fx.config.ssh?.lastConnection ?? null

      case 'list_contexts':
        return [
          { id: 'local', kind: 'local', label: 'Local', status: 'connected', isActive: true, host: null },
          { id: 'ssh-mock-prod', kind: 'ssh', label: 'mock-prod', status: 'connected', isActive: false, host: 'mock-prod' },
        ]

      case 'switch_context':
        return null

      case 'get_active_context':
        return { id: 'local', kind: 'local', label: 'Local', status: 'connected', isActive: true, host: null }

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

      case 'is_running_under_rosetta': {
        // 浏览器 dev 模式默认不弹 banner；?rosetta=1 时模拟 Rosetta 翻译，
        // 便于在浏览器里调试 UI。
        const params = new URLSearchParams(window.location.search)
        return params.get('rosetta') === '1'
      }

      case 'list_wsl_distros': {
        // 浏览器调试：?wsl=single | multi | empty | distros-without-home
        const params = new URLSearchParams(window.location.search)
        const variant = params.get('wsl') ?? 'empty'
        switch (variant) {
          case 'single':
            return {
              candidates: [
                {
                  distro: 'Ubuntu',
                  homePath: '/home/alice',
                  claudeRootPath: '\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude',
                  claudeRootExists: true,
                },
              ],
              distrosWithoutHome: [],
            }
          case 'multi':
            return {
              candidates: [
                {
                  distro: 'Debian-12',
                  homePath: '/root',
                  claudeRootPath: '\\\\wsl.localhost\\Debian-12\\root\\.claude',
                  claudeRootExists: false,
                },
                {
                  distro: 'Ubuntu',
                  homePath: '/home/alice',
                  claudeRootPath: '\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude',
                  claudeRootExists: true,
                },
              ],
              distrosWithoutHome: [],
            }
          case 'distros-without-home':
            return {
              candidates: [],
              distrosWithoutHome: ['Ubuntu', 'Debian-12'],
            }
          case 'empty':
          default:
            return { candidates: [], distrosWithoutHome: [] }
        }
      }

      case 'check_for_update': {
        // Mock 模式不真正访问 GitHub Release endpoint。
        // URL `?mock=1&update=available` 切换为返回 Available fixture，
        // 默认返回 UpToDate。
        const params = new URLSearchParams(window.location.search)
        if (params.get('update') === 'available') {
          return {
            status: 'available',
            currentVersion: '0.2.0',
            newVersion: '0.3.0',
            notes: '## 0.3.0\n\n- Mock 模式新版本通知\n- 用于 UI 调试',
            signatureOk: true,
          }
        }
        return { status: 'up_to_date', currentVersion: '0.2.0' }
      }

      case 'plugin:opener|open_url': {
        // 浏览器 mockIPC 调试模式下，把外链路由到 window.open(_blank)。
        // 真 Tauri runtime 这条 IPC 由 tauri-plugin-opener 处理走系统浏览器。
        const url = (rawPayload as { url?: string } | undefined)?.url
        if (typeof url === 'string' && url.length > 0) {
          window.open(url, '_blank', 'noopener,noreferrer')
        }
        return undefined
      }

      case 'plugin:opener|open_path': {
        const path = (rawPayload as { path?: string } | undefined)?.path
        if (typeof path === 'string' && path.length > 0) {
          window.dispatchEvent(new CustomEvent('__cdtMockOpenPath', { detail: path }))
        }
        return undefined
      }

      case 'plugin:opener|reveal_item_in_dir': {
        const paths = (rawPayload as { paths?: string[] } | undefined)?.paths
        if (Array.isArray(paths) && paths.length > 0 && typeof paths[0] === 'string') {
          window.dispatchEvent(new CustomEvent('__cdtMockRevealPath', { detail: paths[0] }))
        }
        return undefined
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
export function setupMockIPC(fixtureName?: string | Fixture | null): void {
  const fx = typeof fixtureName === 'object' && fixtureName !== null
    ? fixtureName
    : selectFixture(fixtureName)
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
