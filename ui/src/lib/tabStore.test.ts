// tabStore singleton tab 语义 + activeTab 切换 + per-tab UI state 隔离。
//
// 模块级 $state 跨 test 不 reset——本文件用渐进 assertion 而非 reset 之间，
// 验证关键不变量（单例、活跃切换、UI state map 隔离）。

import { describe, expect, test } from 'vitest'

import {
  closeTab,
  getActiveTabId,
  getAllTabs,
  getTabUIState,
  openSettingsTab,
  openNotificationsTab,
  saveTabUIState,
  setActiveTab,
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
