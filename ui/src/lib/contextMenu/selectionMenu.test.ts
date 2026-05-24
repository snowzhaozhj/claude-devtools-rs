// window-level 文本选区菜单单测（Task 8 vitest 部分；Playwright e2e 留 task 9 后补）。
//
// 覆盖：
// - HMR 幂等：window sentinel + 重复 install 不重复注册
// - target 跳过白名单（input/textarea/contenteditable/data-allow-native-context）
// - 选区为空时跳过（不 preventDefault，让 Layer 3 兜底）
// - ctxProvider 返回 null 时跳过（启动期 settings 未就绪）
// - defaultPrevented 已被 Layer 1 拦截时跳过

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'
import { installSelectionContextMenu } from './selectionMenu'
import type { MenuItemContext } from './menu-items'

beforeEach(() => {
  document.body.innerHTML = ''
  // 清 sentinel 让每个测试都从 fresh install 状态开始
  delete (window as Window & { __cdtSelectionMenuInstalled?: boolean }).__cdtSelectionMenuInstalled
  delete (window as Window & { __cdtSelectionMenuCtxProvider?: unknown }).__cdtSelectionMenuCtxProvider
  // 也清 Layer 3 全局 close listeners sentinel，避免污染
  delete (window as Window & { __cdtContextMenuCloseListenersInstalled?: boolean }).__cdtContextMenuCloseListenersInstalled
})

afterEach(() => {
  document.body.innerHTML = ''
  window.getSelection()?.removeAllRanges()
})

function makeMockCtx(overrides: Partial<MenuItemContext> = {}): MenuItemContext {
  return {
    sessionId: 's1',
    projectId: 'p1',
    settings: {
      externalEditor: 'system',
      searchEngine: { type: 'google' },
      terminalApp: 'terminal',
    },
    selectionText: '',
    dispatch: {
      copyToClipboard: vi.fn(() => Promise.resolve()),
      openInEditor: vi.fn(() => Promise.resolve()),
      openInTerminal: vi.fn(() => Promise.resolve()),
      revealInDir: vi.fn(() => Promise.resolve()),
      openUrl: vi.fn(() => Promise.resolve()),
    },
    ...overrides,
  }
}

function dispatchContextMenu(target: EventTarget, init: MouseEventInit = {}): MouseEvent {
  const e = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, button: 2, ...init })
  target.dispatchEvent(e)
  return e
}

function selectText(node: Node): void {
  const range = document.createRange()
  range.selectNodeContents(node)
  const sel = window.getSelection()!
  sel.removeAllRanges()
  sel.addRange(range)
}

describe('installSelectionContextMenu', () => {
  test('HMR 幂等：重复 install 仅注册一次 listener', () => {
    const provider1 = vi.fn(() => makeMockCtx())
    const provider2 = vi.fn(() => makeMockCtx())
    installSelectionContextMenu(provider1)
    installSelectionContextMenu(provider2)
    expect(window.__cdtSelectionMenuInstalled).toBe(true)
    // 第一次 install 写入 provider；第二次 install 因 sentinel 直接 return，
    // 不覆盖 provider（即第一个 install 调用方"赢"）
    expect(window.__cdtSelectionMenuCtxProvider).toBe(provider1)
  })

  test('选区为空时跳过（不 preventDefault）', () => {
    const provider = vi.fn(() => makeMockCtx())
    installSelectionContextMenu(provider)
    const div = document.createElement('div')
    div.textContent = 'some text'
    document.body.appendChild(div)
    // 不创建 selection
    const e = dispatchContextMenu(div)
    expect(e.defaultPrevented).toBe(false)
  })

  test('选区非空时 preventDefault + 调 ctxProvider', () => {
    const provider = vi.fn(() => makeMockCtx())
    installSelectionContextMenu(provider)
    const div = document.createElement('div')
    div.textContent = 'some text'
    document.body.appendChild(div)
    selectText(div)
    const e = dispatchContextMenu(div)
    // jsdom 支持 selection.toString，verify provider 被调
    expect(provider).toHaveBeenCalled()
    expect(e.defaultPrevented).toBe(true)
  })

  test('target 是 input：跳过（让浏览器原生菜单接管）', () => {
    const provider = vi.fn(() => makeMockCtx())
    installSelectionContextMenu(provider)
    const input = document.createElement('input')
    input.value = 'text'
    document.body.appendChild(input)
    // 即使有选区也跳过——input 走原生菜单
    selectText(input)
    const e = dispatchContextMenu(input)
    expect(provider).not.toHaveBeenCalled()
    expect(e.defaultPrevented).toBe(false)
  })

  test('target 是 textarea：跳过', () => {
    const provider = vi.fn(() => makeMockCtx())
    installSelectionContextMenu(provider)
    const ta = document.createElement('textarea')
    ta.value = 'text'
    document.body.appendChild(ta)
    selectText(ta)
    const e = dispatchContextMenu(ta)
    expect(provider).not.toHaveBeenCalled()
    expect(e.defaultPrevented).toBe(false)
  })

  test('target 是 contenteditable: 跳过', () => {
    const provider = vi.fn(() => makeMockCtx())
    installSelectionContextMenu(provider)
    const div = document.createElement('div')
    div.setAttribute('contenteditable', 'true')
    div.textContent = 'editable'
    document.body.appendChild(div)
    selectText(div)
    const e = dispatchContextMenu(div)
    expect(provider).not.toHaveBeenCalled()
    expect(e.defaultPrevented).toBe(false)
  })

  test('target 是 [data-allow-native-context]：跳过', () => {
    const provider = vi.fn(() => makeMockCtx())
    installSelectionContextMenu(provider)
    const div = document.createElement('div')
    div.setAttribute('data-allow-native-context', '')
    div.textContent = 'native ok'
    document.body.appendChild(div)
    selectText(div)
    const e = dispatchContextMenu(div)
    expect(provider).not.toHaveBeenCalled()
    expect(e.defaultPrevented).toBe(false)
  })

  test('event.defaultPrevented 已设：跳过（Layer 1 已处理）', () => {
    const provider = vi.fn(() => makeMockCtx())
    installSelectionContextMenu(provider)
    const div = document.createElement('div')
    div.textContent = 'some text'
    document.body.appendChild(div)
    selectText(div)
    // 模拟 Layer 1 surface action 已 preventDefault + stopPropagation
    div.addEventListener('contextmenu', (e) => {
      e.preventDefault()
      e.stopPropagation()
    }, { once: true })
    const e = dispatchContextMenu(div)
    // selection menu listener 因 stopPropagation 不会触发；即便触发也因 defaultPrevented 跳过
    expect(provider).not.toHaveBeenCalled()
    // defaultPrevented 由 Layer 1 设
    expect(e.defaultPrevented).toBe(true)
  })

  test('ctxProvider 返回 null 时跳过', () => {
    const provider = vi.fn(() => null)
    installSelectionContextMenu(provider)
    const div = document.createElement('div')
    div.textContent = 'some text'
    document.body.appendChild(div)
    selectText(div)
    const e = dispatchContextMenu(div)
    expect(provider).toHaveBeenCalled()
    expect(e.defaultPrevented).toBe(false)
  })
})
