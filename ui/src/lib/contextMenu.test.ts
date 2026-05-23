// contextMenu 基础设施单测。
//
// 覆盖：
// - installGlobalContextMenuFallback 三态决策（白名单元素放行 / defaultPrevented
//   跳过 / 兜底 preventDefault）+ HMR 幂等
// - use:contextMenu action 的 smart-select 防护（mousedown 右键无选区时
//   preventDefault；有选区时不动）
// - portal mount 行为：菜单挂到 document.body；新 instance 替换旧；destroy 清理

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import {
  contextMenu,
  installGlobalContextMenuFallback,
  type ContextMenuItem,
} from './contextMenu.svelte'

// ---- 测试基础 ----

beforeEach(() => {
  document.body.innerHTML = ''
})

afterEach(() => {
  document.body.innerHTML = ''
  // 清理可能残留的 selection
  window.getSelection()?.removeAllRanges()
})

function dispatchContextMenu(target: EventTarget, init: MouseEventInit = {}): MouseEvent {
  const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, ...init })
  target.dispatchEvent(e)
  return e
}

function dispatchMouseDown(target: EventTarget, init: MouseEventInit = {}): MouseEvent {
  const e = new MouseEvent('mousedown', { bubbles: true, cancelable: true, ...init })
  target.dispatchEvent(e)
  return e
}

// ---- installGlobalContextMenuFallback ----

describe('installGlobalContextMenuFallback', () => {
  test('preventDefault 兜底未自处理的元素', () => {
    installGlobalContextMenuFallback()
    const div = document.createElement('div')
    document.body.appendChild(div)
    const e = dispatchContextMenu(div)
    expect(e.defaultPrevented).toBe(true)
  })

  test('input 元素放行系统菜单', () => {
    installGlobalContextMenuFallback()
    const input = document.createElement('input')
    document.body.appendChild(input)
    const e = dispatchContextMenu(input)
    expect(e.defaultPrevented).toBe(false)
  })

  test('textarea 元素放行', () => {
    installGlobalContextMenuFallback()
    const ta = document.createElement('textarea')
    document.body.appendChild(ta)
    const e = dispatchContextMenu(ta)
    expect(e.defaultPrevented).toBe(false)
  })

  test('contenteditable="true" 元素放行', () => {
    installGlobalContextMenuFallback()
    const div = document.createElement('div')
    div.setAttribute('contenteditable', 'true')
    document.body.appendChild(div)
    const e = dispatchContextMenu(div)
    expect(e.defaultPrevented).toBe(false)
  })

  test('contenteditable="" 空值放行（HTML 规范合法的 truthy 形式）', () => {
    installGlobalContextMenuFallback()
    const div = document.createElement('div')
    div.setAttribute('contenteditable', '')
    document.body.appendChild(div)
    const e = dispatchContextMenu(div)
    expect(e.defaultPrevented).toBe(false)
  })

  test('contenteditable="plaintext-only" 放行', () => {
    installGlobalContextMenuFallback()
    const div = document.createElement('div')
    div.setAttribute('contenteditable', 'plaintext-only')
    document.body.appendChild(div)
    const e = dispatchContextMenu(div)
    expect(e.defaultPrevented).toBe(false)
  })

  test('contenteditable="false" 仍然兜底 preventDefault（编辑被显式关闭）', () => {
    installGlobalContextMenuFallback()
    const div = document.createElement('div')
    div.setAttribute('contenteditable', 'false')
    document.body.appendChild(div)
    const e = dispatchContextMenu(div)
    expect(e.defaultPrevented).toBe(true)
  })

  test('contenteditable 父元素的子节点也放行（继承可编辑）', () => {
    installGlobalContextMenuFallback()
    const parent = document.createElement('div')
    parent.setAttribute('contenteditable', 'true')
    const child = document.createElement('span')
    child.textContent = 'child text'
    parent.appendChild(child)
    document.body.appendChild(parent)
    const e = dispatchContextMenu(child)
    expect(e.defaultPrevented).toBe(false)
  })

  test('contenteditable="false" 嵌套在 contenteditable="true" 内 → 兜底 preventDefault', () => {
    // 嵌套关闭可编辑：内部 false 区域不可编辑（HTML 规范），right-click 应走兜底。
    // 反例：用 [contenteditable]:not(="false") selector 跨祖先匹配会越过最近的
    // false 命中外层 true，错误放行——这是 codex PR 二审第二轮捕获的 bug。
    installGlobalContextMenuFallback()
    const outer = document.createElement('div')
    outer.setAttribute('contenteditable', 'true')
    const inner = document.createElement('div')
    inner.setAttribute('contenteditable', 'false')
    const target = document.createElement('span')
    target.textContent = 'inside disabled subtree'
    inner.appendChild(target)
    outer.appendChild(inner)
    document.body.appendChild(outer)
    const e = dispatchContextMenu(target)
    expect(e.defaultPrevented).toBe(true)
  })

  test('data-allow-native-context 元素放行', () => {
    installGlobalContextMenuFallback()
    const div = document.createElement('div')
    div.setAttribute('data-allow-native-context', '')
    document.body.appendChild(div)
    const e = dispatchContextMenu(div)
    expect(e.defaultPrevented).toBe(false)
  })

  test('元素已 preventDefault 时全局兜底跳过', () => {
    installGlobalContextMenuFallback()
    const div = document.createElement('div')
    document.body.appendChild(div)
    div.addEventListener('contextmenu', (e) => {
      e.preventDefault()
    })
    const e = dispatchContextMenu(div)
    expect(e.defaultPrevented).toBe(true)
    // 这里只能确认 defaultPrevented 仍为 true（即使再调一次 preventDefault 也是
    // 一样），关键不变量是"不报错、不重复阻止"——见下条幂等测试。
  })

  test('install 幂等：reset sentinel 后重新 add 一次，再调用不再叠加', () => {
    // 显式 reset window sentinel 模拟"app 启动后第一次调用"，验证：
    //   第一次 install → contextmenu listener 真正被 add 一次
    //   后续 install → 跳过，不再 add
    delete window.__cdtContextMenuFallbackInstalled
    const addSpy = vi.spyOn(window, 'addEventListener')
    installGlobalContextMenuFallback()
    let ctxAdds = addSpy.mock.calls.filter((c) => c[0] === 'contextmenu')
    expect(ctxAdds.length).toBe(1)
    installGlobalContextMenuFallback()
    installGlobalContextMenuFallback()
    ctxAdds = addSpy.mock.calls.filter((c) => c[0] === 'contextmenu')
    expect(ctxAdds.length).toBe(1) // 仍为 1，sentinel 拦住后续调用
    addSpy.mockRestore()
  })
})

// ---- use:contextMenu smart-select 防护 ----

describe('use:contextMenu smart-select 防护', () => {
  function makeNode(items: ContextMenuItem[]) {
    const node = document.createElement('div')
    node.textContent = '试探 hover 文字'
    document.body.appendChild(node)
    const action = contextMenu(node, items)
    return { node, action }
  }

  test('右键 mousedown 无选区时 preventDefault', () => {
    const { node, action } = makeNode([{ label: 'foo', action: () => {} }])
    // jsdom 下 window.getSelection() 默认 null/empty
    const e = dispatchMouseDown(node, { button: 2 })
    expect(e.defaultPrevented).toBe(true)
    action.destroy()
  })

  test('左键 mousedown 不影响', () => {
    const { node, action } = makeNode([{ label: 'foo', action: () => {} }])
    const e = dispatchMouseDown(node, { button: 0 })
    expect(e.defaultPrevented).toBe(false)
    action.destroy()
  })

  test('右键 mousedown 已有选区时不 preventDefault', () => {
    const { node, action } = makeNode([{ label: 'foo', action: () => {} }])
    // 模拟"已有选区"——mock window.getSelection
    const origGetSelection = window.getSelection.bind(window)
    const fakeSel = {
      toString: () => 'some selected text',
      removeAllRanges: () => {},
    } as unknown as Selection
    vi.spyOn(window, 'getSelection').mockReturnValue(fakeSel)
    const e = dispatchMouseDown(node, { button: 2 })
    expect(e.defaultPrevented).toBe(false)
    vi.mocked(window.getSelection).mockRestore()
    void origGetSelection
    action.destroy()
  })
})

// ---- use:contextMenu portal 行为 ----

describe('use:contextMenu portal mount', () => {
  test('右键触发后菜单 mount 到 document.body 末尾', () => {
    const node = document.createElement('div')
    document.body.appendChild(node)
    const action = contextMenu(node, [{ label: 'foo', action: () => {} }])
    const before = document.body.children.length
    dispatchContextMenu(node, { clientX: 50, clientY: 50 })
    const after = document.body.children.length
    expect(after).toBeGreaterThan(before)
    // 菜单根 host 是 body 末尾元素
    const last = document.body.lastElementChild as HTMLElement
    expect(last).not.toBeNull()
    expect(last.contains(node)).toBe(false)
    expect(last.querySelector('[role="menu"]')).not.toBeNull()
    action.destroy()
  })

  test('连续右键不同元素：仅保留 1 个菜单 instance', () => {
    const a = document.createElement('div')
    const b = document.createElement('div')
    document.body.appendChild(a)
    document.body.appendChild(b)
    const actA = contextMenu(a, [{ label: 'a', action: () => {} }])
    const actB = contextMenu(b, [{ label: 'b', action: () => {} }])
    dispatchContextMenu(a, { clientX: 10, clientY: 10 })
    dispatchContextMenu(b, { clientX: 100, clientY: 100 })
    const menus = document.body.querySelectorAll('[role="menu"]')
    expect(menus.length).toBe(1)
    actA.destroy()
    actB.destroy()
  })

  test('action destroy 清理菜单残骸', () => {
    const node = document.createElement('div')
    document.body.appendChild(node)
    const action = contextMenu(node, [{ label: 'foo', action: () => {} }])
    dispatchContextMenu(node, { clientX: 10, clientY: 10 })
    expect(document.body.querySelector('[role="menu"]')).not.toBeNull()
    action.destroy()
    expect(document.body.querySelector('[role="menu"]')).toBeNull()
  })

  test('Esc 关闭菜单', () => {
    const node = document.createElement('div')
    document.body.appendChild(node)
    const action = contextMenu(node, [{ label: 'foo', action: () => {} }])
    dispatchContextMenu(node, { clientX: 10, clientY: 10 })
    expect(document.body.querySelector('[role="menu"]')).not.toBeNull()
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    expect(document.body.querySelector('[role="menu"]')).toBeNull()
    action.destroy()
  })

  test('外点关闭菜单', () => {
    const node = document.createElement('div')
    document.body.appendChild(node)
    const action = contextMenu(node, [{ label: 'foo', action: () => {} }])
    dispatchContextMenu(node, { clientX: 10, clientY: 10 })
    expect(document.body.querySelector('[role="menu"]')).not.toBeNull()
    // 在菜单外的元素上 mousedown（用 body 自身）
    document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }))
    expect(document.body.querySelector('[role="menu"]')).toBeNull()
    action.destroy()
  })
})

// ---- use:contextMenu items provider 函数 ----

describe('use:contextMenu items provider', () => {
  test('static 数组：直接使用', () => {
    const node = document.createElement('div')
    document.body.appendChild(node)
    const action = contextMenu(node, [{ label: 'static', action: () => {} }])
    dispatchContextMenu(node, { clientX: 10, clientY: 10 })
    const menu = document.body.querySelector('[role="menu"]')
    expect(menu?.textContent).toContain('static')
    action.destroy()
  })

  test('函数 provider：每次右键调用一次', () => {
    const node = document.createElement('div')
    document.body.appendChild(node)
    const provider = vi.fn(() => [{ label: 'dynamic', action: () => {} }])
    const action = contextMenu(node, provider)
    dispatchContextMenu(node, { clientX: 10, clientY: 10 })
    expect(provider).toHaveBeenCalledTimes(1)
    // 关菜单后再触发，provider 再被调
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    dispatchContextMenu(node, { clientX: 10, clientY: 10 })
    expect(provider).toHaveBeenCalledTimes(2)
    action.destroy()
  })

  test('provider 返回空数组：不 mount 菜单', () => {
    const node = document.createElement('div')
    document.body.appendChild(node)
    const action = contextMenu(node, () => [])
    dispatchContextMenu(node, { clientX: 10, clientY: 10 })
    expect(document.body.querySelector('[role="menu"]')).toBeNull()
    action.destroy()
  })
})
