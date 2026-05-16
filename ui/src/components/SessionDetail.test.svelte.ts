// SessionDetail smoke 单测：组件依赖 getSessionDetail IPC + IntersectionObserver
// （lazyMarkdown）+ mermaid（动态 import）。用 setupMockIPC 提供 fixture，stub
// IntersectionObserver / ResizeObserver，验证骨架→详情切换不抛。
//
// 注：组件位于 ../routes/SessionDetail.svelte（不是 components/）；测试文件按
// 任务约定放在 components/ 目录便于扫描视图。

import { describe, expect, test, afterEach, beforeEach, vi } from 'vitest'
import { render, cleanup, waitFor } from '@testing-library/svelte'
import { clearMocks } from '@tauri-apps/api/mocks'

import SessionDetail from '../routes/SessionDetail.svelte'
import { setupMockIPC } from '../lib/tauriMock'
import { singleProjectFixture } from '../lib/__fixtures__'

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
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeEach(() => {
  // single-project fixture 已有完整 SessionDetail（含 user/ai chunks + 1 Read tool）
  setupMockIPC(singleProjectFixture)
  vi.stubGlobal('IntersectionObserver', IntersectionObserverStub)
  vi.stubGlobal('ResizeObserver', ResizeObserverStub)
})

afterEach(() => {
  cleanup()
  clearMocks()
  vi.unstubAllGlobals()
})

const PROJECT_ID = singleProjectFixture.projects[0].id
const SESSION_ID = singleProjectFixture.sessions[PROJECT_ID][0].sessionId

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
})
