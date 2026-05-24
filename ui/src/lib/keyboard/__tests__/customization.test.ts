/**
 * customization.ts 单测：mergeOverrides / bootstrapOverrides / persistOverrides /
 * retryBootstrap 路径。
 *
 * 覆盖 tasks.md §4.10 / §4.14。
 *
 * mockIPC fixture 通过 setupMockIPC() 注入；fixture 默认 keyboardShortcuts={}，
 * 单测里手动 mockIPC 注入异常路径来测 IPC fallback。
 */

import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import {
  bootstrapOverrides,
  mergeOverrides,
  persistOverrides,
  retryBootstrap,
} from '../customization'
import { SHORTCUT_DEFAULTS } from '../defaults'
import {
  _resetForTest,
  getConfigLoadError,
  getPendingOverrides,
  registerShortcut,
  findConflict,
  type ShortcutSpec,
} from '../registry'
import { _resetPlatformCache } from '../../platform'

beforeEach(() => {
  Object.defineProperty(navigator, 'userAgentData', {
    value: { platform: 'macOS' },
    configurable: true,
    writable: true,
  })
  _resetPlatformCache()
  _resetForTest()
})

afterEach(() => {
  clearMocks()
  _resetForTest()
  _resetPlatformCache()
})

function makeSpec(over: Partial<ShortcutSpec> = {}): ShortcutSpec {
  return {
    id: 'sidebar.toggle',
    category: 'sidebar',
    description: 'sidebar',
    defaultBinding: 'mod+b',
    handler: () => {},
    ...over,
  }
}

// ---------------------------------------------------------------------------
// §4.10 mergeOverrides 纯函数：defaults + overrides + 幽灵 ID 跳过
// ---------------------------------------------------------------------------

describe('§4.10 mergeOverrides', () => {
  test('overrides 命中 default id → 进 result', () => {
    const r = mergeOverrides(SHORTCUT_DEFAULTS, { 'sidebar.toggle': 'mod+shift+b' })
    expect(r['sidebar.toggle']).toBe('mod+shift+b')
  })

  test('幽灵 ID（不在 defaults）→ 跳过', () => {
    const r = mergeOverrides(SHORTCUT_DEFAULTS, {
      'sidebar.toggle': 'mod+shift+b',
      'ghost.removed': 'mod+g',
    })
    expect(Object.keys(r)).toEqual(['sidebar.toggle'])
  })

  test('空字符串 / 非字符串值 → 跳过', () => {
    const r = mergeOverrides(SHORTCUT_DEFAULTS, {
      'sidebar.toggle': '',
      // @ts-expect-error 故意传错类型测净化
      'tab.close': null,
    })
    expect(r).toEqual({})
  })

  test('空 overrides → 空 result', () => {
    expect(mergeOverrides(SHORTCUT_DEFAULTS, {})).toEqual({})
  })

  // ---------------------------------------------------------------------------
  // 跨平台 binding 字面量迁移（spec keyboard-shortcuts::用户自定义覆盖
  // 「存量 meta / ctrl 字面量启动迁移为 mod」）
  // ---------------------------------------------------------------------------

  test('mac 录入的 meta+x 字面量迁移为 mod+x', () => {
    const r = mergeOverrides(SHORTCUT_DEFAULTS, {
      'command-palette.toggle': 'meta+shift+p',
    })
    expect(r['command-palette.toggle']).toBe('mod+shift+p')
  })

  test('win 录入的 ctrl+x 字面量迁移为 mod+x', () => {
    const r = mergeOverrides(SHORTCUT_DEFAULTS, {
      'sidebar.toggle': 'ctrl+b',
    })
    expect(r['sidebar.toggle']).toBe('mod+b')
  })

  test('已含 mod 的 binding 幂等返回', () => {
    const r = mergeOverrides(SHORTCUT_DEFAULTS, {
      'sidebar.toggle': 'mod+shift+b',
    })
    expect(r['sidebar.toggle']).toBe('mod+shift+b')
  })

  test('alt+ctrl+x 迁移为 alt+mod+x（仅替换主修饰键 ctrl）', () => {
    const r = mergeOverrides(SHORTCUT_DEFAULTS, {
      'sidebar.toggle': 'alt+ctrl+b',
    })
    expect(r['sidebar.toggle']).toBe('alt+mod+b')
  })

  test('mac 双修饰键 ctrl+meta+x 迁移为 ctrl+mod+x（meta 优先级 > ctrl）', () => {
    const r = mergeOverrides(SHORTCUT_DEFAULTS, {
      'sidebar.toggle': 'ctrl+meta+b',
    })
    expect(r['sidebar.toggle']).toBe('ctrl+mod+b')
  })

  test('异常字面量 meta+mod+x 迁移为 mod+x', () => {
    const r = mergeOverrides(SHORTCUT_DEFAULTS, {
      'sidebar.toggle': 'meta+mod+b',
    })
    expect(r['sidebar.toggle']).toBe('mod+b')
  })

  test('alt+x（无主修饰键）原样返回', () => {
    const r = mergeOverrides(SHORTCUT_DEFAULTS, {
      'sidebar.toggle': 'alt+b',
    })
    expect(r['sidebar.toggle']).toBe('alt+b')
  })
})

// ---------------------------------------------------------------------------
// §4.14 IPC 失败 fallback：bootstrapOverrides 抛 error → builtin defaults +
// configLoadError store 被 set
// ---------------------------------------------------------------------------

describe('§4.14 bootstrapOverrides IPC fallback', () => {
  test('getConfig 抛 error → setConfigLoadError 设上 reason，pendingOverrides 为空', async () => {
    mockIPC((cmd) => {
      if (cmd === 'get_config') return Promise.reject(new Error('ipc boom'))
      return Promise.resolve(null)
    })
    await bootstrapOverrides()
    expect(getConfigLoadError()).toMatch(/ipc boom/)
    expect(getPendingOverrides()).toEqual({})
  })

  test('getConfig 成功且 keyboardShortcuts 含一条 override → pendingOverrides 写入', async () => {
    mockIPC((cmd) => {
      if (cmd === 'get_config') {
        return Promise.resolve({
          general: { theme: 'light' },
          keyboardShortcuts: { 'sidebar.toggle': 'mod+shift+b' },
        })
      }
      return Promise.resolve(null)
    })
    await bootstrapOverrides()
    expect(getConfigLoadError()).toBeNull()
    expect(getPendingOverrides()).toEqual({ 'sidebar.toggle': 'mod+shift+b' })
  })

  test('getConfig 成功但 keyboardShortcuts 缺失 → pendingOverrides 为空（兜底）', async () => {
    mockIPC((cmd) => {
      if (cmd === 'get_config') return Promise.resolve({ general: {} })
      return Promise.resolve(null)
    })
    await bootstrapOverrides()
    expect(getConfigLoadError()).toBeNull()
    expect(getPendingOverrides()).toEqual({})
  })
})

// ---------------------------------------------------------------------------
// persistOverrides：updateConfig section='keyboardShortcuts' 整体替换 + applyOverrides
// ---------------------------------------------------------------------------

describe('persistOverrides', () => {
  test('IPC 成功后 applyOverrides 触发 keymap rebuild', async () => {
    let captured: { section?: string; data?: Record<string, string> } = {}
    mockIPC((cmd, payload) => {
      if (cmd === 'update_config') {
        const args = payload as { section: string; configData: Record<string, string> }
        captured = { section: args.section, data: args.configData }
        return Promise.resolve({ keyboardShortcuts: args.configData })
      }
      return Promise.resolve(null)
    })

    // 已注册 sidebar.toggle 走 default mod+b
    registerShortcut(makeSpec({ id: 'sidebar.toggle', defaultBinding: 'mod+b' }))
    expect(findConflict('mod+b')).toBe('sidebar.toggle')

    await persistOverrides({ 'sidebar.toggle': 'mod+shift+b' })

    expect(captured.section).toBe('keyboardShortcuts')
    expect(captured.data).toEqual({ 'sidebar.toggle': 'mod+shift+b' })
    // applyOverrides 后 keymap：mod+b 空，mod+shift+b 占
    expect(findConflict('mod+b')).toBeNull()
    expect(findConflict('mod+shift+b')).toBe('sidebar.toggle')
  })

  test('IPC 失败 → throw 给调用方，registry 未变', async () => {
    mockIPC((cmd) => {
      if (cmd === 'update_config') return Promise.reject(new Error('write fail'))
      return Promise.resolve(null)
    })
    registerShortcut(makeSpec({ id: 'sidebar.toggle', defaultBinding: 'mod+b' }))
    await expect(
      persistOverrides({ 'sidebar.toggle': 'mod+shift+b' }),
    ).rejects.toThrow(/write fail/)
    // registry 未变
    expect(findConflict('mod+b')).toBe('sidebar.toggle')
    expect(findConflict('mod+shift+b')).toBeNull()
  })

  test('幽灵 ID 在 IPC 写入前被剥离', async () => {
    let captured: Record<string, string> | undefined
    mockIPC((cmd, payload) => {
      if (cmd === 'update_config') {
        const args = payload as { section: string; configData: Record<string, string> }
        captured = args.configData
        return Promise.resolve({ keyboardShortcuts: args.configData })
      }
      return Promise.resolve(null)
    })
    await persistOverrides({
      'sidebar.toggle': 'mod+shift+b',
      'ghost.x': 'mod+g',
    })
    expect(captured).toEqual({ 'sidebar.toggle': 'mod+shift+b' })
  })
})

// ---------------------------------------------------------------------------
// retryBootstrap：错误条上"重试"路径
// ---------------------------------------------------------------------------

describe('retryBootstrap', () => {
  test('成功 → 清空 configLoadError，applyOverrides 触发 keymap rebuild', async () => {
    // 第一次失败
    mockIPC((cmd) => {
      if (cmd === 'get_config') return Promise.reject(new Error('first fail'))
      return Promise.resolve(null)
    })
    await bootstrapOverrides()
    expect(getConfigLoadError()).toMatch(/first fail/)

    // 注册 spec（走 default）
    registerShortcut(makeSpec({ id: 'sidebar.toggle', defaultBinding: 'mod+b' }))

    // 第二次成功，带 override
    mockIPC((cmd) => {
      if (cmd === 'get_config') {
        return Promise.resolve({
          keyboardShortcuts: { 'sidebar.toggle': 'mod+shift+b' },
        })
      }
      return Promise.resolve(null)
    })
    await retryBootstrap()

    expect(getConfigLoadError()).toBeNull()
    // applyOverrides 已 rebuild keymap
    expect(findConflict('mod+b')).toBeNull()
    expect(findConflict('mod+shift+b')).toBe('sidebar.toggle')
  })

  test('失败 → 保留旧 error reason', async () => {
    mockIPC((cmd) => {
      if (cmd === 'get_config') return Promise.reject(new Error('still down'))
      return Promise.resolve(null)
    })
    await retryBootstrap()
    expect(getConfigLoadError()).toMatch(/still down/)
  })
})

