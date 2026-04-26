// buildDisplayItemsCached：按内容指纹 memo，相同 chunk 指纹命中复用。

import { beforeEach, describe, expect, test } from 'vitest'

import type { AIChunk, SubagentProcess, ToolExecution } from './api'
import {
  _resetDisplayItemsCacheForTest,
  buildDisplayItemsCached,
} from './displayItemBuilder'

function makeAIChunk(
  uuid: string,
  steps: number,
  lastTs = '2026-04-26T00:00:00Z',
): AIChunk {
  return {
    kind: 'ai',
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
