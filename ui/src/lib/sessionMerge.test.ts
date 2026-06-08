import { describe, expect, test } from 'vitest'
import type { SessionMetadataUpdate, SessionSummary } from './api'
import {
  applyPendingMetadata,
  applySilentRefresh,
  mergeRecoveryResponse,
  mergeSessions,
  mergeSilentMetadata,
} from './sessionMerge'

function skel(id: string, ts: number, overrides: Partial<SessionSummary> = {}): SessionSummary {
  return {
    sessionId: id,
    projectId: 'projectA',
    timestamp: ts,
    created: ts,
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
        created: 2000,
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

  test('prev 为空时透传 firstPage 不报错', () => {
    const firstPage = [skel('s1', 1000), skel('s2', 500)]
    const result = applySilentRefresh([], prevCursor, firstPage)
    expect(result.sessions.map((s) => s.sessionId)).toEqual(['s1', 's2'])
    expect(result.nextCursor).toBe(prevCursor)
  })

  test('firstPage 为空时保留 prev 与 cursor', () => {
    const prev = [patched('s1', 1000, 'kept')]
    const result = applySilentRefresh(prev, prevCursor, [])
    expect(result.sessions).toEqual(prev)
    expect(result.nextCursor).toBe(prevCursor)
  })

  test('silent 合并不丢失任何 prev sessionId（scrollTop 锚定保障）', () => {
    const prev: SessionSummary[] = [
      patched('p1-a', 5000, 'page1-a'),
      patched('p1-b', 4500, 'page1-b'),
      patched('p2-a', 3500, 'page2-a'),
      patched('p2-b', 3000, 'page2-b'),
      patched('p3-a', 2000, 'page3-a'),
      patched('p3-b', 1500, 'page3-b'),
    ]
    // 第一页只覆盖 prev 前两条，prev 后四条（用户翻到的第二、三页内容）
    // 在新第一页响应中完全缺席——bug 触发场景。
    const firstPage = [skel('p1-a', 5000), skel('p1-b', 4500)]
    const result = applySilentRefresh(prev, prevCursor, firstPage)
    for (const prevSession of prev) {
      expect(result.sessions.some((s) => s.sessionId === prevSession.sessionId)).toBe(true)
    }
  })
})

function update(
  sessionId: string,
  title: string | null,
  overrides: Partial<SessionMetadataUpdate> = {},
): SessionMetadataUpdate {
  return {
    projectId: 'projectA',
    sessionId,
    title,
    messageCount: title ? 12 : 0,
    isOngoing: false,
    gitBranch: null,
    ...overrides,
  }
}

describe('applyPendingMetadata', () => {
  test('空 buffer 返回原数组（同引用语义）', () => {
    const arr = [skel('s1', 1000)]
    const empty = new Map<string, SessionMetadataUpdate>()
    expect(applyPendingMetadata(arr, empty)).toBe(arr)
  })

  test('buffer 中匹配 sessionId 的 update 应用到对应条目，其他不变', () => {
    const arr = [skel('s1', 1000), skel('s2', 2000), skel('s3', 3000)]
    const buffer = new Map<string, SessionMetadataUpdate>([
      ['s2', update('s2', '真标题', { messageCount: 42, isOngoing: true, gitBranch: 'main' })],
    ])
    const result = applyPendingMetadata(arr, buffer)
    expect(result[0]).toEqual(arr[0])
    expect(result[1]).toEqual({
      sessionId: 's2',
      projectId: 'projectA',
      timestamp: 2000,
      created: 2000,
      messageCount: 42,
      title: '真标题',
      isOngoing: true,
      gitBranch: 'main',
    })
    expect(result[2]).toEqual(arr[2])
  })

  test('buffer 中 sessionId 不在 arr 时不抛错也不应用', () => {
    const arr = [skel('s1', 1000)]
    const buffer = new Map<string, SessionMetadataUpdate>([
      ['s_new', update('s_new', '只在 buffer 的')],
    ])
    const result = applyPendingMetadata(arr, buffer)
    expect(result).toEqual(arr)
  })

  test('race 兜底场景：listener 先收到 update，loadMoreSessions 后扩展 sessions 应用 buffer', () => {
    // page 1 sessions
    const page1 = [skel('p1-a', 5000), skel('p1-b', 4500)]
    // listener 收到 page 2 的 s_new update（page 2 IPC return 之前）
    const buffer = new Map<string, SessionMetadataUpdate>([
      ['s_new', update('s_new', '后台先扫到的标题', { messageCount: 5 })],
    ])
    // 此时 sessions = page1，apply buffer → s_new 不在 page1 → 无变化
    expect(applyPendingMetadata(page1, buffer)).toEqual(page1)

    // page 2 IPC return：sessions 扩展到含 s_new
    const expanded = mergeSessions(page1, [skel('s_new', 4000)], false)
    expect(expanded.some((s) => s.sessionId === 's_new')).toBe(true)
    // 立刻 apply buffer：s_new 应被 patch 上真实 title / messageCount
    const finalSessions = applyPendingMetadata(expanded, buffer)
    const newSession = finalSessions.find((s) => s.sessionId === 's_new')!
    expect(newSession.title).toBe('后台先扫到的标题')
    expect(newSession.messageCount).toBe(5)
  })

  test('buffer 中保留多个 update（不被 apply 后清空），后续 sessions 写入仍能复用', () => {
    const buffer = new Map<string, SessionMetadataUpdate>([
      ['a', update('a', 'A 标题')],
      ['b', update('b', 'B 标题')],
    ])
    const arr1 = [skel('a', 1000)]
    const result1 = applyPendingMetadata(arr1, buffer)
    expect(result1[0].title).toBe('A 标题')
    // buffer 仍含 b 的 update（apply 不删 entry）——后续 sessions 含 b 时仍生效
    expect(buffer.has('b')).toBe(true)

    const arr2 = [skel('a', 1000), skel('b', 500)]
    const result2 = applyPendingMetadata(arr2, buffer)
    expect(result2[1].title).toBe('B 标题')
  })
})

describe('mergeRecoveryResponse', () => {
  test('response 真值（cache hit）覆盖 prev stale 真值', () => {
    // 关键场景（codex 二审 round 5）：prev 是几分钟前的 stale 真值，
    // SSE 期间断/lagged，response 来自 cache hit 是当前最新值——SHALL
    // 用 response 覆盖 prev
    const prev = [patched('s1', 1000, 'old stale title', { messageCount: 5 })]
    const next = [patched('s1', 1500, 'new fresh title', { messageCount: 12 })]
    const result = mergeRecoveryResponse(prev, next)
    expect(result).toHaveLength(1)
    expect(result[0].title).toBe('new fresh title')
    expect(result[0].messageCount).toBe(12)
  })

  test('response 骨架（cache miss）保留 prev 已 patched 真值不被覆盖', () => {
    // 后端 cache miss 项 inline 返骨架；SSE patch 后续会通过 listener 写
    // 真值。这里 prev 已含真值（之前 listener 写入），SHALL 保留
    const prev = [patched('s1', 1000, 'patched title', { isOngoing: true })]
    const next = [skel('s1', 2000)]
    const result = mergeRecoveryResponse(prev, next)
    expect(result[0].title).toBe('patched title')
    expect(result[0].isOngoing).toBe(true)
  })

  test('response 真值与 prev 骨架时让 response 覆盖', () => {
    const prev = [skel('s1', 1000)]
    const next = [patched('s1', 1500, 'fresh', { gitBranch: 'main' })]
    const result = mergeRecoveryResponse(prev, next)
    expect(result[0].title).toBe('fresh')
    expect(result[0].gitBranch).toBe('main')
  })

  test('prev tail 不在 next 内的条目保留', () => {
    // 极端：response 因后端 sessions 列表已变（例如某 jsonl 被删）少了
    // 一条；不应让 prev 中那条凭空消失（删除走专门路径）
    const prev = [patched('s1', 2000, 'a'), patched('s2', 1000, 'b')]
    const next = [patched('s1', 2000, 'a updated')]
    const result = mergeRecoveryResponse(prev, next)
    expect(result).toHaveLength(2)
    expect(result.find((s) => s.sessionId === 's1')!.title).toBe('a updated')
    expect(result.find((s) => s.sessionId === 's2')!.title).toBe('b')
  })

  test('next 中新增 sessionId 直接加入', () => {
    const prev = [patched('s1', 2000, 'a')]
    const next = [patched('s1', 2000, 'a updated'), patched('s_new', 1500, 'new')]
    const result = mergeRecoveryResponse(prev, next)
    expect(result).toHaveLength(2)
    expect(result.find((s) => s.sessionId === 's_new')!.title).toBe('new')
  })

  test('结果按 timestamp 倒序排序', () => {
    const prev = [patched('a', 100, 'a'), patched('b', 200, 'b')]
    const next = [patched('a', 100, 'a updated'), patched('b', 200, 'b updated')]
    const result = mergeRecoveryResponse(prev, next)
    expect(result.map((s) => s.sessionId)).toEqual(['b', 'a'])
  })
})
