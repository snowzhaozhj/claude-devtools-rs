// theme 单测（spec frontend-test-pyramid §"Vitest 单测覆盖纯逻辑层"）。
//
// 实际 applyTheme 仅设 data-theme attribute，'system' 模式由 CSS
// @media (prefers-color-scheme) 跟随，不在 JS 层 query matchMedia。
// 与原 spec 「system 跟随 prefers-color-scheme media query」描述的差异
// 已在 design D7c 修订。

import { afterEach, describe, expect, test } from 'vitest'

import { applyTheme } from './theme'

afterEach(() => {
  document.documentElement.removeAttribute('data-theme')
})

describe('applyTheme', () => {
  test('设置 light → data-theme=light', () => {
    applyTheme('light')
    expect(document.documentElement.getAttribute('data-theme')).toBe('light')
  })

  test('设置 dark → data-theme=dark', () => {
    applyTheme('dark')
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark')
  })

  test('设置 system → data-theme=system（由 CSS media query 处理）', () => {
    applyTheme('system')
    expect(document.documentElement.getAttribute('data-theme')).toBe('system')
  })

  test('多次调用以最后一次为准', () => {
    applyTheme('light')
    applyTheme('dark')
    applyTheme('system')
    expect(document.documentElement.getAttribute('data-theme')).toBe('system')
  })
})
