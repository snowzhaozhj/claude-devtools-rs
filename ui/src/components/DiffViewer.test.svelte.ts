// DiffViewer smoke 单测：纯 props 组件，无 IPC / 无观察者，验证渲染 + 基本 DOM。

import { describe, expect, test, afterEach } from 'vitest'
import { render, cleanup } from '@testing-library/svelte'

import DiffViewer from './DiffViewer.svelte'

afterEach(() => {
  cleanup()
})

describe('DiffViewer smoke', () => {
  test('给定 old/new 字符串可渲染不抛错', () => {
    const { container } = render(DiffViewer, {
      props: {
        fileName: '/src/main.rs',
        oldString: 'fn main() {}\n',
        newString: 'fn main() { println!("hi"); }\n',
      },
    })
    expect(container.querySelector('.diff-viewer')).not.toBeNull()
    expect(container.querySelector('.diff-header')).not.toBeNull()
    expect(container.querySelector('.diff-body')).not.toBeNull()
  })

  test('header 包含语言 tag + 短文件名', () => {
    const { container } = render(DiffViewer, {
      props: {
        fileName: '/a/b/c/main.rs',
        oldString: '',
        newString: 'x\n',
      },
    })
    const tag = container.querySelector('.diff-lang-tag')
    expect(tag?.textContent?.trim()).toBe('rust')
    expect(container.querySelector('.diff-filename')?.textContent).toContain('main.rs')
  })

  test('added/removed 行都生成对应 class', () => {
    const { container } = render(DiffViewer, {
      props: {
        fileName: 'f.txt',
        oldString: 'a\nb\n',
        newString: 'a\nc\n',
      },
    })
    expect(container.querySelector('.diff-line-added')).not.toBeNull()
    expect(container.querySelector('.diff-line-removed')).not.toBeNull()
  })

  test('完全相同的 old/new 不渲染 added/removed', () => {
    const { container } = render(DiffViewer, {
      props: {
        fileName: 'f.txt',
        oldString: 'same\n',
        newString: 'same\n',
      },
    })
    expect(container.querySelector('.diff-line-added')).toBeNull()
    expect(container.querySelector('.diff-line-removed')).toBeNull()
  })

  // spec tool-viewer-routing::编辑型工具无输出时按差异内容分档 + 超大行导向输出首尾切片
  test('中长 diff 显示信息气味，短 diff 不显示', () => {
    const long = Array.from({ length: 120 }, (_, i) => `line ${i}`).join('\n')
    const { container } = render(DiffViewer, {
      props: { fileName: 'f.txt', oldString: '', newString: long },
    })
    const scent = container.querySelector('.diff-scent')
    expect(scent).not.toBeNull()
    expect(scent!.textContent).toContain('预览')

    cleanup()
    const { container: c2 } = render(DiffViewer, {
      props: { fileName: 'f.txt', oldString: 'a\n', newString: 'b\n' },
    })
    expect(c2.querySelector('.diff-scent')).toBeNull()
  })

  test('超大 diff 首尾切片 + 省略接缝，中段不在 DOM', () => {
    const oldStr = Array.from({ length: 2000 }, (_, i) => `old ${i}`).join('\n')
    const { container } = render(DiffViewer, {
      props: { fileName: 'f.txt', oldString: oldStr, newString: '' },
    })
    const seam = container.querySelector('.diff-seam')
    expect(seam).not.toBeNull()
    expect(seam!.textContent).toContain('已省略')
    expect(container.textContent).not.toContain('old 1000')
  })

  test('header 提供复制完整差异入口', () => {
    const { container } = render(DiffViewer, {
      props: { fileName: 'f.txt', oldString: 'a\n', newString: 'b\n' },
    })
    const btn = container.querySelector('.diff-header button[aria-label="复制完整差异"]')
    expect(btn).not.toBeNull()
  })
})
