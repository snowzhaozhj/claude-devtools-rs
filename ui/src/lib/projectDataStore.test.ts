import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, test, vi } from 'vitest'

import { loadProjectData } from './projectDataStore.svelte'
import type { RepositoryGroup } from './api'

const repositoryGroups: RepositoryGroup[] = [
  {
    id: 'group-1',
    identity: { id: 'repo-1', name: 'repo' },
    name: 'repo',
    mostRecentSession: 10,
    totalSessions: 2,
    worktrees: [
      {
        id: 'project-1',
        path: '/repo',
        name: 'repo',
        gitBranch: 'main',
        isMainWorktree: true,
        sessions: ['s1', 's2'],
        createdAt: null,
        mostRecentSession: 10,
      },
    ],
  },
]

afterEach(() => {
  clearMocks()
})

describe('projectDataStore', () => {
  test('并发加载复用同一次 listRepositoryGroups 请求并派生 projects', async () => {
    const calls: string[] = []
    mockIPC(vi.fn((cmd) => {
      calls.push(cmd)
      if (cmd === 'list_repository_groups') return repositoryGroups
      throw new Error(`unexpected command: ${cmd}`)
    }))

    const [first, second] = await Promise.all([
      loadProjectData({ refresh: true }),
      loadProjectData({ refresh: true }),
    ])

    expect(first).toBe(second)
    expect(calls).toEqual(['list_repository_groups'])
    expect(first.projects).toEqual([
      { id: 'project-1', path: '/repo', displayName: 'repo', sessionCount: 2 },
    ])
  })
})
