## 1. 核心类型定义（cdt-core）

- [x] 1.1 新增 `crates/cdt-core/src/chunk.rs`，定义 `Chunk` 枚举与 `UserChunk`、`AIChunk`、`SystemChunk`、`CompactChunk`、`AssistantResponse` 结构体，全部派生 `Debug/Clone/PartialEq/Serialize`
- [x] 1.2 在 `chunk.rs` 定义 `ChunkMetrics { input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens, tool_count, cost_usd: Option<f64> }`
- [x] 1.3 在 `chunk.rs` 定义 `SemanticStep` 枚举的四个变体（Thinking / Text / ToolExecution / SubagentSpawn），使用 `String` + `DateTime<Utc>` 占位字段
- [x] 1.4 为 `AIChunk` 加上 `tool_executions: Vec<ToolExecutionPlaceholder>`、`subagents: Vec<SubagentPlaceholder>` 两个留白字段（类型可为空结构体 newtype），并附 `/// TODO(port-tool-execution-linking)` 文档注释指向下一个 change
- [x] 1.5 在 `crates/cdt-core/src/lib.rs` re-export 上述新类型，保持 `#![forbid(unsafe_code)]` 与模块公共 API 约定
- [x] 1.6 为每个结构体写最小 `serde` roundtrip 单元测试（构造一个实例 → to_json → from_json → assert_eq），放在 `chunk.rs` 文件底部 `#[cfg(test)] mod tests`

## 2. cdt-analyze 依赖与脚手架

- [x] 2.1 在 `crates/cdt-analyze/Cargo.toml` 添加 `cdt-core = { workspace = true }`、`chrono = { workspace = true }`、`serde = { workspace = true }`、`tracing = { workspace = true }`，确认不引入 `tokio`
- [x] 2.2 新增 `crates/cdt-analyze/src/chunk/mod.rs`，声明子模块 `builder`、`metrics`、`semantic`
- [x] 2.3 在 `crates/cdt-analyze/src/lib.rs` 暴露 `pub mod chunk;` 并在顶层 re-export `build_chunks`
- [x] 2.4 确认 `cdt-analyze` 根模块有 `//!` 链接到 `openspec/specs/chunk-building/spec.md`

## 3. Chunk 构造器实现

- [x] 3.1 `chunk/builder.rs` 实现 `pub fn build_chunks(messages: &[ParsedMessage]) -> Vec<Chunk>`，纯同步
- [x] 3.2 实现过滤 pass：`filter(|m| !m.is_sidechain && !m.category.is_hard_noise())`
- [x] 3.3 实现主状态机：assistant buffer、真实用户/system/compact 边界判断、末尾 flush
- [x] 3.4 实现 `<local-command-stdout>...</local-command-stdout>` 包裹的用户消息 → `SystemChunk` 识别（注意仅匹配首尾精确包裹，非空内容）
- [x] 3.5 实现 `is_compact_summary == true` → `CompactChunk { summary_text, timestamp }`，并在产出前 flush assistant buffer
- [x] 3.6 把 tool_result-only 用户消息并入上一条 `AIChunk.responses` 的最后一项（若存在），否则降级为普通 `UserChunk`

## 4. 指标与语义步骤

- [x] 4.1 `chunk/metrics.rs` 实现 `aggregate_metrics(responses: &[AssistantResponse]) -> ChunkMetrics`，累加 token、统计 `tool_calls.len()` 之和
- [x] 4.2 `aggregate_metrics` 对 `UserChunk`、`SystemChunk`、`CompactChunk` 返回全零 `ChunkMetrics`，`cost_usd = None`
- [x] 4.3 `chunk/semantic.rs` 实现 `extract_semantic_steps(responses: &[AssistantResponse]) -> Vec<SemanticStep>`：按 block 顺序发射 Thinking/Text/ToolExecution，本 port 不产出 SubagentSpawn
- [x] 4.4 `builder.rs` 在构造 `AIChunk` 时调用 `aggregate_metrics` 与 `extract_semantic_steps`

## 5. 单元测试（scenario → test）

- [x] 5.1 `user_question_then_ai_response_emits_two_chunks`：真实用户 + assistant → `[UserChunk, AIChunk]`
- [x] 5.2 `multiple_assistant_turns_coalesce_into_one_ai_chunk`：三条连续 assistant → 1 个 AIChunk 含 3 response
- [x] 5.3 `assistant_buffer_flushes_before_new_user`：assistant×2 + user → `[AIChunk(2), UserChunk]`
- [x] 5.4 `local_command_stdout_becomes_system_chunk`：`<local-command-stdout>ls output</local-command-stdout>` → `SystemChunk`，且打断 assistant buffer
- [x] 5.5 `sidechain_messages_are_dropped`：混入 `is_sidechain=true` assistant → 不出现在任何 chunk 中
- [x] 5.6 `hard_noise_messages_are_dropped`：合成 assistant placeholder / empty command output → 不出现
- [x] 5.7 `ai_chunk_metrics_sum_tool_calls`：AIChunk 含 3 个 tool_use block → `metrics.tool_count == 3`
- [x] 5.8 `user_chunk_metrics_all_zero_and_duration_none`
- [x] 5.9 `compact_summary_emits_compact_chunk_and_flushes_buffer`：assistant×2 + compact → `[AIChunk, CompactChunk]`
- [x] 5.10 `semantic_steps_follow_block_order`：thinking → text → tool_use → 三步按序
- [x] 5.11 `subagent_spawn_variant_not_emitted_yet`：不产出 `SemanticStep::SubagentSpawn`
- [x] 5.12 `tool_execution_list_is_empty_placeholder`：AIChunk 有 tool_use block 时 `tool_executions` 字段仍为空 Vec
- [x] 5.13 所有测试放在 `crates/cdt-analyze/src/chunk/builder.rs` 的 `#[cfg(test)] mod tests`（跨多文件的集成测试走第 6 节）

## 6. 集成快照测试

- [x] 6.1 在工作区 `Cargo.toml` `[workspace.dependencies]` 加 `insta = "1"`，`cdt-analyze` 的 `[dev-dependencies]` 引用
- [x] 6.2 新建 `crates/cdt-analyze/tests/fixtures/`，放 3 份最小 JSONL fixture：`simple.jsonl`（user+ai）、`multi_ai.jsonl`（多轮 ai coalescing）、`with_compact.jsonl`（带一次 compact summary）
- [x] 6.3 新建 `crates/cdt-analyze/tests/chunks.rs`，用 `cdt_parse::parse_file`（或已有同步解析入口）把 fixture 解析成 `Vec<ParsedMessage>`，传入 `build_chunks`，用 `insta::assert_debug_snapshot!` 锁定结果
- [x] 6.4 运行 `cargo insta accept -p cdt-analyze` 生成首版 snapshot 并提交 `.snap` 文件

## 7. 校验

- [x] 7.1 `cargo fmt --all`
- [x] 7.2 `cargo clippy -p cdt-core -p cdt-analyze --all-targets -- -D warnings`
- [x] 7.3 `cargo test -p cdt-core -p cdt-analyze`
- [x] 7.4 `cargo build --workspace` 确认整个 workspace 仍然编译
- [x] 7.5 `openspec validate port-chunk-building --strict` 通过
- [x] 7.6 在 tasks.md 末尾记录本次 port 刻意留白的三处（tool 链接、Task 过滤、subagent 归集）以供 `port-tool-execution-linking` 开始时回读

## 8. 下次 port 需要同步更新的位点

> 以下是本次 port 刻意留白的位点，供后续 port 开始时回读；不算作本 change 的任务。

- [ ] 8.1 `port-tool-execution-linking` 须更新 `crates/cdt-analyze/tests/chunks.rs` 的 snapshot（Task 过滤会改变 `metrics.tool_count` 与 `tool_executions` 列表）
- [ ] 8.2 `port-tool-execution-linking` 须把 `ChunkMetrics::tool_count` 的统计语义从"所有 tool_use"改为"过滤 Task 后的 tool_use"，并在 spec delta 中移除本 change 在 "Compute per-chunk metrics" 里的过渡性说明
- [ ] 8.3 `port-team-coordination-metadata` 须填充 `AIChunk.subagents` 并在 `SemanticStep` 中开始产出 `SubagentSpawn`
