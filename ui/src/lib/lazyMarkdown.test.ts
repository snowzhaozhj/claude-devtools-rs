// lazyMarkdown 单测（spec session-display §"Lazy markdown rendering for first paint performance"）。
//
// 覆盖 flushAll 行为：搜索 / 打印等需要全文 DOM 的场景调用一次后，
// 所有 pending 占位 SHALL 被同步渲染为真实 HTML。

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest'

import { createLazyMarkdownObserver, LAZY_MARKDOWN_ENABLED, estimatePlaceholderHeight } from './lazyMarkdown.svelte'
import { BOUNDED_BYTE_THRESHOLD } from './outputSizing'

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
  // Mock ResizeObserver for scroll compensation
  if (typeof globalThis.ResizeObserver === "undefined") {
    ;(globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    }
  }
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

// spec session-display §"对话流文本输出按内容规模自适应展示" + tasks 2.4：
// bounded prose 的 lazy 占位 min-height 落在**内层** .lazy-md，限高滚动落在
// **外层** .ao-viewport（max-block-size 上限 22rem = 352px @ 16px root）。
// 外层几何在"占位清除→真实高度接管"前后保持稳定的充分条件：
// 占位估算与真实渲染高度都 ≥ viewport 上限（外层恒钳在上限）。
// 本块锁字节触发路径的该不变量，防未来调估算公式 / viewport 上限时静默破坏。
describe('bounded prose 占位估算 ≥ 限高 viewport 上限（外层几何稳定）', () => {
  const VIEWPORT_MAX_PX = 352 // clamp(10rem, 30dvh, 22rem) 的上限（AdaptiveOutputFrame）

  test('字节达 bounded 阈值的 prose 估算高度远超 viewport 上限', () => {
    const text = 'x'.repeat(BOUNDED_BYTE_THRESHOLD)
    expect(estimatePlaceholderHeight(text, 'output')).toBeGreaterThanOrEqual(VIEWPORT_MAX_PX)
  })

  test('user_message prose 同一路径同一保证', () => {
    const text = 'x'.repeat(BOUNDED_BYTE_THRESHOLD)
    expect(estimatePlaceholderHeight(text, 'user')).toBeGreaterThanOrEqual(VIEWPORT_MAX_PX)
  })
})
