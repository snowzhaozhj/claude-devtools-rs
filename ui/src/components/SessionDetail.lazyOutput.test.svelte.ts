// SessionDetail 工具输出懒加载分档状态机组件测试。
//
// 对应 spec（tool-viewer-routing::工具输出懒加载态的稳定分档）：
// - 展开即触发懒加载并立即展开，加载期以稳定占位渲染（fetch-first 不阻塞展开）
// - missing 结果终结占位态（不永久"正在载入"）
// - 加载到达后按真实内容规模确定最终档位
// - 失败态显式呈现（复制禁用 + 可重试提示），不停留在 aria-busy 假占位
//
// 该状态机在 SessionDetail / ExecutionTrace 各一份拷贝——本测试锚定 SessionDetail
// 路径（顶层对话流），防"恢复 missing 早退 / isOutputLoading 改判"类回归。

import { describe, expect, test, afterEach, beforeEach, vi } from 'vitest'
import { render, cleanup, waitFor } from '@testing-library/svelte'
import { clearMocks } from '@tauri-apps/api/mocks'

import SessionDetail from '../routes/SessionDetail.svelte'
import { setupMockIPC } from '../lib/tauriMock'
import { resetWorkspaceTabsToDashboard } from '../lib/tabStore.svelte'
import { singleProjectFixture } from '../lib/__fixtures__'
import type { Fixture } from '../lib/__fixtures__'
import type { AIChunk, ToolOutput } from '../lib/api'

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
  vi.stubGlobal('IntersectionObserver', IntersectionObserverStub)
  vi.stubGlobal('ResizeObserver', ResizeObserverStub)
})

afterEach(() => {
  cleanup()
  clearMocks()
  vi.unstubAllGlobals()
  resetWorkspaceTabsToDashboard()
})

const PROJECT_ID = singleProjectFixture.projects[0].id
const SESSION_ID = singleProjectFixture.sessions[PROJECT_ID][0].sessionId
const TOOL_USE_ID = 'tu-lazy-1'

/** 单 AIChunk + 单 outputOmitted Bash 工具的 fixture；toolOutputs 控制懒拉结果。 */
function lazyFixture(toolOutputs: Record<string, ToolOutput | 'error' | 'pending'>): Fixture {
  const chunk: AIChunk = {
    kind: 'ai',
    chunkId: 'ai:lazy:0',
    timestamp: '2026-04-11T10:00:00Z',
    durationMs: null,
    responses: [{ uuid: 'a-lazy', timestamp: '2026-04-11T10:00:00Z', content: 'run', toolCalls: [], usage: null, model: 'claude-sonnet-4-6' }],
    metrics: { inputTokens: 0, outputTokens: 0, cacheCreationTokens: 0, cacheReadTokens: 0, toolCount: 1, costUsd: null },
    semanticSteps: [
      { kind: 'tool_execution', toolUseId: TOOL_USE_ID, toolName: 'Bash', timestamp: '2026-04-11T10:00:01Z' },
      { kind: 'text', text: 'done', timestamp: '2026-04-11T10:00:02Z' },
    ],
    toolExecutions: [
      {
        toolUseId: TOOL_USE_ID,
        toolName: 'Bash',
        input: { command: 'echo omitted-test' },
        // 裁剪空值占位：outputOmitted=true + 空 text（不得被当 0 字节判 inline）
        output: { kind: 'text', text: '' },
        isError: false,
        startTs: '2026-04-11T10:00:01Z',
        endTs: '2026-04-11T10:00:01.5Z',
        sourceAssistantUuid: 'a-lazy',
        outputOmitted: true,
        outputBytes: 4096,
      },
    ],
    subagents: [],
    slashCommands: [],
  }
  return {
    ...singleProjectFixture,
    sessionDetails: {
      [`${PROJECT_ID}:${SESSION_ID}`]: {
        ...singleProjectFixture.sessionDetails[`${PROJECT_ID}:${SESSION_ID}`],
        chunks: [chunk],
      },
    },
    toolOutputs,
  }
}

async function renderAndExpandTool(tabId: string) {
  const { container } = render(SessionDetail, {
    props: { tabId, projectId: PROJECT_ID, sessionId: SESSION_ID },
  })
  // AI chunk 工具列表默认折叠：先点"展开工具调用列表"
  let chunkToggle: HTMLButtonElement | null = null
  await waitFor(() => {
    chunkToggle = container.querySelector<HTMLButtonElement>('button[aria-label="展开工具调用列表"]')
    expect(chunkToggle).not.toBeNull()
  })
  chunkToggle!.click()
  // 再点 Bash 工具行展开 disclosure
  let toolHeader: HTMLButtonElement | null = null
  await waitFor(() => {
    const headers = [...container.querySelectorAll<HTMLButtonElement>('.base-item-header')]
    toolHeader = headers.find((h) => h.textContent?.includes('echo omitted-test')) ?? null
    expect(toolHeader).not.toBeNull()
  })
  toolHeader!.click()
  return container
}

describe('SessionDetail 工具输出懒加载分档状态机', () => {
  test('fetch-first：展开立即发生且加载期渲染稳定占位（aria-busy + 复制禁用）', async () => {
    setupMockIPC(lazyFixture({ [TOOL_USE_ID]: 'pending' }))
    const container = await renderAndExpandTool('tab-lazy-pending')
    await waitFor(() => {
      const busy = container.querySelector('[aria-busy="true"]')
      expect(busy).not.toBeNull()
      expect(busy!.textContent).toContain('正在载入')
    })
    const copyBtn = container.querySelector<HTMLButtonElement>('.ao-header button')
    expect(copyBtn?.disabled).toBe(true)
  })

  test('missing 终结占位态：不永久"正在载入"，退回空态展示', async () => {
    setupMockIPC(lazyFixture({})) // 缺省 → { kind: "missing" }
    const container = await renderAndExpandTool('tab-lazy-missing')
    await waitFor(() => {
      expect(container.querySelector('[aria-busy="true"]')).toBeNull()
      expect(container.textContent).not.toContain('正在载入')
    })
  })

  test('加载到达后按真实内容分档：100 行 → 限高预览 + 信息气味', async () => {
    const text = Array.from({ length: 100 }, (_, i) => `row ${i}`).join('\n')
    setupMockIPC(lazyFixture({ [TOOL_USE_ID]: { kind: 'text', text } }))
    const container = await renderAndExpandTool('tab-lazy-loaded')
    await waitFor(() => {
      const scent = container.querySelector('.ao-scent')
      expect(scent).not.toBeNull()
      expect(scent!.textContent).toContain('100 行')
      expect(scent!.textContent).toContain('预览')
    })
    expect(container.querySelector('[aria-busy="true"]')).toBeNull()
  })

  test('失败态显式呈现：不停留 aria-busy 假占位，复制禁用 + 可重试提示', async () => {
    setupMockIPC(lazyFixture({ [TOOL_USE_ID]: 'error' }))
    const container = await renderAndExpandTool('tab-lazy-failed')
    await waitFor(() => {
      expect(container.textContent).toContain('加载失败')
    })
    expect(container.querySelector('[aria-busy="true"]')).toBeNull()
    const copyBtn = container.querySelector<HTMLButtonElement>('.ao-header button')
    expect(copyBtn?.disabled).toBe(true)
    expect(copyBtn?.getAttribute('aria-label')).toContain('加载失败')
  })
})
