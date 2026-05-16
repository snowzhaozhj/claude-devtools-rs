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
})
