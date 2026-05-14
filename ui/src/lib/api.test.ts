import { clearMocks } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, test } from 'vitest'

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
})
