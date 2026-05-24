/**
 * registry.ts 单测：注册 / 更新 / unregister / findConflict / listAll /
 * setPendingOverrides / applyOverrides 路径。
 *
 * 覆盖 tasks.md §4.1 / §4.2 / §4.3 / §4.4 / §4.15。
 *
 * 测试默认假定 mac 平台（modKey() = "meta"）；非 mac 路径走 dispatcher.test.ts。
 */

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import {
  _resetForTest,
  applyOverrides,
  findConflict,
  getPendingOverrides,
  listAll,
  registerShortcut,
  setPendingOverrides,
  unregister,
  update,
  type ShortcutSpec,
} from '../registry'
import { _resetPlatformCache } from '../../platform'

// 强制 mac 平台让 mod → meta
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

function makeSpec(over: Partial<ShortcutSpec> = {}): ShortcutSpec {
  return {
    id: 'test.shortcut',
    category: 'global',
    description: 'test',
    defaultBinding: 'mod+k',
    handler: () => {},
    ...over,
  }
}

// ---------------------------------------------------------------------------
// §4.1 注册 / 更新 / unregister 基本路径
// ---------------------------------------------------------------------------

describe('§4.1 register / update / unregister 基本路径', () => {
  test('registerShortcut 把 spec 写进 listAll 与 keymap', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    expect(listAll()).toHaveLength(1)
    expect(listAll()[0].id).toBe('a')
  })

  test('update 改 binding 后老 binding 不再命中、新 binding 命中', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    const r = update('a', 'mod+shift+k')
    expect(r.ok).toBe(true)
    // 老 binding 已让出
    expect(findConflict('mod+k')).toBeNull()
    // 新 binding 已被 'a' 占
    expect(findConflict('mod+shift+k')).toBe('a')
  })

  test('unregister 后 spec 与 keymap 都清掉，再调一次安全 no-op', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    unregister('a')
    expect(listAll()).toHaveLength(0)
    expect(findConflict('mod+k')).toBeNull()
    // 二次 unregister 不抛错
    expect(() => unregister('a')).not.toThrow()
  })

  test('registerShortcut 返回的 cleanup 闭包等价 unregister', () => {
    const off = registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    expect(listAll()).toHaveLength(1)
    off()
    expect(listAll()).toHaveLength(0)
    expect(findConflict('mod+k')).toBeNull()
  })

  test('update 把 meta+x 字面量归一为 mod+x（runtime 护栏）', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+l' }))
    const r = update('a', 'meta+shift+k')
    expect(r.ok).toBe(true)
    // 内存 keymap 走 mod 展开（mac 平台 mod → meta），所以 normalizeBinding('mod+shift+k') 与
    // normalizeBinding('meta+shift+k') 同 effective key——但护栏的核心是确保 update 不留
    // 平台特化字面量在 entry.effective 里。直接断言：findConflict 命中新展开 key
    expect(findConflict('meta+shift+k')).toBe('a')
    // 命中后老 default 也已让出
    expect(findConflict('mod+l')).toBeNull()
  })

  test('update 把 ctrl+x 字面量归一为 mod+x（dual-platform binding 护栏）', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+l' }))
    // 传 dual-platform 字面量，两边都不是 mod
    const r = update('a', { mac: 'meta+shift+k', other: 'ctrl+shift+k' })
    expect(r.ok).toBe(true)
    // 在 mac 平台上 effective 应该是 meta+shift+k（mac 分支被 normalizeBindingToMod
    // 转 mod+shift+k 后由 normalizeBinding 在 mac 展开为 meta+shift+k）
    expect(findConflict('meta+shift+k')).toBe('a')
  })
})

// ---------------------------------------------------------------------------
// §4.2 重复 ID 注册抛错
// ---------------------------------------------------------------------------

describe('§4.2 重复 ID 抛错', () => {
  test('同 ID 第二次 registerShortcut 抛 Error', () => {
    registerShortcut(makeSpec({ id: 'dup', defaultBinding: 'mod+k' }))
    expect(() =>
      registerShortcut(makeSpec({ id: 'dup', defaultBinding: 'mod+l' })),
    ).toThrow(/already registered/)
  })

  test('启动期 binding 冲突仅 console.warn，spec 注册成功但无 binding 占位', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    registerShortcut(makeSpec({ id: 'b', defaultBinding: 'mod+k' }))
    expect(listAll().map((s) => s.id).sort()).toEqual(['a', 'b'])
    // 'a' 占位 keymap，'b' 未占位
    expect(findConflict('mod+k')).toBe('a')
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })
})

// ---------------------------------------------------------------------------
// §4.3 findConflict 命中 / 排除自身 / null 返回
// ---------------------------------------------------------------------------

describe('§4.3 findConflict', () => {
  test('占用 binding 返回 conflict id', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    expect(findConflict('mod+k')).toBe('a')
  })

  test('未占用返回 null', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    expect(findConflict('mod+l')).toBeNull()
  })

  test('excludeId 排除自身，避免改键时把自己当冲突', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    expect(findConflict('mod+k', 'a')).toBeNull()
  })

  test('空 / 非法 binding 归一化失败 → 返回 null（不抛错）', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    expect(findConflict('')).toBeNull()
    expect(findConflict('+++')).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// §4.4 update 冲突时返回 Result.Err，不动 keymap
// ---------------------------------------------------------------------------

describe('§4.4 update 冲突', () => {
  test('改到已占用 binding → ok=false + Conflict + sourceId', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    registerShortcut(makeSpec({ id: 'b', defaultBinding: 'mod+l' }))
    const r = update('b', 'mod+k')
    expect(r.ok).toBe(false)
    if (!r.ok) {
      expect(r.error.kind).toBe('Conflict')
      expect(r.error.conflictId).toBe('a')
      expect(r.error.sourceId).toBe('b')
    }
    // keymap 不变：b 仍占 mod+l
    expect(findConflict('mod+l')).toBe('b')
    expect(findConflict('mod+k')).toBe('a')
  })

  test('未注册 ID → ok=false + conflictId="<unknown-id>"', () => {
    const r = update('ghost', 'mod+k')
    expect(r.ok).toBe(false)
    if (!r.ok) {
      expect(r.error.conflictId).toBe('<unknown-id>')
      expect(r.error.sourceId).toBe('ghost')
    }
  })
})

// ---------------------------------------------------------------------------
// §4.15 findConflict 接受 overlay：传入 pending overlay 视图命中
// ---------------------------------------------------------------------------

describe('§4.15 findConflict overlay', () => {
  test('overlay 把 b 移到 a 的当前 binding → 视图层冲突', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    registerShortcut(makeSpec({ id: 'b', defaultBinding: 'mod+l' }))
    // 把 a 暂时挪开（pending），让 c 可以占 mod+k
    const overlay = new Map<string, string>([
      ['a', 'mod+m'],
      ['b', 'mod+k'],
    ])
    // 查询：'mod+k' 在 overlay 视图里被 b 占，excluding 'b' 自己 → 应当无冲突
    expect(findConflict('mod+k', 'b', overlay)).toBeNull()
    // 但若 c 想去占 'mod+k'：b 已在 overlay 视图占 → 冲突
    expect(findConflict('mod+k', 'c', overlay)).toBe('b')
  })

  test('overlay 不传时只查实际 keymap', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    expect(findConflict('mod+k')).toBe('a')
  })

  test('overlay 把 a 自己挪开后，新位置应被识别为 a 占（而非 a 旧位置）', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    // overlay 把 a 挪到 mod+m
    const overlay = new Map<string, string>([['a', 'mod+m']])
    // 旧位置 'mod+k' 在视图里已被腾空 → 无冲突
    expect(findConflict('mod+k', 'b', overlay)).toBeNull()
    // 新位置 'mod+m' 在视图里被 a 占 → b 想占冲突
    expect(findConflict('mod+m', 'b', overlay)).toBe('a')
  })
})

// ---------------------------------------------------------------------------
// pendingOverrides + applyOverrides
// ---------------------------------------------------------------------------

describe('pendingOverrides / applyOverrides', () => {
  test('setPendingOverrides 后 register 用 override 替代 defaultBinding', () => {
    setPendingOverrides({ a: 'mod+shift+k' })
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    // override 生效：mod+shift+k 占位、mod+k 空闲
    expect(findConflict('mod+shift+k')).toBe('a')
    expect(findConflict('mod+k')).toBeNull()
  })

  test('applyOverrides 全量重建 keymap', () => {
    registerShortcut(makeSpec({ id: 'a', defaultBinding: 'mod+k' }))
    registerShortcut(makeSpec({ id: 'b', defaultBinding: 'mod+l' }))
    applyOverrides({ a: 'mod+shift+k' })
    // a 改到 mod+shift+k，b 仍 default
    expect(findConflict('mod+shift+k')).toBe('a')
    expect(findConflict('mod+l')).toBe('b')
    expect(findConflict('mod+k')).toBeNull()
  })

  test('getPendingOverrides 返回 snapshot 不暴露内部引用', () => {
    setPendingOverrides({ a: 'mod+k' })
    const snap = getPendingOverrides()
    snap.a = 'mod+l' // 改 snapshot
    expect(getPendingOverrides().a).toBe('mod+k')
  })
})

// ---------------------------------------------------------------------------
// listAll：保持注册顺序（Settings UI 渲染 stable）
// ---------------------------------------------------------------------------

describe('listAll 顺序', () => {
  test('listAll 按注册顺序返回', () => {
    registerShortcut(makeSpec({ id: 'first', defaultBinding: 'mod+1' }))
    registerShortcut(makeSpec({ id: 'second', defaultBinding: 'mod+2' }))
    registerShortcut(makeSpec({ id: 'third', defaultBinding: 'mod+3' }))
    expect(listAll().map((s) => s.id)).toEqual(['first', 'second', 'third'])
  })
})

