## Context

PR #37 archive 后 codex 二审报告 file-change × SearchBar 索引过期 bug：

- `SessionDetail.svelte::refreshDetail` 在 file-change handler 触发时 `detail = d`，chunk 用稳定 key DOM 不重建
- `SearchBar.svelte::doSearch` 在用户输入时跑 `highlightMatches` 在容器内插入 `<mark data-search-match="N">` 元素，`totalMatches` / `currentIndex` 是 SearchBar 局部状态
- file-change 后场景：(a) 已渲染 chunk 的 `<mark>` 元素仍在（lazy observer 跳过 `dataset.rendered === "1"` 不重新 `renderInto`）；(b) 新增 chunk 是新占位 div 没有 mark；(c) 已渲染但**文本变化**的 chunk —— 实际不存在该路径，因为 lazy observer 跳过已渲染元素，文本不会更新

窄场景但用户走 next / prev 时按索引 0..N-1 循环，N 是旧总数，不反映新内容。修法成本低（一个 prop + 一个 `$effect`），适合在 follow-up PR 关闭。

## Goals / Non-Goals

**Goals:**
- file-change 写入新 chunk 后 SearchBar 自动重搜，匹配总数与新内容同步
- 不影响未启用 SearchBar / SearchBar 关闭场景的渲染开销
- prop 设计向后兼容：未传 `contentVersion` 的调用方维持旧行为

**Non-Goals:**
- 不优化 file-change 路径本身（节流 / 合并已在 `fileChangeStore::dedupeRefresh` / `scheduleRefresh` 处理）
- 不引入"局部增量重搜"（按新增 chunk 跑 incremental highlight），保持每次 `doSearch` 全量重跑
- 不改 SearchBar 的 mark 索引算法（仍按 DOM 顺序 0..N-1）

## Decisions

### D1: 版本号递增 vs 直接传 detail 引用

**选版本号递增**：SessionDetail 持有 `searchContentVersion: number`，`refreshDetail` 后 `searchContentVersion++`，作为数字 prop 传 SearchBar。

**候选**：
- A. **版本号递增**（选）：原生 number，Svelte 5 reactive 系统天然支持；SearchBar 的 `$effect` 仅依赖一个标量；测试时 mock 简单
- B. **直接传 `detail` 引用**：SearchBar `$effect` 监听 detail 变化。耦合 SearchBar 与 SessionDetail 的数据结构（detail 类型），SearchBar 应保持容器无关
- C. **EventEmitter / store**：引入额外 store 协调过度设计

选 A 的理由：
- SearchBar 只关心"内容变了"这一信号，无需理解 detail 结构
- 版本号是 monotonic counter，幂等可重入
- prop 类型最简（number 比 object 引用更清晰）

### D2: `contentVersion` 变化的响应策略

**选自动重跑 doSearch**：`$effect(() => { contentVersion; if (visible && query) doSearch(); })`，仅在 visible + 有 query 时触发，避免不必要工作。

**候选**：
- A. **自动重跑**（选）：用户感知一致，不需要手动重新输入
- B. **仅在导航触发**（next/prev 时检查 version）：需要在 next/prev 函数里加 stale 检查，分散逻辑
- C. **加按钮提示用户重搜**：UX 干扰，违反"对齐原版（无该交互）"原则

选 A 的理由：
- 自动重搜是用户预期（搜索结果应反映当前内容）
- `$effect` 的依赖追踪保证幂等：连续 file-change 触发多次 contentVersion 变化，每次都重搜（debounce 由上游 `scheduleRefresh` 已节流）
- 性能成本：每次 `doSearch` 已包含 `flushAll` + `highlightMatches`，是同步操作，与用户输入触发的 doSearch 等价

### D3: `$effect` 依赖追踪与初次 mount

**实现**：`$effect` 依赖 `contentVersion`，但 `visible` 与 `query` 是条件 gate 不放到依赖追踪里。Svelte 5 自动追踪 `$effect` 内读取的所有 `$state`，所以 `if (visible && query)` 内的 `visible` / `query` 也会被追踪 — 即 SearchBar 可见时输入 query 也会触发 effect 一次。这是期望行为：第一次输入触发 `doSearch`，后续 contentVersion 变化也触发。

为避免初次 mount 触发空 doSearch（`query == ""` 直接 return），在 `doSearch` 入口已有 `if (!query) { totalMatches = 0; ... return; }` 保护，无副作用。

### D4: SessionDetail 的版本号位置与递增时机

**实现**：`searchContentVersion = $state(0)`，在 `refreshDetail` 函数内 `try { ... detail = d; ... searchContentVersion++; }`。注意：cache 命中路径（`onMount` 内的 `if (cached)`）不递增 — cache 是 SessionDetail 切走再切回的旧快照，但首次渲染时 SearchBar 不可见或 query 为空，`$effect` 内 gate 自然短路。

### D5: 测试策略

vitest 覆盖：
- SearchBar `contentVersion` 变化触发 doSearch（mock highlightMatches spy 验证调用次数）
- SearchBar visible=false 时 contentVersion 变化不触发 doSearch
- SearchBar query="" 时 contentVersion 变化不触发 highlightMatches（短路在 doSearch 内）

## Risks / Trade-offs

- [file-change 频率高时 SearchBar 重复 `doSearch` 开销] → 上游 `scheduleRefresh` 已节流；`doSearch` 内 `flushAll` 幂等无成本（已渲染元素跳过）
- [`$effect` 依赖追踪意外触发] → Svelte 5 的 `$effect` 在依赖未变化时不重跑；`visible` / `query` 变化也会触发但已有 `doSearch` 内 gate 保护
- [未传 `contentVersion` 的调用方退化为旧 bug] → 当前唯一调用方是 SessionDetail，本 change 同步透传；spec scenario 锁定调用方契约，未来新调用方需补 prop

## Migration Plan

1. SearchBar 加 `contentVersion?: number` prop + `$effect` 重搜
2. SessionDetail 加 `searchContentVersion` 状态 + `refreshDetail` 内递增 + 透传
3. vitest 单测
4. `just preflight` + 手动 smoke（构造 file-change 场景）
5. 回滚：revert 两个文件 commit + spec delta
