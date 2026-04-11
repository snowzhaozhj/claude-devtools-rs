## Why

`context-tracking` 是 dashboard 视图里"当前 context 用了多少 token / 按来源分解"这一核心可视化能力的唯一数据源。TS 侧它由 `renderer/utils/contextTracker.ts` + `claudeMdTracker.ts` 两个文件共 1800+ 行实现，既是 session-parsing / chunk-building 之后最大的纯数据转换层，也是后续 `ipc-data-api` / `http-data-api` 暴露给 UI 的关键 payload 之一。把它 port 过来，意味着 Rust 数据层第一次产出"每 AI group 的 ContextStats"的结构化结果，为后续 API 层铺路。

此外，本次 port 还要：
- 按 followups §context-tracking 的 coverage-gap 给 `compute_context_stats` / `process_session_context_with_phases` 补上单测 —— TS 侧这两个核心函数完全没测试。
- 冻结一条"token 估计契约"（`chars / 4`，与 TS `estimateTokens` 对齐），并放进 `cdt-core` 里作为所有下游复用的工具函数，避免每个消费方各写一份。
- 显式确定 I/O 边界：CLAUDE.md 文件的实际读取 / 落盘不在本 port 内，由后续 `port-configuration-management` 接管；本 port 只接受"外部注入的 token 数据字典"，`cdt-analyze` 保持同步 / 零 runtime。

## What Changes

- **新 capability crate 模块**：`cdt-analyze::context` 子模块，包含：
  - `types.rs` / `injection.rs` / `stats.rs` / `phase.rs` / `session.rs` 分层，按职责拆分纯函数。
  - 6 种 `ContextInjection` variant（`ClaudeMd` / `MentionedFile` / `ToolOutput` / `ThinkingText` / `TeamCoordination` / `UserMessage`），对应 spec 里的 6 大类别。
  - 聚合函数 `aggregate_tool_outputs` / `aggregate_task_coordination` / `aggregate_thinking_text` / `create_user_message_injection`，各自纯函数、可单测。
  - 组合函数 `compute_context_stats(params) -> (ContextStats, PreviousPaths)`，对齐 TS `computeContextStats`。
  - 会话级函数 `process_session_context_with_phases(chunks, params) -> SessionContextResult`，对齐 TS `processSessionContextWithPhases`，内部处理 compact 边界与 phase 切换。
- **cdt-core 新增共享类型与工具**：
  - `cdt-core::context` 模块，承载 `ContextInjection`、`ContextStats`、`TokensByCategory`、`ContextPhase`、`ContextPhaseInfo`、`CompactionTokenDelta` 等 API-facing 类型。
  - `cdt-core::tokens::estimate_tokens(text: &str) -> usize`，以 `(len + 3) / 4` 实现，与 TS `Math.ceil(length / 4)` 语义对齐。外加 `estimate_content_tokens(value: &serde_json::Value)` 处理 array / object（`serde_json::to_string` 后再估）。
- **spec delta**：
  - **ADDED Requirement**：`Expose a pure synchronous API driven by chunk output` —— 显式冻结 Rust 侧"上游传入 `&[Chunk]` + 外部注入的 token 数据字典，函数纯同步返回 `ContextStats` / `SessionContextResult`"的 API 契约。这是 Rust 版对 TS 里"React hook 驱动"API 的语义平移，需要写进 spec 让下游 port 依赖稳定。
  - **ADDED Requirement**：`Estimate token counts with a 4-character heuristic` —— 冻结 token 估计算法（`⌈len/4⌉`），并规定该函数由 `cdt-core::tokens` 统一提供；任何 context-tracking 消费方不得自己 re-implement。
  - **MODIFIED Requirement**：`Compute cumulative context statistics per turn` —— 原文保留，新增 1 个 scenario 覆盖"空 AI group 返回空 stats"的边界，堵 TS 侧 coverage-gap。
  - **MODIFIED Requirement**：`Reset accumulated context on compaction boundaries` —— 原文保留，新增 1 个 scenario 覆盖"compact 边界后第一个 AI group 的 `CompactionTokenDelta` 计算"，堵 TS 侧的隐式行为。
- **不包含**：
  - 真实读 CLAUDE.md 文件、真实 `@mention` 的文件系统 resolve（属于 `port-configuration-management`）。
  - Teammate message token 计算里"teammate 身份识别"的部分（属于 `port-team-coordination-metadata`，本 port 只给一个 `teammate_message.token_count` 透传字段，不做身份拆解）。
  - UI 层 `ContextBadge` / `ContextPanel` 的展示逻辑。
  - Read tool 返回内容的深度解析（只取 `tokenCount`/`tokenCount` 字段求和）。
- **coverage-gap 修复**：`crates/cdt-analyze/tests/context_tracking.rs` 集成测试补齐 `compute_context_stats` × 2 scenario、`process_session_context_with_phases` × 3 scenario（对应 followups §context-tracking 第 3 条）。

## Capabilities

### New Capabilities
<!-- 无：context-tracking 已存在于 openspec/specs/ -->

### Modified Capabilities
- `context-tracking`: 以 Rust 实现替代 TS baseline，显式冻结两条此前未写进 spec 的契约（Rust 侧 sync API shape、token 估计算法），并给已有的两条 Requirement 补上 coverage-gap scenario。不改动既有 Requirement 的主文。

## Impact

- **新代码**：
  - `crates/cdt-core/src/context.rs`（共享类型）
  - `crates/cdt-core/src/tokens.rs`（`estimate_tokens` / `estimate_content_tokens`）
  - `crates/cdt-analyze/src/context/mod.rs` + `types.rs` / `injection.rs` / `aggregator.rs` / `stats.rs` / `phase.rs` / `session.rs`
  - `crates/cdt-analyze/tests/context_tracking.rs`
- **依赖新增**：无新 crate 级依赖；`cdt-analyze` 已经有 `serde`、`serde_json`，本 port 只新增内部模块。`cdt-core` 也无新依赖。
- **下游影响**：`ContextInjection` / `ContextStats` 是 `cdt-api` 后续要暴露给前端的类型；本次 port 要把它们的 `Serialize` / `Deserialize` 形状与 TS `ContextInjection` 等字段的 JSON shape 对齐（`snake_case` ↔ `camelCase` 通过 `#[serde(rename_all = "camelCase")]`），避免 API port 时再破坏兼容。
- **followups 联动**：`openspec/followups.md` §context-tracking 的第 3 条 coverage-gap 标记为 ✅，指向本 port 的测试文件。
- **无破坏**：`cdt-analyze` 原有的 chunk / tool_linking 模块零改动。
