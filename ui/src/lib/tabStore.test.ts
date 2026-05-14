// tabStore singleton tab 语义 + activeTab 切换 + per-tab UI state 隔离。
//
// 模块级 $state 跨 test 不 reset——本文件用渐进 assertion 而非 reset 之间，
// 验证关键不变量（单例、活跃切换、UI state map 隔离）。

import { describe, expect, test } from 'vitest'

import {
  closeTab,
  getActiveTabId,
  getAllTabs,
  getPaneLayout,
  getSessionClickBehavior,
  getTabUIState,
  openOrReplaceTab,
  openSessionTab,
  openSettingsTab,
  openNotificationsTab,
  openMemoryTab,
  openTab,
  saveTabUIState,
  setActiveTab,
  setSessionClickBehavior,
} from './tabStore.svelte'

describe('singleton tab semantics', () => {
  test('openSettingsTab 多次只产生 1 个 settings tab', () => {
    openSettingsTab()
    const before = getAllTabs().filter((t) => t.type === 'settings').length
    openSettingsTab()
    openSettingsTab()
    const after = getAllTabs().filter((t) => t.type === 'settings').length
    expect(before).toBe(1)
    expect(after).toBe(1)
  })

  test('openNotificationsTab 同样单例', () => {
    openNotificationsTab()
    openNotificationsTab()
    expect(
      getAllTabs().filter((t) => t.type === 'notifications').length,
    ).toBe(1)
  })

  test('openMemoryTab 按 projectId 单例并激活已有 tab', () => {
    openMemoryTab('proj-memory-a')
    const first = getAllTabs().find((t) => t.type === 'memory' && t.projectId === 'proj-memory-a')!
    openMemoryTab('proj-memory-b')
    openMemoryTab('proj-memory-a')
    const memoryTabs = getAllTabs().filter((t) => t.type === 'memory')
    expect(memoryTabs.filter((t) => t.projectId === 'proj-memory-a')).toHaveLength(1)
    expect(memoryTabs.filter((t) => t.projectId === 'proj-memory-b')).toHaveLength(1)
    expect(getActiveTabId()).toBe(first.id)
  })
})

describe('active tab switching', () => {
  test('setActiveTab 切换 activeTabId', () => {
    openSettingsTab()
    const settingsTab = getAllTabs().find((t) => t.type === 'settings')
    expect(settingsTab).toBeDefined()
    setActiveTab(settingsTab!.id)
    expect(getActiveTabId()).toBe(settingsTab!.id)
  })

  test('closeTab 不存在的 id 不抛错', () => {
    expect(() => closeTab('nonexistent-id')).not.toThrow()
  })
})

describe('openOrReplaceTab 替换语义', () => {
  test('focused pane active 是 session tab → 原地替换 sessionId/projectId/label', () => {
    // 用 openTab 先制造一个 session tab + active
    openTab('sess-A', 'proj-X', 'Label A')
    const layoutBefore = getPaneLayout()
    const paneBefore = layoutBefore.panes.find(
      (p) => p.id === layoutBefore.focusedPaneId,
    )!
    const activeBefore = paneBefore.tabs.find(
      (t) => t.id === paneBefore.activeTabId,
    )!
    expect(activeBefore.type).toBe('session')
    expect(activeBefore.sessionId).toBe('sess-A')
    const tabIdBefore = activeBefore.id
    const tabsCountBefore = paneBefore.tabs.length

    openOrReplaceTab('sess-B', 'proj-Y', 'Label B')

    const layoutAfter = getPaneLayout()
    const paneAfter = layoutAfter.panes.find(
      (p) => p.id === layoutAfter.focusedPaneId,
    )!
    expect(paneAfter.tabs.length).toBe(tabsCountBefore) // 不新增 tab
    const activeAfter = paneAfter.tabs.find((t) => t.id === tabIdBefore)!
    // tabId 保留
    expect(paneAfter.activeTabId).toBe(tabIdBefore)
    // 内容已替换
    expect(activeAfter.sessionId).toBe('sess-B')
    expect(activeAfter.projectId).toBe('proj-Y')
    expect(activeAfter.label).toBe('Label B')
  })

  test('session 已在任意 pane 打开 → focus 已存在不重复 / 不替换', () => {
    openTab('sess-existing', 'proj-X', 'Existing')
    openTab('sess-other', 'proj-X', 'Other') // 另开一个变成 active
    const layoutBefore = getPaneLayout()
    const paneBefore = layoutBefore.panes.find(
      (p) => p.id === layoutBefore.focusedPaneId,
    )!
    const totalBefore = paneBefore.tabs.length

    openOrReplaceTab('sess-existing', 'proj-X', 'Existing v2')

    const layoutAfter = getPaneLayout()
    const paneAfter = layoutAfter.panes.find(
      (p) => p.id === layoutAfter.focusedPaneId,
    )!
    // 不新增 tab，激活的是已存在的 existing
    expect(paneAfter.tabs.length).toBe(totalBefore)
    const activeAfter = paneAfter.tabs.find((t) => t.id === paneAfter.activeTabId)!
    expect(activeAfter.sessionId).toBe('sess-existing')
    // 已存在 tab 不被替换为新 label
    expect(activeAfter.label).toBe('Existing')
  })
})

describe('openSessionTab 路由', () => {
  test('forceNewTab 不论 behavior 都开新 tab', () => {
    setSessionClickBehavior('replace')
    expect(getSessionClickBehavior()).toBe('replace')
    openTab('sess-base', 'proj-X', 'Base') // active 为 session tab
    const beforeLen = getPaneLayout().panes.find(
      (p) => p.id === getPaneLayout().focusedPaneId,
    )!.tabs.length

    openSessionTab('sess-new', 'proj-X', 'New', { forceNewTab: true })

    const after = getPaneLayout().panes.find(
      (p) => p.id === getPaneLayout().focusedPaneId,
    )!
    expect(after.tabs.length).toBe(beforeLen + 1)
    const active = after.tabs.find((t) => t.id === after.activeTabId)!
    expect(active.sessionId).toBe('sess-new')
  })

  test('behavior=new-tab 默认开新 tab', () => {
    setSessionClickBehavior('new-tab')
    openTab('sess-x', 'proj-X', 'X')
    const beforeLen = getPaneLayout().panes.find(
      (p) => p.id === getPaneLayout().focusedPaneId,
    )!.tabs.length

    openSessionTab('sess-fresh', 'proj-X', 'Fresh') // 默认走 new-tab

    const after = getPaneLayout().panes.find(
      (p) => p.id === getPaneLayout().focusedPaneId,
    )!
    expect(after.tabs.length).toBe(beforeLen + 1)
    setSessionClickBehavior('replace') // 还原全局默认避免污染后续 test
  })
})

describe('per-tab UI state isolation', () => {
  test('getTabUIState 返回独立对象，per-tab 隔离', () => {
    openSettingsTab()
    const tabs = getAllTabs()
    expect(tabs.length).toBeGreaterThan(0)
    const id1 = tabs[0].id

    const state1 = getTabUIState(id1)
    state1.searchVisible = true
    saveTabUIState(id1, state1)

    const reloaded = getTabUIState(id1)
    expect(reloaded.searchVisible).toBe(true)
  })
})
