## Why

archive change `2026-05-16-session-detail-scroll-cpu-opt`（PR #108）的 D1 在 SessionDetail 对话流容器上引入 `content-visibility: auto` + `contain-intrinsic-size: auto 220px` 估算占位。当 chunk 真实高度（多数为 23-100 px 短消息）远小于 220 px 估算值时，离屏容器 contain↔uncontain 切换让 conversation 的 `scrollHeight` 在用户滚动经过时反复跳变，触发视觉抖动。

客观测量证据（本仓 2026-05-24 长会话现场，34 chunk）：未禁用时 10 秒滚动期间 `scrollHeight` 变化 11 次 / 总幅度 5291 px / 单次最大 4180 px / dh 分布 `-197 / -139 / -174 / -108` 恰为 `220 px estimate − 真实高度` 的指纹；全局 CSS inject `content-visibility: visible !important` 后同一操作 `scrollHeight` 变化 0 次 / 0 px，物理上完全 stable。键盘方向键滚动同段亦不抖（与触控板惯性末段微震区分）。

`scroll-cpu-opt` 的另一决策 D3（无语言 fenced code block 默认 plaintext、关闭 `highlightAuto`）才是 CPU 大头；删除 D1 不动 D3 即可消除抖动，不丧失主性能优化。

## What Changes

- 删除 `ui/src/routes/SessionDetail.svelte` 中 `.msg-row-contained` 样式类整段定义（含 `content-visibility: auto` / `contain: layout style` / `contain-intrinsic-size: auto 220px` 三个属性，以及含 `.mermaid-block` 的豁免规则）
- 删除模板里所有 `class="... msg-row-contained"` 与 `class:msg-row-contained={...}` 应用点（UserChunk / AIChunk `.ai-body` / SystemChunk / CompactChunk 共 4 处）
- 删除 `ui/src/components/SessionDetail.test.svelte.ts` 中针对"含 mermaid 的 contained 区域通过 CSS 关闭 content-visibility"的测试 case
- 删除 SessionDetail.svelte 中 3 处与 D1 trade-off 直接相关的遗留注释
- `session-display` capability spec 中 `Requirement: SessionDetail 滚动路径渲染隔离` 整段移除；同时新增反模式约束句沉淀历史经验，禁止后续以"性能优化"为由再次引入 `content-visibility` / `contain-intrinsic-size` 做高度估算占位的同类机制
- `Requirement: 无语言代码块高亮自动检测限制`（D3）**保留不动**

## Capabilities

### New Capabilities

无

### Modified Capabilities

- `session-display`: 移除 `Requirement: SessionDetail 滚动路径渲染隔离` 整条；新增反模式约束句进入对话流渲染相关 Requirement 防回归

## Impact

- 影响前端：`ui/src/routes/SessionDetail.svelte`（CSS + 4 处模板 + 3 处注释）、`ui/src/components/SessionDetail.test.svelte.ts`（1 条 test case）
- 不影响 Rust 后端 / Tauri IPC / 其它 capability
- 性能 trade-off：CPU 大头由 D3 承担，删 D1 单独影响有限；merge 前 SHALL 在长会话上用 Activity Monitor 验证 `claude-devtools-tauri` 滚动 CPU 仍 < 15%（PR #108 design.md 当时记录的"超过 10%"阈值的 1.5x 上限）
