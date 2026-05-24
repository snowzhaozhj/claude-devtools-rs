// scrollAnchor 单测：jsdom 不模拟真布局——所有 rect / scrollTop / scrollHeight
// 都通过 mock Element.prototype 拿到。覆盖 isAtBottom 阈值边界 +
// captureScrollAnchor 选锚点逻辑（含跨视口顶 / 完全可见 / 全在视口外三场景）+
// restoreScrollAnchor 三路分支（粘底 / anchor 命中 / anchor 失效兜底）。
//
// jsdom 不模拟真渲染，bottom pin 状态机的 MutationObserver / setTimeout 时序
// 行为靠 Playwright e2e 兜底（见 ui/tests/e2e/tab-scroll-preserve.spec.ts）。

import { describe, expect, test, vi, beforeEach, afterEach } from 'vitest'
import {
  isAtBottom,
  captureScrollAnchor,
  restoreScrollAnchor,
  startBottomPin,
  type ScrollAnchorState,
} from './scrollAnchor'

// jsdom 不实现 scrollIntoView——stub 成 noop
beforeEach(() => {
  Element.prototype.scrollIntoView = vi.fn()
})

afterEach(() => {
  vi.restoreAllMocks()
})

function makeContainer(opts: {
  scrollTop: number
  scrollHeight: number
  clientHeight: number
  rectTop: number
  chunks?: Array<{ id: string; rectTop: number; rectBottom: number }>
}): HTMLElement {
  const container = document.createElement('div')
  Object.defineProperty(container, 'scrollTop', {
    value: opts.scrollTop,
    writable: true,
    configurable: true,
  })
  Object.defineProperty(container, 'scrollHeight', { value: opts.scrollHeight, configurable: true })
  Object.defineProperty(container, 'clientHeight', { value: opts.clientHeight, configurable: true })
  container.getBoundingClientRect = () =>
    ({ top: opts.rectTop, bottom: opts.rectTop + opts.clientHeight, left: 0, right: 0, width: 0, height: opts.clientHeight, x: 0, y: opts.rectTop, toJSON: () => ({}) } as DOMRect)

  for (const c of opts.chunks ?? []) {
    const el = document.createElement('div')
    el.dataset.chunkId = c.id
    el.getBoundingClientRect = () =>
      ({ top: c.rectTop, bottom: c.rectBottom, left: 0, right: 0, width: 0, height: c.rectBottom - c.rectTop, x: 0, y: c.rectTop, toJSON: () => ({}) } as DOMRect)
    container.appendChild(el)
  }
  document.body.appendChild(container)
  return container
}

describe('isAtBottom 阈值', () => {
  test('distanceFromBottom = 0 → true', () => {
    const el = makeContainer({ scrollTop: 9200, scrollHeight: 10000, clientHeight: 800, rectTop: 0 })
    expect(isAtBottom(el)).toBe(true)
  })
  test('distanceFromBottom = 16 → true（边界）', () => {
    const el = makeContainer({ scrollTop: 9184, scrollHeight: 10000, clientHeight: 800, rectTop: 0 })
    expect(isAtBottom(el)).toBe(true)
  })
  test('distanceFromBottom = 17 → false', () => {
    const el = makeContainer({ scrollTop: 9183, scrollHeight: 10000, clientHeight: 800, rectTop: 0 })
    expect(isAtBottom(el)).toBe(false)
  })
})

describe('captureScrollAnchor', () => {
  test('粘底 → atBottom=true，无 chunk anchor', () => {
    const el = makeContainer({
      scrollTop: 9200,
      scrollHeight: 10000,
      clientHeight: 800,
      rectTop: 0,
      chunks: [
        { id: 'chunk-a', rectTop: 0, rectBottom: 200 },
        { id: 'chunk-b', rectTop: 200, rectBottom: 800 },
      ],
    })
    const result = captureScrollAnchor(el)
    expect(result).toEqual({ atBottom: true, anchorChunkId: null, anchorOffsetPx: 0 })
  })

  test('中间位置 → 第一个 bottom > containerTop 的 chunk 当 anchor（完全在视口内场景）', () => {
    // 容器 rectTop=100，scroll 状态：scrollTop=500、clientHeight=800、scrollHeight=10000
    // 多个 chunk：chunk-a 已经被滚出（rect.bottom=80 < containerTop=100）→ skip
    // chunk-b 完全在视口内（rect.top=120, bottom=300）→ anchor
    const el = makeContainer({
      scrollTop: 500,
      scrollHeight: 10000,
      clientHeight: 800,
      rectTop: 100,
      chunks: [
        { id: 'chunk-a', rectTop: 0, rectBottom: 80 },
        { id: 'chunk-b', rectTop: 120, rectBottom: 300 },
        { id: 'chunk-c', rectTop: 300, rectBottom: 500 },
      ],
    })
    const result = captureScrollAnchor(el)
    expect(result.atBottom).toBe(false)
    expect(result.anchorChunkId).toBe('chunk-b')
    // offset = rect.top - containerTop = 120 - 100 = 20
    expect(result.anchorOffsetPx).toBe(20)
  })

  test('跨越视口顶 → anchor 选跨越的 chunk，offset 为负', () => {
    // chunk-a 跨越视口顶：rect.top=80（< containerTop=100），rect.bottom=250（> containerTop=100）
    const el = makeContainer({
      scrollTop: 200,
      scrollHeight: 10000,
      clientHeight: 800,
      rectTop: 100,
      chunks: [
        { id: 'chunk-a', rectTop: 80, rectBottom: 250 },
        { id: 'chunk-b', rectTop: 250, rectBottom: 400 },
      ],
    })
    const result = captureScrollAnchor(el)
    expect(result.anchorChunkId).toBe('chunk-a')
    // offset = 80 - 100 = -20（chunk 顶被滚出视口 20 px）
    expect(result.anchorOffsetPx).toBe(-20)
  })

  test('全部 chunk 已被滚出（兜底） → 三件套全 0/null', () => {
    const el = makeContainer({
      scrollTop: 500,
      scrollHeight: 10000,
      clientHeight: 800,
      rectTop: 100,
      chunks: [
        { id: 'chunk-a', rectTop: 0, rectBottom: 50 },
        { id: 'chunk-b', rectTop: 50, rectBottom: 99 },
      ],
    })
    const result = captureScrollAnchor(el)
    expect(result).toEqual({ atBottom: false, anchorChunkId: null, anchorOffsetPx: 0 })
  })

  test('conversationEl undefined → 默认非粘底空 anchor', () => {
    expect(captureScrollAnchor(undefined)).toEqual({
      atBottom: false,
      anchorChunkId: null,
      anchorOffsetPx: 0,
    })
  })
})

describe('restoreScrollAnchor', () => {
  test('atBottom=true → 启动 bottom pin（返回 cleanup 函数 + 单次写 scrollTop）', () => {
    const el = makeContainer({ scrollTop: 0, scrollHeight: 10000, clientHeight: 800, rectTop: 0 })
    const state: ScrollAnchorState = { atBottom: true, anchorChunkId: null, anchorOffsetPx: 0 }
    const cleanup = restoreScrollAnchor(el, state)
    expect(typeof cleanup).toBe('function')
    expect(el.scrollTop).toBe(10000)  // 单次粘底
    cleanup?.()  // 清理 MO + timer
  })

  test('anchor 命中 + first apply 对齐 → scrollIntoView 被调 + scrollTop 减去 offset；返回 null', () => {
    const el = makeContainer({
      scrollTop: 0,
      scrollHeight: 10000,
      clientHeight: 800,
      rectTop: 0,
      chunks: [{ id: 'chunk-mid', rectTop: 200, rectBottom: 400 }],
    })
    const target = el.querySelector('[data-chunk-id="chunk-mid"]') as HTMLElement
    const intoViewSpy = vi.spyOn(target, 'scrollIntoView')
    // 模拟 scrollIntoView 把 anchor 顶贴齐视口顶 + 同步更新 chunk rect 反映 apply 后位置
    intoViewSpy.mockImplementation(function (this: HTMLElement) {
      ;(el as HTMLElement).scrollTop = 1000
      // apply 后 chunk rect.top 贴齐到 anchorOffsetPx=30 处（视口顶下 30 px）
      this.getBoundingClientRect = () =>
        ({ top: 30, bottom: 230, left: 0, right: 0, width: 0, height: 200, x: 0, y: 30, toJSON: () => ({}) } as DOMRect)
    })
    const state: ScrollAnchorState = {
      atBottom: false,
      anchorChunkId: 'chunk-mid',
      anchorOffsetPx: 30,
    }
    const cleanup = restoreScrollAnchor(el, state)
    expect(cleanup).toBeNull()
    expect(intoViewSpy).toHaveBeenCalledWith({ block: 'start' })
    expect(el.scrollTop).toBe(1000 - 30)  // -= offset 还原原始视口偏移
  })

  test('anchor 命中 + first apply 因 scrollHeight 不足被 clamp（未对齐）→ 返回 MO 状态机 cleanup', () => {
    // 模拟 spec `tab-management::滚动位置恢复 - 切回时 lazy chunks 尚未 hydrate`：
    // conversation scrollHeight 短，scrollIntoView 后 anchor 实际位置仍偏离 saved offset
    // → 应启动 MutationObserver 等子树后续 mutation 后 re-apply，cleanup 非 null
    const el = makeContainer({
      scrollTop: 0,
      scrollHeight: 1500,  // 远小于 anchor 期望 scrollTop
      clientHeight: 800,
      rectTop: 0,
      chunks: [{ id: 'chunk-far', rectTop: 700, rectBottom: 900 }],
    })
    const target = el.querySelector('[data-chunk-id="chunk-far"]') as HTMLElement
    vi.spyOn(target, 'scrollIntoView').mockImplementation(function (this: HTMLElement) {
      // 模拟 clamp：scrollTop 被设为 max（700 = 1500 - 800），anchor rect 没贴齐 saved offset
      ;(el as HTMLElement).scrollTop = 700
      // chunk rect 保持 mock 初值（未对齐 saved offset=30）
    })
    const state: ScrollAnchorState = {
      atBottom: false,
      anchorChunkId: 'chunk-far',
      anchorOffsetPx: 30,
    }
    const cleanup = restoreScrollAnchor(el, state)
    expect(cleanup).toBeInstanceOf(Function)
    // first apply 仍执行：scrollTop 写入 700 - 30 = 670
    expect(el.scrollTop).toBe(700 - 30)
    cleanup?.()  // 清理 MO + timer
  })

  test('anchor 失效（DOM 找不到） → console.warn + scrollTop 不变 + 返回 null', () => {
    const el = makeContainer({
      scrollTop: 0,
      scrollHeight: 10000,
      clientHeight: 800,
      rectTop: 0,
      chunks: [{ id: 'chunk-real', rectTop: 200, rectBottom: 400 }],
    })
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})
    const state: ScrollAnchorState = {
      atBottom: false,
      anchorChunkId: 'nonexistent',
      anchorOffsetPx: 50,
    }
    const cleanup = restoreScrollAnchor(el, state)
    expect(cleanup).toBeNull()
    expect(warnSpy).toHaveBeenCalledWith(
      expect.stringContaining('anchorChunkId not found: nonexistent'),
    )
    expect(el.scrollTop).toBe(0)  // 没动
  })

  test('anchorChunkId=null（兜底） → 不写 scrollTop，返回 null', () => {
    const el = makeContainer({ scrollTop: 0, scrollHeight: 10000, clientHeight: 800, rectTop: 0 })
    const state: ScrollAnchorState = { atBottom: false, anchorChunkId: null, anchorOffsetPx: 0 }
    const cleanup = restoreScrollAnchor(el, state)
    expect(cleanup).toBeNull()
    expect(el.scrollTop).toBe(0)
  })

  test('conversationEl undefined → 返回 null（不抛）', () => {
    const state: ScrollAnchorState = { atBottom: true, anchorChunkId: null, anchorOffsetPx: 0 }
    expect(restoreScrollAnchor(undefined, state)).toBeNull()
  })
})

describe('startBottomPin 状态机', () => {
  test('用户主动滚动（distanceFromBottom > 16） → 立即 stopPin', () => {
    const el = makeContainer({ scrollTop: 0, scrollHeight: 10000, clientHeight: 800, rectTop: 0 })
    const cleanup = startBottomPin(el)
    expect(el.scrollTop).toBe(10000)  // 单次粘底

    // 模拟用户主动滚到顶部（distanceFromBottom = 9200）
    Object.defineProperty(el, 'scrollTop', { value: 0, writable: true, configurable: true })
    el.dispatchEvent(new Event('scroll'))

    // 触发 mutation 检验 pin 已停（不应再写 scrollTop = scrollHeight）
    const newChild = document.createElement('div')
    newChild.dataset.rendered = '1'
    el.appendChild(newChild)
    // MutationObserver 是异步 microtask，等下一帧
    return Promise.resolve().then(() => {
      expect(el.scrollTop).toBe(0)  // pin 已 stop，没有重写
      cleanup()
    })
  })

  test('hardLimit 5 s 上限 → 兜底 stopPin', () => {
    vi.useFakeTimers()
    const el = makeContainer({ scrollTop: 0, scrollHeight: 10000, clientHeight: 800, rectTop: 0 })
    const cleanup = startBottomPin(el)
    expect(el.scrollTop).toBe(10000)

    vi.advanceTimersByTime(5001)
    // 模拟新 mutation，pin 已 stop 不会再写 scrollTop
    Object.defineProperty(el, 'scrollTop', { value: 5000, writable: true, configurable: true })
    const newChild = document.createElement('div')
    newChild.dataset.rendered = '1'
    el.appendChild(newChild)
    return Promise.resolve().then(() => {
      expect(el.scrollTop).toBe(5000)  // pin 已 stop
      cleanup()
      vi.useRealTimers()
    })
  })
})
