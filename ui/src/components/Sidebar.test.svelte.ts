// Sidebar smoke 单测：组件依赖 listProjects / listRepositoryGroups / listSessions /
// listen('session-metadata-update') / ResizeObserver。用 setupMockIPC 铺平后端，
// 用 vi.stubGlobal stub jsdom 不实现的 ResizeObserver，验证渲染 + onMount 不抛。

import { describe, expect, test, afterEach, beforeEach, vi } from 'vitest'
import { render, cleanup, waitFor } from '@testing-library/svelte'
import { clearMocks } from '@tauri-apps/api/mocks'
import { tick } from 'svelte'

import Sidebar from './Sidebar.svelte'
import { setupMockIPC } from '../lib/tauriMock'

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeEach(() => {
  setupMockIPC('multi-project-rich')
  vi.stubGlobal('ResizeObserver', ResizeObserverStub)
})

afterEach(() => {
  cleanup()
  clearMocks()
  vi.unstubAllGlobals()
})

describe('Sidebar smoke', () => {
  test('给定空 selectedProjectId 可渲染 sidebar 容器', async () => {
    const { container } = render(Sidebar, {
      props: {
        selectedProjectId: '',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    // sidebar 根节点存在
    expect(container.querySelector('.sidebar, [class*=sidebar]')).not.toBeNull()
    // session-list 容器始终渲染（虚拟滚动入口）
    expect(container.querySelector('.session-list')).not.toBeNull()
    await tick()
  })

  test('onMount 后 listRepositoryGroups 返回 → onSelectProject 被调用', async () => {
    const onSelectProject = vi.fn()
    render(Sidebar, {
      props: {
        selectedProjectId: '',
        activeSessionId: null,
        onSelectProject,
        onSelectSession: () => {},
      },
    })
    // multi-project-rich fixture 至少含 1 个 project，loadProjects 异步完成后
    // 默认选中第一个 worktree → 触发 onSelectProject 回调。
    await waitFor(() => {
      expect(onSelectProject).toHaveBeenCalled()
    })
    const [id, name] = onSelectProject.mock.calls[0]
    expect(typeof id).toBe('string')
    expect(id.length).toBeGreaterThan(0)
    expect(typeof name).toBe('string')
  })

  test('collapsed=true 渲染不抛错', async () => {
    const { container } = render(Sidebar, {
      props: {
        selectedProjectId: '',
        activeSessionId: null,
        collapsed: true,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    await tick()
    // collapsed 状态下 sidebar 仍渲染 DOM（宽度由 CSS 控制，不影响节点存在）
    expect(container.querySelector('.session-list')).not.toBeNull()
  })
})
