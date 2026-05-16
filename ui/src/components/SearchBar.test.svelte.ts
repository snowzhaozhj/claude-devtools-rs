// SearchBar smoke 单测：纯 props + DOM 操作，无 IPC。
// 验证可见性切换、关键按钮渲染、onClose 回调被调用。

import { describe, expect, test, afterEach, vi } from 'vitest'
import { render, cleanup } from '@testing-library/svelte'

import SearchBar from './SearchBar.svelte'

afterEach(() => {
  cleanup()
})

describe('SearchBar smoke', () => {
  test('visible=false 时不渲染 search-bar', () => {
    const { container } = render(SearchBar, {
      props: {
        visible: false,
        containerEl: null,
        onClose: () => {},
      },
    })
    expect(container.querySelector('.search-bar')).toBeNull()
  })

  test('visible=true 时渲染输入框 + 三个按钮', () => {
    const { container } = render(SearchBar, {
      props: {
        visible: true,
        containerEl: null,
        onClose: () => {},
      },
    })
    expect(container.querySelector('.search-bar')).not.toBeNull()
    expect(container.querySelector('input.search-input')).not.toBeNull()
    // 上一个 / 下一个 / 关闭
    expect(container.querySelectorAll('.search-nav-btn').length).toBe(2)
    expect(container.querySelector('.search-close-btn')).not.toBeNull()
  })

  test('点击关闭按钮触发 onClose', async () => {
    const onClose = vi.fn()
    const { container } = render(SearchBar, {
      props: {
        visible: true,
        containerEl: null,
        onClose,
      },
    })
    const btn = container.querySelector<HTMLButtonElement>('.search-close-btn')!
    btn.click()
    expect(onClose).toHaveBeenCalledTimes(1)
  })

  test('containerEl 提供时初始状态不抛错（containerEl=document.body）', () => {
    const { container } = render(SearchBar, {
      props: {
        visible: true,
        containerEl: document.body,
        onClose: () => {},
      },
    })
    expect(container.querySelector('.search-bar')).not.toBeNull()
  })
})
