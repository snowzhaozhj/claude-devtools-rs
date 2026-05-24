# Svelte 5 结构反模式（仓特定）

本文聚焦 **结构维度** 反模式：runes 用法 / 反应式时序 / 列表 key / 组件边界。

**不在本 catalog 范围**（其它 skill / reviewer 的本职）：
- 性能向（列表 flicker / re-render 频繁）
- 资源泄漏 / 副作用 bug（onMount 长 IPC 不取消等行为问题）
- 测试基础设施（mockIPC fixture 同步 / Tauri smoke）
- CSS / 视觉规范

## 1. Runes 误用

| category | 反模式 | 期望 |
|---|---|---|
| `svelte-mixed-runes` | 同组件里既用 `$:` 又用 `$state` / `$derived` | 全迁移 runes；纯机械迁移走真 refactor，但要 grep 调用方确认无依赖反应式时序（→ §2 boundary guard #4 评估） |
| `svelte-state-in-derived` | `$derived` 内部读外部 store 又写另一个 store（副作用） | `$derived` 只能纯计算；副作用用 `$effect` |
| `svelte-effect-overuse` | 任何 reactivity 都塞进 `$effect`，丢失 derived 优化 | 派生值优先 `$derived`；`$effect` 只用于真正的副作用 |
| `svelte-rune-in-class` | class 字段用 `$state` 但消费方不在 component context | runes 只在 `.svelte` / `.svelte.ts` 文件内有效；其它用 store |

## 2. 反应式时序结构

| category | 反模式 | 期望 |
|---|---|---|
| `svelte-cache-fallback` | `cache + ?? fallback` 兜底反模式（cache 写完永远不 null fallback 失效） | 真相源详 `ui/CLAUDE.md`；本 catalog 仅复用 category 命名 |
| `svelte-derived-stale-dep` | `$derived(expensive(deps))` 但 deps 之一是非 reactive 值 | 拆开看每个 dep 是不是真 reactive；非 reactive 用 props |
| `svelte-effect-loop` | `$effect` 写 state 又被自己读到，引发循环 | 加 untrack / 拆 effect / 重新设计数据流 |

## 3. 列表 / key

| category | 反模式 | 期望 |
|---|---|---|
| `svelte-key-index` | `{#each items as item, i (i)}` —— index 当 key 让 reorder 后整个 DOM 重渲 | 用稳定唯一 ID `(item.id)` |
| `svelte-key-missing` | `{#each items as item}` 不带 key，子组件 state 错位 | 加 key |

## 4. 组件边界

| category | 反模式 | 期望 |
|---|---|---|
| `svelte-prop-drilling` | 同一 prop 跨 ≥ 3 层往下传 | context API 或 store |
| `svelte-bivariant-binding` | `bind:` 跨组件双向绑定深层字段 | 改单向 props 下传 + event 上抛 |
