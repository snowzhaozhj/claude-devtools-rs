import { afterEach, describe, expect, test, vi } from 'vitest'

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { listen } from '@tauri-apps/api/event'
import {
  BrowserUnsupportedError,
  __resetBrowserTransportForTests,
  getTransport,
  subscribeEvent,
} from './transport'

afterEach(() => {
  vi.restoreAllMocks()
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  __resetBrowserTransportForTests()
})

describe('BrowserTransport', () => {
  test('浏览器 runtime 下 listProjects 走 HTTP /api/projects', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify([{ id: 'p1', path: '/p1', displayName: 'p1', sessionCount: 1 }]), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    )

    const result = await getTransport().invoke('list_projects')

    expect(fetchMock).toHaveBeenCalledWith(`${window.location.origin}/api/projects`, {
      method: 'GET',
      headers: undefined,
      body: undefined,
    })
    expect(result).toEqual([{ id: 'p1', path: '/p1', displayName: 'p1', sessionCount: 1 }])
  })

  test('浏览器 runtime 下 lazy endpoint 映射到 HTTP 路由', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify('tool-output'), { status: 200 }),
    )

    await getTransport().invoke('get_tool_output', {
      rootSessionId: 'root/a',
      sessionId: 'sub b',
      toolUseId: 'tool:1',
    })

    expect(fetchMock).toHaveBeenCalledWith(
      `${window.location.origin}/api/sessions/root%2Fa/subagents/sub%20b/tools/tool%3A1/output`,
      { method: 'GET', headers: undefined, body: undefined },
    )
  })

  test('浏览器 runtime 下 updateConfig 使用 HTTP data 字段', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ httpServer: { enabled: false, port: 3500 } }), { status: 200 }),
    )

    await getTransport().invoke('update_config', {
      section: 'httpServer',
      configData: { port: 3500 },
    })

    expect(fetchMock).toHaveBeenCalledWith(`${window.location.origin}/api/config`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ section: 'httpServer', data: { port: 3500 } }),
    })
  })

  test('浏览器 runtime 下 SSH/context command 映射到 HTTP 路由', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockImplementation(async () =>
      new Response(JSON.stringify({ success: true }), { status: 200 }),
    )

    await getTransport().invoke('switch_context', { contextId: 'ctx-1' })
    await getTransport().invoke('ssh_disconnect', { contextId: 'ctx-1' })
    await getTransport().invoke('ssh_resolve_host', { alias: 'dev host' })

    expect(fetchMock).toHaveBeenNthCalledWith(1, `${window.location.origin}/api/contexts/switch`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ context_id: 'ctx-1' }),
    })
    expect(fetchMock).toHaveBeenNthCalledWith(2, `${window.location.origin}/api/ssh/disconnect`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ context_id: 'ctx-1' }),
    })
    expect(fetchMock).toHaveBeenNthCalledWith(3, `${window.location.origin}/api/ssh/resolve-host?alias=dev%20host`, {
      method: 'GET',
      headers: undefined,
      body: undefined,
    })
  })

  test('Tauri runtime 订阅 session-metadata-update 与 SSH/context 事件', async () => {
    ;(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
    await subscribeEvent('session-metadata-update', vi.fn())

    expect(vi.mocked(listen)).toHaveBeenCalledWith('session-metadata-update', expect.any(Function))
    expect(vi.mocked(listen)).toHaveBeenCalledWith('ssh_status', expect.any(Function))
    expect(vi.mocked(listen)).toHaveBeenCalledWith('context_changed', expect.any(Function))
  })

  test('浏览器 runtime 下桌面专属 command 抛 BrowserUnsupportedError', async () => {
    await expect(getTransport().invoke('check_for_update')).rejects.toBeInstanceOf(BrowserUnsupportedError)
  })

  test('SSE file_change 事件转换为 file-change payload', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })
    const handler = vi.fn()

    const unsubscribe = await subscribeEvent('file-change', handler)
    instances[0].emit({
      type: 'file_change',
      project_id: 'p1',
      session_id: 's1',
      deleted: false,
      project_list_changed: true,
    })

    expect(handler).toHaveBeenCalledWith({
      event: 'file-change',
      id: 0,
      payload: { projectId: 'p1', sessionId: 's1', deleted: false, projectListChanged: true },
    })
    unsubscribe()
    expect(instances[0].closed).toBe(true)
  })

  test('SSE session_metadata_update 事件转换为 Sidebar payload', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })
    const handler = vi.fn()

    const unsubscribe = await subscribeEvent('session-metadata-update', handler)
    instances[0].emit({
      type: 'session_metadata_update',
      project_id: 'p1',
      session_id: 's1',
      title: 'Hello',
      message_count: 12,
      is_ongoing: true,
      git_branch: 'main',
    })

    expect(handler).toHaveBeenCalledWith({
      event: 'session-metadata-update',
      id: 0,
      payload: {
        projectId: 'p1',
        sessionId: 's1',
        title: 'Hello',
        messageCount: 12,
        isOngoing: true,
        gitBranch: 'main',
      },
    })
    unsubscribe()
  })

  test('多次 subscribeEvent 共享同一条 SSE 连接', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })

    const u1 = await subscribeEvent('file-change', vi.fn())
    const u2 = await subscribeEvent('session-metadata-update', vi.fn())
    const u3 = await subscribeEvent('notification-update', vi.fn())

    expect(instances).toHaveLength(1)
    expect(instances[0].closed).toBe(false)

    u1(); u2(); u3()
  })

  test('单条 SSE fan-out 到不同事件名的多个 handler', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })

    const fileHandler = vi.fn()
    const sessionHandler = vi.fn()
    const u1 = await subscribeEvent('file-change', fileHandler)
    const u2 = await subscribeEvent('session-metadata-update', sessionHandler)

    instances[0].emit({ type: 'file_change', project_id: 'p1', session_id: 's1', deleted: false, project_list_changed: false })
    instances[0].emit({
      type: 'session_metadata_update', project_id: 'p1', session_id: 's1',
      title: 'T', message_count: 1, is_ongoing: false, git_branch: null,
    })

    expect(fileHandler).toHaveBeenCalledTimes(1)
    expect(sessionHandler).toHaveBeenCalledTimes(1)

    u1(); u2()
  })

  test('部分 unsubscribe 保持连接，全部 unsubscribe 后才关', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })

    const u1 = await subscribeEvent('file-change', vi.fn())
    const u2 = await subscribeEvent('session-metadata-update', vi.fn())
    expect(instances[0].closed).toBe(false)

    u1()
    expect(instances[0].closed).toBe(false)

    u2()
    expect(instances[0].closed).toBe(true)
  })

  test('全部 unsubscribe 后再 subscribe 建新连接', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })

    const u1 = await subscribeEvent('file-change', vi.fn())
    u1()
    expect(instances).toHaveLength(1)
    expect(instances[0].closed).toBe(true)

    const u2 = await subscribeEvent('file-change', vi.fn())
    expect(instances).toHaveLength(2)
    expect(instances[1].closed).toBe(false)

    u2()
  })

  test('SSE 进入 CLOSED 终态后延迟重建（server toggle off→on 场景）', async () => {
    vi.useFakeTimers()
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })

    const handler = vi.fn()
    const unsubscribe = await subscribeEvent('file-change', handler)
    expect(instances).toHaveLength(1)

    // 模拟 server toggle off → EventSource CLOSED 终态错误
    instances[0].triggerClosedError()

    // 延迟重建：1s 退避前 instances 还是 1
    expect(instances).toHaveLength(1)

    // 推进 1s + flush microtask 队列让 ensureSource 真跑
    await vi.advanceTimersByTimeAsync(1000)

    // 应建第二条 source 而不是永远丢事件
    expect(instances).toHaveLength(2)
    expect(instances[1].closed).toBe(false)

    // 第二条 source 应能正常分发
    instances[1].emit({ type: 'file_change', project_id: 'p1', session_id: 's1', deleted: false, project_list_changed: false })
    expect(handler).toHaveBeenCalledTimes(1)

    unsubscribe()
    vi.useRealTimers()
  })

  test('连续 CLOSED 错误触发指数回退（1s → 2s → 4s → ...）', async () => {
    vi.useFakeTimers()
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })

    const unsubscribe = await subscribeEvent('file-change', vi.fn())
    expect(instances).toHaveLength(1)

    // 第 1 次 CLOSED：1s 后重建
    instances[0].triggerClosedError()
    await vi.advanceTimersByTimeAsync(999)
    expect(instances).toHaveLength(1)
    await vi.advanceTimersByTimeAsync(1)
    expect(instances).toHaveLength(2)

    // 第 2 次 CLOSED：2s 后重建（指数 = 2^1 * 1000）
    instances[1].triggerClosedError()
    await vi.advanceTimersByTimeAsync(1999)
    expect(instances).toHaveLength(2)
    await vi.advanceTimersByTimeAsync(1)
    expect(instances).toHaveLength(3)

    // 第 3 次 CLOSED：4s 后重建
    instances[2].triggerClosedError()
    await vi.advanceTimersByTimeAsync(3999)
    expect(instances).toHaveLength(3)
    await vi.advanceTimersByTimeAsync(1)
    expect(instances).toHaveLength(4)

    unsubscribe()
    vi.useRealTimers()
  })

  test('onopen 重置指数回退步数', async () => {
    vi.useFakeTimers()
    const instances: (FakeEventSource & { triggerOpen: () => void })[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      onopen: (() => void) | null = null
      constructor(url: string) {
        super(url)
        instances.push(this as FakeEventSource & { triggerOpen: () => void })
      }
      triggerOpen() {
        this.readyState = FakeEventSource.OPEN
        this.onopen?.()
      }
    })

    const unsubscribe = await subscribeEvent('file-change', vi.fn())

    // 触发两次连续 CLOSED 把 attempt 推到 2（下一次会用 4s 退避）
    instances[0].triggerClosedError()
    await vi.advanceTimersByTimeAsync(1000)
    instances[1].triggerClosedError()
    await vi.advanceTimersByTimeAsync(2000)
    expect(instances).toHaveLength(3)

    // 第 3 条 source 成功打开 → 重置 attempt
    instances[2].triggerOpen()

    // 之后再 CLOSED 应该回到首次 1s（不是 8s）
    instances[2].triggerClosedError()
    await vi.advanceTimersByTimeAsync(999)
    expect(instances).toHaveLength(3)
    await vi.advanceTimersByTimeAsync(1)
    expect(instances).toHaveLength(4)

    unsubscribe()
    vi.useRealTimers()
  })

  test('handlers 全空时 CLOSED 不触发重建', async () => {
    vi.useFakeTimers()
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })

    const unsubscribe = await subscribeEvent('file-change', vi.fn())
    unsubscribe() // handlers.size = 0，source 被关
    expect(instances[0].closed).toBe(true)

    // CLOSED 错误不该触发 ghost 重连
    instances[0].triggerClosedError()
    await vi.advanceTimersByTimeAsync(2000)
    expect(instances).toHaveLength(1)

    vi.useRealTimers()
  })

  test('handler 内同步 unsubscribe 不影响当前事件分发到其他 handler', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })

    const order: string[] = []
    let u1: () => void = () => {}
    const h1 = vi.fn(() => {
      order.push('h1')
      u1() // 同步 unsubscribe 自己
    })
    const h2 = vi.fn(() => {
      order.push('h2')
    })

    u1 = await subscribeEvent('file-change', h1)
    const u2 = await subscribeEvent('file-change', h2)

    instances[0].emit({ type: 'file_change', project_id: 'p1', session_id: 's1', deleted: false, project_list_changed: false })

    // h1 同步取消自己，h2 仍应收到本次事件（快照迭代语义）
    expect(order).toEqual(['h1', 'h2'])

    u2()
  })

  test('重复调 unsubscribe 是幂等的，不会重复关 source', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })

    const unsubscribe = await subscribeEvent('file-change', vi.fn())
    unsubscribe()
    expect(instances[0].closed).toBe(true)

    // 第二次 unsubscribe 不应抛错，也不应造成异常副作用
    expect(() => unsubscribe()).not.toThrow()
    expect(instances).toHaveLength(1)
  })

  test('ensureSseReady：list_sessions 在 SSE OPEN 时不阻塞', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        this.readyState = FakeEventSource.OPEN
        instances.push(this)
      }
    })
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ items: [], nextCursor: null, total: 0 }), { status: 200 }),
    )

    const start = performance.now()
    await getTransport().invoke('list_sessions', { projectId: 'p1', pageSize: 20 })
    const elapsed = performance.now() - start

    // 应建一条 SSE 但本测仅校验 fetch 立即发出 + 无超时 console.warn
    expect(instances.length).toBeGreaterThanOrEqual(1)
    expect(fetchMock).toHaveBeenCalledWith(
      `${window.location.origin}/api/projects/p1/sessions?pageSize=20`,
      expect.objectContaining({ method: 'GET' }),
    )
    expect(elapsed).toBeLessThan(500) // OPEN 立返，不应有 50ms+ poll 延迟
  })

  test('ensureSseReady：cursor=Some 翻页时 CONNECTING 状态等待 onopen 后才发 fetch', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal(
      'EventSource',
      class extends FakeEventSource {
        constructor(url: string) {
          super(url)
          this.readyState = FakeEventSource.CONNECTING
          instances.push(this)
        }
      },
    )
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ items: [], nextCursor: null, total: 0 }), { status: 200 }),
    )

    // cursor=Some 走 await 闸门（翻页路径）
    const invokePromise = getTransport().invoke('list_sessions', { projectId: 'p2', pageSize: 20, cursor: '20' })

    // 给 ensureSseReady 跑一两轮 poll（≥ 50ms）让 source 创建
    await new Promise<void>((r) => setTimeout(r, 80))
    expect(instances.length).toBeGreaterThanOrEqual(1)
    // 此时 fetch 不应发出（SSE 还在 CONNECTING）
    expect(fetchMock).not.toHaveBeenCalled()

    // 模拟 SSE 进入 OPEN
    instances[0].readyState = FakeEventSource.OPEN

    await invokePromise
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })

  test('ensureSseReady：cursor=Some 翻页时 1000ms 超时后仍放行 fetch + console.warn', async () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.stubGlobal(
      'EventSource',
      class extends FakeEventSource {
        constructor(url: string) {
          super(url)
          // 永不进入 OPEN
          this.readyState = FakeEventSource.CONNECTING
        }
      },
    )
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ items: [], nextCursor: null, total: 0 }), { status: 200 }),
    )

    const start = performance.now()
    // cursor=Some 走 await 闸门：才会到 1000 ms 超时 + console.warn 路径
    await getTransport().invoke('list_sessions', { projectId: 'p3', pageSize: 20, cursor: '20' })
    const elapsed = performance.now() - start

    expect(elapsed).toBeGreaterThanOrEqual(950) // 等满 ~1000ms（含轮询粒度抖动）
    expect(elapsed).toBeLessThan(2000)
    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(warnSpy).toHaveBeenCalledWith(expect.stringContaining('SSE not OPEN within 1000ms'))
  })

  test('ensureSseReady：list_repository_groups / get_worktree_sessions 也走 SSE-ready 闸门', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        this.readyState = FakeEventSource.OPEN
        instances.push(this)
      }
    })
    // 每次返新 Response 实例，避免 "Body has already been read"
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () => new Response('[]', { status: 200 }))

    await getTransport().invoke('list_repository_groups')
    await getTransport().invoke('get_worktree_sessions', { groupId: 'g1', pageSize: 20 })

    expect(instances.length).toBeGreaterThanOrEqual(1) // ensureSource 已触发
  })

  test('ensureSseReady：cursor=Some 翻页 1000ms 超时后下一次 onopen 触发 sse-recovered 兜底事件', async () => {
    vi.useFakeTimers()
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const instances: (FakeEventSource & { triggerOpen: () => void })[] = []
    vi.stubGlobal(
      'EventSource',
      class extends FakeEventSource {
        onopen: (() => void) | null = null
        constructor(url: string) {
          super(url)
          this.readyState = FakeEventSource.CONNECTING
          instances.push(this as FakeEventSource & { triggerOpen: () => void })
        }
        triggerOpen() {
          this.readyState = FakeEventSource.OPEN
          this.onopen?.()
        }
      },
    )
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ items: [], nextCursor: null, total: 0 }), { status: 200 }),
    )

    const recoveredHandler = vi.fn()
    await subscribeEvent('sse-recovered', recoveredHandler)
    expect(instances.length).toBeGreaterThanOrEqual(1)

    // cursor=Some 翻页 → ensureSseReady 永等不到 OPEN → 1000 ms 超时放行 fetch
    const invokePromise = getTransport().invoke('list_sessions', { projectId: 'p-recover', pageSize: 20, cursor: '20' })
    await vi.advanceTimersByTimeAsync(1000)
    await invokePromise
    expect(warnSpy).toHaveBeenCalledWith(expect.stringContaining('SSE not OPEN within 1000ms'))
    expect(recoveredHandler).not.toHaveBeenCalled()

    // SSE 真正 OPEN → 触发 sse-recovered 兜底 event
    instances[0].triggerOpen()
    expect(recoveredHandler).toHaveBeenCalledTimes(1)
    expect(recoveredHandler).toHaveBeenCalledWith({
      event: 'sse-recovered',
      id: 0,
      payload: {},
    })

    // 再次 OPEN（重连场景）不应再 emit——flag 已被清空
    instances[0].triggerOpen()
    expect(recoveredHandler).toHaveBeenCalledTimes(1)

    vi.useRealTimers()
  })

  test('D9 + D9b：list_sessions(cursor=null) 不 await ensureSseReady 立即发 fetch', async () => {
    // 让 EventSource 永远 CONNECTING——cursor=null 路径 SHALL 仍立即发 fetch
    // 不被 SSE 状态阻塞（首页 eager 路径 inline 真值已含 metadata）
    vi.stubGlobal(
      'EventSource',
      class extends FakeEventSource {
        constructor(url: string) {
          super(url)
          this.readyState = FakeEventSource.CONNECTING
        }
      },
    )
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ items: [], nextCursor: null, total: 0 }), { status: 200 }),
    )

    const start = performance.now()
    await getTransport().invoke('list_sessions', { projectId: 'p-eager', pageSize: 20 })
    const elapsed = performance.now() - start

    // 不该等满 1000 ms 闸门——cursor=null 走 fire-and-forget
    expect(elapsed).toBeLessThan(300)
    expect(fetchMock).toHaveBeenCalledTimes(1)
  })

  test('D9 + D9b：list_sessions(cursor=null) fire-and-forget 仍触发 ensureSource 订阅 SSE', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal(
      'EventSource',
      class extends FakeEventSource {
        constructor(url: string) {
          super(url)
          this.readyState = FakeEventSource.CONNECTING
          instances.push(this)
        }
      },
    )
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ items: [], nextCursor: null, total: 0 }), { status: 200 }),
    )

    await getTransport().invoke('list_sessions', { projectId: 'p-eager-sub', pageSize: 20 })

    // 即便不 await ensureSseReady，ensureSource 也 SHALL 同步建出 SSE 实例
    // 让后续 metadata patch 走得通
    expect(instances).toHaveLength(1)
  })

  test('codex v3 issue 1：list_sessions(cursor=null) 在 SSE CONNECTING 时立即设 sseRecoveryPending；OPEN 后 emit sse-recovered', async () => {
    const instances: (FakeEventSource & { triggerOpen: () => void })[] = []
    vi.stubGlobal(
      'EventSource',
      class extends FakeEventSource {
        onopen: (() => void) | null = null
        constructor(url: string) {
          super(url)
          this.readyState = FakeEventSource.CONNECTING
          instances.push(this as FakeEventSource & { triggerOpen: () => void })
        }
        triggerOpen() {
          this.readyState = FakeEventSource.OPEN
          this.onopen?.()
        }
      },
    )
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ items: [], nextCursor: null, total: 0 }), { status: 200 }),
    )

    const recoveredHandler = vi.fn()
    await subscribeEvent('sse-recovered', recoveredHandler)
    expect(instances).toHaveLength(1)
    expect(instances[0].readyState).toBe(FakeEventSource.CONNECTING)

    // cursor=null：fetch 立即发出（不等 SSE OPEN），但 sseRecoveryPending SHALL
    // 被置 true——这样 SSE 后续 OPEN 时（即使 < 1000 ms 快速 OPEN）也会 emit
    // sse-recovered 兜底事件覆盖 fetch 在 OPEN 前发出窗口期内丢失的 metadata patch
    await getTransport().invoke('list_sessions', { projectId: 'p-fast-open', pageSize: 20 })
    expect(recoveredHandler).not.toHaveBeenCalled()

    // SSE 在 fetch 后 < 1000 ms 内成功 OPEN（fast-open 路径，不走 1000 ms 超时）
    instances[0].triggerOpen()
    expect(recoveredHandler).toHaveBeenCalledTimes(1)
    expect(recoveredHandler).toHaveBeenCalledWith({
      event: 'sse-recovered',
      id: 0,
      payload: {},
    })
  })

  test('codex v3 issue 1：list_sessions(cursor=null) 在 SSE 已 OPEN 时不设 sseRecoveryPending 不 emit 多余 sse-recovered', async () => {
    const instances: (FakeEventSource & { triggerOpen: () => void })[] = []
    vi.stubGlobal(
      'EventSource',
      class extends FakeEventSource {
        onopen: (() => void) | null = null
        constructor(url: string) {
          super(url)
          this.readyState = FakeEventSource.OPEN
          instances.push(this as FakeEventSource & { triggerOpen: () => void })
        }
        triggerOpen() {
          this.readyState = FakeEventSource.OPEN
          this.onopen?.()
        }
      },
    )
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ items: [], nextCursor: null, total: 0 }), { status: 200 }),
    )

    const recoveredHandler = vi.fn()
    await subscribeEvent('sse-recovered', recoveredHandler)
    expect(instances).toHaveLength(1)
    expect(instances[0].readyState).toBe(FakeEventSource.OPEN)

    // SSE 已 OPEN：fetch 之前 SSE 就在听了，没有 patch 丢失窗口——SHALL 不设
    // sseRecoveryPending，后续重连 onopen 也不会触发多余 sse-recovered
    await getTransport().invoke('list_sessions', { projectId: 'p-already-open', pageSize: 20 })
    instances[0].triggerOpen() // 模拟 spurious onopen（重连成功等）
    expect(recoveredHandler).not.toHaveBeenCalled()
  })

  test('SSE sse_lagged 事件转换为 sse-lagged event（broadcast 容量打满兜底）', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })
    const handler = vi.fn()

    const unsubscribe = await subscribeEvent('sse-lagged', handler)
    instances[0].emit({ type: 'sse_lagged' })

    expect(handler).toHaveBeenCalledWith({
      event: 'sse-lagged',
      id: 0,
      payload: {},
    })
    unsubscribe()
  })

  test('SSE context_changed 事件转换为 context_changed payload', async () => {
    const instances: FakeEventSource[] = []
    vi.stubGlobal('EventSource', class extends FakeEventSource {
      constructor(url: string) {
        super(url)
        instances.push(this)
      }
    })
    const handler = vi.fn()

    const unsubscribe = await subscribeEvent('context_changed', handler)
    instances[0].emit({
      type: 'context_changed',
      active_context: { id: 'ctx-1', name: 'Local' },
    })

    expect(handler).toHaveBeenCalledWith({
      event: 'context_changed',
      id: 0,
      payload: { activeContext: { id: 'ctx-1', name: 'Local' } },
    })
    unsubscribe()
  })
})

class FakeEventSource {
  static readonly CONNECTING = 0
  static readonly OPEN = 1
  static readonly CLOSED = 2

  onmessage: ((event: MessageEvent<string>) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  closed = false
  readyState = FakeEventSource.OPEN

  constructor(readonly url: string) {}

  emit(payload: unknown) {
    this.onmessage?.({ data: JSON.stringify(payload) } as MessageEvent<string>)
  }

  /** 模拟 EventSource CLOSED 终态错误（server toggle off → 浏览器重连失败
   * N 次进入 CLOSED）：设 readyState=CLOSED + 触发 onerror。 */
  triggerClosedError() {
    this.readyState = FakeEventSource.CLOSED
    this.onerror?.(new Event('error'))
  }

  close() {
    this.closed = true
    this.readyState = FakeEventSource.CLOSED
  }
}
