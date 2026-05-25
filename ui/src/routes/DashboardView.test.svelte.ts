// DashboardView synthetic event 守护测试（task 4.2）:
// 后端 broadcast lag 路径 emit synthetic FileChangeEvent { projectId: "",
// sessionId: "", deleted: false, projectListChanged: true,
// sessionListChanged: true }。DashboardView handler SHALL 触发 loadData(true)
// 兜底全量（通过 list_repository_groups / list_projects 调用计数验证）。
//
// 绕过 emit → listen 事件链避免 transformCallback 时序 race，通过
// fileChangeStore._dispatchForTest 直接触发已注册 handler。

import { describe, expect, test, afterEach, beforeEach, vi } from 'vitest'
import { render, cleanup, waitFor } from '@testing-library/svelte'
import { mockIPC, mockWindows, clearMocks } from '@tauri-apps/api/mocks'
import type { InvokeArgs } from '@tauri-apps/api/core'
import { tick } from 'svelte'

import DashboardView from './DashboardView.svelte'
import { _dispatchForTest, _resetScheduleRefreshForTest } from '../lib/fileChangeStore.svelte'

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeEach(() => {
  vi.stubGlobal('ResizeObserver', ResizeObserverStub)
})

afterEach(() => {
  cleanup()
  clearMocks()
  vi.unstubAllGlobals()
})

describe('DashboardView synthetic event 守护（task 4.2）', () => {
  // 注意：projectDataStore 模块级 $state 跨 test 不 reset。首次 mount 的
  // loadData(false) 可能命中 cache 不走 IPC；synthetic event 触发的
  // loadData(true) 必走 IPC（refresh=true bypass cache）。

  test('synthetic payload SHALL 触发 loadData(true)（list_repository_groups 被调用）', async () => {
    _resetScheduleRefreshForTest()
    let listRepoGroupsCalls = 0

    mockWindows('main')
    mockIPC((cmd: string, _args?: InvokeArgs): unknown => {
      switch (cmd) {
        case 'list_repository_groups':
          listRepoGroupsCalls += 1
          return [{
            id: 'g-A',
            identity: { id: 'g-A', name: 'A' },
            name: 'A',
            mostRecentSession: Date.now(),
            totalSessions: 3,
            worktrees: [{
              id: 'g-A', path: '/a', name: 'A', gitBranch: null,
              isMainWorktree: true, isRepoRoot: true,
              sessions: ['s1', 's2', 's3'],
              createdAt: null, mostRecentSession: Date.now(),
            }],
          }]
        case 'list_projects':
          return [{ id: 'g-A', path: '/a', displayName: 'Project A', sessionCount: 3 }]
        case 'get_project_memory':
          return { has_memory: false, layers: [], count: 0 }
        case 'get_config':
          return {
            general: { theme: 'system', language: 'zh' },
            notifications: { enabled: true, triggers: [] },
          }
        default:
          return null
      }
    }, { shouldMockEvents: true })

    const { container } = render(DashboardView, {
      props: {
        selectedProjectId: 'g-A',
        onSelectProject: () => {},
      },
    })

    // 等组件 mount 完成（dashboard 内容渲染即就位）
    await waitFor(() => {
      expect(container.querySelector('.dash-list, .dash-grid, .dash-row')).not.toBeNull()
    })
    await tick()
    // 记录 baseline（mount 可能走 cache，counter 可能是 0）
    const baseRepoGroupsCalls = listRepoGroupsCalls

    // 派发 synthetic event
    _dispatchForTest({
      projectId: '',
      sessionId: '',
      deleted: false,
      projectListChanged: true,
      sessionListChanged: true,
    })

    // scheduleRefresh leading 立即触发 → loadData(true) → loadProjectData({ refresh: true }) → IPC
    await waitFor(() => expect(listRepoGroupsCalls).toBeGreaterThan(baseRepoGroupsCalls), {
      timeout: 2000,
    })
  })

  test('普通 append payload（三个标志全 false）SHALL NOT 触发 loadData', async () => {
    _resetScheduleRefreshForTest()
    let listRepoGroupsCalls = 0

    mockWindows('main')
    mockIPC((cmd: string, _args?: InvokeArgs): unknown => {
      switch (cmd) {
        case 'list_repository_groups':
          listRepoGroupsCalls += 1
          return [{
            id: 'g-A',
            identity: { id: 'g-A', name: 'A' },
            name: 'A',
            mostRecentSession: Date.now(),
            totalSessions: 3,
            worktrees: [{
              id: 'g-A', path: '/a', name: 'A', gitBranch: null,
              isMainWorktree: true, isRepoRoot: true,
              sessions: ['s1', 's2', 's3'],
              createdAt: null, mostRecentSession: Date.now(),
            }],
          }]
        case 'list_projects':
          return [{ id: 'g-A', path: '/a', displayName: 'Project A', sessionCount: 3 }]
        case 'get_project_memory':
          return { has_memory: false, layers: [], count: 0 }
        case 'get_config':
          return {
            general: { theme: 'system', language: 'zh' },
            notifications: { enabled: true, triggers: [] },
          }
        default:
          return null
      }
    }, { shouldMockEvents: true })

    const { container } = render(DashboardView, {
      props: {
        selectedProjectId: 'g-A',
        onSelectProject: () => {},
      },
    })

    // 等组件 mount 完成
    await waitFor(() => {
      expect(container.querySelector('.dash-list, .dash-grid, .dash-row')).not.toBeNull()
    })
    await tick()
    const baseRepoGroupsCalls = listRepoGroupsCalls

    // 派发普通 append（无结构性标志）
    _dispatchForTest({
      projectId: 'g-A',
      sessionId: 'sess-1',
      deleted: false,
      projectListChanged: false,
      sessionListChanged: false,
    })

    // 等 scheduleRefresh 窗口过去
    await new Promise((r) => setTimeout(r, 400))
    await tick()

    // 不应触发新的 list_repository_groups 调用
    expect(listRepoGroupsCalls).toBe(baseRepoGroupsCalls)
  })
})
