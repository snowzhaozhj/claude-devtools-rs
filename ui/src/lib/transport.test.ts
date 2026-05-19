import { afterEach, describe, expect, test, vi } from 'vitest'

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

import { listen } from '@tauri-apps/api/event'
import { BrowserUnsupportedError, getTransport, subscribeEvent } from './transport'

afterEach(() => {
  vi.restoreAllMocks()
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
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
  onmessage: ((event: MessageEvent<string>) => void) | null = null
  closed = false

  constructor(readonly url: string) {}

  emit(payload: unknown) {
    this.onmessage?.({ data: JSON.stringify(payload) } as MessageEvent<string>)
  }

  close() {
    this.closed = true
  }
}
