// lazyMarkdown 单测（spec session-display §"Lazy markdown rendering for first paint performance"）。
//
// 覆盖 flushAll 行为：搜索 / 打印等需要全文 DOM 的场景调用一次后，
// 所有 pending 占位 SHALL 被同步渲染为真实 HTML。

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import { createLazyMarkdownObserver, LAZY_MARKDOWN_ENABLED } from './lazyMarkdown.svelte'

class MockIntersectionObserver {
  observed: Element[] = []
  unobserved: Element[] = []
  disconnected = false
  callback: IntersectionObserverCallback
  options?: IntersectionObserverInit

  constructor(cb: IntersectionObserverCallback, opts?: IntersectionObserverInit) {
    this.callback = cb
    this.options = opts
    lastObserver = this
  }

  observe(el: Element) {
    this.observed.push(el)
  }

  unobserve(el: Element) {
    this.unobserved.push(el)
    this.observed = this.observed.filter((x) => x !== el)
  }

  disconnect() {
    this.disconnected = true
    this.observed = []
  }

  takeRecords(): IntersectionObserverEntry[] {
    return []
  }

  root = null
  rootMargin = '0px'
  thresholds: number[] = [0]
}

let lastObserver: MockIntersectionObserver | undefined

beforeEach(() => {
  lastObserver = undefined
  ;(globalThis as unknown as { IntersectionObserver: typeof IntersectionObserver }).IntersectionObserver =
    MockIntersectionObserver as unknown as typeof IntersectionObserver
})

afterEach(() => {
  vi.restoreAllMocks()
})

function makePlaceholder(): HTMLElement {
  const el = document.createElement('div')
  document.body.appendChild(el)
  return el
}

describe('createLazyMarkdownObserver.flushAll', () => {
  test('LAZY_MARKDOWN_ENABLED 为 true 是本测试前提', () => {
    // 防御：本套用例假定 enabled 分支；若有人改回滚开关需先调整测试
    expect(LAZY_MARKDOWN_ENABLED).toBe(true)
  })

  test('flushAll 把所有 pending 占位同步渲染为真实 HTML', () => {
    const root = makePlaceholder()
    const observer = createLazyMarkdownObserver(root)

    const els = [makePlaceholder(), makePlaceholder(), makePlaceholder()]
    els.forEach((el, i) => observer.observe(el, `# heading ${i}`))

    expect(lastObserver?.observed.length).toBe(3)

    observer.flushAll()

    for (const el of els) {
      expect(el.dataset.rendered).toBe('1')
      expect(el.innerHTML.length).toBeGreaterThan(0)
    }
    expect(lastObserver?.unobserved.length).toBe(3)
  })

  test('flushAll 幂等：连续两次调用不报错，第二次 no-op', () => {
    const root = makePlaceholder()
    const observer = createLazyMarkdownObserver(root)
    const el = makePlaceholder()
    observer.observe(el, 'hello world')

    observer.flushAll()
    const htmlAfterFirst = el.innerHTML

    expect(() => observer.flushAll()).not.toThrow()
    expect(el.innerHTML).toBe(htmlAfterFirst)
  })

  test('flushAll 触发 onRendered 回调（mermaid 后处理钩子）', () => {
    const root = makePlaceholder()
    const observer = createLazyMarkdownObserver(root)
    const el = makePlaceholder()
    const onRendered = vi.fn()
    observer.observe(el, '# heading', onRendered)

    observer.flushAll()

    expect(onRendered).toHaveBeenCalledTimes(1)
    expect(onRendered).toHaveBeenCalledWith(el)
  })

  test('已标记 data-rendered 的元素不重复渲染，但仍触发 onRendered 清理钩子', () => {
    const root = makePlaceholder()
    const observer = createLazyMarkdownObserver(root)
    const el = makePlaceholder()
    el.dataset.rendered = '1'
    el.innerHTML = '<p>已存在</p>'
    el.style.minHeight = '220px'

    observer.observe(el, '# new', (rendered) => {
      rendered.style.minHeight = ''
    })

    expect(lastObserver?.observed.length).toBe(0)
    expect(el.innerHTML).toBe('<p>已存在</p>')
    expect(el.style.minHeight).toBe('')

    observer.flushAll()
    expect(el.innerHTML).toBe('<p>已存在</p>')
  })

  test('disconnect 后 pending 清空，避免内存泄漏', () => {
    const root = makePlaceholder()
    const observer = createLazyMarkdownObserver(root)
    observer.observe(makePlaceholder(), 'a')
    observer.observe(makePlaceholder(), 'b')

    observer.disconnect()
    expect(lastObserver?.disconnected).toBe(true)

    // disconnect 后再 flushAll 不应报错
    expect(() => observer.flushAll()).not.toThrow()
  })
})
