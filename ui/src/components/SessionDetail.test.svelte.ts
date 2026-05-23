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
import {
  saveTabUIState,
  getTabUIState,
  openTab,
  getActiveTab,
  closeTab,
} from '../lib/tabStore.svelte'
import { singleProjectFixture } from '../lib/__fixtures__'
import type { Fixture } from '../lib/__fixtures__'
import type { AIChunk, Chunk, CompactChunk } from '../lib/api'

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

function fixtureWithChunks(chunks: Chunk[]): Fixture {
  return {
    ...singleProjectFixture,
    sessionDetails: {
      [`${PROJECT_ID}:${SESSION_ID}`]: {
        ...singleProjectFixture.sessionDetails[`${PROJECT_ID}:${SESSION_ID}`],
        chunks,
      },
    },
  }
}

function aiChunk(uuid: string, ordinal: number, timestamp: string, text: string): AIChunk {
  return {
    kind: 'ai',
    chunkId: `ai:${uuid}:${ordinal}`,
    timestamp,
    durationMs: null,
    responses: [{ uuid, timestamp, content: text, toolCalls: [], usage: null, model: 'claude-sonnet-4-6' }],
    metrics: { inputTokens: 0, outputTokens: 0, cacheCreationTokens: 0, cacheReadTokens: 0, toolCount: 0, costUsd: null },
    semanticSteps: [{ kind: 'text', text, timestamp }],
    toolExecutions: [],
    subagents: [],
    slashCommands: [],
  }
}

function compactChunk(): CompactChunk {
  return {
    kind: 'compact',
    chunkId: 'compact-1',
    uuid: 'compact-1',
    timestamp: '2026-04-11T10:00:02Z',
    durationMs: null,
    summaryText: 'compact summary',
    metrics: { inputTokens: 0, outputTokens: 0, cacheCreationTokens: 0, cacheReadTokens: 0, toolCount: 0, costUsd: null },
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

  test('IPC 返回的 chunks 渲染 containment 边界且不包住 AI header', async () => {
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
      expect(container.querySelector('.msg-row-user.msg-row-contained')).not.toBeNull()
      expect(container.querySelector('.msg-row-ai.msg-row-contained')).toBeNull()
      expect(container.querySelector('.msg-row-ai .ai-body.msg-row-contained')).not.toBeNull()
    })
  })


  test('恢复展开状态时工具列表容器不使用 containment', async () => {
    const tabId = 'tab-smoke-expanded-tools'
    saveTabUIState(tabId, {
      expandedChunks: new Set(['ai:a1:0']),
      expandedItems: new Set(['ai:a1:0-tool-tu1']),
      searchVisible: false,
      contextPanelVisible: false,
      atBottom: false,
      anchorChunkId: null,
      anchorOffsetPx: 0,
    })

    const { container } = render(SessionDetail, {
      props: {
        tabId,
        projectId: PROJECT_ID,
        sessionId: SESSION_ID,
      },
    })

    await waitFor(() => {
      const toolsSection = container.querySelector('.ai-tools-section')
      expect(toolsSection).not.toBeNull()
      expect(toolsSection?.classList.contains('msg-row-contained')).toBe(false)
      expect(toolsSection?.querySelector('.base-item-content')).not.toBeNull()
    })
  })

  test('compact 后重复 AI response uuid 不会让 keyed each 崩溃', async () => {
    const duplicateUuid = 'replayed-assistant-uuid'
    setupMockIPC(fixtureWithChunks([
      aiChunk(duplicateUuid, 0, '2026-04-11T10:00:01Z', 'before compact'),
      compactChunk(),
      aiChunk(duplicateUuid, 1, '2026-04-11T10:00:03Z', 'replayed after compact'),
    ]))

    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-smoke-duplicate-ai-key',
        projectId: PROJECT_ID,
        sessionId: SESSION_ID,
      },
    })

    await waitFor(() => {
      expect(container.querySelectorAll('.msg-row').length).toBe(3)
    })
  })

  test('含 mermaid 的 contained 区域通过 CSS 关闭 content-visibility', async () => {
    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-smoke-3',
        projectId: PROJECT_ID,
        sessionId: SESSION_ID,
      },
    })
    await waitFor(() => {
      expect(container.querySelector('.msg-row-contained')).not.toBeNull()
    })

    const contained = container.querySelector('.msg-row-contained') as HTMLElement
    contained.innerHTML = '<div class="mermaid-block"></div>'
    const computed = getComputedStyle(contained)
    expect(computed.contentVisibility).toBe('visible')
    expect(computed.contain).toBe('none')
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

  // ── Quick Anchor Navigation（change `session-jump-to-latest`）──
  // jsdom 不实现真 scroll 物理（scrollHeight / clientHeight = 0），无法测距底
  // 判定与状态机；这部分由 playwright e2e 兜底（真浏览器有真 scroll）。
  // 单测仅做 DOM 存在性 + 初始 a11y 状态 smoke——验证按钮渲染、初始隐藏、
  // aria-label 与平台分流 tooltip 正确。
  test('jump-to-latest：按钮存在，初始隐藏（aria-hidden + tabindex=-1）', async () => {
    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-jump-1',
        projectId: PROJECT_ID,
        sessionId: SESSION_ID,
      },
    })
    await waitFor(() => {
      expect(container.querySelector('.conversation')).not.toBeNull()
    })
    const btn = container.querySelector('.jump-to-latest') as HTMLButtonElement | null
    expect(btn).not.toBeNull()
    // 初始 isFar=false → aria-hidden=true + tabindex=-1（不在 Tab 序列）
    expect(btn?.getAttribute('aria-hidden')).toBe('true')
    expect(btn?.getAttribute('tabindex')).toBe('-1')
    expect(btn?.getAttribute('aria-label')).toBe('跳到最新消息')
    // 默认隐藏 visual class：不带 .jump-to-latest-visible
    expect(btn?.classList.contains('jump-to-latest-visible')).toBe(false)
    // tooltip 文案含快捷键提示（mac ⌘↓ 或 Win/Linux Ctrl+End，按 navigator.platform 分流）
    const title = btn?.getAttribute('title') ?? ''
    expect(title.startsWith('跳到最新消息')).toBe(true)
    expect(/⌘↓|Ctrl\+End/.test(title)).toBe(true)
  })

  // ── 滚动位置保留（spec tab-management::滚动位置恢复）──
  // 真浏览器特异行为（detached element scrollTop=0、lazy markdown 占位 vs 真实
  // 渲染高度差、IntersectionObserver / MutationObserver 时序）jsdom 都不复现，
  // 锚点法核心契约（捕获 / 恢复 / 粘底 pin）的回归测试只能 Playwright e2e 兜底
  // （见 ui/tests/e2e/tab-scroll-preserve.spec.ts）。本节 unit 测试只覆盖
  // jsdom 能模拟的 sessionId guard 行为：用 anchorChunkId sentinel 验证不被覆盖。
  test('滚动位置保留：tab 已被替换 sessionId 时不写脏 anchor', async () => {
    openTab(SESSION_ID, PROJECT_ID, 'preserve-replaced')
    const tabId = getActiveTab()!.id
    saveTabUIState(tabId, {
      expandedChunks: new Set(),
      expandedItems: new Set(),
      searchVisible: false,
      contextPanelVisible: false,
      atBottom: false,
      anchorChunkId: 'sentinel-from-prior-session',
      anchorOffsetPx: 42,
    })

    const { container, unmount } = render(SessionDetail, {
      props: {
        tabId,
        projectId: PROJECT_ID,
        // 故意与 tabStore 内 tab.sessionId 不一致 → guard 拒写
        sessionId: 'orphan-session-not-in-any-tab',
      },
    })
    await waitFor(() => {
      expect(container.querySelector('.session-detail')).not.toBeNull()
    })
    const conv = container.querySelector('.conversation') as HTMLElement | null
    if (conv) {
      conv.scrollTop = 333
      conv.dispatchEvent(new Event('scroll'))
    }
    unmount()

    // guard 应拒写——保留之前的 sentinel
    const after = getTabUIState(tabId)
    expect(after.anchorChunkId).toBe('sentinel-from-prior-session')
    expect(after.anchorOffsetPx).toBe(42)

    closeTab(tabId)
  })

  test('detail.title 存在时 <h1> 直接渲染该值（与 sidebar 派生一致）', async () => {
    const fx = {
      ...singleProjectFixture,
      sessionDetails: {
        [`${PROJECT_ID}:${SESSION_ID}`]: {
          ...singleProjectFixture.sessionDetails[`${PROJECT_ID}:${SESSION_ID}`],
          title: '修复登录页样式',
        },
      },
    }
    setupMockIPC(fx)
    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-title-1',
        projectId: PROJECT_ID,
        sessionId: SESSION_ID,
      },
    })
    await waitFor(() => {
      const h1 = container.querySelector('h1.top-title')
      expect(h1?.textContent).toBe('修复登录页样式')
    })
  })

  test('detail.title 缺失时 <h1> fallback 到 sessionId.slice(0, 8)（与 sidebar 一致）', async () => {
    const fx = {
      ...singleProjectFixture,
      sessionDetails: {
        [`${PROJECT_ID}:${SESSION_ID}`]: {
          ...singleProjectFixture.sessionDetails[`${PROJECT_ID}:${SESSION_ID}`],
          title: null,
        },
      },
    }
    setupMockIPC(fx)
    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-title-2',
        projectId: PROJECT_ID,
        sessionId: SESSION_ID,
      },
    })
    await waitFor(() => {
      const h1 = container.querySelector('h1.top-title')
      expect(h1?.textContent).toBe(SESSION_ID.slice(0, 8))
    })
  })

  test('jump-to-latest：未打开 ContextPanel 时按钮不带 shifted class', async () => {
    const { container } = render(SessionDetail, {
      props: {
        tabId: 'tab-jump-2',
        projectId: PROJECT_ID,
        sessionId: SESSION_ID,
      },
    })
    await waitFor(() => {
      expect(container.querySelector('.conversation')).not.toBeNull()
    })
    const btn = container.querySelector('.jump-to-latest') as HTMLButtonElement | null
    expect(btn).not.toBeNull()
    // 默认 contextPanelVisible=false → 不带 shifted class（right offset = 16px）
    expect(btn?.classList.contains('jump-to-latest-shifted')).toBe(false)
  })
})
