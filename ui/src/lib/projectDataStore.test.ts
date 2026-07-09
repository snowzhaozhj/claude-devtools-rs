import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, test, vi } from 'vitest'

import { loadProjectData, reloadProjectDataForRootSwitch, getProjectData } from './projectDataStore.svelte'
import type { RepositoryGroup } from './api'

const repositoryGroups: RepositoryGroup[] = [
  {
    id: 'group-1',
    identity: { id: 'repo-1', name: 'repo' },
    name: 'repo',
    mostRecentSession: 20,
    totalSessions: 3,
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
      {
        id: 'project-1-feat',
        path: '/repo/.claude/worktrees/feat-x',
        name: 'feat-x',
        gitBranch: 'feat/x',
        isMainWorktree: false,
        sessions: ['s3'],
        createdAt: null,
        mostRecentSession: 20,
      },
    ],
  },
]

afterEach(() => {
  clearMocks()
})

describe('projectDataStore', () => {
  test('并发加载复用同一次 listRepositoryGroups 请求并派生 dashboard 与 sidebar 数据', async () => {
    const calls: string[] = []
    mockIPC(vi.fn((cmd) => {
      calls.push(cmd)
      if (cmd === 'list_repository_groups') return repositoryGroups
      throw new Error(`unexpected command: ${cmd}`)
    }))

    const [first, second] = await Promise.all([
      loadProjectData({ refresh: true }),
      loadProjectData(),
    ])

    expect(first).toBe(second)
    expect(calls).toEqual(['list_repository_groups'])
    expect(first.projects).toEqual([
      // change `simplify-repository-as-project::D7`: projects[i].id 持 group.id
      // 而非 worktrees[0].id，使 ProjectSwitcher selectedProjectId 语义切到 group。
      { id: 'group-1', path: '/repo', displayName: 'repo', sessionCount: 3 },
    ])
    expect(first.worktreeProjects).toEqual([
      { id: 'project-1-feat', path: '/repo/.claude/worktrees/feat-x', displayName: 'feat-x', sessionCount: 1 },
      { id: 'project-1', path: '/repo', displayName: 'repo', sessionCount: 2 },
    ])
  })

  test('listRepositoryGroups 返回空数组时 fallback 到 listProjects', async () => {
    mockIPC(vi.fn((cmd) => {
      if (cmd === 'list_repository_groups') return []
      if (cmd === 'list_projects') {
        return [{ id: 'project-flat', path: '/flat', displayName: 'flat', sessionCount: 1 }]
      }
      throw new Error(`unexpected command: ${cmd}`)
    }))

    const result = await loadProjectData({ refresh: true })

    expect(result.repositoryGroups).toEqual([])
    expect(result.projects).toEqual([
      { id: 'project-flat', path: '/flat', displayName: 'flat', sessionCount: 1 },
    ])
    expect(result.worktreeProjects).toBe(result.projects)
  })

  test('root switch reload 丢弃旧 in-flight project data，只写入新 root 结果', async () => {
    let resolveOld: (v: RepositoryGroup[]) => void = () => {}
    const oldRequest = new Promise<RepositoryGroup[]>((resolve) => {
      resolveOld = resolve
    })
    const newGroups: RepositoryGroup[] = [
      {
        ...repositoryGroups[0],
        id: 'group-new-root',
        name: 'new-root',
        totalSessions: 1,
        worktrees: [{ ...repositoryGroups[0].worktrees[0], id: 'project-new-root', name: 'new-root', sessions: ['n1'] }],
      },
    ]
    mockIPC(vi.fn((cmd) => {
      if (cmd === 'list_repository_groups') return oldRequest
      throw new Error(`unexpected command: ${cmd}`)
    }))

    const stale = loadProjectData({ refresh: true })
    clearMocks()
    mockIPC(vi.fn((cmd) => {
      if (cmd === 'list_repository_groups') return newGroups
      throw new Error(`unexpected command: ${cmd}`)
    }))

    const fresh = await reloadProjectDataForRootSwitch()
    resolveOld(repositoryGroups)
    await expect(stale).rejects.toThrow(/superseded/)

    expect(fresh.projects[0].id).toBe('group-new-root')
    expect(getProjectData()?.projects[0].id).toBe('group-new-root')
  })
})
