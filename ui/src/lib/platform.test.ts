// platform.ts unit test：mock navigator.userAgentData / navigator.platform 验证
// isMac() 平台分流。SessionDetail.svelte 的键盘快捷键 + tooltip 依赖此 helper，
// 平台判错会让 macOS 用户拿到 Ctrl+End tooltip / Win 用户拿到 ⌘+↓。

import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import { _resetPlatformCache, isMac } from './platform'

const origPlatform = Object.getOwnPropertyDescriptor(navigator, 'platform')
const origUserAgent = Object.getOwnPropertyDescriptor(navigator, 'userAgent')
const origUaData = Object.getOwnPropertyDescriptor(navigator, 'userAgentData')

function setNav(props: { uaData?: { platform: string }; platform?: string; userAgent?: string }) {
  Object.defineProperty(navigator, 'userAgentData', {
    value: props.uaData,
    configurable: true,
    writable: true,
  })
  Object.defineProperty(navigator, 'platform', {
    value: props.platform ?? '',
    configurable: true,
    writable: true,
  })
  Object.defineProperty(navigator, 'userAgent', {
    value: props.userAgent ?? '',
    configurable: true,
    writable: true,
  })
}

beforeEach(() => {
  _resetPlatformCache()
})

afterEach(() => {
  if (origPlatform) Object.defineProperty(navigator, 'platform', origPlatform)
  if (origUserAgent) Object.defineProperty(navigator, 'userAgent', origUserAgent)
  if (origUaData) Object.defineProperty(navigator, 'userAgentData', origUaData)
  else Object.defineProperty(navigator, 'userAgentData', { value: undefined, configurable: true })
  _resetPlatformCache()
})

describe('platform.isMac', () => {
  test('userAgentData.platform = "macOS" → true', () => {
    setNav({ uaData: { platform: 'macOS' } })
    expect(isMac()).toBe(true)
  })

  test('userAgentData.platform = "Windows" → false', () => {
    setNav({ uaData: { platform: 'Windows' } })
    expect(isMac()).toBe(false)
  })

  test('userAgentData 缺失，navigator.platform = "MacIntel" → true', () => {
    setNav({ platform: 'MacIntel' })
    expect(isMac()).toBe(true)
  })

  test('userAgentData 缺失，navigator.platform = "Win32" → false', () => {
    setNav({ platform: 'Win32' })
    expect(isMac()).toBe(false)
  })

  test('userAgentData 缺失，navigator.platform = "Linux x86_64" → false', () => {
    setNav({ platform: 'Linux x86_64' })
    expect(isMac()).toBe(false)
  })

  test('platform / userAgentData 都缺失，userAgent 含 Mac OS X → true', () => {
    setNav({ userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)' })
    expect(isMac()).toBe(true)
  })

  test('结果被 cache，后续调用不重读 navigator', () => {
    setNav({ platform: 'MacIntel' })
    expect(isMac()).toBe(true)
    // 改 navigator 但不 reset cache → 仍返回 cached
    setNav({ platform: 'Win32' })
    expect(isMac()).toBe(true)
    // 显式 reset 后重读
    _resetPlatformCache()
    expect(isMac()).toBe(false)
  })
})
