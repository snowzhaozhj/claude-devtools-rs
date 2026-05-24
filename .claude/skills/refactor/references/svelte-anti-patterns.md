# Svelte 5 反模式（仓特定）

本仓 UI 是 Svelte 5 + Vite + vitest + Playwright，详细约束见 `ui/CLAUDE.md`。本文聚焦 audit 视角的反模式识别。

## 1. Runes 误用

| category | 反模式 | 期望 |
|---|---|---|
| `svelte-mixed-runes` | 同组件里既用 `$:` 又用 `$state` / `$derived` | 全迁移 runes；纯机械迁移走纯结构改，但要 grep 调用方确认无依赖反应式时序 |
| `svelte-state-in-rerun` | `$derived` 内部读外部 store 又写另一个 store（副作用） | `$derived` 只能纯计算；副作用用 `$effect` 或 event handler |
| `svelte-effect-overuse` | 任何 reactivity 都塞进 `$effect`，丢失 derived 优化 | 派生值优先 `$derived`；`$effect` 只用于真正的副作用（DOM 操作 / IPC 触发） |
| `svelte-rune-in-class` | class 字段用 `$state` 但消费方不在 component context | runes 只在 `.svelte` / `.svelte.ts` 文件内有效；其它用 store |

## 2. 反应式时序陷阱

| category | 反模式 | 期望 |
|---|---|---|
| `svelte-cache-fallback` | `cache + ?? fallback` 兜底反模式（cache 写完永远不 null fallback 失效） | 详 **`ui/CLAUDE.md`**（codex 三轮 CR 重复抓到的真相源；本 skill 仅复用 category 命名作 cross-reference）|
| `svelte-derived-stale-dep` | `$derived(expensive(deps))` 但 deps 之一是 ref / 非 reactive 值 | 拆开看每个 dep 是不是真 reactive；非 reactive 用 props |
| `svelte-effect-loop` | `$effect` 写 state 又被自己读到，引发循环 | 加 untrack / 拆 effect / 重新设计数据流 |

## 3. 列表 / key

| category | 反模式 | 期望 |
|---|---|---|
| `svelte-key-index` | `{#each items as item, i (i)}` —— index 当 key 让 reorder 后整个 DOM 重渲 | 用稳定唯一 ID `(item.id)` |
| `svelte-key-missing` | `{#each items as item}` 不带 key，子组件 state 错位 | 加 key |
| `svelte-list-flicker` | 列表频繁 re-render 致 shimmer / 闪烁 | 看是否 source array reference 频繁替换；`$derived` 缓存或 `key` 稳定 |

## 4. 组件边界

| category | 反模式 | 期望 |
|---|---|---|
| `svelte-prop-drilling` | 同一 prop 跨 ≥ 3 层往下传 | context API 或 store |
| `svelte-bivariant-binding` | `bind:` 跨组件双向绑定深层字段 | 改单向 props 下传 + event 上抛；防止深层修改难追踪 |
| `svelte-side-effect-mount` | `onMount` / `$effect` 内做长 IPC + 不取消 | 加 cancellation；组件 unmount 后 IPC 仍写 state 会 leak |

## 5. 测试 / mockIPC

| category | 反模式 | 期望 |
|---|---|---|
| `svelte-test-mockipc-stale` | mockIPC fixture 的字段形状与真后端不同步 | IPC 字段改后 SHALL 同步 mockIPC fixture + IPC contract test |
| `svelte-test-no-tauri-smoke` | 只跑 vitest + mockIPC 不测真桌面端 | 涉及 Tauri-only API（通知 / 托盘 / setBadgeCount）SHALL `just dev` 手动 smoke |

## 6. CSS / 视觉规范

视觉细节由 `impeccable` skill + `DESIGN.md` 负责。本 audit 只列**违反硬约束**的：

- 硬编码颜色值（应用 CSS 变量）
- inline style 滥用（应走 class）
- `!important` 滥用（应走 specificity）

具体规约 → `ui/CLAUDE.md::CSS / 视觉` 与 `DESIGN.md::Named Rules`。
