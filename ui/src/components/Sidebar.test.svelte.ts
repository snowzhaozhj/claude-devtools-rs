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

  test('selectedProjectId 非空时 session-filter-bar 始终渲染（不因 sessionsLoading 隐藏）', async () => {
    // 抗回归：若改回 `{#if !sessionsLoading && selectedProjectId}` guard，
    // 切项目 / 首次打开期间 filter-bar 会先消失再出现（高度 ~40px），
    // 下方 session-list 会跟随上下抖动一格——用户视觉感受为"切换项目时
    // 元素位置跳动一下"。本 test 锁住"filter-bar 在 selectedProjectId
    // 存在时 SHALL 渲染"的契约。
    const { container } = render(Sidebar, {
      props: {
        selectedProjectId: 'mock-rich-rust',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    await waitFor(() => {
      expect(container.querySelector('.session-filter-bar')).not.toBeNull()
    })
  })

  test('切回已访问过的 project 时 memory-entry 通过 cache 同步 hydrate', async () => {
    // 抗回归：若移除 memoryCache，切项目时 projectMemory 仍是上一个 project
    // 的值直到 async getProjectMemory return，期间 memory-entry 保持上一次
    // 状态 → IPC return 后才切到新值——若新旧 project 的 memoryCount 一个
    // 为 0 一个非 0，entry 显隐切换（高度 ~52px）会让 sidebar 顶部抖动。
    // cache 命中后 loadProjectMemory 同步 set projectMemory，无中间空档。
    const { container, rerender } = render(Sidebar, {
      props: {
        selectedProjectId: 'mock-rich-rust',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    // 第一次访问：等首次 IPC return 后 memory-entry 出现（fixture 中
    // mock-rich-rust hasMemory=true count=3）
    await waitFor(() => {
      expect(container.querySelector('.memory-entry')).not.toBeNull()
    })
    // 切到无 memory 的项目（fixture 中 mock-rich-rust-wt-feat hasMemory=false）
    await rerender({
      selectedProjectId: 'mock-rich-rust-wt-feat',
      activeSessionId: null,
      onSelectProject: () => {},
      onSelectSession: () => {},
    })
    await waitFor(() => {
      expect(container.querySelector('.memory-entry')).toBeNull()
    })
    // 切回 mock-rich-rust：cache 命中后 SHALL 同步显示 memory-entry
    // （仅靠 svelte 1 个 reactivity microtask）。如果还要等 IPC return
    // 才显示，说明 cache 路径未生效。
    await rerender({
      selectedProjectId: 'mock-rich-rust',
      activeSessionId: null,
      onSelectProject: () => {},
      onSelectSession: () => {},
    })
    await tick()
    await tick()
    expect(container.querySelector('.memory-entry')).not.toBeNull()
  })
})
