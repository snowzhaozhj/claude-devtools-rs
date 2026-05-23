/**
 * platform.ts 键盘归一化 helper 单测：normalize / normalizeBinding /
 * resolveBinding / formatShortcut / canonicalKey / parseShortcut。
 *
 * 覆盖 tasks.md §4.5：normalize 修饰键顺序归一 / mac vs win mod 展开 /
 * `event.code` 兜底物理位置键 / `formatShortcut` 双平台输出。
 *
 * 这是与 platform.test.ts（仅测 isMac）解耦的键盘归一化专用单测。
 */

import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import {
  _resetPlatformCache,
  canonicalKey,
  formatShortcut,
  matchEvent,
  modKey,
  normalize,
  normalizeBinding,
  parseShortcut,
  resolveBinding,
} from '../../platform'

function pinMac(mac = true): void {
  Object.defineProperty(navigator, 'userAgentData', {
    value: { platform: mac ? 'macOS' : 'Windows' },
    configurable: true,
    writable: true,
  })
  _resetPlatformCache()
}

function evt(init: KeyboardEventInit): KeyboardEvent {
  return new KeyboardEvent('keydown', init)
}

beforeEach(() => {
  pinMac(true)
})

afterEach(() => {
  _resetPlatformCache()
})

// ---------------------------------------------------------------------------
// modKey + normalizeBinding 平台分流
// ---------------------------------------------------------------------------

describe('modKey / normalizeBinding 平台分流', () => {
  test('mac → modKey()=meta，"mod+k" 归一化为 "meta+k"', () => {
    pinMac(true)
    expect(modKey()).toBe('meta')
    expect(normalizeBinding('mod+k')).toBe('meta+k')
  })

  test('win → modKey()=ctrl，"mod+k" 归一化为 "ctrl+k"', () => {
    pinMac(false)
    expect(modKey()).toBe('ctrl')
    expect(normalizeBinding('mod+k')).toBe('ctrl+k')
  })

  test('修饰键大小写 / 别名容错：Cmd / Command / Ctrl / Control / Option / Opt', () => {
    pinMac(true)
    expect(normalizeBinding('CMD+k')).toBe('meta+k')
    expect(normalizeBinding('Command+K')).toBe('meta+k')
    expect(normalizeBinding('Ctrl+K')).toBe('ctrl+k')
    expect(normalizeBinding('Control+k')).toBe('ctrl+k')
    expect(normalizeBinding('Alt+k')).toBe('alt+k')
    expect(normalizeBinding('Option+k')).toBe('alt+k')
    expect(normalizeBinding('Opt+k')).toBe('alt+k')
  })

  test('修饰键按内部字母顺序排（alt < ctrl < meta < shift）', () => {
    pinMac(true)
    expect(normalizeBinding('shift+meta+k')).toBe('meta+shift+k')
    expect(normalizeBinding('shift+alt+ctrl+meta+k')).toBe('alt+ctrl+meta+shift+k')
  })

  test('字母键归小写、命名键保持 PascalCase', () => {
    expect(normalizeBinding('mod+K')).toBe('meta+k')
    expect(normalizeBinding('mod+ArrowDown')).toBe('meta+ArrowDown')
    expect(normalizeBinding('mod+Enter')).toBe('meta+Enter')
    expect(normalizeBinding('mod+Escape')).toBe('meta+Escape')
  })

  test('空 / 仅修饰键 / 非法 binding 返回空串', () => {
    expect(normalizeBinding('')).toBe('')
    expect(normalizeBinding('mod')).toBe('')
    expect(normalizeBinding('+++')).toBe('')
  })
})

// ---------------------------------------------------------------------------
// resolveBinding：双平台分支 { mac, other }
// ---------------------------------------------------------------------------

describe('resolveBinding 双平台分支', () => {
  test('mac 走 mac 分支', () => {
    pinMac(true)
    const r = resolveBinding({ mac: 'mod+ArrowDown', other: 'ctrl+End' })
    expect(r).toBe('meta+ArrowDown')
  })

  test('win 走 other 分支', () => {
    pinMac(false)
    const r = resolveBinding({ mac: 'mod+ArrowDown', other: 'ctrl+End' })
    expect(r).toBe('ctrl+End')
  })

  test('string binding 在两平台都用 mod 关键字展开', () => {
    pinMac(true)
    expect(resolveBinding('mod+k')).toBe('meta+k')
    pinMac(false)
    expect(resolveBinding('mod+k')).toBe('ctrl+k')
  })
})

// ---------------------------------------------------------------------------
// normalize KeyboardEvent
// ---------------------------------------------------------------------------

describe('normalize(event) 修饰键 + 主键归一', () => {
  test('mod+k on mac', () => {
    pinMac(true)
    expect(normalize(evt({ key: 'k', metaKey: true }))).toBe('meta+k')
  })

  test('mod+k on win → ctrl+k', () => {
    pinMac(false)
    expect(normalize(evt({ key: 'k', ctrlKey: true }))).toBe('ctrl+k')
  })

  test('修饰键内部字母顺序：shift+meta+k → meta+shift+k', () => {
    pinMac(true)
    expect(normalize(evt({ key: 'k', metaKey: true, shiftKey: true }))).toBe(
      'meta+shift+k',
    )
  })

  test('单按修饰键（key=Meta） → 空串', () => {
    pinMac(true)
    expect(normalize(evt({ key: 'Meta', metaKey: true }))).toBe('')
    expect(normalize(evt({ key: 'Control', ctrlKey: true }))).toBe('')
    expect(normalize(evt({ key: 'Alt', altKey: true }))).toBe('')
    expect(normalize(evt({ key: 'Shift', shiftKey: true }))).toBe('')
  })

  test('字母统一小写（K → k）', () => {
    pinMac(true)
    expect(normalize(evt({ key: 'K', metaKey: true, shiftKey: true }))).toBe(
      'meta+shift+k',
    )
  })
})

// ---------------------------------------------------------------------------
// canonicalKey：event.code 物理位置兜底（AZERTY / Dvorak）
// ---------------------------------------------------------------------------

describe('canonicalKey event.code 物理位置兜底', () => {
  test('Slash / BracketLeft / BracketRight / Backslash 用 code 解出 / [ ] \\', () => {
    expect(canonicalKey('/', 'Slash')).toBe('/')
    expect(canonicalKey('[', 'BracketLeft')).toBe('[')
    expect(canonicalKey(']', 'BracketRight')).toBe(']')
    expect(canonicalKey('\\', 'Backslash')).toBe('\\')
  })

  test('Numpad 数字键归一为顶部数字', () => {
    expect(canonicalKey('1', 'Numpad1')).toBe('1')
    expect(canonicalKey('9', 'Numpad9')).toBe('9')
  })

  test('Numpad 功能键归一为对应 main row', () => {
    expect(canonicalKey('Enter', 'NumpadEnter')).toBe('Enter')
    expect(canonicalKey('+', 'NumpadAdd')).toBe('+')
    expect(canonicalKey('-', 'NumpadSubtract')).toBe('-')
    expect(canonicalKey('*', 'NumpadMultiply')).toBe('*')
    expect(canonicalKey('/', 'NumpadDivide')).toBe('/')
    expect(canonicalKey('.', 'NumpadDecimal')).toBe('.')
  })

  test('Digit0-9 code 兜底（即便 key 是空 / 非数字）', () => {
    expect(canonicalKey('1', 'Digit1')).toBe('1')
    expect(canonicalKey('0', 'Digit0')).toBe('0')
  })

  test('Space 归一', () => {
    expect(canonicalKey(' ', 'Space')).toBe('Space')
    expect(canonicalKey('Space', 'Space')).toBe('Space')
  })

  test('修饰键自身返回空串', () => {
    expect(canonicalKey('Meta', 'MetaLeft')).toBe('')
    expect(canonicalKey('Control', 'ControlLeft')).toBe('')
    expect(canonicalKey('Alt', 'AltLeft')).toBe('')
    expect(canonicalKey('Shift', 'ShiftLeft')).toBe('')
  })
})

// ---------------------------------------------------------------------------
// formatShortcut 双平台显示
// ---------------------------------------------------------------------------

describe('formatShortcut 双平台显示', () => {
  test('mac 走 Apple HIG 顺序 ⌃⌥⇧⌘ + 主键大写', () => {
    pinMac(true)
    expect(formatShortcut('mod+k')).toBe('⌘K')
    expect(formatShortcut('mod+shift+k')).toBe('⇧⌘K')
    expect(formatShortcut('mod+alt+shift+k')).toBe('⌥⇧⌘K')
    expect(formatShortcut('ctrl+alt+shift+meta+k')).toBe('⌃⌥⇧⌘K')
  })

  test('mac 把方向键转 Unicode 箭头', () => {
    pinMac(true)
    expect(formatShortcut('mod+ArrowDown')).toBe('⌘↓')
    expect(formatShortcut('mod+ArrowUp')).toBe('⌘↑')
    expect(formatShortcut('mod+ArrowLeft')).toBe('⌘←')
    expect(formatShortcut('mod+ArrowRight')).toBe('⌘→')
  })

  test('win 走 Ctrl+Alt+Shift+K 文本前缀', () => {
    pinMac(false)
    expect(formatShortcut('mod+k')).toBe('Ctrl+K')
    expect(formatShortcut('mod+shift+k')).toBe('Ctrl+Shift+K')
    expect(formatShortcut('mod+alt+shift+k')).toBe('Ctrl+Alt+Shift+K')
  })

  test('双平台 binding 各自走对应分支', () => {
    pinMac(true)
    expect(formatShortcut({ mac: 'mod+ArrowDown', other: 'ctrl+End' })).toBe('⌘↓')
    pinMac(false)
    expect(formatShortcut({ mac: 'mod+ArrowDown', other: 'ctrl+End' })).toBe('Ctrl+End')
  })
})

// ---------------------------------------------------------------------------
// matchEvent + parseShortcut
// ---------------------------------------------------------------------------

describe('matchEvent', () => {
  test('event 正好匹配 binding', () => {
    pinMac(true)
    expect(matchEvent('mod+k', evt({ key: 'k', metaKey: true }))).toBe(true)
    expect(matchEvent('mod+shift+k', evt({ key: 'k', metaKey: true }))).toBe(false)
  })
})

describe('parseShortcut', () => {
  test('结构化 mods + key 返回', () => {
    pinMac(true)
    expect(parseShortcut('mod+shift+K')).toEqual({ mods: ['meta', 'shift'], key: 'k' })
  })

  test('归一化失败 → null', () => {
    expect(parseShortcut('+++')).toBeNull()
  })
})
