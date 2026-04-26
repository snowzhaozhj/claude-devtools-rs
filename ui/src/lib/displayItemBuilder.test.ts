// buildDisplayItemsCached：按内容指纹 memo，相同 chunk 指纹命中复用。

import { beforeEach, describe, expect, test } from 'vitest'

import type { AIChunk } from './api'
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
})
