/**
 * SHORTCUT_DEFAULTS meta 不变量守护——防止运行期 dispatcher 路径 regression。
 *
 * 这些不变量来自 spec keyboard-shortcuts / ui-search / ui-command-palette：
 * - search.in-session 必须 allowInInput: true（在 input focus 内仍打开 SearchBar）
 * - command-palette.toggle 必须 allowInInput: true（input focus 内仍能拉起 palette）
 * - sidebar.toggle 默认 false（input 内 mod+B 给浏览器原生 / 用户文本）
 *
 * dispatcher 行为本身（allowInInput=true 在 input focus 触发 / =false 跳过）由
 * dispatcher.test.ts §4.7 覆盖；本文件守 defaults.ts 元数据正确性。
 */

import { describe, expect, test } from 'vitest'

import { SHORTCUT_DEFAULTS } from '../defaults'

describe('SHORTCUT_DEFAULTS allowInInput 不变量', () => {
  function findMeta(id: string) {
    return SHORTCUT_DEFAULTS.find((m) => m.id === id)
  }

  test('search.in-session.allowInInput === true（codex P1-1 fix）', () => {
    const meta = findMeta('search.in-session')
    expect(meta).toBeDefined()
    expect(meta?.allowInInput).toBe(true)
  })

  test('command-palette.toggle.allowInInput === true（regression guard）', () => {
    const meta = findMeta('command-palette.toggle')
    expect(meta?.allowInInput).toBe(true)
  })

  test('sidebar.toggle.allowInInput 不显式 true（默认 false：input 内不抢键）', () => {
    const meta = findMeta('sidebar.toggle')
    expect(meta?.allowInInput).not.toBe(true)
  })

  test('search.focus.allowInInput 不显式 true（"/" 在 input 内是字面字符）', () => {
    const meta = findMeta('search.focus')
    expect(meta?.allowInInput).not.toBe(true)
  })
})
