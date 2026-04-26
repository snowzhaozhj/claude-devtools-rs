// sidebarStore 宽度 clamp 单测。
//
// pin/hide 状态机依赖 invoke，已在 mockIPC + Playwright 集成测覆盖；
// 本文件仅测纯函数 clamp 逻辑，不引入 mockIPC 噪声。

import { describe, expect, test } from 'vitest'

import { getSidebarWidth, setSidebarWidth } from './sidebarStore.svelte'

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
