import { describe, expect, test } from 'vitest'
import { cwdRelativeHintLabel } from './cwdHint'

describe('cwdRelativeHintLabel', () => {
  test('null / undefined / 空串返回空', () => {
    expect(cwdRelativeHintLabel(null)).toBe('')
    expect(cwdRelativeHintLabel(undefined)).toBe('')
    expect(cwdRelativeHintLabel('')).toBe('')
  })

  test('单段路径直接全显（spec Scenario 子目录 cwd 行 chip）', () => {
    expect(cwdRelativeHintLabel('crates')).toBe('crates')
  })

  test('两段路径全显', () => {
    expect(cwdRelativeHintLabel('crates/cdt-discover')).toBe('crates/cdt-discover')
  })

  test('深层路径截取最后两段（spec Scenario 深层子目录截取最后两段）', () => {
    expect(cwdRelativeHintLabel('.claude/worktrees/feat-x')).toBe('worktrees/feat-x')
  })

  test('Windows 反斜杠分隔同样命中（IPC 已统一 / 但本地接受双形态防御）', () => {
    expect(cwdRelativeHintLabel('crates\\cdt-discover')).toBe('crates/cdt-discover')
  })

  test('只有分隔符的退化输入返回空', () => {
    expect(cwdRelativeHintLabel('///')).toBe('')
  })
})
