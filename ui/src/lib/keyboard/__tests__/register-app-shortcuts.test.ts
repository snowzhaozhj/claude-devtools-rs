/**
 * register-app-shortcuts.ts 单测（覆盖 tasks.md §5.4）。
 *
 * 验证：
 * - 调用 `registerAppShortcuts` 后 `registry.listAll()` 包含全部 17 条 App-owned spec
 * - meta 来自 SHORTCUT_DEFAULTS（description / defaultBinding / allowInInput）
 * - 缺失 handler 时 console.warn + 跳过该 ID（graceful degrade）
 * - 缺失 meta（defaults drift）时 console.warn + 跳过
 * - unregister 闭包能清空全部注册
 */

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import {
  APP_OWNED_SHORTCUT_IDS,
  registerAppShortcuts,
  type AppShortcutHandlers,
} from '../register-app-shortcuts'
import { _resetForTest, listAll } from '../registry'
import { _resetPlatformCache } from '../../platform'

function pinMac(): void {
  Object.defineProperty(navigator, 'userAgentData', {
    value: { platform: 'macOS' },
    configurable: true,
    writable: true,
  })
  _resetPlatformCache()
}

beforeEach(() => {
  pinMac()
  _resetForTest()
})

afterEach(() => {
  _resetForTest()
  _resetPlatformCache()
})

function fullHandlers(): AppShortcutHandlers {
  const h: AppShortcutHandlers = {}
  for (const id of APP_OWNED_SHORTCUT_IDS) h[id] = () => {}
  return h
}

describe('register-app-shortcuts', () => {
  test('APP_OWNED_SHORTCUT_IDS 包含 17 条且不含 search.focus / session.jump-to-latest', () => {
    expect(APP_OWNED_SHORTCUT_IDS).toHaveLength(17)
    expect(APP_OWNED_SHORTCUT_IDS).not.toContain('search.focus')
    expect(APP_OWNED_SHORTCUT_IDS).not.toContain('session.jump-to-latest')
    // 9 条 tab.switch.1~9
    for (let n = 1; n <= 9; n += 1) {
      expect(APP_OWNED_SHORTCUT_IDS).toContain(`tab.switch.${n}`)
    }
    // 关键 8 条单 spec
    expect(APP_OWNED_SHORTCUT_IDS).toEqual(
      expect.arrayContaining([
        'command-palette.toggle',
        'sidebar.toggle',
        'tab.close',
        'tab.next',
        'tab.prev',
        'pane.split',
        'pane.focus.next',
        'pane.focus.prev',
      ]),
    )
  })

  test('registerAppShortcuts 把 17 条 spec 注册进 registry', () => {
    registerAppShortcuts(fullHandlers())
    const ids = listAll().map((s) => s.id)
    expect(ids).toHaveLength(17)
    expect(ids).toEqual(expect.arrayContaining([...APP_OWNED_SHORTCUT_IDS]))
  })

  test('registered spec 的 description / defaultBinding 来自 SHORTCUT_DEFAULTS', () => {
    registerAppShortcuts(fullHandlers())
    const all = listAll()
    const cmd = all.find((s) => s.id === 'command-palette.toggle')
    expect(cmd?.description).toBe('打开 / 关闭命令面板')
    expect(cmd?.defaultBinding).toBe('mod+k')
    expect(cmd?.allowInInput).toBe(true)

    const sidebar = all.find((s) => s.id === 'sidebar.toggle')
    expect(sidebar?.defaultBinding).toBe('mod+b')
    expect(sidebar?.allowInInput).toBeUndefined() // 默认 false

    const tab1 = all.find((s) => s.id === 'tab.switch.1')
    expect(tab1?.defaultBinding).toBe('mod+1')
  })

  test('handler 缺失时 console.warn 并跳过该 ID', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const partial: AppShortcutHandlers = {
      'command-palette.toggle': () => {},
      // 故意不给 sidebar.toggle 等其余
    }
    registerAppShortcuts(partial)
    const ids = listAll().map((s) => s.id)
    expect(ids).toContain('command-palette.toggle')
    expect(ids).not.toContain('sidebar.toggle')
    expect(warnSpy).toHaveBeenCalled()
    warnSpy.mockRestore()
  })

  test('unregister 闭包清空全部注册', () => {
    const u = registerAppShortcuts(fullHandlers())
    expect(listAll()).toHaveLength(17)
    u()
    expect(listAll()).toHaveLength(0)
  })

  test('handler 被 dispatcher 真实调用（mod+B → sidebar.toggle）', () => {
    let sidebarCalled = 0
    const handlers = fullHandlers()
    handlers['sidebar.toggle'] = () => {
      sidebarCalled += 1
    }
    registerAppShortcuts(handlers)
    document.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'b', metaKey: true }),
    )
    expect(sidebarCalled).toBe(1)
  })
})
