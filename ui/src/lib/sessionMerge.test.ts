import { describe, expect, test } from 'vitest'
import type { SessionSummary } from './api'
import { applySilentRefresh, mergeSessions, mergeSilentMetadata } from './sessionMerge'

function skel(id: string, ts: number, overrides: Partial<SessionSummary> = {}): SessionSummary {
  return {
    sessionId: id,
    projectId: 'projectA',
    timestamp: ts,
    messageCount: 0,
    title: null,
    isOngoing: false,
    gitBranch: null,
    ...overrides,
  }
}

function patched(id: string, ts: number, title: string, overrides: Partial<SessionSummary> = {}): SessionSummary {
  return skel(id, ts, { title, messageCount: 12, ...overrides })
}

describe('mergeSilentMetadata', () => {
  test('prev 已 patch 元数据时用 prev 字段覆盖 next 骨架', () => {
    const prev = [patched('s1', 1000, 'old title', { isOngoing: true, gitBranch: 'main' })]
    const next = [skel('s1', 2000)]
    const merged = mergeSilentMetadata(prev, next)
    expect(merged).toEqual([
      {
        sessionId: 's1',
        projectId: 'projectA',
        timestamp: 2000,
        messageCount: 12,
        title: 'old title',
        isOngoing: true,
        gitBranch: 'main',
      },
    ])
  })

  test('prev 中无对应条目时直接返回 next 骨架', () => {
    const prev: SessionSummary[] = []
    const next = [skel('s1', 1000)]
    expect(mergeSilentMetadata(prev, next)).toEqual(next)
  })

  test('prev 条目无 patch 元数据时让 next 骨架原样通过', () => {
    const prev = [skel('s1', 500)]
    const next = [skel('s1', 1000)]
    expect(mergeSilentMetadata(prev, next)).toEqual(next)
  })
})

describe('mergeSessions', () => {
  test('prev 中超出 next 范围的尾部条目保留', () => {
    const prev = [
      patched('s1', 3000, 'page1-a'),
      patched('s2', 2500, 'page1-b'),
      patched('s3', 1500, 'page2-a'),
      patched('s4', 1000, 'page2-b'),
    ]
    const next = [skel('s1', 3000), skel('s2', 2500)]
    const merged = mergeSessions(prev, next, true)
    expect(merged.map((s) => s.sessionId)).toEqual(['s1', 's2', 's3', 's4'])
  })

  test('prev 与 next 共有的 sessionId 用 prev 元数据填新骨架', () => {
    const prev = [patched('s1', 3000, 'patched title')]
    const next = [skel('s1', 4000)]
    const merged = mergeSessions(prev, next, true)
    expect(merged[0].title).toBe('patched title')
    expect(merged[0].timestamp).toBe(4000)
  })

  test('sort=true 按 timestamp 倒序，sort=false 保持 prev 顺序', () => {
    const prev = [patched('s1', 1000, 'a'), patched('s2', 3000, 'b')]
    const next = [skel('s3', 2000)]
    const sortedDesc = mergeSessions(prev, next, true)
    expect(sortedDesc.map((s) => s.sessionId)).toEqual(['s2', 's3', 's1'])
    const unsorted = mergeSessions(prev, next, false)
    expect(unsorted.map((s) => s.sessionId)).toEqual(['s1', 's2', 's3'])
  })

  test('timestamp 相同时按 stable sort 保留 prev 相对顺序', () => {
    const prev = [patched('s1', 2000, 'first'), patched('s2', 2000, 'second')]
    const next: SessionSummary[] = []
    const merged = mergeSessions(prev, next, true)
    expect(merged.map((s) => s.sessionId)).toEqual(['s1', 's2'])
  })
})

describe('applySilentRefresh', () => {
  const prevCursor = 'cursor3'
  const firstPageCursorWontReplace = 'cursor1'

  test('保留 prev 中超出第一页的尾部 sessions', () => {
    const prev: SessionSummary[] = [
      patched('s1', 3000, 'page1-a'),
      patched('s2', 2500, 'page1-b'),
      patched('s3', 1500, 'page2-a'),
      patched('s4', 1000, 'page2-b'),
    ]
    const firstPage: SessionSummary[] = [skel('s1', 3000), skel('s2', 2500)]
    const result = applySilentRefresh(prev, prevCursor, firstPage)
    expect(result.sessions.length).toBeGreaterThanOrEqual(prev.length)
    expect(result.sessions.map((s) => s.sessionId)).toEqual(['s1', 's2', 's3', 's4'])
  })

  test('cursor 保留 prev 的 cursor，不取 firstPage 的 nextCursor', () => {
    const prev = [patched('s1', 1000, 'a')]
    const firstPage = [skel('s1', 1000)]
    const result = applySilentRefresh(prev, prevCursor, firstPage)
    expect(result.nextCursor).toBe(prevCursor)
    expect(result.nextCursor).not.toBe(firstPageCursorWontReplace)
  })

  test('prev 中已 patch 元数据的 session 在 silent 合并后保留元数据', () => {
    const prev = [patched('s1', 1000, 'patched', { isOngoing: true })]
    const firstPage = [skel('s1', 1000)]
    const result = applySilentRefresh(prev, prevCursor, firstPage)
    expect(result.sessions[0].title).toBe('patched')
    expect(result.sessions[0].isOngoing).toBe(true)
  })

  test('cursor=null 也透传 null（首次加载后 cursor 已耗尽场景）', () => {
    const prev = [patched('s1', 1000, 'a')]
    const firstPage = [skel('s1', 1000)]
    const result = applySilentRefresh(prev, null, firstPage)
    expect(result.nextCursor).toBeNull()
  })

  test('firstPage 含 prev 未有的新 session 时追加到合并列表', () => {
    const prev = [patched('s1', 1000, 'old')]
    const firstPage = [skel('s2', 3000), skel('s1', 1000)]
    const result = applySilentRefresh(prev, prevCursor, firstPage)
    expect(result.sessions.map((s) => s.sessionId)).toEqual(['s2', 's1'])
  })
})
