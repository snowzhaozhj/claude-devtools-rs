## Why

`cdt-parse` 现在能把 JSONL 会话转换成 `ParsedMessage` 流，但下游展示/统计依赖按语义分组的 Chunk。作为依赖链的第二步，本次把 chunk-building 能力从 TS 移植到 Rust，为 tool-execution-linking、context-tracking 等后续 port 提供基础数据结构。

## What Changes

- 在 `cdt-core` 新增 Chunk 相关类型：`Chunk`（枚举 User/AI/System/Compact）、`UserChunk`、`AIChunk`、`SystemChunk`、`CompactChunk`、`AssistantResponse`、`ChunkMetrics`、`SemanticStep`。
- 在 `cdt-analyze` 新增 `chunk` 模块，暴露 `build_chunks(messages: &[ParsedMessage]) -> Vec<Chunk>`，实现：
  - 过滤 `is_sidechain == true` 与 `MessageCategory::HardNoise(_)`；
  - 将连续 assistant 消息合并到同一个 `AIChunk.responses`；
  - 为 `<local-command-stdout>` 内联消息产出独立 `SystemChunk`；
  - 为每个 `is_compact_summary` 消息产出 `CompactChunk`；
  - 按 `TokenUsage` 累计 `ChunkMetrics`（input/output/cache tokens、成本估算、tool 调用数）；
  - 按时间顺序提取 `AIChunk` 的 `SemanticStep` 列表（thinking / text / tool_use 占位 / subagent 占位）。
- `cdt-analyze` 当前留出 `tool_executions: Vec<ToolExecution>` / `subagents: Vec<Process>` 字段为空 `Vec`，由后续 `port-tool-execution-linking` 与 `port-team-coordination-metadata` 填充；Task 过滤与三阶段匹配不在本次范围。
- 引入工作区依赖 `insta`（dev-only）用于 chunk 快照测试。
- **MODIFIED** `chunk-building` spec：
  - 把"Link tool uses to tool results"、"Filter Task tool uses when subagent data is available"、"Attach subagents to AIChunks" 三个 requirement 标注为由 `tool-execution-linking` / `team-coordination-metadata` 履行，chunk-building 只负责预留结构位；并澄清 `SemanticStep::ToolExecution` / `::SubagentSpawn` 在本 capability 下仅是占位符。
  - 补充 scenario：coalescing 多个连续 assistant 消息、`is_compact_summary == true` 消息触发 `CompactChunk`、`HardNoise` 消息在进入 chunk 构造前被丢弃。

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `chunk-building`:
  - 明确 chunk-building 只负责结构、过滤、指标、语义步骤；tool 链接与 subagent 归集的 requirement 迁出到对应下游 capability。
  - 新增 hard-noise 过滤、assistant coalescing、compact summary → CompactChunk 的 scenario。

## Impact

- 代码：
  - `crates/cdt-core/src/lib.rs`、新增 `crates/cdt-core/src/chunk.rs`；
  - `crates/cdt-analyze/src/lib.rs`、新增 `crates/cdt-analyze/src/chunk/{mod.rs,builder.rs,metrics.rs,semantic.rs}`；
  - `crates/cdt-analyze/Cargo.toml` 增加 `cdt-core`、`serde`、`chrono` 依赖（保持 sync，不引入 tokio）；
  - 工作区 `Cargo.toml` 的 `[workspace.dependencies]` 增加 `insta`。
- Spec：`openspec/specs/chunk-building/spec.md` 将由本 change 的 delta 更新（spec-delta）。
- 风险：
  - `SemanticStep` 占位设计若与 tool-execution-linking 的最终方案不兼容，下一次 port 需要做一次类型微调；通过 `ToolExecutionRef`/`SubagentRef` newtype 包一层以降低扩散。
  - 成本估算函数若引入模型价格表会膨胀本次范围 → 本次只提供 `ChunkMetrics::cost_usd: Option<f64>`，默认 `None`，价格表延后。
