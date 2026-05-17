// SessionDetail smoke 单测：组件依赖 getSessionDetail IPC + IntersectionObserver
// （lazyMarkdown）+ mermaid（动态 import）。用 setupMockIPC 提供 fixture，stub
// IntersectionObserver / ResizeObserver，验证骨架→详情切换不抛。
//
// 注：组件位于 ../routes/SessionDetail.svelte（不是 components/）；测试文件按
// 任务约定放在 components/ 目录便于扫描视图。

import { describe, expect, test, afterEach, beforeEach, vi } from 'vitest'
import { render, cleanup, waitFor } from '@testing-library/svelte'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { clearMocks } from '@tauri-apps/api/mocks'

import SessionDetail from '../routes/SessionDetail.svelte'
import { setupMockIPC } from '../lib/tauriMock'
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


const SESSION_DETAIL_SOURCE = readFileSync(join(process.cwd(), 'src/routes/SessionDetail.svelte'), 'utf8')

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


  test('工具列表容器不使用 containment，避免刷新后展开详情保留错误高度', () => {
    const source = SESSION_DETAIL_SOURCE
    expect(source).toContain('class="ai-tools-section"')
    expect(source).not.toContain('class="ai-tools-section msg-row-contained"')
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
})
