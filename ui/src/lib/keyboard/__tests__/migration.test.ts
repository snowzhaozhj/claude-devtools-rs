/**
 * §6 迁移路径单测：覆盖 SessionDetail per-tab fanout + 多 instance 单注册边界。
 *
 * 验证 design.md::D8 单 binding 单 spec 1:1 关系下，多 SessionDetail tab 共用 PaneContainer
 * 一个 dispatcher handler 的语义：
 * - register/unregister tabId 回调
 * - getActiveTabId 命中 → 调对应 tab 的回调
 * - getActiveTabId 未命中（其它 tab type / null） → trigger 返回 false 让 dispatcher 不 preventDefault
 * - 多 tab 同时 mount → 各自回调隔离，按 active 派发
 *
 * 不覆盖：真实 mount Svelte 组件（PaneContainer / SessionDetail）的 lifecycle，那是 §10 e2e 范畴。
 */

import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import {
  registerSessionDetailHandlers,
  unregisterSessionDetailHandlers,
  triggerJumpToLatest,
  triggerOpenSearch,
  _resetForTest as _resetHandlersForTest,
  _registeredTabIdsForTest,
} from '../session-detail-handlers'

beforeEach(() => {
  _resetHandlersForTest()
})

afterEach(() => {
  _resetHandlersForTest()
})

describe('session-detail-handlers fanout', () => {
  test('register + trigger jumpToLatest by active tabId', () => {
    let aCalled = 0
    let bCalled = 0
    registerSessionDetailHandlers('tab-a', {
      jumpToLatest: () => {
        aCalled += 1
      },
      openSearch: () => {},
    })
    registerSessionDetailHandlers('tab-b', {
      jumpToLatest: () => {
        bCalled += 1
      },
      openSearch: () => {},
    })

    expect(triggerJumpToLatest('tab-a')).toBe(true)
    expect(aCalled).toBe(1)
    expect(bCalled).toBe(0)

    expect(triggerJumpToLatest('tab-b')).toBe(true)
    expect(aCalled).toBe(1)
    expect(bCalled).toBe(1)
  })

  test('register + trigger openSearch by active tabId', () => {
    let opened = ''
    registerSessionDetailHandlers('tab-a', {
      jumpToLatest: () => {},
      openSearch: () => {
        opened = 'a'
      },
    })
    registerSessionDetailHandlers('tab-b', {
      jumpToLatest: () => {},
      openSearch: () => {
        opened = 'b'
      },
    })
    expect(triggerOpenSearch('tab-a')).toBe(true)
    expect(opened).toBe('a')
    expect(triggerOpenSearch('tab-b')).toBe(true)
    expect(opened).toBe('b')
  })

  test('unknown tabId → trigger 返回 false（dispatcher 不 preventDefault）', () => {
    registerSessionDetailHandlers('tab-a', {
      jumpToLatest: () => {},
      openSearch: () => {},
    })
    expect(triggerJumpToLatest('tab-zzz-not-registered')).toBe(false)
    expect(triggerOpenSearch('tab-zzz-not-registered')).toBe(false)
  })

  test('null/empty tabId → trigger 返回 false', () => {
    registerSessionDetailHandlers('tab-a', {
      jumpToLatest: () => {},
      openSearch: () => {},
    })
    expect(triggerJumpToLatest(null)).toBe(false)
    expect(triggerOpenSearch(null)).toBe(false)
    expect(triggerJumpToLatest('')).toBe(false)
    expect(triggerOpenSearch('')).toBe(false)
  })

  test('unregister 后 trigger 该 tabId 返回 false', () => {
    let called = 0
    registerSessionDetailHandlers('tab-a', {
      jumpToLatest: () => {
        called += 1
      },
      openSearch: () => {},
    })
    expect(triggerJumpToLatest('tab-a')).toBe(true)
    expect(called).toBe(1)

    unregisterSessionDetailHandlers('tab-a')
    expect(triggerJumpToLatest('tab-a')).toBe(false)
    expect(called).toBe(1) // 没再增
  })

  test('unregister 不存在的 tabId 安全 no-op', () => {
    // 不抛错即合格
    expect(() => unregisterSessionDetailHandlers('never-registered')).not.toThrow()
  })

  test('同 tabId 重复 register 覆盖旧回调（hot-reload 场景）', () => {
    let oldCalled = 0
    let newCalled = 0
    registerSessionDetailHandlers('tab-a', {
      jumpToLatest: () => {
        oldCalled += 1
      },
      openSearch: () => {},
    })
    // 模拟 file-change 触发 SessionDetail 重 mount——同 tabId 二次注册
    registerSessionDetailHandlers('tab-a', {
      jumpToLatest: () => {
        newCalled += 1
      },
      openSearch: () => {},
    })
    expect(triggerJumpToLatest('tab-a')).toBe(true)
    expect(oldCalled).toBe(0)
    expect(newCalled).toBe(1)
  })

  test('_registeredTabIdsForTest 反映当前注册集合', () => {
    expect(_registeredTabIdsForTest()).toEqual([])
    registerSessionDetailHandlers('tab-a', {
      jumpToLatest: () => {},
      openSearch: () => {},
    })
    registerSessionDetailHandlers('tab-b', {
      jumpToLatest: () => {},
      openSearch: () => {},
    })
    const ids = _registeredTabIdsForTest()
    expect(ids).toHaveLength(2)
    expect(ids).toEqual(expect.arrayContaining(['tab-a', 'tab-b']))

    unregisterSessionDetailHandlers('tab-a')
    expect(_registeredTabIdsForTest()).toEqual(['tab-b'])
  })

  test('jumpToLatest 不影响 openSearch（同 tabId 内回调隔离）', () => {
    let jumped = 0
    let searched = 0
    registerSessionDetailHandlers('tab-a', {
      jumpToLatest: () => {
        jumped += 1
      },
      openSearch: () => {
        searched += 1
      },
    })
    triggerJumpToLatest('tab-a')
    expect(jumped).toBe(1)
    expect(searched).toBe(0)
    triggerOpenSearch('tab-a')
    expect(jumped).toBe(1)
    expect(searched).toBe(1)
  })
})
