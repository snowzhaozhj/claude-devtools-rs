// SessionDetail smoke 单测：组件依赖 getSessionDetail IPC + IntersectionObserver
// （lazyMarkdown）+ mermaid（动态 import）。用 setupMockIPC 提供 fixture，stub
// IntersectionObserver / ResizeObserver，验证骨架→详情切换不抛。
//
// 注：组件位于 ../routes/SessionDetail.svelte（不是 components/）；测试文件按
// 任务约定放在 components/ 目录便于扫描视图。

import { describe, expect, test, afterEach, beforeEach, vi } from 'vitest'
import { render, cleanup, waitFor, fireEvent } from '@testing-library/svelte'
import { clearMocks } from '@tauri-apps/api/mocks'

import SessionDetail from '../routes/SessionDetail.svelte'
import { setupMockIPC } from '../lib/tauriMock'
import { singleProjectFixture } from '../lib/__fixtures__'
import type { Chunk, UserChunk } from '../lib/api'
import type { Fixture } from '../lib/__fixtures__/types'

class IntersectionObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
  takeRecords(): IntersectionObserverEntry[] { return [] }
  root = null
  rootMargin = ''
  thresholds = []
}

class ResizeObserverStub {
  callback: ResizeObserverCallback

  constructor(callback: ResizeObserverCallback) {
    this.callback = callback
  }

  observe(target: Element) {
    this.callback([{ target, contentRect: target.getBoundingClientRect() } as ResizeObserverEntry], this as ResizeObserver)
  }

  unobserve() {}
  disconnect() {}
}

beforeEach(() => {
  // single-project fixture 已有完整 SessionDetail（含 user/ai chunks + 1 Read tool）
  setupMockIPC(singleProjectFixture)
  vi.stubGlobal('IntersectionObserver', IntersectionObserverStub)
  vi.stubGlobal('ResizeObserver', ResizeObserverStub)
  Element.prototype.scrollIntoView = vi.fn()
})

afterEach(() => {
  cleanup()
  clearMocks()
  vi.unstubAllGlobals()
})

const PROJECT_ID = singleProjectFixture.projects[0].id
const SESSION_ID = singleProjectFixture.sessions[PROJECT_ID][0].sessionId

function userChunk(index: number): UserChunk {
  return {
    kind: 'user',
    uuid: `u-long-${index}`,
    timestamp: `2026-04-11T10:${String(index % 60).padStart(2, '0')}:00Z`,
    durationMs: null,
    content: `long chunk ${index} ${index === 180 ? 'remote-search-needle' : ''}`,
    metrics: {
      inputTokens: 0,
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0,
      toolCount: 0,
      costUsd: null,
    },
  }
}

function longSessionFixture(): Fixture {
  const projectId = 'mock-long-proj'
  const sessionId = 'mock-long-sess'
  const chunks: Chunk[] = Array.from({ length: 200 }, (_, index) => userChunk(index))
  return {
    ...singleProjectFixture,
    name: 'long-session',
    projects: [{ id: projectId, path: '/Users/test/long-proj', displayName: 'long-proj', sessionCount: 1 }],
    sessions: {
      [projectId]: [{
        sessionId,
        projectId,
        timestamp: 1_712_822_400_000,
        messageCount: chunks.length,
        title: 'long session',
        isOngoing: false,
        gitBranch: 'main',
      }],
    },
    sessionDetails: {
      [`${projectId}:${sessionId}`]: {
        sessionId,
        projectId,
        chunks,
        metrics: {},
        metadata: {},
        contextInjections: [],
        isOngoing: false,
      },
    },
  }
}

describe('SessionDetail smoke', () => {
  test('给定合法 projectId/sessionId 渲染骨架，loading 完成后展示 top-bar', async () => {
    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-smoke-1',
        projectId: PROJECT_ID,
        sessionId: SESSION_ID,
      },
    })
    // 立即可见：session-detail 容器
    expect(container.querySelector('.session-detail')).not.toBeNull()
    // getSessionDetail 异步完成后 top-bar 渲染
    await waitFor(() => {
      expect(container.querySelector('.top-bar')).not.toBeNull()
    })
    expect(container.querySelector('.conversation')).not.toBeNull()
  })

  test('IPC 返回的 chunks 至少渲染一个 msg-row', async () => {
    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-smoke-2',
        projectId: PROJECT_ID,
        sessionId: SESSION_ID,
      },
    })
    await waitFor(() => {
      const rows = container.querySelectorAll('.msg-row')
      expect(rows.length).toBeGreaterThan(0)
    })
  })

  test('未知 sessionId 不崩，进入 error 分支或保留骨架', async () => {
    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-smoke-3',
        projectId: PROJECT_ID,
        sessionId: 'no-such-session',
      },
    })
    await waitFor(() => {
      // 要么 error 状态渲染（fixture 未命中 IPC 抛错），要么仍在 session-detail
      // 容器内（loading=false 后退化为空 conversation）。两种都视为"不崩"。
      expect(container.querySelector('.session-detail')).not.toBeNull()
    })
  })

  test('长会话默认只挂载虚拟窗口内的 chunk row', async () => {
    const fx = longSessionFixture()
    const projectId = fx.projects[0].id
    const sessionId = fx.sessions[projectId][0].sessionId
    setupMockIPC(fx)

    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-long-1',
        projectId,
        sessionId,
      },
    })

    await waitFor(() => {
      expect(container.querySelector('.conversation')).not.toBeNull()
    })

    const rows = container.querySelectorAll('.virtual-row')
    expect(rows.length).toBeGreaterThan(0)
    expect(rows.length).toBeLessThan(200)
    expect(container.querySelector('.conversation')?.getAttribute('data-virtualized')).toBe('true')
    expect(container.querySelectorAll('.virtual-spacer').length).toBe(2)
  })

  test('搜索打开时长会话退回全量 DOM 以保留远端匹配', async () => {
    const fx = longSessionFixture()
    const projectId = fx.projects[0].id
    const sessionId = fx.sessions[projectId][0].sessionId
    setupMockIPC(fx)

    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-long-search',
        projectId,
        sessionId,
      },
    })

    await waitFor(() => {
      expect(container.querySelector('.conversation')?.getAttribute('data-virtualized')).toBe('true')
    })

    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'f', metaKey: true }))

    await waitFor(() => {
      expect(container.querySelector('.conversation')?.getAttribute('data-virtualized')).toBe('false')
    })
    const input = container.querySelector('.search-input') as HTMLInputElement
    await fireEvent.input(input, { target: { value: 'remote-search-needle' } })
    await fireEvent.keyDown(input, { key: 'Enter' })

    await waitFor(() => {
      expect(container.textContent).toContain('remote-search-needle')
      expect(container.querySelectorAll('.virtual-row').length).toBe(200)
    })
  })
})
