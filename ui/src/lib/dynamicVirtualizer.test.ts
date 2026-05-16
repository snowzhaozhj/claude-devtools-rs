import { describe, expect, test } from 'vitest'

import { createDynamicVirtualizer, type DynamicVirtualizer } from './dynamicVirtualizer.svelte'

function createVirtualizer(count = 20): DynamicVirtualizer {
  return createDynamicVirtualizer({
    count: () => count,
    itemKey: (index) => `item-${index}`,
    estimateSize: () => 100,
    overscanPx: 50,
  })
}

function makeScroller(height: number, scrollTop = 0): HTMLElement {
  const el = document.createElement('div')
  Object.defineProperty(el, 'clientHeight', { value: height, configurable: true })
  el.scrollTop = scrollTop
  return el
}

describe('createDynamicVirtualizer', () => {
  test('根据 scrollTop 和 overscan 计算可见窗口与 spacer', () => {
    const v = createVirtualizer()
    const el = makeScroller(300, 450)
    v.bindScrollEl(el)

    const items = v.virtualItems()
    expect(items[0].index).toBe(4)
    expect(items.at(-1)?.index).toBe(8)
    expect(v.topSpacer()).toBe(400)
    expect(v.bottomSpacer()).toBe(1100)
  })

  test('startOffset 落在 row 内部时包含覆盖该 offset 的 row', () => {
    const v = createVirtualizer()
    const el = makeScroller(300, 475)
    v.bindScrollEl(el)

    expect(v.virtualItems()[0].index).toBe(4)
    expect(v.topSpacer()).toBe(400)
  })

  test('实测高度覆盖估算并更新总高度', () => {
    const v = createVirtualizer(3)
    const el = makeScroller(200)
    v.bindScrollEl(el)

    expect(v.totalSize()).toBe(300)
    v.measure(1, 180.2)

    expect(v.totalSize()).toBe(381)
    expect(v.virtualItems().find((item) => item.index === 1)?.size).toBe(181)
  })

  test('scrollToEnd 使用动态总高度定位到底部', () => {
    const v = createVirtualizer(5)
    const el = makeScroller(120)
    v.bindScrollEl(el)
    v.measure(4, 250)

    v.scrollToEnd()

    expect(el.scrollTop).toBe(530)
    expect(v.scrollTop()).toBe(530)
  })

  test('resetMeasurements 清空旧 key 的测量结果', () => {
    const v = createVirtualizer(2)
    const el = makeScroller(200)
    v.bindScrollEl(el)
    v.measure(0, 300)
    expect(v.totalSize()).toBe(400)

    v.resetMeasurements()

    expect(v.totalSize()).toBe(200)
  })
})
