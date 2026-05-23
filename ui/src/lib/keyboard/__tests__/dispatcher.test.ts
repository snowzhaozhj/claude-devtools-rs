/**
 * registry dispatcher 守卫与命中单测。
 *
 * 覆盖 tasks.md §4.6 / §4.7 / §4.8 / §4.9 / §4.11 / §4.12 / §4.13。
 *
 * 走 `_dispatcherForTest()` 直接喂 KeyboardEvent；不依赖真 listener，避免 jsdom
 * `keydown` 事件冒泡 / dispatch 的边界差异。
 */

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import {
  _dispatcherForTest,
  _resetForTest,
  registerShortcut,
  resume,
  suspend,
  type ShortcutSpec,
} from '../registry'
import { _resetPlatformCache } from '../../platform'

function pinMac(mac = true): void {
  Object.defineProperty(navigator, 'userAgentData', {
    value: { platform: mac ? 'macOS' : 'Windows' },
    configurable: true,
    writable: true,
  })
  _resetPlatformCache()
}

function makeSpec(over: Partial<ShortcutSpec> = {}): ShortcutSpec {
  return {
    id: 'test',
    category: 'global',
    description: 'test',
    defaultBinding: 'mod+k',
    handler: () => {},
    ...over,
  }
}

/**
 * 构造一个 KeyboardEvent；jsdom 默认 isComposing/repeat/keyCode 都 false/0，
 * 用 Object.defineProperty 覆写来模拟 IME / 长按 / 229。
 */
function makeEvent(
  init: KeyboardEventInit & {
    isComposing?: boolean
    repeat?: boolean
    keyCode?: number
  } = {},
): KeyboardEvent {
  const evt = new KeyboardEvent('keydown', init)
  if (init.isComposing) {
    Object.defineProperty(evt, 'isComposing', { value: true, configurable: true })
  }
  if (init.repeat) {
    Object.defineProperty(evt, 'repeat', { value: true, configurable: true })
  }
  if (init.keyCode != null) {
    Object.defineProperty(evt, 'keyCode', { value: init.keyCode, configurable: true })
  }
  return evt
}

beforeEach(() => {
  pinMac(true)
  _resetForTest()
})

afterEach(() => {
  _resetForTest()
  _resetPlatformCache()
})

// ---------------------------------------------------------------------------
// §4.6 IME composition guard：isComposing / keyCode === 229 直接 return
// ---------------------------------------------------------------------------

describe('§4.6 IME composition guard', () => {
  test('isComposing=true → handler 不调', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    const dispatch = _dispatcherForTest()
    dispatch(makeEvent({ key: 'k', metaKey: true, isComposing: true }))
    expect(handler).not.toHaveBeenCalled()
  })

  test('keyCode=229 → handler 不调（中文 / 日文 IME 兼容）', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    const dispatch = _dispatcherForTest()
    dispatch(makeEvent({ key: 'k', metaKey: true, keyCode: 229 }))
    expect(handler).not.toHaveBeenCalled()
  })
})

// ---------------------------------------------------------------------------
// §4.11 key-repeat guard：event.repeat=true 直接 return
// ---------------------------------------------------------------------------

describe('§4.11 key-repeat guard', () => {
  test('event.repeat=true → handler 不调', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    const dispatch = _dispatcherForTest()
    dispatch(makeEvent({ key: 'k', metaKey: true, repeat: true }))
    expect(handler).not.toHaveBeenCalled()
  })

  test('event.repeat=false → handler 调', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    const dispatch = _dispatcherForTest()
    dispatch(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).toHaveBeenCalledTimes(1)
  })
})

// ---------------------------------------------------------------------------
// §4.7 input 焦点守卫：allowInInput=false 时 input focus 跳过
// ---------------------------------------------------------------------------

describe('§4.7 input 焦点守卫', () => {
  test('input focus + allowInInput=false → handler 不调', () => {
    const input = document.createElement('input')
    document.body.appendChild(input)
    input.focus()
    const handler = vi.fn()
    registerShortcut(
      makeSpec({ id: 'a', defaultBinding: 'mod+k', handler, allowInInput: false }),
    )
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).not.toHaveBeenCalled()
    input.remove()
  })

  test('input focus + allowInInput=true → handler 调（命令面板等需要）', () => {
    const input = document.createElement('input')
    document.body.appendChild(input)
    input.focus()
    const handler = vi.fn()
    registerShortcut(
      makeSpec({ id: 'a', defaultBinding: 'mod+k', handler, allowInInput: true }),
    )
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).toHaveBeenCalledTimes(1)
    input.remove()
  })

  test('textarea focus 同 input', () => {
    const ta = document.createElement('textarea')
    document.body.appendChild(ta)
    ta.focus()
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).not.toHaveBeenCalled()
    ta.remove()
  })

  test('contenteditable=true div focus 同 input', () => {
    const div = document.createElement('div')
    div.contentEditable = 'true'
    div.tabIndex = 0
    document.body.appendChild(div)
    div.focus()
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).not.toHaveBeenCalled()
    div.remove()
  })

  test('无 input focus → handler 正常调', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).toHaveBeenCalledTimes(1)
  })
})

// ---------------------------------------------------------------------------
// §4.8 suspend / resume 引用计数（多次 suspend / 部分 resume / 全部 resume）
// ---------------------------------------------------------------------------

describe('§4.8 suspend / resume 引用计数', () => {
  test('单次 suspend → handler 不调；resume 后恢复', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    suspend()
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).not.toHaveBeenCalled()
    resume()
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).toHaveBeenCalledTimes(1)
  })

  test('suspend 两次仅 resume 一次 → 仍 suspended', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    suspend()
    suspend()
    resume()
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).not.toHaveBeenCalled()
    resume()
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).toHaveBeenCalledTimes(1)
  })

  test('resume 多于 suspend 不抛错且地板 0', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    resume()
    resume()
    resume()
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).toHaveBeenCalledTimes(1)
  })
})

// ---------------------------------------------------------------------------
// §4.9 handler 返回 false 不 preventDefault
// ---------------------------------------------------------------------------

describe('§4.9 handler 返回 false 放行', () => {
  test('handler 返回 false → event.preventDefault 不调', () => {
    const handler = vi.fn(() => false as boolean)
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    const evt = makeEvent({ key: 'k', metaKey: true })
    const pd = vi.spyOn(evt, 'preventDefault')
    _dispatcherForTest()(evt)
    expect(handler).toHaveBeenCalled()
    expect(pd).not.toHaveBeenCalled()
  })

  test('handler 返回 undefined / true → preventDefault 调', () => {
    const handler = vi.fn(() => {})
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    const evt = makeEvent({ key: 'k', metaKey: true })
    const pd = vi.spyOn(evt, 'preventDefault')
    _dispatcherForTest()(evt)
    expect(pd).toHaveBeenCalled()
  })

  test('preventDefault: false 选项 → handler 命中也不 preventDefault', () => {
    const handler = vi.fn()
    registerShortcut(
      makeSpec({ id: 'a', defaultBinding: 'mod+k', handler, preventDefault: false }),
    )
    const evt = makeEvent({ key: 'k', metaKey: true })
    const pd = vi.spyOn(evt, 'preventDefault')
    _dispatcherForTest()(evt)
    expect(handler).toHaveBeenCalled()
    expect(pd).not.toHaveBeenCalled()
  })
})

// ---------------------------------------------------------------------------
// §4.12 non-mac metaKey 不识别为 mod
// ---------------------------------------------------------------------------

describe('§4.12 non-mac metaKey 不识别为 mod', () => {
  test('Win 平台 + metaKey=true → 不命中 mod+k', () => {
    pinMac(false)
    const handler = vi.fn()
    // 非 mac 环境下 mod+k 归一化为 ctrl+k；metaKey 单按不应触发
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).not.toHaveBeenCalled()
  })

  test('Win 平台 + ctrlKey=true → 命中 mod+k', () => {
    pinMac(false)
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    _dispatcherForTest()(makeEvent({ key: 'k', ctrlKey: true }))
    expect(handler).toHaveBeenCalledTimes(1)
  })

  test('Mac 平台 + metaKey=true → 命中 mod+k（baseline 反向验证）', () => {
    pinMac(true)
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    _dispatcherForTest()(makeEvent({ key: 'k', metaKey: true }))
    expect(handler).toHaveBeenCalledTimes(1)
  })
})

// ---------------------------------------------------------------------------
// §4.13 Numpad 数字键归一化：Numpad1 与 Digit1 命中同一 NormalizedKey
// ---------------------------------------------------------------------------

describe('§4.13 Numpad 归一化', () => {
  test('Numpad1 与 Digit1 命中同一 spec', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+1', handler }))
    const dispatch = _dispatcherForTest()
    // 顶部数字键
    dispatch(makeEvent({ key: '1', code: 'Digit1', metaKey: true }))
    // 数字小键盘
    dispatch(makeEvent({ key: '1', code: 'Numpad1', metaKey: true }))
    expect(handler).toHaveBeenCalledTimes(2)
  })

  test('NumpadEnter 与 Enter 命中同一 spec', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+Enter', handler }))
    const dispatch = _dispatcherForTest()
    dispatch(makeEvent({ key: 'Enter', code: 'Enter', metaKey: true }))
    dispatch(makeEvent({ key: 'Enter', code: 'NumpadEnter', metaKey: true }))
    expect(handler).toHaveBeenCalledTimes(2)
  })
})

// ---------------------------------------------------------------------------
// 命中算法基础：未匹配 binding 不调 handler
// ---------------------------------------------------------------------------

describe('未匹配不调 handler', () => {
  test('binding mod+k，敲 mod+l → 不调', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    _dispatcherForTest()(makeEvent({ key: 'l', metaKey: true }))
    expect(handler).not.toHaveBeenCalled()
  })

  test('单按修饰键（无主键）→ 不调（normalize 返回空串）', () => {
    const handler = vi.fn()
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k', handler }))
    _dispatcherForTest()(makeEvent({ key: 'Meta', metaKey: true }))
    expect(handler).not.toHaveBeenCalled()
  })
})
