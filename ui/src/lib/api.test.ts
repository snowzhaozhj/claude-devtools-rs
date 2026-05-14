import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, test, vi } from 'vitest'

import { listAllSessions } from './api'
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

  test('会话数量变化导致第二次仍有 nextCursor 时继续扩大请求', async () => {
    const calls: Array<{ pageSize: number; cursor: string | null }> = []
    const responses = [
      { items: Array.from({ length: 50 }, (_, i) => session(i)), nextCursor: '50', total: 51 },
      { items: Array.from({ length: 51 }, (_, i) => session(i)), nextCursor: '51', total: 52 },
      { items: Array.from({ length: 52 }, (_, i) => session(i)), nextCursor: null, total: 52 },
    ]
    mockIPC(vi.fn((cmd, payload) => {
      expect(cmd).toBe('list_sessions')
      const args = payload as { pageSize: number; cursor: string | null }
      calls.push({ pageSize: args.pageSize, cursor: args.cursor })
      return responses.shift()
    }))

    const result = await listAllSessions('project-with-history')

    expect(result.items).toHaveLength(52)
    expect(result.nextCursor).toBeNull()
    expect(calls).toEqual([
      { pageSize: 50, cursor: null },
      { pageSize: 51, cursor: null },
      { pageSize: 52, cursor: null },
    ])
  })
})
