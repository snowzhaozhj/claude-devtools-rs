/**
 * dispatcher 命中算法 microbench（覆盖 tasks.md §3.8）。
 *
 * 验证：14 spec 注册下 `normalize + Map.get + guard chain + handler` 单次
 * dispatch ≤ 0.5ms（dev 机基线）。回归阈值由 `bench` 自身的 hz/throughput
 * 兜底——若 hz < 2000（即均值 > 0.5ms）就是回归。
 *
 * 跑：`pnpm exec vitest bench src/lib/keyboard/__tests__/dispatcher.bench.ts`
 *
 * 注意：vitest `bench()` 与 `test()` 互斥，本文件**只**含 bench。
 */

import { bench, beforeAll } from 'vitest'

import {
  _dispatcherForTest,
  _resetForTest,
  registerShortcut,
  type ShortcutSpec,
} from '../registry'
import { _resetPlatformCache } from '../../platform'

function makeSpec(over: Partial<ShortcutSpec> = {}): ShortcutSpec {
  return {
    id: 'x',
    category: 'global',
    description: 'x',
    defaultBinding: 'mod+k',
    handler: () => {},
    ...over,
  }
}

beforeAll(() => {
  // 锁 mac → mod=meta
  Object.defineProperty(navigator, 'userAgentData', {
    value: { platform: 'macOS' },
    configurable: true,
    writable: true,
  })
  _resetPlatformCache()
  _resetForTest()

  // 注册 14 条 spec（贴近 §3.4 默认表 ~17 条规模），覆盖字母 / 数字 / 方向键 / 多修饰键
  const bindings: Array<[string, string]> = [
    ['command-palette.toggle', 'mod+k'],
    ['sidebar.toggle', 'mod+b'],
    ['tab.close', 'mod+w'],
    ['tab.next', 'mod+]'],
    ['tab.prev', 'mod+['],
    ['pane.split', 'mod+\\'],
    ['pane.focus.next', 'mod+alt+ArrowRight'],
    ['pane.focus.prev', 'mod+alt+ArrowLeft'],
    ['session.jump-to-latest', 'mod+ArrowDown'],
    ['tab.switch.1', 'mod+1'],
    ['tab.switch.2', 'mod+2'],
    ['tab.switch.3', 'mod+3'],
    ['tab.switch.4', 'mod+4'],
    ['tab.switch.5', 'mod+5'],
  ]
  for (const [id, b] of bindings) {
    registerShortcut(makeSpec({ id, defaultBinding: b }))
  }
})

const dispatch = (() => {
  // 尽量接近真实路径，但避免 module-eval 阶段触发 _dispatcherForTest
  let cached: ((e: KeyboardEvent) => void) | null = null
  return (e: KeyboardEvent) => {
    if (!cached) cached = _dispatcherForTest()
    cached(e)
  }
})()

function ev(init: KeyboardEventInit): KeyboardEvent {
  return new KeyboardEvent('keydown', init)
}

// 命中：mod+k 走 handler + preventDefault
bench(
  'dispatch hit (mod+k)',
  () => {
    dispatch(ev({ key: 'k', metaKey: true }))
  },
  { iterations: 1000 },
)

// 命中：复合修饰键 mod+alt+ArrowRight
bench(
  'dispatch hit (mod+alt+ArrowRight)',
  () => {
    dispatch(ev({ key: 'ArrowRight', metaKey: true, altKey: true }))
  },
  { iterations: 1000 },
)

// 未命中：normalize 出来但 keymap 无 entry，提前 return
bench(
  'dispatch miss (mod+z 未注册)',
  () => {
    dispatch(ev({ key: 'z', metaKey: true }))
  },
  { iterations: 1000 },
)

// 单按修饰键：normalize 返回空串，提前 return（最快路径）
bench(
  'dispatch miss (单按 Meta)',
  () => {
    dispatch(ev({ key: 'Meta', metaKey: true }))
  },
  { iterations: 1000 },
)
