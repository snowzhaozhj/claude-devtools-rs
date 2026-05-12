import { describe, expect, test } from 'vitest'
import { generateDiff } from './diff'

describe('generateDiff', () => {
  test('为修改内容生成 old/new 双列行号', () => {
    expect(generateDiff('a\nb\nc', 'a\nB\nc')).toEqual([
      { type: 'context', content: 'a', oldLineNumber: 1, newLineNumber: 1 },
      { type: 'removed', content: 'b', oldLineNumber: 2, newLineNumber: null },
      { type: 'added', content: 'B', oldLineNumber: null, newLineNumber: 2 },
      { type: 'context', content: 'c', oldLineNumber: 3, newLineNumber: 3 },
    ])
  })

  test('纯新增内容只填 new 行号', () => {
    expect(generateDiff('', 'one\ntwo')).toEqual([
      { type: 'added', content: 'one', oldLineNumber: null, newLineNumber: 1 },
      { type: 'added', content: 'two', oldLineNumber: null, newLineNumber: 2 },
    ])
  })

  test('尾随换行不产生额外空白 context 行', () => {
    expect(generateDiff('a\n', 'b\n')).toEqual([
      { type: 'removed', content: 'a', oldLineNumber: 1, newLineNumber: null },
      { type: 'added', content: 'b', oldLineNumber: null, newLineNumber: 1 },
    ])
  })
})
