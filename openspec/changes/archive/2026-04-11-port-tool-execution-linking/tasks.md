## 1. 核心类型（cdt-core）

- [x] 1.1 新增 `crates/cdt-core/src/tool_execution.rs`：定义 `ToolExecution`、`ToolOutput`（`Text` / `Structured` / `Missing`）两种类型，派生 `Debug/Clone/PartialEq/Serialize/Deserialize`
- [x] 1.2 新增 `crates/cdt-core/src/process.rs`：定义 `Process`、`TeamMeta`、`SubagentCandidate` 三种类型，派生同上；`Process.team` 与 `TeamMeta` 对应字段先标注 `/// TODO(port-team-coordination-metadata)`
- [x] 1.3 **BREAKING**：在 `crates/cdt-core/src/chunk.rs` 里把 `AIChunk.tool_executions` 类型改为 `Vec<ToolExecution>`、`AIChunk.subagents` 改为 `Vec<Process>`；删除 `ToolExecutionPlaceholder` 与 `SubagentPlaceholder`
- [x] 1.4 在 `crates/cdt-core/src/lib.rs` 与 `prelude` 中 re-export 新类型，删除占位符的 re-export
- [x] 1.5 为每个新类型补最小 `serde` roundtrip 单元测试（构造实例 → to_json → from_json → assert_eq）

## 2. tool_linking 模块脚手架（cdt-analyze）

- [x] 2.1 新增 `crates/cdt-analyze/src/tool_linking/mod.rs`，声明子模块 `pair`、`resolver`、`filter`；在 `lib.rs` 暴露 `pub mod tool_linking;` 与顶层 re-export
- [x] 2.2 定义 `ToolLinkingResult { executions: Vec<ToolExecution>, duplicates_dropped: usize }`、`ResolvedTask { task_use_id, resolution }`、`Resolution` 枚举，放在 `tool_linking/mod.rs`
- [x] 2.3 移除 `cdt-analyze/src/lib.rs` 中 `pub mod tool_linking { ... }` 的空占位模块块

## 3. pair 实现

- [x] 3.1 `tool_linking/pair.rs` 实现 `pub fn pair_tool_executions(messages: &[ParsedMessage]) -> ToolLinkingResult`，纯同步、O(n)
- [x] 3.2 第一遍扫描：遍历 assistant 消息的 `tool_calls`，在 `HashMap<String, PendingToolUse>` 里登记 `tool_use_id → (source_assistant_uuid, tool_name, input, start_ts)`
- [x] 3.3 第二遍扫描：遍历 user 消息的 `tool_results`，按 id 取出 pending；命中即产 `ToolExecution { output, end_ts, is_error }` 并从 map 删除
- [x] 3.4 重复 id 处理：`tracing::warn!(tool_use_id = %id, "duplicate tool_result; keeping first")`，`duplicates_dropped += 1`
- [x] 3.5 扫描结束后把 map 里剩余项作为 orphan（`output = Missing`、`end_ts = None`）追加到 `executions`
- [x] 3.6 将 `tool_result` content 按 serde_json 值判定：`Value::String(_)` → `Text`，其他 → `Structured`

## 4. resolver 实现

- [x] 4.1 `tool_linking/resolver.rs` 定义 `pub fn resolve_subagents(task_calls: &[ToolCall], executions: &[ToolExecution], candidates: &[SubagentCandidate]) -> Vec<ResolvedTask>`
- [x] 4.2 Phase 1（result-based）：对每个 Task，查其 `ToolExecution.output`；若为 `Structured(Value)` 且 `Value` 含 `teammate_spawned` 或顶层 `session_id`，从 `candidates` 里按 id 查 Process 返回
- [x] 4.3 Phase 2（description-based）：`description_match(task_description, candidate.description_hint)` 用归一化后的前缀包含判定；时间窗 `TIME_WINDOW_SECS = 60`
- [x] 4.4 Phase 2 的唯一性判定：若某 Task 只匹配到 1 个未分配 candidate → link；否则放入"歧义集合"进入 phase 3
- [x] 4.5 Phase 3（positional）：仅当"未分配 Task 数 == 未分配 candidate 数"时按 spawn order 一一配对
- [x] 4.6 剩余未分配 Task → `Resolution::Orphan`
- [x] 4.7 `Process` 从 `SubagentCandidate` 构造：`session_id`、`spawn_ts`、`metrics` 复用 candidate，`end_ts = None`、`team = None`、`root_task_description = Some(task.description)`

## 5. filter 实现

- [x] 5.1 `tool_linking/filter.rs` 实现 `pub fn filter_resolved_tasks(executions: &mut Vec<ToolExecution>, resolutions: &[ResolvedTask])`
- [x] 5.2 按 `resolutions` 构造 `HashSet<&str>`（`Resolution != Orphan` 的 `task_use_id`）；`executions.retain(|e| !resolved.contains(e.tool_use_id.as_str()))`

## 6. build_chunks 接入

- [x] 6.1 修改 `crates/cdt-analyze/src/chunk/builder.rs`：在函数开头调用 `pair_tool_executions(messages)`，拿到 `ToolLinkingResult`
- [x] 6.2 把 `executions` 按 `source_assistant_uuid` 分组到 `HashMap<String, Vec<ToolExecution>>`
- [x] 6.3 `flush_buffer` 在构造 `AIChunk` 时，按 `responses[i].uuid` 从该 map 取出对应 `ToolExecution`，合并排序后填入 `tool_executions`
- [x] 6.4 orphan（`source_assistant_uuid` 不属于任何已 flush 的 AIChunk）用 `tracing::warn!` 记一次，不丢数据——通常不应发生
- [x] 6.5 `AIChunk.subagents` 继续保持 `Vec::new()`（下次 port 再填）
- [x] 6.6 更新 `cdt-analyze::lib.rs` 的 `//!` 注释，删除"stub"字样，列出 chunk-building 与 tool-execution-linking 两项 port 状态

## 7. 单元测试（tool_linking）

- [x] 7.1 `pair.rs` 测试：immediate result → 配对成功、`end_ts` 正确
- [x] 7.2 `pair.rs` 测试：delayed result（中间插 2 条其他消息）→ 仍然配对
- [x] 7.3 `pair.rs` 测试：duplicate tool_use_id → `duplicates_dropped == 1`，首个 result 获胜
- [x] 7.4 `pair.rs` 测试：orphan → `output == Missing`、`end_ts == None`
- [x] 7.5 `pair.rs` 测试：error result（`is_error = true`）→ `ToolExecution.is_error == true`、原文保留
- [x] 7.6 `pair.rs` 测试：Bash 结构化 result（`{stdout, stderr}`）→ `ToolOutput::Structured(...)`
- [x] 7.7 `pair.rs` 测试：legacy 字符串 result → `ToolOutput::Text`
- [x] 7.8 `resolver.rs` 测试：phase 1 result-based → `teammate_spawned` JSON 命中
- [x] 7.9 `resolver.rs` 测试：phase 2 description-based → 唯一匹配
- [x] 7.10 `resolver.rs` 测试：phase 3 positional → 数量相等时按顺序配对
- [x] 7.11 `resolver.rs` 测试：unrelated candidate 使数量不等 → 保持 orphan，不误配
- [x] 7.12 `resolver.rs` 测试：所有阶段都失败 → `Resolution::Orphan`
- [x] 7.13 `filter.rs` 测试：两条 Task resolved + 一条 orphan Task + 一条 Bash → 过滤后只剩后两条
- [x] 7.14 `filter.rs` 测试：空 resolutions → executions 不变

## 8. chunk-building 回归

- [x] 8.1 `chunk/builder.rs` 增补测试：Bash tool_use + user tool_result → `AIChunk.tool_executions` 长度 1、`end_ts` 非 None
- [x] 8.2 `chunk/builder.rs` 增补测试：orphan tool_use（没有后续 result）→ `tool_executions` 仍产出 1 条 orphan 记录
- [x] 8.3 `chunk/builder.rs` 增补测试：多个 AIChunk 的 tool_use 分别落到各自 chunk（验证 `source_assistant_uuid` 分发）
- [x] 8.4 更新既有测试 `tool_execution_list_is_empty_placeholder`：rename 为 `tool_executions_populated_for_tool_use`，断言有内容而非空

## 9. 集成快照更新

- [x] 9.1 扩充 `crates/cdt-analyze/tests/fixtures/multi_ai.jsonl`：补一条用户 tool_result 消息，与 `Bash` tool_use 配对（避免单一 orphan 案例）；或新增 `with_tool_result.jsonl`
- [x] 9.2 `tests/chunks.rs` 的 `summarize` 函数增加 tool execution 概要（例如 `tool_exec=N, orphans=M`）
- [x] 9.3 `INSTA_UPDATE=always cargo test -p cdt-analyze --test chunks` 重新接受 3 份 snapshot，人工 review diff 后提交

## 10. 校验

- [x] 10.1 `cargo fmt --all`
- [x] 10.2 `cargo clippy -p cdt-core -p cdt-analyze --all-targets -- -D warnings`
- [x] 10.3 `cargo test -p cdt-core -p cdt-analyze`
- [x] 10.4 `cargo build --workspace`
- [x] 10.5 `openspec validate port-tool-execution-linking --strict`

## 11. 下次 port 同步位点

> 以下条目不是本 change 的任务，保留给 `port-team-coordination-metadata` 与 `port-project-discovery` 回读。

- [ ] 11.1 `port-team-coordination-metadata` 须把 candidate 装载、`resolve_subagents` 调用、`filter_resolved_tasks` 接入 `build_chunks` 默认路径，并同步修正 `ChunkMetrics::tool_count` 的统计语义
- [ ] 11.2 `port-team-coordination-metadata` 须填充 `Process.team` 与 `TeamMeta`，并为 `Resolution::ResultBased/DescriptionBased/Positional` 提供 team-aware 变体
- [ ] 11.3 `port-project-discovery` 须为 `SubagentCandidate` 提供真实装载路径（从 session 文件抽取 `spawn_ts` / `description_hint` / `metrics`）
