// Sidebar smoke 单测：组件依赖 listProjects / listRepositoryGroups / listSessions /
// listen('session-metadata-update') / ResizeObserver。用 setupMockIPC 铺平后端，
// 用 vi.stubGlobal stub jsdom 不实现的 ResizeObserver，验证渲染 + onMount 不抛。

import { describe, expect, test, afterEach, beforeEach, vi } from 'vitest'
import { render, cleanup, waitFor } from '@testing-library/svelte'
import { clearMocks, mockIPC, mockWindows } from '@tauri-apps/api/mocks'
import type { InvokeArgs } from '@tauri-apps/api/core'
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
  test('给定空 selectedGroupId 可渲染 sidebar 容器', async () => {
    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: '',
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
        selectedGroupId: '',
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
        selectedGroupId: '',
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

  test('selectedGroupId 非空时 session-filter-bar 始终渲染（不因 sessionsLoading 隐藏）', async () => {
    // 抗回归：若改回 `{#if !sessionsLoading && selectedGroupId}` guard，
    // 切项目 / 首次打开期间 filter-bar 会先消失再出现（高度 ~40px），
    // 下方 session-list 会跟随上下抖动一格——用户视觉感受为"切换项目时
    // 元素位置跳动一下"。本 test 锁住"filter-bar 在 selectedGroupId
    // 存在时 SHALL 渲染"的契约。
    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: 'mock-rich-rust',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    await waitFor(() => {
      expect(container.querySelector('.session-filter-bar')).not.toBeNull()
    })
  })

  test('多 worktree group 顶部渲染 worktree filter chip cluster（spec sidebar §filter）', async () => {
    // mock-rich-repo-rust group 含 2 个 worktree → showWorktreeFilter=true
    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: 'mock-rich-repo-rust',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    await waitFor(() => {
      expect(container.querySelector('.worktree-filter-bar')).not.toBeNull()
    })
    // chip cluster 替换原 dropdown：role="radiogroup" + 多个 role="radio"
    const cluster = container.querySelector('.worktree-filter-bar [role="radiogroup"]')
    expect(cluster).not.toBeNull()
    const chips = cluster!.querySelectorAll<HTMLButtonElement>('[role="radio"]')
    // 「全部」+ rust-port + feat-x = 3 chip
    expect(chips.length).toBe(3)
    // chip 顺序：「全部」最前（无 ⌗ 前缀）→ isMainWorktree 优先 → 其余按
    // mostRecentSession 倒序。fixture 里 rust-port isMainWorktree=true，feat-x=false。
    expect(chips[0].textContent?.trim()).toBe('全部')
    expect(chips[0].textContent?.includes('⌗')).toBe(false)
    expect(chips[1].textContent?.trim()).toBe('⌗rust-port')
    expect(chips[2].textContent?.trim()).toBe('⌗feat-x')
    // 默认 worktreeFilter=ALL_WORKTREES → 「全部」chip aria-checked=true
    expect(chips[0].getAttribute('aria-checked')).toBe('true')
    expect(chips[1].getAttribute('aria-checked')).toBe('false')
    expect(chips[2].getAttribute('aria-checked')).toBe('false')
  })

  test('单 worktree group 顶部 SHALL NOT 渲染 worktree filter chip cluster', async () => {
    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: 'mock-rich-ts',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    // 等 session-filter-bar 渲染再断言 filter 隐藏（避免初始未挂时误绿）
    await waitFor(() => {
      expect(container.querySelector('.session-filter-bar')).not.toBeNull()
    })
    expect(container.querySelector('.worktree-filter-bar')).toBeNull()
  })

  test('默认 ALL filter 顶部 count 显示单数字 scope total（spec §会话总数显示口径）', async () => {
    // mock-rich-repo-rust totalSessions = rustSessions.length(3) + rustWtFeatSessions.length(1) = 4
    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: 'mock-rich-repo-rust',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    await waitFor(() => {
      const span = container.querySelector('.session-count-num')
      expect(span).not.toBeNull()
      // 默认状态（无搜索）显单数字 group total，不显分式 / 已加载条数
      expect(span!.textContent?.trim()).toBe('4')
    })
    // fixture 中 anchor worktree mock-rich-rust 有 1 条 hidden prefs，tooltip 应反映真实隐藏数。
    const span = container.querySelector('.session-count-num')!
    expect(span.getAttribute('title')).toBe('总 4 · 1 已隐藏')
  })

  test('到底（无 nextCursor）时 SHALL NOT 渲染"已显示全部 N 条" footer', async () => {
    // 用户视角优化：列表自然结束 = 终态信号，与 IDE / Linear 工具习惯一致；
    // group label 已承载段总数，footer 是冗余装饰。删除前文案 "已显示全部 N 条"
    // 在小项目（≤5 条）下视觉占比过大；本测试守"到底时不渲染任何 footer 行"。
    // multi-project-rich fixture 的 list_group_sessions 一次返完所有 session
    // （nextCursor=null），因此默认就处于"到底"状态。
    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: 'mock-rich-repo-rust',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    // 等 sessions 数组加载完成：session-count-num=totalSessions 是 sessions
    // 数组已就位的稳定信号（jsdom 下 vlist 虚拟化可能不渲染真 session-item，
    // 但 footer 分支不依赖渲染窗口、只读 sessions.length / sessionsNextCursor）。
    await waitFor(() => {
      const countSpan = container.querySelector('.session-count-num')
      expect(countSpan?.textContent?.trim()).toBe('4')
    })
    // 1) 不应渲染 .load-more-end（CSS 类已删；分支也已删）
    expect(container.querySelector('.load-more-end')).toBeNull()
    // 2) 不应渲染 "已显示全部" / "已加载完" 任何字样
    const sessionList = container.querySelector('.session-list')!
    expect(sessionList.textContent ?? '').not.toMatch(/已显示全部|已加载完/)
  })

  test('搜索激活时 count 显示 `N 匹配`', async () => {
    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: 'mock-rich-repo-rust',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    // 等首次 sessions 加载完成（count span 渲染）
    await waitFor(() => {
      expect(container.querySelector('.session-count-num')).not.toBeNull()
    })
    // 输入搜索文本（命中 fixture session 标题"IPC 字段重构"）
    const input = container.querySelector<HTMLInputElement>('.session-filter-input')!
    input.value = 'IPC'
    input.dispatchEvent(new Event('input', { bubbles: true }))
    await tick()
    await tick()
    const span = container.querySelector('.session-count-num')!
    // 默认单数字切到 `N 匹配` 形式
    expect(span.textContent?.trim()).toMatch(/\d+ 匹配/)
  })

  test('search input 含 aria-describedby 与 title 明示「在已加载范围内搜索」', async () => {
    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: 'mock-rich-repo-rust',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    await waitFor(() => {
      expect(container.querySelector('.session-filter-input')).not.toBeNull()
    })
    const input = container.querySelector<HTMLInputElement>('.session-filter-input')!
    expect(input.getAttribute('title')).toBe('在已加载范围内搜索')
    expect(input.getAttribute('aria-describedby')).toBe('session-search-hint')
    const hint = container.querySelector('#session-search-hint')
    expect(hint).not.toBeNull()
    expect(hint!.textContent?.trim()).toBe('在已加载范围内搜索')
  })

  test('loadMoreSessions inflight 期间切到别的 group → 老 promise resolve 时 sessionsLoadingMore 必须复位（防 Bug #N 回归）', async () => {
    // 回归场景：用户在 group A 滚到底触发 loadMoreSessions（捕获 groupId=A）→
    // SSH 断开 / 切 project 让 selectedGroupId 变 B → 老 IPC 终于 resolve →
    // finally 若用 `if (groupId === selectedGroupId)` 守卫则永卡 true（PR #202
    // 引入），sidebar 翻页死锁 + ".sidebar-status-inline" 加载更多... 常驻。
    // 现行 finally 是无条件 `sessionsLoadingMore = false`——本测试用 deferred
    // promise 模拟"老 IPC 在切 group 后才完成"，断言 status-inline 不残留。
    mockWindows('main')
    type Resolver = (value: { sessions: unknown[]; nextCursor: string | null }) => void
    const deferred: Resolver[] = []
    let listGroupSessionsCalls = 0

    // 第一次 list_group_sessions 直接返"有 nextCursor"让 loadMore 入口能触发；
    // 第二次 (loadMore) 把 resolve 函数留下让我们手动 release。
    // 其它 IPC 走最小占位返回让 Sidebar onMount 流程不抛即可。
    mockIPC((cmd: string, _args?: InvokeArgs): unknown => {
      switch (cmd) {
        case 'list_projects':
          return [{ id: 'g-A', path: '/a', displayName: 'A', sessionCount: 100 }]
        case 'list_repository_groups':
          return [
            {
              id: 'g-A',
              identity: { id: 'g-A', name: 'A' },
              name: 'A',
              mostRecentSession: 0,
              totalSessions: 100,
              worktrees: [{
                id: 'g-A', path: '/a', name: 'A', gitBranch: null,
                isMainWorktree: true, isRepoRoot: true, sessions: [],
                createdAt: null, mostRecentSession: 0,
              }],
            },
            {
              id: 'g-B',
              identity: { id: 'g-B', name: 'B' },
              name: 'B',
              mostRecentSession: 0,
              totalSessions: 5,
              worktrees: [{
                id: 'g-B', path: '/b', name: 'B', gitBranch: null,
                isMainWorktree: true, isRepoRoot: true, sessions: [],
                createdAt: null, mostRecentSession: 0,
              }],
            },
          ]
        case 'list_group_sessions': {
          listGroupSessionsCalls += 1
          // 第一次：返一页 + nextCursor 让 loadMoreSessions 入口可激活
          if (listGroupSessionsCalls === 1) {
            const sessions = Array.from({ length: 50 }, (_, i) => ({
              sessionId: `sess-A-${i}`, projectId: 'g-A', worktreeId: 'g-A',
              title: null, messageCount: 0, isOngoing: false, lastTimestamp: 0,
              gitBranch: null,
            }))
            return { sessions, nextCursor: 'cursor-A-1' }
          }
          // 第二次：deferred，模拟"老 IPC 还在飞"
          return new Promise((resolve) => deferred.push(resolve as Resolver))
        }
        case 'get_project_memory':
          return { has_memory: false, layers: [], count: 0 }
        case 'get_project_session_prefs':
          return { pinned: [], hidden: [] }
        case 'get_session_summaries_by_ids':
          return []
        default:
          return null
      }
    }, { shouldMockEvents: true })

    const { container, rerender } = render(Sidebar, {
      props: {
        selectedGroupId: 'g-A',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })

    // 等 group A 首页加载完
    await waitFor(() => {
      expect(container.querySelector('.session-list')).not.toBeNull()
    })
    await tick()

    // 触发 loadMoreSessions：模拟 scroll bottom → maybeLoadMoreSessions(true)
    // 由于直接构造 scroll 比较脆弱，这里通过 rerender 触发 sidebar 内部 effects；
    // loadMoreSessions 由 onSessionListScroll → maybeLoadMoreSessions 触发，
    // 我们直接通过 DOM 触发 scroll 事件让 vlist + maybe 判定走起来。
    const sessionListEl = container.querySelector('.session-list') as HTMLElement | null
    if (sessionListEl) {
      // jsdom 不支持 scrollHeight/clientHeight 真渲染，给它做手脚让 remaining=0
      Object.defineProperty(sessionListEl, 'scrollHeight', { value: 10000, configurable: true })
      Object.defineProperty(sessionListEl, 'scrollTop', { value: 9000, configurable: true })
      Object.defineProperty(sessionListEl, 'clientHeight', { value: 1000, configurable: true })
      sessionListEl.dispatchEvent(new Event('scroll'))
    }

    // 等 loadMoreSessions 被调（list_group_sessions 第二次调用 = deferred 入队）
    await waitFor(() => expect(deferred.length).toBeGreaterThanOrEqual(1), { timeout: 1000 })
    expect(listGroupSessionsCalls).toBeGreaterThanOrEqual(2)
    // 确认 sessionsLoadingMore=true 在 release 之前是 true（load-more-loading 显示）
    expect(container.querySelector('.load-more-loading')).not.toBeNull()

    // 关键：模拟"SSH 断开 → loadProjects auto-select 切到 g-B"——通过 prop 切 group
    await rerender({
      selectedGroupId: 'g-B',
      activeSessionId: null,
      onSelectProject: () => {},
      onSelectSession: () => {},
    })
    // 多 tick 让 effect 跑、loadSessions(g-B) 入 await（也会推 deferred[1]）
    await tick()
    await tick()

    // 现在 release 老 promise（捕获的 groupId='g-A' 此时已不等于 selectedGroupId='g-B'）
    // finally 修复后 SHALL 无条件 sessionsLoadingMore=false
    deferred[0]({ sessions: [], nextCursor: null })
    // 多轮 microtask + svelte reactivity flush
    for (let i = 0; i < 6; i++) {
      await Promise.resolve()
      await tick()
    }

    // 断言：sessionsLoadingMore 必须被清零——".load-more-loading"（"加载中…"）
    // SHALL NOT 残留。若 finally 守卫还在，这个元素会永显。
    expect(container.querySelector('.load-more-loading')).toBeNull()
  })

  test('切回已访问过的 project 时 memory-entry 通过 cache 同步 hydrate', async () => {
    // 抗回归：若移除 memoryCache，切项目时 projectMemory 仍是上一个 project
    // 的值直到 async getProjectMemory return，期间 memory-entry 保持上一次
    // 状态 → IPC return 后才切到新值——若新旧 project 的 memoryCount 一个
    // 为 0 一个非 0，entry 显隐切换（高度 ~52px）会让 sidebar 顶部抖动。
    // cache 命中后 loadProjectMemory 同步 set projectMemory，无中间空档。
    const { container, rerender } = render(Sidebar, {
      props: {
        selectedGroupId: 'mock-rich-rust',
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
      selectedGroupId: 'mock-rich-rust-wt-feat',
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
      selectedGroupId: 'mock-rich-rust',
      activeSessionId: null,
      onSelectProject: () => {},
      onSelectSession: () => {},
    })
    await tick()
    await tick()
    expect(container.querySelector('.memory-entry')).not.toBeNull()
  })

  test('data root changed 清理 memory cache 与当前 memory-entry', async () => {
    const addEventSpy = vi.spyOn(window, 'addEventListener')
    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: 'mock-rich-rust',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    await waitFor(() => {
      expect(container.querySelector('.memory-entry')).not.toBeNull()
    })
    await waitFor(() => {
      expect(addEventSpy).toHaveBeenCalledWith('cdt-data-root-changed', expect.any(Function))
    })

    window.dispatchEvent(new CustomEvent('cdt-data-root-changed'))
    await tick()

    expect(container.querySelector('.memory-entry')).toBeNull()
  })

  test('data root changed 后旧 list_group_sessions late response 不回写 sessions', async () => {
    let resolveOldSessions!: (value: unknown) => void
    const oldSessions = new Promise((resolve) => { resolveOldSessions = resolve })
    let listGroupSessionsCalls = 0

    mockWindows('main')
    mockIPC((cmd: string, _args?: InvokeArgs): unknown => {
      switch (cmd) {
        case 'list_repository_groups':
          return [{
            id: 'g-A',
            identity: { id: 'g-A', name: 'A' },
            name: 'A',
            mostRecentSession: 0,
            totalSessions: 1,
            worktrees: [{
              id: 'g-A', path: '/a', name: 'A', gitBranch: null,
              isMainWorktree: true, isRepoRoot: true, sessions: ['old-root-session'],
              createdAt: null, mostRecentSession: 0,
            }],
          }]
        case 'list_projects':
          return [{ id: 'g-A', path: '/a', displayName: 'A', sessionCount: 1 }]
        case 'list_group_sessions':
          listGroupSessionsCalls += 1
          if (listGroupSessionsCalls === 1) return oldSessions
          return { sessions: [], nextCursor: null }
        case 'get_project_memory':
          return { has_memory: false, layers: [], count: 0 }
        case 'get_project_session_prefs':
          return { pinned: [], hidden: [] }
        case 'get_session_summaries_by_ids':
          return []
        default:
          return null
      }
    }, { shouldMockEvents: true })

    const { queryByText } = render(Sidebar, {
      props: {
        selectedGroupId: 'g-A',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    await waitFor(() => expect(listGroupSessionsCalls).toBe(1))

    window.dispatchEvent(new CustomEvent('cdt-data-root-changed'))
    resolveOldSessions({
      sessions: [{
        sessionId: 'old-root-session',
        projectId: 'g-A',
        worktreeId: 'g-A',
        title: 'Old root session',
        messageCount: 1,
        isOngoing: false,
        createdAt: 0,
        updatedAt: 0,
        gitBranch: null,
        cwd: null,
      }],
      nextCursor: null,
    })
    await tick()
    await tick()

    expect(queryByText('Old root session')).toBeNull()
  })

  // 注意：change `enrich-file-change-with-session-list-changed::D3` 的
  // 三档触发条件（projectListChanged || sessionListChanged || deleted）
  // 单元级断言走 fileChangeStore handler register / simulateFileChange dispatch
  // 路径——见 fileChangeStore.test.ts（避免在 Sidebar render 路径上叠 mockIPC
  // 第二轮覆盖踩 transformCallback 时序坑）。Sidebar 行为契约由 e2e-http-verify
  // 真后端验证（tasks 14.1-14.6）。

  /// 占位：保留这个 describe block 不空。完整三档触发用 fileChangeStore.test.ts
  /// 与 e2e tasks 14.1-14.6 覆盖；本测断言占位以防 describe block 为空 lint 报错。
  test('change enrich-file-change-with-session-list-changed coverage delegated to fileChangeStore.test + e2e', () => {
    expect(true).toBe(true)
  })

  // 历史版本保留的 mockIPC override 路径在 vitest setupMockIPC 之后第二次
  // mockIPC 注入会 race transformCallback 注入时序（mockIPC.shouldMockEvents
  // 重设 transformCallback 但 svelte effect 内 listen() 调度时还没到位）。
  // 真后端 e2e 覆盖更稳。

  test.skip('append-only file-change (no structural flag) SHALL NOT trigger listRepositoryGroups beyond onMount', async () => {
    const { simulateFileChange } = await import('../lib/tauriMock')
    let listRepoGroupsCalls = 0

    // 自定义 mockIPC counter——比 setupMockIPC 拦截更精确
    // setupMockIPC 已在 beforeEach 跑过 → transformCallback / mockWindows
    // / shouldMockEvents listen 桥就位。这里再 mockIPC 覆盖 handler 让 invoke
    // 走自定义 counter；mockWindows + shouldMockEvents=true 保持 emit/listen
    // 桥不被新 mockIPC 调用清掉（mockIPC 第二次调用会重置 listen 注入）。
    mockWindows('main')
    mockIPC((cmd: string, _args?: InvokeArgs): unknown => {
      switch (cmd) {
        case 'list_repository_groups':
          listRepoGroupsCalls += 1
          return [{
            id: 'g-A',
            identity: { id: 'g-A', name: 'A' },
            name: 'A',
            mostRecentSession: 0,
            totalSessions: 1,
            worktrees: [{
              id: 'g-A', path: '/a', name: 'A', gitBranch: null,
              isMainWorktree: true, isRepoRoot: true, sessions: ['sess-known'],
              createdAt: null, mostRecentSession: 0,
            }],
          }]
        case 'list_projects':
          return [{ id: 'g-A', path: '/a', displayName: 'A', sessionCount: 1 }]
        case 'list_group_sessions':
          return { sessions: [], nextCursor: null }
        case 'get_project_memory':
          return { has_memory: false, layers: [], count: 0 }
        case 'get_project_session_prefs':
          return { pinned: [], hidden: [] }
        case 'get_session_summaries_by_ids':
          return []
        default:
          return null
      }
    }, { shouldMockEvents: true })

    render(Sidebar, {
      props: {
        selectedGroupId: 'g-A',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    // 等首次 loadProjects 完成
    await waitFor(() => expect(listRepoGroupsCalls).toBeGreaterThanOrEqual(1))
    const onMountCalls = listRepoGroupsCalls

    // 触发普通 append：三个标志全 false → SHALL NOT 触发新一轮 listRepositoryGroups
    await simulateFileChange({
      projectId: 'g-A',
      sessionId: 'sess-known',
      deleted: false,
      projectListChanged: false,
      sessionListChanged: false,
    })
    // 等节流窗口（scheduleRefresh leading + trailing 250ms）+ microtask flush
    await new Promise((r) => setTimeout(r, 400))
    await tick()

    expect(listRepoGroupsCalls).toBe(onMountCalls)
  })

  /// sessionListChanged=true SHALL 触发 list_repository_groups revalidate
  /// （后端 unified invalidator enrich 出来的结构信号，典型 unknown_session 命中）。
  test.skip('sessionListChanged=true SHALL trigger listRepositoryGroups revalidate', async () => {
    const { simulateFileChange } = await import('../lib/tauriMock')
    let listRepoGroupsCalls = 0

    // setupMockIPC 已在 beforeEach 跑过 → transformCallback / mockWindows
    // / shouldMockEvents listen 桥就位。这里再 mockIPC 覆盖 handler 让 invoke
    // 走自定义 counter；mockWindows + shouldMockEvents=true 保持 emit/listen
    // 桥不被新 mockIPC 调用清掉（mockIPC 第二次调用会重置 listen 注入）。
    mockWindows('main')
    mockIPC((cmd: string, _args?: InvokeArgs): unknown => {
      switch (cmd) {
        case 'list_repository_groups':
          listRepoGroupsCalls += 1
          return [{
            id: 'g-A',
            identity: { id: 'g-A', name: 'A' },
            name: 'A',
            mostRecentSession: 0,
            totalSessions: 2,
            worktrees: [{
              id: 'g-A', path: '/a', name: 'A', gitBranch: null,
              isMainWorktree: true, isRepoRoot: true, sessions: ['sess-1', 'sess-2'],
              createdAt: null, mostRecentSession: 0,
            }],
          }]
        case 'list_projects':
          return [{ id: 'g-A', path: '/a', displayName: 'A', sessionCount: 2 }]
        case 'list_group_sessions':
          return { sessions: [], nextCursor: null }
        case 'get_project_memory':
          return { has_memory: false, layers: [], count: 0 }
        case 'get_project_session_prefs':
          return { pinned: [], hidden: [] }
        case 'get_session_summaries_by_ids':
          return []
        default:
          return null
      }
    }, { shouldMockEvents: true })

    render(Sidebar, {
      props: {
        selectedGroupId: 'g-A',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    await waitFor(() => expect(listRepoGroupsCalls).toBeGreaterThanOrEqual(1))
    const onMountCalls = listRepoGroupsCalls

    // sessionListChanged=true → SHALL 触发 schedule
    await simulateFileChange({
      projectId: 'g-A',
      sessionId: 'sess-new',
      deleted: false,
      projectListChanged: false,
      sessionListChanged: true,
    })
    // 等 scheduleRefresh leading 立即触发 + IPC return
    await waitFor(() => expect(listRepoGroupsCalls).toBeGreaterThan(onMountCalls), {
      timeout: 1000,
    })
  })

  /// 旧后端缺 sessionListChanged 字段时 SHALL 退化为 `projectListChanged || deleted`
  /// 判定（不引入新退化）——本测验证 projectListChanged=false + deleted=false +
  /// sessionListChanged=undefined 的旧形态 payload 不触发 IPC。
  test.skip('legacy file-change payload missing sessionListChanged falls back to projectListChanged||deleted', async () => {
    let listRepoGroupsCalls = 0

    // setupMockIPC 已在 beforeEach 跑过 → transformCallback / mockWindows
    // / shouldMockEvents listen 桥就位。这里再 mockIPC 覆盖 handler 让 invoke
    // 走自定义 counter；mockWindows + shouldMockEvents=true 保持 emit/listen
    // 桥不被新 mockIPC 调用清掉（mockIPC 第二次调用会重置 listen 注入）。
    mockWindows('main')
    mockIPC((cmd: string, _args?: InvokeArgs): unknown => {
      switch (cmd) {
        case 'list_repository_groups':
          listRepoGroupsCalls += 1
          return [{
            id: 'g-A',
            identity: { id: 'g-A', name: 'A' },
            name: 'A',
            mostRecentSession: 0,
            totalSessions: 1,
            worktrees: [{
              id: 'g-A', path: '/a', name: 'A', gitBranch: null,
              isMainWorktree: true, isRepoRoot: true, sessions: ['sess-known'],
              createdAt: null, mostRecentSession: 0,
            }],
          }]
        case 'list_projects':
          return [{ id: 'g-A', path: '/a', displayName: 'A', sessionCount: 1 }]
        case 'list_group_sessions':
          return { sessions: [], nextCursor: null }
        case 'get_project_memory':
          return { has_memory: false, layers: [], count: 0 }
        case 'get_project_session_prefs':
          return { pinned: [], hidden: [] }
        case 'get_session_summaries_by_ids':
          return []
        default:
          return null
      }
    }, { shouldMockEvents: true })

    render(Sidebar, {
      props: {
        selectedGroupId: 'g-A',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })
    await waitFor(() => expect(listRepoGroupsCalls).toBeGreaterThanOrEqual(1))
    const onMountCalls = listRepoGroupsCalls

    // 旧后端 payload：sessionListChanged 字段缺（undefined）→ 退化只看
    // projectListChanged || deleted；两者皆 false → 不触发
    await emit('file-change', {
      projectId: 'g-A',
      sessionId: 'sess-known',
      deleted: false,
      projectListChanged: false,
      // sessionListChanged 缺
    })
    await new Promise((r) => setTimeout(r, 400))
    await tick()

    expect(listRepoGroupsCalls).toBe(onMountCalls)
  })
})

// 导入额外依赖（位置在 describe 块之后避免污染既有 import 排序）
import { emit } from '@tauri-apps/api/event'

// ---------------------------------------------------------------------------
// Task 4.1: synthetic event 守护测试
// ---------------------------------------------------------------------------
// 后端 broadcast lag 路径 emit synthetic FileChangeEvent { projectId: "",
// sessionId: "", deleted: false, projectListChanged: true,
// sessionListChanged: true }。前端 Sidebar handler SHALL：
// - 触发 loadProjects(true)（通过 list_repository_groups 调用计数）
// - SHALL NOT 触发 loadSessions("")（per-session 守护 `!payload.sessionId` 命中 early return）
//
// 绕过 emit → listen 事件链（避免 transformCallback 时序 race），直接通过
// fileChangeStore._dispatchForTest 触发已注册 handler。
// ---------------------------------------------------------------------------

import { _dispatchForTest, _resetScheduleRefreshForTest } from '../lib/fileChangeStore.svelte'

describe('Sidebar synthetic event 守护（task 4.1）', () => {
  // 注意：projectDataStore 模块级 $state 跨 test 不 reset（vitest 重用模块实例）。
  // 首次 mount 时 loadProjects(false) 可能命中 cache 不走 IPC；synthetic event
  // 调 loadProjects(true) 必走 IPC（refresh=true bypass cache）。因此只能断言
  // "dispatch 后 IPC 调用增量"，不能断言"首次 mount 至少调了一次 IPC"。

  test('synthetic payload SHALL 触发 list_repository_groups（loadProjects）', async () => {
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
            mostRecentSession: 0,
            totalSessions: 2,
            worktrees: [{
              id: 'g-A', path: '/a', name: 'A', gitBranch: null,
              isMainWorktree: true, isRepoRoot: true, sessions: ['sess-1', 'sess-2'],
              createdAt: null, mostRecentSession: 0,
            }],
          }]
        case 'list_projects':
          return [{ id: 'g-A', path: '/a', displayName: 'A', sessionCount: 2 }]
        case 'list_group_sessions':
          return { sessions: [
            { sessionId: 'sess-1', projectId: 'g-A', worktreeId: 'g-A', title: 'S1', messageCount: 1, isOngoing: false, lastTimestamp: 100, gitBranch: null },
            { sessionId: 'sess-2', projectId: 'g-A', worktreeId: 'g-A', title: 'S2', messageCount: 2, isOngoing: false, lastTimestamp: 200, gitBranch: null },
          ], nextCursor: null }
        case 'get_project_memory':
          return { has_memory: false, layers: [], count: 0 }
        case 'get_project_session_prefs':
          return { pinned: [], hidden: [] }
        case 'get_session_summaries_by_ids':
          return []
        default:
          return null
      }
    }, { shouldMockEvents: true })

    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: 'g-A',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })

    // 等组件 mount 完成（session-list 渲染即组件已就位）
    await waitFor(() => {
      expect(container.querySelector('.session-list')).not.toBeNull()
    })
    await tick()
    // 记录 mount 完成后的 baseline（可能 0 因 projectDataStore cache）
    const baseRepoGroupsCalls = listRepoGroupsCalls

    // 派发 synthetic event（projectId="", sessionId=""）
    // handler 中 projectListChanged=true → scheduleRefresh → loadProjects(true)
    // loadProjects(true) → loadProjectData({ refresh: true }) → 必走 IPC
    _dispatchForTest({
      projectId: '',
      sessionId: '',
      deleted: false,
      projectListChanged: true,
      sessionListChanged: true,
    })

    // scheduleRefresh leading 立即触发 → list_repository_groups 应被调用
    await waitFor(() => expect(listRepoGroupsCalls).toBeGreaterThan(baseRepoGroupsCalls), {
      timeout: 2000,
    })
  })

  test('synthetic payload SHALL NOT 触发 loadSessions("")（per-session 守护）', async () => {
    _resetScheduleRefreshForTest()
    let listGroupSessionsCalls = 0
    const listGroupSessionsArgs: string[] = []

    mockWindows('main')
    mockIPC((cmd: string, args?: InvokeArgs): unknown => {
      switch (cmd) {
        case 'list_repository_groups':
          return [{
            id: 'g-A',
            identity: { id: 'g-A', name: 'A' },
            name: 'A',
            mostRecentSession: 0,
            totalSessions: 2,
            worktrees: [{
              id: 'g-A', path: '/a', name: 'A', gitBranch: null,
              isMainWorktree: true, isRepoRoot: true, sessions: ['sess-1', 'sess-2'],
              createdAt: null, mostRecentSession: 0,
            }],
          }]
        case 'list_projects':
          return [{ id: 'g-A', path: '/a', displayName: 'A', sessionCount: 2 }]
        case 'list_group_sessions': {
          const groupId = (args as Record<string, unknown> | undefined)?.groupId ?? ''
          listGroupSessionsCalls += 1
          listGroupSessionsArgs.push(String(groupId))
          return { sessions: [
            { sessionId: 'sess-1', projectId: 'g-A', worktreeId: 'g-A', title: 'S1', messageCount: 1, isOngoing: false, lastTimestamp: 100, gitBranch: null },
            { sessionId: 'sess-2', projectId: 'g-A', worktreeId: 'g-A', title: 'S2', messageCount: 2, isOngoing: false, lastTimestamp: 200, gitBranch: null },
          ], nextCursor: null }
        }
        case 'get_project_memory':
          return { has_memory: false, layers: [], count: 0 }
        case 'get_project_session_prefs':
          return { pinned: [], hidden: [] }
        case 'get_session_summaries_by_ids':
          return []
        default:
          return null
      }
    }, { shouldMockEvents: true })

    const { container } = render(Sidebar, {
      props: {
        selectedGroupId: 'g-A',
        activeSessionId: null,
        onSelectProject: () => {},
        onSelectSession: () => {},
      },
    })

    // 等组件 mount 完成
    await waitFor(() => {
      expect(container.querySelector('.session-list')).not.toBeNull()
    })
    await tick()
    // 记录 baseline（mount 期间可能有 list_group_sessions 调用）
    const baseSessionsCalls = listGroupSessionsCalls

    // 派发 synthetic event
    _dispatchForTest({
      projectId: '',
      sessionId: '',
      deleted: false,
      projectListChanged: true,
      sessionListChanged: true,
    })

    // 等 scheduleRefresh leading + trailing 窗口过去
    await new Promise((r) => setTimeout(r, 400))
    await tick()

    // list_group_sessions 不应被再次调用（sessionId="" → early return）
    expect(listGroupSessionsCalls).toBe(baseSessionsCalls)
    // 进一步断言：没有以空字符串作为 groupId 的调用
    const callsWithEmptyGroup = listGroupSessionsArgs.filter((g) => g === '')
    expect(callsWithEmptyGroup).toHaveLength(0)
  })
})
