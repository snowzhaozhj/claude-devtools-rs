// buildDisplayItemsCached：按内容指纹 memo，相同 chunk 指纹命中复用。
// buildDisplayItems：tool / subagent / 工具名识别等渲染选择行为。

import { beforeEach, describe, expect, test } from 'vitest'

import type { AIChunk, SubagentProcess, TeammateMessage, ToolExecution } from './api'
import {
  _resetDisplayItemsCacheForTest,
  buildDisplayItems,
  buildDisplayItemsCached,
} from './displayItemBuilder'

function makeAIChunk(
  uuid: string,
  steps: number,
  lastTs = '2026-04-26T00:00:00Z',
): AIChunk {
  return {
    kind: 'ai',
    chunkId: `ai:${uuid}:0`,
    timestamp: '2026-04-26T00:00:00Z',
    durationMs: null,
    responses: [
      {
        uuid,
        timestamp: '2026-04-26T00:00:00Z',
        content: 'hi',
        toolCalls: [],
        usage: null,
        model: null,
      },
    ],
    metrics: {
      inputTokens: 0,
      outputTokens: 0,
      cacheCreationTokens: 0,
      cacheReadTokens: 0,
      toolCount: 0,
      costUsd: null,
    },
    semanticSteps: Array.from({ length: steps }, (_, i) => ({
      kind: 'text' as const,
      text: `step-${i}`,
      timestamp: i === steps - 1 ? lastTs : '2026-04-26T00:00:00Z',
    })),
    toolExecutions: [],
    subagents: [],
    slashCommands: [],
  }
}

describe('buildDisplayItemsCached', () => {
  beforeEach(() => {
    _resetDisplayItemsCacheForTest()
  })

  test('相同指纹命中缓存（返回同一引用）', () => {
    const chunk = makeAIChunk('uuid-a', 3)
    const r1 = buildDisplayItemsCached(chunk)
    const r2 = buildDisplayItemsCached(chunk)
    expect(r1).toBe(r2)
  })

  test('不同 chunk 但指纹相同也命中（说明跨 refresh 不重算）', () => {
    const chunkA = makeAIChunk('uuid-b', 3)
    const chunkB = makeAIChunk('uuid-b', 3)
    expect(chunkA).not.toBe(chunkB)
    const r1 = buildDisplayItemsCached(chunkA)
    const r2 = buildDisplayItemsCached(chunkB)
    expect(r1).toBe(r2)
  })

  test('semanticSteps 长度变化触发重算', () => {
    const chunkA = makeAIChunk('uuid-c', 2)
    const chunkB = makeAIChunk('uuid-c', 3)
    const r1 = buildDisplayItemsCached(chunkA)
    const r2 = buildDisplayItemsCached(chunkB)
    expect(r1).not.toBe(r2)
  })

  test('最后一条 step 的 timestamp 变化触发重算', () => {
    const chunkA = makeAIChunk('uuid-d', 2, '2026-04-26T00:00:00Z')
    const chunkB = makeAIChunk('uuid-d', 2, '2026-04-26T00:00:01Z')
    const r1 = buildDisplayItemsCached(chunkA)
    const r2 = buildDisplayItemsCached(chunkB)
    expect(r1).not.toBe(r2)
  })

  test('responses[0].uuid 变化触发重算', () => {
    const chunkA = makeAIChunk('uuid-e1', 2)
    const chunkB = makeAIChunk('uuid-e2', 2)
    const r1 = buildDisplayItemsCached(chunkA)
    const r2 = buildDisplayItemsCached(chunkB)
    expect(r1).not.toBe(r2)
  })

  test('responses 缺失走 timestamp fallback 仍可命中', () => {
    const base = makeAIChunk('x', 1)
    const chunkA: AIChunk = { ...base, responses: [] }
    const chunkB: AIChunk = { ...base, responses: [] }
    const r1 = buildDisplayItemsCached(chunkA)
    const r2 = buildDisplayItemsCached(chunkB)
    expect(r1).toBe(r2)
  })

  test('toolExecutions 长度不变但 endTs 变化触发重算（codex bug 2）', () => {
    const baseExec: ToolExecution = {
      toolUseId: 'tool-1',
      toolName: 'Bash',
      input: {},
      output: { kind: 'text', text: '' },
      isError: false,
      startTs: '2026-04-26T00:00:00Z',
      endTs: null,
      sourceAssistantUuid: 'src-1',
    }
    const chunkA: AIChunk = { ...makeAIChunk('uuid-tool', 1), toolExecutions: [baseExec] }
    const chunkB: AIChunk = {
      ...makeAIChunk('uuid-tool', 1),
      toolExecutions: [{ ...baseExec, endTs: '2026-04-26T00:00:01Z' }],
    }
    const r1 = buildDisplayItemsCached(chunkA)
    const r2 = buildDisplayItemsCached(chunkB)
    expect(r1).not.toBe(r2)
  })

  test('toolExecutions isError 变化触发重算', () => {
    const baseExec: ToolExecution = {
      toolUseId: 'tool-2',
      toolName: 'Bash',
      input: {},
      output: { kind: 'text', text: '' },
      isError: false,
      startTs: '2026-04-26T00:00:00Z',
      endTs: '2026-04-26T00:00:01Z',
      sourceAssistantUuid: 'src-1',
    }
    const chunkA: AIChunk = { ...makeAIChunk('uuid-err', 1), toolExecutions: [baseExec] }
    const chunkB: AIChunk = {
      ...makeAIChunk('uuid-err', 1),
      toolExecutions: [{ ...baseExec, isError: true }],
    }
    const r1 = buildDisplayItemsCached(chunkA)
    const r2 = buildDisplayItemsCached(chunkB)
    expect(r1).not.toBe(r2)
  })

  test('subagent isOngoing 翻转触发重算', () => {
    const sub: SubagentProcess = {
      sessionId: 'sub-1',
      rootTaskDescription: null,
      spawnTs: '2026-04-26T00:00:00Z',
      endTs: null,
      metrics: {
        inputTokens: 0,
        outputTokens: 0,
        cacheCreationTokens: 0,
        cacheReadTokens: 0,
        toolCount: 0,
        costUsd: null,
      },
      team: null,
      subagentType: null,
      messages: [],
      mainSessionImpact: null,
      isOngoing: true,
      durationMs: null,
      parentTaskId: null,
      description: null,
    }
    const chunkA: AIChunk = { ...makeAIChunk('uuid-sub', 1), subagents: [sub] }
    const chunkB: AIChunk = {
      ...makeAIChunk('uuid-sub', 1),
      subagents: [{ ...sub, isOngoing: false, endTs: '2026-04-26T00:00:05Z' }],
    }
    const r1 = buildDisplayItemsCached(chunkA)
    const r2 = buildDisplayItemsCached(chunkB)
    expect(r1).not.toBe(r2)
  })
})

// =============================================================================
// spec `session-display` "Subagent 卡片与 Task tool 就地交错渲染"
// =============================================================================

describe('buildDisplayItems — task tool 去重与 Agent 工具识别（R3）', () => {
  /** 构造一个含单个 task 工具 + 一个 subagent_spawn 的 AIChunk。 */
  function makeChunkWithTaskAndSubagent(opts: {
    toolName: 'Task' | 'Agent'
    toolUseId: string
    /** subagent.parentTaskId；null 模拟 Orphan */
    parentTaskId: string | null
  }): AIChunk {
    const ts = '2026-05-13T00:00:00Z'
    const exec: ToolExecution = {
      toolUseId: opts.toolUseId,
      toolName: opts.toolName,
      input: { description: 'do it' },
      output: { kind: 'structured', value: { session_id: 'sub-1' } },
      isError: false,
      startTs: ts,
      endTs: ts,
      sourceAssistantUuid: 'src-1',
    }
    const sub: SubagentProcess = {
      sessionId: 'sub-1',
      rootTaskDescription: 'do it',
      spawnTs: ts,
      endTs: null,
      metrics: {
        inputTokens: 0,
        outputTokens: 0,
        cacheCreationTokens: 0,
        cacheReadTokens: 0,
        toolCount: 0,
        costUsd: null,
      },
      team: null,
      subagentType: null,
      messages: [],
      mainSessionImpact: null,
      isOngoing: true,
      durationMs: null,
      parentTaskId: opts.parentTaskId,
      description: null,
    }
    return {
      kind: 'ai',
      chunkId: 'ai:r-1:0',
      timestamp: ts,
      durationMs: null,
      responses: [
        {
          uuid: 'r-1',
          timestamp: ts,
          content: '',
          toolCalls: [],
          usage: null,
          model: null,
        },
      ],
      metrics: {
        inputTokens: 0,
        outputTokens: 0,
        cacheCreationTokens: 0,
        cacheReadTokens: 0,
        toolCount: 0,
        costUsd: null,
      },
      semanticSteps: [
        { kind: 'tool_execution', toolUseId: opts.toolUseId, toolName: opts.toolName, timestamp: ts },
        { kind: 'subagent_spawn', placeholderId: 'sub-1', timestamp: ts },
      ],
      toolExecutions: [exec],
      subagents: [sub],
      slashCommands: [],
    }
  }

  test('Task 工具关联 subagent 时 SHALL 被过滤，仅渲染 SubagentItem', () => {
    const chunk = makeChunkWithTaskAndSubagent({
      toolName: 'Task',
      toolUseId: 't-1',
      parentTaskId: 't-1',
    })
    const { items } = buildDisplayItems(chunk)
    expect(items.find((i) => i.type === 'tool')).toBeUndefined()
    const subItem = items.find((i) => i.type === 'subagent')
    expect(subItem).toBeDefined()
  })

  test('Agent 工具关联 subagent 时 SHALL 同样被过滤（R3 修复点）', () => {
    const chunk = makeChunkWithTaskAndSubagent({
      toolName: 'Agent',
      toolUseId: 'a-1',
      parentTaskId: 'a-1',
    })
    const { items } = buildDisplayItems(chunk)
    expect(
      items.find((i) => i.type === 'tool'),
      'Agent 工具关联 subagent 不应再以 ToolItem 渲染',
    ).toBeUndefined()
    const subItem = items.find((i) => i.type === 'subagent')
    expect(subItem).toBeDefined()
  })

  test('Orphan Agent 工具（未关联 subagent）SHALL 保留为 ToolItem', () => {
    const chunk = makeChunkWithTaskAndSubagent({
      toolName: 'Agent',
      toolUseId: 'a-2',
      parentTaskId: null, // subagent 存在但 parentTaskId=null → 不关联
    })
    const { items } = buildDisplayItems(chunk)
    const toolItem = items.find((i) => i.type === 'tool')
    expect(toolItem).toBeDefined()
    if (toolItem && toolItem.type === 'tool') {
      expect(toolItem.execution.toolName).toBe('Agent')
    }
  })

  test('Orphan Task 工具 SHALL 保留为 ToolItem（回归保险）', () => {
    const chunk = makeChunkWithTaskAndSubagent({
      toolName: 'Task',
      toolUseId: 't-2',
      parentTaskId: null,
    })
    const { items } = buildDisplayItems(chunk)
    const toolItem = items.find((i) => i.type === 'tool')
    expect(toolItem).toBeDefined()
    if (toolItem && toolItem.type === 'tool') {
      expect(toolItem.execution.toolName).toBe('Task')
    }
  })
})

// =============================================================================
// 边界：empty-responses AIChunk（chunk-building::Embed teammate messages
// into AIChunk 第 5 条规则——orphan teammate before user-side flush）。
// =============================================================================

describe('buildDisplayItems — empty-responses AIChunk with teammate', () => {
  function makeEmptyAIChunk(teammates: TeammateMessage[]): AIChunk {
    const ts = '2026-05-23T22:08:00Z'
    return {
      kind: 'ai',
      chunkId: 'tm-1:0',
      timestamp: ts,
      durationMs: null,
      responses: [],
      metrics: {
        inputTokens: 0,
        outputTokens: 0,
        cacheCreationTokens: 0,
        cacheReadTokens: 0,
        toolCount: 0,
        costUsd: null,
      },
      semanticSteps: [],
      toolExecutions: [],
      subagents: [],
      slashCommands: [],
      teammateMessages: teammates,
    }
  }

  test('empty-responses AIChunk 含 teammate_message SHALL 仅产 teammate_message item', () => {
    const tm: TeammateMessage = {
      uuid: 'tm-1',
      teammateId: 'team-lead',
      color: 'blue',
      summary: 'you are frontend',
      body: '你是 kb-shortcuts team 的 frontend teammate',
      timestamp: '2026-05-23T22:08:00Z',
      replyToToolUseId: null,
      tokenCount: 100,
      isNoise: false,
      isResend: false,
    }
    const chunk = makeEmptyAIChunk([tm])
    const { items, lastOutput } = buildDisplayItems(chunk)
    expect(items.length).toBe(1)
    expect(items[0].type).toBe('teammate_message')
    if (items[0].type === 'teammate_message') {
      expect(items[0].teammateMessage.body).toBe(
        '你是 kb-shortcuts team 的 frontend teammate',
      )
    }
    expect(lastOutput).toBeNull()
  })

  test('empty-responses AIChunk 无 teammate SHALL 产空 items', () => {
    const chunk = makeEmptyAIChunk([])
    const { items, lastOutput } = buildDisplayItems(chunk)
    expect(items.length).toBe(0)
    expect(lastOutput).toBeNull()
  })
})
