// issue #259 metadata-pending shimmer 阈值判定单测。
// 三态契约：pending 但阈值前 → false；pending 且超阈值 → true；
// resolved（任一字段非占位）→ 永远 false。
// Sidebar.svelte 内 SvelteMap + 250 ms ticker 的 reactivity 桥由 e2e 覆盖；
// 本文件锁纯函数侧的判定，让阈值修改 / 边界 case 有快速 unit gate。

import { describe, expect, test } from 'vitest'

import type { SessionSummary } from './api'
import {
  METADATA_SHIMMER_DELAY_MS,
  isSessionMetadataPending,
  shouldShowMetadataShimmer,
} from './metadataShimmer'

const skeleton: SessionSummary = {
  sessionId: 's1',
  projectId: 'p1',
  timestamp: 0,
  messageCount: 0,
  title: null,
  isOngoing: false,
  gitBranch: null,
}

describe('isSessionMetadataPending', () => {
  test('全占位 → true', () => {
    expect(isSessionMetadataPending(skeleton)).toBe(true)
  })

  test('title 非空 → false', () => {
    expect(isSessionMetadataPending({ ...skeleton, title: '真实标题' })).toBe(false)
  })

  test('messageCount > 0 → false', () => {
    expect(isSessionMetadataPending({ ...skeleton, messageCount: 1 })).toBe(false)
  })

  test('isOngoing → false', () => {
    expect(isSessionMetadataPending({ ...skeleton, isOngoing: true })).toBe(false)
  })

  // gitBranch 非空但其他字段都 placeholder 的 session 仍视作 pending：spec
  // sidebar-navigation §"Metadata 占位字段视觉渐显" 把 gitBranch 当独立维度
  // 处理（branch 在骨架阶段也可能由 cwd hint 提前带回），不参与 pending 判定。
  test('gitBranch 非空但 title/count/ongoing 仍占位 → 仍 pending', () => {
    expect(isSessionMetadataPending({ ...skeleton, gitBranch: 'main' })).toBe(true)
  })
})

describe('shouldShowMetadataShimmer', () => {
  const baseRequestedAt = 1_000_000

  test('pending && now - requestedAt > 阈值 → true', () => {
    const now = baseRequestedAt + METADATA_SHIMMER_DELAY_MS + 1
    expect(shouldShowMetadataShimmer(skeleton, baseRequestedAt, now)).toBe(true)
  })

  test('pending && now - requestedAt === 阈值 → false（严格大于）', () => {
    const now = baseRequestedAt + METADATA_SHIMMER_DELAY_MS
    expect(shouldShowMetadataShimmer(skeleton, baseRequestedAt, now)).toBe(false)
  })

  test('pending && now - requestedAt < 阈值 → false', () => {
    const now = baseRequestedAt + 100
    expect(shouldShowMetadataShimmer(skeleton, baseRequestedAt, now)).toBe(false)
  })

  test('requestedAt = null → false（尚未登记，不显 shimmer）', () => {
    const now = baseRequestedAt + 9999
    expect(shouldShowMetadataShimmer(skeleton, null, now)).toBe(false)
  })

  test('requestedAt = undefined → false（map.get 未命中等价）', () => {
    const now = baseRequestedAt + 9999
    expect(shouldShowMetadataShimmer(skeleton, undefined, now)).toBe(false)
  })

  test('resolved session（title 非空）即使等待很久也不显', () => {
    const resolved = { ...skeleton, title: 'real' }
    const now = baseRequestedAt + METADATA_SHIMMER_DELAY_MS * 10
    expect(shouldShowMetadataShimmer(resolved, baseRequestedAt, now)).toBe(false)
  })

  test('resolved session（messageCount > 0）即使等待很久也不显', () => {
    const resolved = { ...skeleton, messageCount: 3 }
    const now = baseRequestedAt + METADATA_SHIMMER_DELAY_MS * 10
    expect(shouldShowMetadataShimmer(resolved, baseRequestedAt, now)).toBe(false)
  })

  test('thresholdMs 参数可注入（便于其他时序场景测试）', () => {
    const now = baseRequestedAt + 700
    expect(shouldShowMetadataShimmer(skeleton, baseRequestedAt, now, 500)).toBe(true)
    expect(shouldShowMetadataShimmer(skeleton, baseRequestedAt, now, 1000)).toBe(false)
  })
})

// 防回归：阈值 1500 ms 是 issue #259 + 原 CSS animation 周期共识。
// 改阈值 SHALL 同步：
//   - 本文件 expect(METADATA_SHIMMER_DELAY_MS).toBe(1500)
//   - Sidebar.svelte 注释「issue #259 触发条件」一段
//   - 启动 5 s 内 shimmer 元素 < 5 个的验收预算（issue 验收标准）
test('METADATA_SHIMMER_DELAY_MS 锁定 1500 ms', () => {
  expect(METADATA_SHIMMER_DELAY_MS).toBe(1500)
})
