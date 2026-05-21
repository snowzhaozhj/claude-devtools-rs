// groupCursor 单测：buildFilterCursor 构造 server-side filter cursor，
// sessionListCacheKey 组合缓存键。spec sidebar-navigation §"Worktree filter
// dropdown for multi-worktree group" Scenario "切 filter 构造 server-side
// filter cursor" + "sessionListStore cache key 含 worktree filter"。

import { describe, expect, test } from 'vitest'
import { buildFilterCursor, sessionListCacheKey } from './groupCursor'
import type { Worktree } from './api'

function wt(id: string): Worktree {
  return {
    id,
    name: id,
    path: `/tmp/${id}`,
    gitBranch: null,
    isRepoRoot: false,
    isMainWorktree: false,
    sessions: [],
    createdAt: null,
    mostRecentSession: null,
  }
}

function decodeBase64Json<T>(s: string): T {
  // 浏览器 / node 双 fallback——与 buildFilterCursor 内 encode 对偶
  if (typeof atob === 'function') {
    const bin = atob(s)
    const bytes = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i)
    return JSON.parse(new TextDecoder().decode(bytes)) as T
  }
  return JSON.parse(Buffer.from(s, 'base64').toString('utf8')) as T
}

describe('buildFilterCursor', () => {
  test('选中的 worktree NotStarted，其它 Exhausted', () => {
    const wts = [wt('wt-A'), wt('wt-B'), wt('wt-C')]
    const cursor = buildFilterCursor(wts, 'wt-B')
    const decoded = decodeBase64Json<{ perWorktree: Record<string, { kind: string }> }>(cursor)
    expect(decoded.perWorktree['wt-A'].kind).toBe('exhausted')
    expect(decoded.perWorktree['wt-B'].kind).toBe('not_started')
    expect(decoded.perWorktree['wt-C'].kind).toBe('exhausted')
  })

  test('未匹配的 selectedWorktreeId 让全部 worktree Exhausted（空结果）', () => {
    const wts = [wt('wt-A'), wt('wt-B')]
    const cursor = buildFilterCursor(wts, 'unknown')
    const decoded = decodeBase64Json<{ perWorktree: Record<string, { kind: string }> }>(cursor)
    for (const v of Object.values(decoded.perWorktree)) {
      expect(v.kind).toBe('exhausted')
    }
  })

  test('空 group 返回空 perWorktree', () => {
    const cursor = buildFilterCursor([], 'wt-A')
    const decoded = decodeBase64Json<{ perWorktree: Record<string, unknown> }>(cursor)
    expect(Object.keys(decoded.perWorktree)).toHaveLength(0)
  })

  test('wire 字段 perWorktree 是 camelCase（Rust serde 改 camelCase 后跨端契约）', () => {
    const wts = [wt('a')]
    const cursor = buildFilterCursor(wts, 'a')
    const decoded = decodeBase64Json<Record<string, unknown>>(cursor)
    expect('perWorktree' in decoded).toBe(true)
    expect('per_worktree' in decoded).toBe(false)
  })
})

describe('sessionListCacheKey', () => {
  test('"全部" filter 用 null 拼空尾段', () => {
    expect(sessionListCacheKey('group-X', null)).toBe('group-X::')
  })

  test('具体 worktree filter 拼 worktree id', () => {
    expect(sessionListCacheKey('group-X', 'wt-1')).toBe('group-X::wt-1')
  })

  test('同 groupId 不同 filter 产生不同 key（避免 cache 串台）', () => {
    expect(sessionListCacheKey('g', null)).not.toBe(sessionListCacheKey('g', 'wt-1'))
    expect(sessionListCacheKey('g', 'wt-1')).not.toBe(sessionListCacheKey('g', 'wt-2'))
  })
})
