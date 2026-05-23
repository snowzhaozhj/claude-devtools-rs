// platform.ts unit test：mock navigator.userAgentData / navigator.platform 验证
// isMac() 平台分流。SessionDetail.svelte 的键盘快捷键 + tooltip 依赖此 helper，
// 平台判错会让 macOS 用户拿到 Ctrl+End tooltip / Win 用户拿到 ⌘+↓。

import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import {
  _resetPlatformCache,
  formatShortcut,
  isMac,
  normalizeBindingToMod,
  recordBindingFromEvent,
} from './platform'

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

// 构造 KeyboardEvent 工厂——jsdom 下 KeyboardEvent 构造支持 init dict
function makeKeyEvent(init: {
  key: string
  code?: string
  ctrlKey?: boolean
  altKey?: boolean
  shiftKey?: boolean
  metaKey?: boolean
}): KeyboardEvent {
  return new KeyboardEvent('keydown', {
    key: init.key,
    code: init.code ?? '',
    ctrlKey: init.ctrlKey ?? false,
    altKey: init.altKey ?? false,
    shiftKey: init.shiftKey ?? false,
    metaKey: init.metaKey ?? false,
  })
}

describe('platform.recordBindingFromEvent', () => {
  test('macOS: Cmd+Shift+P → mod+shift+p', () => {
    setNav({ uaData: { platform: 'macOS' } })
    const ev = makeKeyEvent({ key: 'P', code: 'KeyP', metaKey: true, shiftKey: true })
    expect(recordBindingFromEvent(ev)).toBe('mod+shift+p')
  })

  test('Windows: Ctrl+Shift+P → mod+shift+p', () => {
    setNav({ uaData: { platform: 'Windows' } })
    const ev = makeKeyEvent({ key: 'P', code: 'KeyP', ctrlKey: true, shiftKey: true })
    expect(recordBindingFromEvent(ev)).toBe('mod+shift+p')
  })

  test('Linux: Ctrl+K → mod+k', () => {
    setNav({ uaData: { platform: 'Linux' } })
    const ev = makeKeyEvent({ key: 'k', code: 'KeyK', ctrlKey: true })
    expect(recordBindingFromEvent(ev)).toBe('mod+k')
  })

  test('macOS: 仅按 Cmd（无主键） → null', () => {
    setNav({ uaData: { platform: 'macOS' } })
    const ev = makeKeyEvent({ key: 'Meta', code: 'MetaLeft', metaKey: true })
    expect(recordBindingFromEvent(ev)).toBeNull()
  })

  test('macOS: Cmd+Ctrl+X → ctrl+mod+x（双修饰键，仅反写主修饰键 meta）', () => {
    setNav({ uaData: { platform: 'macOS' } })
    const ev = makeKeyEvent({
      key: 'X',
      code: 'KeyX',
      metaKey: true,
      ctrlKey: true,
    })
    // normalize 在 mac 输出按内部排序 alt < ctrl < meta < shift → "ctrl+meta+x"
    // recordBindingFromEvent 反写主修饰键 meta → mod，保留 ctrl
    expect(recordBindingFromEvent(ev)).toBe('ctrl+mod+x')
  })

  test('macOS: Alt+X（无主修饰键）→ alt+x（不反写）', () => {
    setNav({ uaData: { platform: 'macOS' } })
    const ev = makeKeyEvent({ key: 'X', code: 'KeyX', altKey: true })
    expect(recordBindingFromEvent(ev)).toBe('alt+x')
  })
})

describe('platform.normalizeBindingToMod', () => {
  // 该函数平台无关，但 isMac 默认 false（jsdom 无 navigator）；为保险显式 set
  beforeEach(() => {
    setNav({ uaData: { platform: 'Linux' } })
  })

  test('meta+x → mod+x', () => {
    expect(normalizeBindingToMod('meta+x')).toBe('mod+x')
  })

  test('ctrl+k → mod+k', () => {
    expect(normalizeBindingToMod('ctrl+k')).toBe('mod+k')
  })

  test('mod+x 幂等', () => {
    expect(normalizeBindingToMod('mod+x')).toBe('mod+x')
  })

  test('alt+ctrl+x → alt+mod+x（中间位置 ctrl 替换为 mod，alt 保留）', () => {
    expect(normalizeBindingToMod('alt+ctrl+x')).toBe('alt+mod+x')
  })

  test('shift+meta+p → shift+mod+p（用户手编非 sorted，token-level 找 meta 替换）', () => {
    expect(normalizeBindingToMod('shift+meta+p')).toBe('shift+mod+p')
  })

  test('meta+mod+x → mod+x（异常字面量，移除多余 meta 保留 mod）', () => {
    expect(normalizeBindingToMod('meta+mod+x')).toBe('mod+x')
  })

  test('ctrl+meta+x → ctrl+mod+x（mac 双修饰键 sort 结果，meta 优先级 > ctrl）', () => {
    expect(normalizeBindingToMod('ctrl+meta+x')).toBe('ctrl+mod+x')
  })

  test('alt+x 不变（无主修饰键）', () => {
    expect(normalizeBindingToMod('alt+x')).toBe('alt+x')
  })

  test('shift+x 不变（无主修饰键）', () => {
    expect(normalizeBindingToMod('shift+x')).toBe('shift+x')
  })

  test('F1 不变（无修饰键）', () => {
    expect(normalizeBindingToMod('F1')).toBe('F1')
  })

  test('单字符 x 不变', () => {
    expect(normalizeBindingToMod('x')).toBe('x')
  })

  test('空串返回空串', () => {
    expect(normalizeBindingToMod('')).toBe('')
  })

  test('mod+ctrl+x（mod 已含 + 多余 ctrl 在 modifier 位置）→ mod+x', () => {
    // 防御异常字面量：mod 已存在但还有多余 ctrl token
    expect(normalizeBindingToMod('mod+ctrl+x')).toBe('mod+x')
  })
})

describe('platform.formatShortcut Space 平台分流', () => {
  test('macOS: mod+Space → ⌘␣', () => {
    setNav({ uaData: { platform: 'macOS' } })
    expect(formatShortcut('mod+Space')).toBe('⌘␣')
  })

  test('Windows: mod+Space → Ctrl+Space', () => {
    setNav({ uaData: { platform: 'Windows' } })
    expect(formatShortcut('mod+Space')).toBe('Ctrl+Space')
  })

  test('macOS: 单 Space → ␣', () => {
    setNav({ uaData: { platform: 'macOS' } })
    expect(formatShortcut('Space')).toBe('␣')
  })

  test('Windows: 单 Space → Space', () => {
    setNav({ uaData: { platform: 'Windows' } })
    expect(formatShortcut('Space')).toBe('Space')
  })
})
