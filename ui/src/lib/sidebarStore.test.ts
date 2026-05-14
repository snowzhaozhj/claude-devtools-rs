// sidebarStore 宽度 clamp 单测。
//
// pin/hide 状态机依赖 invoke，已在 mockIPC + Playwright 集成测覆盖；
// 本文件仅测纯函数 clamp 逻辑，不引入 mockIPC 噪声。

import { describe, expect, test } from 'vitest'

import {
  getExpandedGroupIds,
  getSidebarCollapsed,
  getSidebarWidth,
  isGroupExpanded,
  setGroupExpanded,
  setSidebarCollapsed,
  setSidebarWidth,
  toggleGroupExpanded,
  toggleSidebarCollapsed,
} from './sidebarStore.svelte'

describe('sidebar width clamp', () => {
  test('正常值原样保留', () => {
    setSidebarWidth(300)
    expect(getSidebarWidth()).toBe(300)
  })

  test('低于 200 → clamp 到 200', () => {
    setSidebarWidth(50)
    expect(getSidebarWidth()).toBe(200)
  })

  test('高于 500 → clamp 到 500', () => {
    setSidebarWidth(800)
    expect(getSidebarWidth()).toBe(500)
  })

  test('边界值 200 / 500 原样保留', () => {
    setSidebarWidth(200)
    expect(getSidebarWidth()).toBe(200)
    setSidebarWidth(500)
    expect(getSidebarWidth()).toBe(500)
  })
})

describe('sidebar collapsed toggle', () => {
  test('toggleSidebarCollapsed 切换状态', () => {
    setSidebarCollapsed(false)
    expect(getSidebarCollapsed()).toBe(false)
    toggleSidebarCollapsed()
    expect(getSidebarCollapsed()).toBe(true)
    toggleSidebarCollapsed()
    expect(getSidebarCollapsed()).toBe(false)
  })

  test('setSidebarCollapsed 显式 setter', () => {
    setSidebarCollapsed(true)
    expect(getSidebarCollapsed()).toBe(true)
    setSidebarCollapsed(false)
    expect(getSidebarCollapsed()).toBe(false)
  })
})

describe('expandedGroupIds — repository group 折叠/展开', () => {
  test('toggle 在两态间切换', () => {
    setGroupExpanded('g-x', false)
    expect(isGroupExpanded('g-x')).toBe(false)
    toggleGroupExpanded('g-x')
    expect(isGroupExpanded('g-x')).toBe(true)
    toggleGroupExpanded('g-x')
    expect(isGroupExpanded('g-x')).toBe(false)
  })

  test('setGroupExpanded 显式 set 已是相同状态时为 no-op', () => {
    setGroupExpanded('g-y', true)
    expect(isGroupExpanded('g-y')).toBe(true)
    // 第二次同值 set，set 仍应是 expanded
    setGroupExpanded('g-y', true)
    expect(isGroupExpanded('g-y')).toBe(true)
    setGroupExpanded('g-y', false)
    expect(isGroupExpanded('g-y')).toBe(false)
  })

  test('多 group 折叠状态相互独立', () => {
    setGroupExpanded('g-a', true)
    setGroupExpanded('g-b', false)
    setGroupExpanded('g-c', true)
    expect(isGroupExpanded('g-a')).toBe(true)
    expect(isGroupExpanded('g-b')).toBe(false)
    expect(isGroupExpanded('g-c')).toBe(true)
    const ids = getExpandedGroupIds()
    expect(ids.has('g-a')).toBe(true)
    expect(ids.has('g-b')).toBe(false)
    expect(ids.has('g-c')).toBe(true)
  })
})
