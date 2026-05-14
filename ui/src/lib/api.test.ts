import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, test, vi } from 'vitest'

import { getSessionSummariesByIds, listAllSessions, listSessions } from './api'
import { setupMockIPC } from './tauriMock'
import { multiProjectRichFixture } from './__fixtures__'
import type { Fixture } from './__fixtures__'
import type { SessionSummary } from './api'

function session(index: number): SessionSummary {
  return {
    sessionId: `sess-${index}`,
    projectId: 'project-with-history',
    timestamp: index,
    messageCount: index,
    title: `Session ${index}`,
    isOngoing: false,
    gitBranch: null,
  }
}

function fixtureWithSessions(sessions: SessionSummary[]): Fixture {
  return {
    ...multiProjectRichFixture,
    name: 'pagination-test',
    projects: [{ id: 'project-with-history', path: '/project', displayName: 'project', sessionCount: sessions.length }],
    sessions: { 'project-with-history': sessions },
    sessionDetails: {},
    prefs: {},
    searchResults: [],
  }
}

afterEach(() => {
  clearMocks()
})

describe('listSessions', () => {
  test('默认首屏 pageSize 为 20', async () => {
    const calls: Array<{ pageSize: number; cursor: string | null }> = []
    mockIPC(vi.fn((cmd, payload) => {
      expect(cmd).toBe('list_sessions')
      const args = payload as { pageSize: number; cursor: string | null }
      calls.push({ pageSize: args.pageSize, cursor: args.cursor })
      return { items: [], nextCursor: null, total: 0 }
    }))

    await listSessions('project-with-history')

    expect(calls).toEqual([{ pageSize: 20, cursor: null }])
  })
})

describe('getSessionSummariesByIds', () => {
  test('按输入 id 顺序补拉存在的 light summaries 并忽略缺失项', async () => {
    const sessions = [session(0), session(1), session(2)]
    setupMockIPC(fixtureWithSessions(sessions))

    const result = await getSessionSummariesByIds('project-with-history', [
      'sess-2',
      'sess-missing',
      'sess-0',
    ])

    expect(result.map((s) => s.sessionId)).toEqual(['sess-2', 'sess-0'])
  })
})

describe('listAllSessions', () => {
  test('加载超过默认 pageSize 的完整会话历史', async () => {
    const sessions = Array.from({ length: 51 }, (_, i) => session(i))
    setupMockIPC(fixtureWithSessions(sessions))

    const result = await listAllSessions('project-with-history')

    expect(result.items).toHaveLength(51)
    expect(result.items.map((s) => s.sessionId)).toEqual(sessions.map((s) => s.sessionId))
    expect(result.nextCursor).toBeNull()
    expect(result.total).toBe(51)
  })

  test('按 cursor 累加多页且不从头重拉全量', async () => {
    const calls: Array<{ pageSize: number; cursor: string | null }> = []
    const responses = [
      { items: Array.from({ length: 50 }, (_, i) => session(i)), nextCursor: '50', total: 120 },
      { items: Array.from({ length: 50 }, (_, i) => session(i + 50)), nextCursor: '100', total: 120 },
      { items: Array.from({ length: 20 }, (_, i) => session(i + 100)), nextCursor: null, total: 120 },
    ]
    mockIPC(vi.fn((cmd, payload) => {
      expect(cmd).toBe('list_sessions')
      const args = payload as { pageSize: number; cursor: string | null }
      calls.push({ pageSize: args.pageSize, cursor: args.cursor })
      return responses.shift()
    }))

    const result = await listAllSessions('project-with-history')

    expect(result.items).toHaveLength(120)
    expect(result.items.map((s) => s.sessionId)).toEqual(
      Array.from({ length: 120 }, (_, i) => `sess-${i}`),
    )
    expect(result.nextCursor).toBeNull()
    expect(result.total).toBe(120)
    expect(calls).toEqual([
      { pageSize: 50, cursor: null },
      { pageSize: 50, cursor: '50' },
      { pageSize: 50, cursor: '100' },
    ])
  })
})
