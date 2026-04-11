## 1. 前置体检：cdt-core 类型字段盘点

- [x] 1.1 盘点 `cdt-core::chunk::AIChunk` / `SemanticStep` / `AssistantResponse` / `ChunkMetrics` 当前所有字段，输出一份"TS `AIGroup` / `AIGroupDisplayItem` / `ContextStats` 所依赖的每个字段 ↔ Rust 当前有无"的对照表（写到实施笔记或 PR 说明里，不进仓库）。
- [x] 1.2 盘点 `cdt-core::tool_execution::ToolExecution` / `ToolOutput` 的字段，确认 `call_tokens` / `result_tokens` / `skill_instructions_token_count` / `is_error` 是否齐全。
- [x] 1.3 盘点 `cdt-core::message::ParsedMessage` 的 `usage` 字段是否有 `input_tokens` / `output_tokens` / `cache_read_input_tokens` / `cache_creation_input_tokens` —— 为 `CompactionTokenDelta` 计算做准备。
- [x] 1.4 若任何字段缺失，列出补齐清单；若全部满足，本 §1 的后续任务可跳过；否则进入 §2。

## 2. cdt-core 类型扩展（按 §1 结果）

- [x] 2.1 如果 `ToolExecution` 缺 `skill_instructions_token_count: Option<u32>`：加字段，`#[serde(default)]`，默认值 `None`。
- [x] 2.2 如果 `SemanticStep` 缺 `thinking` / `output` / `slash` / `teammate_message` 任一 variant：扩 `enum SemanticStep`，旧变体保持不变；给新变体加最小字段集（`token_count: Option<u32>` 必带）。
- [x] 2.3 如果 `ParsedMessage.usage` 缺 `cache_*` 字段：扩 `TokenUsage` 结构，`#[serde(default)]` 向后兼容。
- [x] 2.4 跑 `cargo test -p cdt-core` + `cargo test -p cdt-parse` + `cargo test -p cdt-analyze` 回归，确认扩字段没破旧测试。
- [x] 2.5 `cargo clippy -p cdt-core --all-targets -- -D warnings` 通过。

## 3. cdt-core::tokens 模块

- [x] 3.1 新建 `crates/cdt-core/src/tokens.rs`，实现 `pub fn estimate_tokens(text: &str) -> usize`：以 `text.chars().count().div_ceil(4)` 计算；空输入返回 0。
- [x] 3.2 实现 `pub fn estimate_content_tokens(value: &serde_json::Value) -> usize`：若是 string 直接 `estimate_tokens`，否则 `serde_json::to_string(value).unwrap_or_default()` 后估。
- [x] 3.3 在 `cdt-core::lib.rs` 里 `pub mod tokens;` 与 `pub use tokens::{estimate_tokens, estimate_content_tokens};`。
- [x] 3.4 单元测试覆盖 spec 的 4 个 scenario：ASCII 长度 16 → 4；空 → 0；空白 `"   "` → 1；中文 4 字符 → 1；JSON 数组 `[1,2,3]` → 2。
- [x] 3.5 `cargo clippy -p cdt-core -- -D warnings` 通过。

## 4. cdt-core::context 类型模块

- [x] 4.1 新建 `crates/cdt-core/src/context.rs`，定义 `#[serde(rename_all = "camelCase")]` 的 `TokensByCategory { claude_md, mentioned_file, tool_output, thinking_text, task_coordination, user_messages }`，字段类型 `u32`。
- [x] 4.2 定义 `NewCountsByCategory` / `AccumulatedCountsByCategory`（字段与 `TokensByCategory` 同名，`usize` 类型，记"本轮新增 / 累计"的 injection 个数）。
- [x] 4.3 定义 `ContextInjection` 枚举，6 个 variant：`ClaudeMd { id, path, scope, estimated_tokens, first_seen_turn_index, ... }`、`MentionedFile { id, path, display_name, estimated_tokens, first_seen_turn_index, first_seen_in_group, exists }`、`ToolOutput { id, turn_index, ai_group_id, estimated_tokens, tool_count, tool_breakdown }`、`ThinkingText { id, turn_index, ai_group_id, estimated_tokens, breakdown }`、`TaskCoordination { id, turn_index, ai_group_id, estimated_tokens, breakdown }`、`UserMessage { id, turn_index, ai_group_id, estimated_tokens, text_preview }`。使用 `#[serde(tag = "category", rename_all = "kebab-case")]` 对齐 TS shape。
- [x] 4.4 定义辅助类型 `ToolTokenBreakdown`、`TaskCoordinationBreakdown`、`ThinkingTextBreakdown`、`ClaudeMdFileInfo`、`MentionedFileInfo`。
- [x] 4.5 定义 `ContextStats { new_injections, accumulated_injections, total_estimated_tokens, tokens_by_category, new_counts, accumulated_counts, phase_number: Option<u32> }`。
- [x] 4.6 定义 `ContextPhase { phase_number, first_ai_group_id, last_ai_group_id, compact_group_id: Option<String> }` 与 `ContextPhaseInfo { phases, compaction_count, ai_group_phase_map: HashMap<String, u32>, compaction_token_deltas: HashMap<String, CompactionTokenDelta> }` 与 `CompactionTokenDelta { pre_compaction_tokens, post_compaction_tokens, delta }`。
- [x] 4.7 在 `cdt-core::lib.rs` 里 `pub mod context;` 与 `pub use context::*;`，加到 `prelude`。
- [x] 4.8 单元测试：`ContextStats` 的 `serde_json::to_value` 结果里 `tokensByCategory` / `totalEstimatedTokens` / `newCounts` 等字段是 camelCase；`ContextInjection::ClaudeMd` 序列化出 `"category":"claude-md"`。
- [x] 4.9 `cargo clippy -p cdt-core --all-targets -- -D warnings` 通过。

## 5. cdt-analyze::context 模块骨架

- [x] 5.1 新建 `crates/cdt-analyze/src/context/mod.rs` + `aggregator.rs` + `stats.rs` + `session.rs`，`mod.rs` 作为 pub 入口。
- [x] 5.2 `cdt-analyze::lib.rs` 添加 `pub mod context;` 并 `pub use context::{process_session_context_with_phases, compute_context_stats, ProcessSessionParams, ComputeStatsParams, SessionContextResult};`。
- [x] 5.3 定义 `ProcessSessionParams<'a>` / `ComputeStatsParams<'a>` / `ComputeStatsResult` / `SessionContextResult` 的具体字段（按 design §决策 2）。
- [x] 5.4 `cargo build -p cdt-analyze` 通过（即便函数体是 `todo!()`）。

## 6. aggregator：tool_outputs / task_coordination / thinking_text / user_message

- [x] 6.1 在 `aggregator.rs` 定义常量 `const TASK_COORDINATION_TOOL_NAMES: &[&str] = &["SendMessage", "TeamCreate", "TeamDelete", "TaskCreate", "TaskUpdate", "TaskList", "TaskGet"];`。
- [x] 6.2 实现 `fn aggregate_tool_outputs(tools: &[ToolExecution], turn_index: u32, ai_group_id: &str, display_items: &[SemanticStep]) -> Option<ContextInjection>`：遍历 tools，跳过 task coordination；对每个工具求 `call_tokens + result_tokens + skill_instructions_token_count`，附加 slash display item 的 token，总和为 0 返回 `None`。Task 工具名显示为 `Task (Subagent)`。
- [x] 6.3 实现 `fn aggregate_task_coordination(tools, turn_index, ai_group_id, display_items) -> Option<ContextInjection>`：遍历 tools 取 7 个 task 工具，`SendMessage` 若 `input.recipient` 存在则 label 加 recipient 名；遍历 display items 取 `TeammateMessage`，总和为 0 返回 `None`。
- [x] 6.4 实现 `fn aggregate_thinking_text(display_items, turn_index, ai_group_id) -> Option<ContextInjection>`：扫 `thinking` + `output` 两种 variant 的 `token_count`，分别聚合为 `ThinkingTextBreakdown`，总和为 0 返回 `None`。
- [x] 6.5 实现 `fn create_user_message_injection(user_chunk: &UserChunk, turn_index, ai_group_id) -> Option<ContextInjection>`：取 `raw_text`（或 `text` fallback），`estimate_tokens` 结果为 0 返回 `None`；`text_preview` 截断到 80 字符加 `…`。
- [x] 6.6 每个函数底部 `#[cfg(test)] mod tests`，各至少 2 个 case（非空 / 空）。
- [x] 6.7 `cargo clippy -p cdt-analyze -- -D warnings` 通过。

## 7. stats：compute_context_stats

- [x] 7.1 在 `stats.rs` 实现 `pub fn compute_context_stats(params: &ComputeStatsParams<'_>) -> ComputeStatsResult`：
  - 调用 4 个 aggregator 产出 new injections；
  - 用 `previous_paths` 对 CLAUDE.md / mentioned-file 做去重；
  - 计算 `tokens_by_category` / `new_counts` / `accumulated_counts` / `total_estimated_tokens`；
  - 返回 `{ stats, next_previous_paths }`。
- [x] 7.2 对"空 AI group"分支产出零 stats（对齐本 port MODIFIED scenario）。
- [x] 7.3 单元测试：
  - 单 tool output + 单 user message → 2 injection、total > 0；
  - 空 AI group → 6 category 全 0、`total == 0`、`new_injections.is_empty()`；
  - 同一 file path 两次 compute 应只在第一次进 `new_injections`。
- [x] 7.4 clippy 通过。

## 8. session：process_session_context_with_phases

- [x] 8.1 在 `session.rs` 实现 `pub fn process_session_context_with_phases(chunks: &[Chunk], params: &ProcessSessionParams<'_>) -> SessionContextResult`。
- [x] 8.2 维护的可变状态：`accumulated_injections`、`previous_paths`、`is_first_ai_group`、`previous_user_chunk`、`current_phase_number`、`phases`、`ai_group_phase_map`、`compaction_token_deltas`、`current_phase_first_ai_group_id`、`current_phase_last_ai_group_id`、`current_phase_compact_group_id`、`last_ai_group_before_compact`。与 TS 一一对应。
- [x] 8.3 遍历 chunks：
  - `Chunk::User`：记 `previous_user_chunk`，跳过；
  - `Chunk::Compact`：backfill 上一个 AI group 的 `accumulated_injections`；finalize 当前 phase；重置；`current_phase_number += 1`；**不重置** `last_ai_group_before_compact`；
  - `Chunk::AI`：调 `compute_context_stats`；若是 phase 第一个 AI group 且 `last_ai_group_before_compact` 不为 `None`，计算 `CompactionTokenDelta`；统计写 `stats_map`；更新 phase 边界；更新状态。
- [x] 8.4 循环结束后 backfill 最后一个 AI group 的 `accumulated_injections` + finalize 最后一个 phase。
- [x] 8.5 返回 `SessionContextResult { stats_map, phase_info }`。
- [x] 8.6 辅助函数 `get_last_assistant_total_tokens(ai_chunk) -> Option<u32>` / `get_first_assistant_total_tokens(ai_chunk) -> Option<u32>`：按 TS 同名函数实现，sum(`input_tokens + output_tokens + cache_read + cache_creation`)。
- [x] 8.7 clippy 通过。

## 9. 集成测试

- [x] 9.1 新建 `crates/cdt-analyze/tests/context_tracking.rs`。
- [x] 9.2 空 chunk 切片 → 空 result（覆盖 ADDED Requirement 的 empty scenario）。
- [x] 9.3 单 AI group + 1 tool output + 1 user message → stats 含 2 injection、total > 0，且 `tokens_by_category.tool_output + tokens_by_category.user_messages == total`。
- [x] 9.4 2 AI group + 中间 1 个 compact + 外部注入 usage（AI_1 last = 1000，AI_2 first = 600）→ `phase_info.phases.len() == 2`；`compaction_token_deltas` 含 1 个 `{ pre: 1000, post: 600, delta: -400 }`（覆盖 MODIFIED Requirement 的新 scenario）。
- [x] 9.5 `[AI_1, compact]` 末尾 compact → `compaction_token_deltas` 为空；`phase_info.phases.len() == 1`（finalize 旧 phase 的分支）。
- [x] 9.6 同一 mentioned-file path 出现在两个 AI group → 只产出 1 个 `MentionedFile` injection（覆盖去重）。
- [x] 9.7 missing token data：在 params 里故意不塞某个 CLAUDE.md 文件的 token info → 返回的 injection `estimated_tokens == 0`，无 panic、无 Err。
- [x] 9.8 JSON shape 断言：对任一 `ContextStats` 实例 `serde_json::to_value`，断言顶层字段名是 `tokensByCategory` / `totalEstimatedTokens` / `newCounts`。

## 10. spec fidelity & followups 联动

- [x] 10.1 手工（或跑 `spec-fidelity-reviewer`）审计 `openspec/specs/context-tracking/spec.md` 的每条 Requirement × Scenario 在 Rust 测试里都有对应 case，包括本 port 新增的 ADDED + MODIFIED scenario。
- [x] 10.2 更新 `openspec/followups.md` 的 `## context-tracking` 段落：第 3 条 coverage-gap（`computeContextStats / processSessionContextWithPhases 无单元测试`）标记为 ✅，指向 `crates/cdt-analyze/src/context/` 与 `tests/context_tracking.rs`。
- [x] 10.3 更新根 `CLAUDE.md`：
  - Capability → crate map 里 `context-tracking` 改为 `done ✓`。
  - "Remaining port order" 去掉 context-tracking 条目，编号顺移。
  - "Known TS impl-bugs" 段补一条 ✓ 条目（coverage-gap 性质）。

## 11. CI 与合规

- [x] 11.1 `cargo fmt --all`。
- [x] 11.2 `cargo clippy --workspace --all-targets -- -D warnings`。
- [x] 11.3 `cargo test --workspace`。
- [x] 11.4 `openspec validate port-context-tracking --strict`，准备 `/opsx:apply`。
