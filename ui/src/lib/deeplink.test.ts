// deeplink hash route 单测（Task 6.6 vitest 部分；Playwright 部分见 e2e）。

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { buildDeeplinkHash, parseDeeplink, installDeeplinkWatcher, type DeeplinkTarget } from './deeplink'

beforeEach(() => {
  // 每个测试开局重置 hash 与 sentinel，避免 HMR 幂等 flag 跨测试污染
  history.replaceState(null, '', window.location.pathname)
  delete (window as Window & { __cdtDeeplinkWatcherInstalled?: boolean }).__cdtDeeplinkWatcherInstalled
})

afterEach(() => {
  history.replaceState(null, '', window.location.pathname)
  delete (window as Window & { __cdtDeeplinkWatcherInstalled?: boolean }).__cdtDeeplinkWatcherInstalled
})

describe('buildDeeplinkHash', () => {
  test('简单 sessionId / chunkId', () => {
    const h = buildDeeplinkHash('s1', 'c1')
    expect(h).toBe('#/session/s1/chunk/c1')
  })

  test('chunkId 含 `:` 走 encodeURIComponent', () => {
    const h = buildDeeplinkHash('sess-uuid', 'msg-uuid:0')
    // `:` 在 URL fragment path component 中需 encode 为 %3A
    expect(h).toBe('#/session/sess-uuid/chunk/msg-uuid%3A0')
  })

  test('特殊字符 encode', () => {
    const h = buildDeeplinkHash('s/1', 'c#2')
    expect(h).toBe('#/session/s%2F1/chunk/c%232')
  })
})

describe('parseDeeplink', () => {
  test('从参数 hash 解析', () => {
    const t = parseDeeplink('#/session/s1/chunk/c1')
    expect(t).toEqual({ sessionId: 's1', chunkId: 'c1' })
  })

  test('encoded `:` 还原', () => {
    const t = parseDeeplink('#/session/s1/chunk/msg%3A0')
    expect(t).toEqual({ sessionId: 's1', chunkId: 'msg:0' })
  })

  test('空 hash 返回 null', () => {
    expect(parseDeeplink('')).toBeNull()
  })

  test('非 deeplink 格式返回 null', () => {
    expect(parseDeeplink('#/foo/bar')).toBeNull()
    expect(parseDeeplink('#/session/s1')).toBeNull()
    expect(parseDeeplink('#random')).toBeNull()
  })

  test('从 location.hash 默认读取', () => {
    history.replaceState(null, '', '#/session/abc/chunk/xyz')
    const t = parseDeeplink()
    expect(t).toEqual({ sessionId: 'abc', chunkId: 'xyz' })
  })

  test('回往 buildDeeplinkHash 输出能解析', () => {
    const original: DeeplinkTarget = { sessionId: 'session-uuid-123', chunkId: 'chunk-uuid:0' }
    const h = buildDeeplinkHash(original.sessionId, original.chunkId)
    const parsed = parseDeeplink(h)
    expect(parsed).toEqual(original)
  })
})

describe('installDeeplinkWatcher', () => {
  test('启动时检查当前 hash（异步派发）', async () => {
    history.replaceState(null, '', '#/session/s1/chunk/c1')
    const onNavigate = vi.fn()
    const cleanup = installDeeplinkWatcher(onNavigate)
    // queueMicrotask → await Promise.resolve() 让 microtask 跑完
    await Promise.resolve()
    expect(onNavigate).toHaveBeenCalledWith({ sessionId: 's1', chunkId: 'c1' })
    cleanup()
  })

  test('hashchange 触发 callback', async () => {
    const onNavigate = vi.fn()
    const cleanup = installDeeplinkWatcher(onNavigate)
    await Promise.resolve()
    onNavigate.mockClear()
    // 模拟 hashchange
    history.replaceState(null, '', '#/session/abc/chunk/xyz')
    window.dispatchEvent(new HashChangeEvent('hashchange'))
    expect(onNavigate).toHaveBeenCalledWith({ sessionId: 'abc', chunkId: 'xyz' })
    cleanup()
  })

  test('cleanup 后 hashchange 不再触发', async () => {
    const onNavigate = vi.fn()
    const cleanup = installDeeplinkWatcher(onNavigate)
    await Promise.resolve()
    cleanup()
    onNavigate.mockClear()
    history.replaceState(null, '', '#/session/x/chunk/y')
    window.dispatchEvent(new HashChangeEvent('hashchange'))
    expect(onNavigate).not.toHaveBeenCalled()
  })

  test('幂等：重复 install 不重复注册', async () => {
    const onNavigate1 = vi.fn()
    const onNavigate2 = vi.fn()
    installDeeplinkWatcher(onNavigate1)
    await Promise.resolve()
    installDeeplinkWatcher(onNavigate2)
    await Promise.resolve()
    onNavigate1.mockClear()
    onNavigate2.mockClear()
    history.replaceState(null, '', '#/session/s/chunk/c')
    window.dispatchEvent(new HashChangeEvent('hashchange'))
    // 第一次 install 注册的 listener 仍生效；第二次 install 因 sentinel 直接返回 noop cleanup
    expect(onNavigate1).toHaveBeenCalledTimes(1)
    expect(onNavigate2).not.toHaveBeenCalled()
  })

  test('非 deeplink hash 不触发 callback', async () => {
    const onNavigate = vi.fn()
    const cleanup = installDeeplinkWatcher(onNavigate)
    await Promise.resolve()
    onNavigate.mockClear()
    history.replaceState(null, '', '#some-anchor')
    window.dispatchEvent(new HashChangeEvent('hashchange'))
    expect(onNavigate).not.toHaveBeenCalled()
    cleanup()
  })
})
