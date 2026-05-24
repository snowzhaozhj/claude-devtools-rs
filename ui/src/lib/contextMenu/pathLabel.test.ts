// pathLabel 中段截断算法单测（Task 4.5）。

import { describe, expect, test } from 'vitest'
import { truncatePath } from './pathLabel'

describe('truncatePath: home 缩写', () => {
  test('macOS /Users/<name>/... → ~/...', () => {
    const r = truncatePath('/Users/zhao/foo.ts')
    expect(r.short).toBe('~/foo.ts')
    expect(r.full).toBe('/Users/zhao/foo.ts')
  })

  test('Linux /home/<name>/... → ~/...', () => {
    const r = truncatePath('/home/dev/foo.ts')
    expect(r.short).toBe('~/foo.ts')
    expect(r.full).toBe('/home/dev/foo.ts')
  })

  test('Linux /root → ~', () => {
    const r = truncatePath('/root/dotfiles/.bashrc')
    expect(r.short).toBe('~/dotfiles/.bashrc')
    expect(r.full).toBe('/root/dotfiles/.bashrc')
  })

  test('Windows C:\\\\Users\\\\<name>\\\\... → ~\\\\...', () => {
    const r = truncatePath('C:\\Users\\zhao\\foo.ts')
    expect(r.short).toBe('~\\foo.ts')
    expect(r.full).toBe('C:\\Users\\zhao\\foo.ts')
  })

  test('非 home 路径不缩写', () => {
    const r = truncatePath('/tmp/foo.ts')
    expect(r.short).toBe('/tmp/foo.ts')
    expect(r.full).toBe('/tmp/foo.ts')
  })
})

describe('truncatePath: 总长 ≤ 50 不截断', () => {
  test('短路径直接返回 short = abbreviated', () => {
    const r = truncatePath('/Users/dev/short.ts')
    expect(r.short).toBe('~/short.ts')
    expect(r.short.length).toBeLessThanOrEqual(50)
  })
})

describe('truncatePath: 总长 > 50 中段截断', () => {
  test('保留 head + parent dir + tail', () => {
    const r = truncatePath('/Users/zhao/RustroverProjects/Project/claude-devtools-rs/ui/src/lib/contextMenu/menu-items.ts')
    expect(r.short.length).toBeLessThanOrEqual(50)
    expect(r.short.startsWith('~/')).toBe(true)
    expect(r.short.endsWith('menu-items.ts')).toBe(true)
    expect(r.short).toContain('…')
    // full 仍是原路径
    expect(r.full).toBe('/Users/zhao/RustroverProjects/Project/claude-devtools-rs/ui/src/lib/contextMenu/menu-items.ts')
  })

  test('parent 段也太长时退到 head/.../tail', () => {
    // 构造极长 parent 段
    const longParent = 'a'.repeat(80)
    const r = truncatePath(`/Users/dev/${longParent}/file.ts`)
    expect(r.short.length).toBeLessThanOrEqual(50)
    expect(r.short.startsWith('~/')).toBe(true)
    expect(r.short).toContain('…')
  })

  test('尾文件名超长时也截断', () => {
    const longName = 'b'.repeat(120) + '.ts'
    const r = truncatePath(`/Users/dev/sub/${longName}`)
    expect(r.short.length).toBeLessThanOrEqual(50)
    // 尾段保留 .ts 后缀
    expect(r.short.endsWith('.ts')).toBe(true)
  })
})

describe('truncatePath: 边界', () => {
  test('空字符串返回空 short / full', () => {
    expect(truncatePath('')).toEqual({ short: '', full: '' })
  })

  test('单段路径（无 separator）走 head/.../tail fallback', () => {
    const long = 'x'.repeat(80)
    const r = truncatePath(long)
    expect(r.short.length).toBeLessThanOrEqual(50)
    expect(r.full).toBe(long)
  })

  test('Windows 反斜杠路径多层截断', () => {
    const r = truncatePath('C:\\Users\\zhao\\projects\\very-long-folder-name\\nested\\sub\\deep.ts')
    expect(r.short.length).toBeLessThanOrEqual(50)
    expect(r.short.startsWith('~\\')).toBe(true)
    expect(r.short.endsWith('deep.ts')).toBe(true)
  })
})
