## Why

上次 port 在 `AIChunk` 里留下了 `tool_executions` / `subagents` 两个占位字段。本次把 **tool-execution-linking** capability 落地，把 `tool_use` 与 `tool_result` 真正配对、生成带错误态的 `ToolExecution` 记录，并实现 TS 版 `SubagentResolver` 的三阶段 Task→subagent 回退匹配。同时一次性修掉 `followups.md` 里两条"必须修正"的 impl-bug：重复 `tool_use_id` 必须告警；Task 过滤逻辑必须真正实现。

## What Changes

- 在 `cdt-core` 新增真实的 `ToolExecution`、`Process`、`SubagentCandidate` 类型；**BREAKING**：删除 `ToolExecutionPlaceholder` 与 `SubagentPlaceholder`，`AIChunk.tool_executions` / `AIChunk.subagents` 的元素类型换成 `ToolExecution` / `Process`。
- 在 `cdt-analyze::tool_linking` 新增纯同步 API：
  - `pair_tool_executions(messages: &[ParsedMessage]) -> ToolLinkingResult`：按 `tool_use_id` 配对，收集 orphans，重复 id 调用 `tracing::warn!` 后仅保留第一个 result（修 impl-bug）。
  - `resolve_subagents(task_calls: &[ToolCall], candidates: &[SubagentCandidate]) -> Vec<ResolvedTask>`：三阶段回退（result-based → description-based → positional）。本 port 的调用方需要外部供给 candidates——candidate 装载留给 `port-project-discovery` 之后的 port 完成。
  - `filter_resolved_tasks(executions: &mut Vec<ToolExecution>, resolutions: &[ResolvedTask])`：按 spec 移除已 resolve 的 Task tool execution。
- `cdt-analyze::chunk::build_chunks` 内部调用 `pair_tool_executions`，把返回的 `ToolExecution` 列表按原始 `tool_use` 所属的 `AIChunk` 分发，填满此前的占位 `Vec`。`AIChunk.subagents` 继续保持 `Vec::new()`，由 `port-team-coordination-metadata` 在端到端链路里填充。
- `ChunkMetrics.tool_count` 维持现状（仍统计全部 `tool_use`，含未过滤的 Task），直到 `port-team-coordination-metadata` 把 candidate 装载与 Task filter 接到 `build_chunks` 默认路径上；本 port 明确把这个过渡窗口的 owner 改成 port 10。
- 更新 `cdt-analyze/tests/chunks.rs` 的 3 份 insta snapshot 以反映 `tool_executions` 从空列表变成真实记录。
- **MODIFIED** tool-execution-linking spec：
  - Req 1（Pair）显式收录 duplicate id → warn + 取首的行为。
  - Req 2（ToolExecution 记录）固定字段形态：`tool_name` / `input` / `output` / `is_error` / `start_ts` / `end_ts` / `orphan` 标记。
  - Req 3（三阶段 resolver）改写成"纯函数，输入为预装载的 `SubagentCandidate` 列表"，澄清候选装载不属本 capability。
  - Req 4 / Req 5 保持原样（属于 team-coordination-metadata，后续 port 再改）。
- **MODIFIED** chunk-building spec：
  - 移除上次 port 留下的"过渡性"说明，把 `tool_count` 与 Task filter 的完成窗口从"下次 port-tool-execution-linking 后"改为"`port-team-coordination-metadata` 把端到端链路接通之后"。
  - "Link tool uses to tool results" 的两个子 scenario 从"verified under tool-execution-linking capability"升级为当前 capability 内即可验证（因为 `AIChunk.tool_executions` 真实填充后可直接断言）。

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `tool-execution-linking`：实现 Req 1/2/3，其中 Req 3 明确改为纯函数签名；Req 4/5 不动。
- `chunk-building`：收紧 `tool_executions` 槽位从占位变成"已由 tool-execution-linking 实际填充"，更新 `tool_count` / Task filter 的过渡说明，snapshot 跟随更新。

## Impact

- 代码：
  - `crates/cdt-core/src/`：新增 `tool_execution.rs`、`process.rs`（或合并到 `chunk.rs` 同目录），lib.rs 新增 re-export；`chunk.rs` 中 `AIChunk` 字段类型迁移。
  - `crates/cdt-analyze/src/tool_linking/`：新增 `mod.rs`、`pair.rs`、`resolver.rs`、`filter.rs`。
  - `crates/cdt-analyze/src/chunk/builder.rs`：调用 `pair_tool_executions`，按 `AIChunk` 切片分发。
  - `crates/cdt-analyze/tests/`：新增 `tool_linking.rs` 集成测试与 mock fixture（模拟已装载的 subagent candidate）；`chunks.rs` snapshot 更新。
- Spec：
  - `openspec/specs/tool-execution-linking/spec.md` 由 delta 更新。
  - `openspec/specs/chunk-building/spec.md` 由 delta 更新。
- 风险：
  - `ToolExecutionPlaceholder` → `ToolExecution` 是 breaking 类型替换，任何 `cdt-core` 外部消费都会 red——当前除 `cdt-analyze` 外没有消费方，风险可控。
  - 三阶段 resolver 的 positional fallback 若实现不稳，可能在 Task 数量与 candidate 数量相等但语义错位时错配；mitigation：严格按 TS `SubagentResolver.ts:207-309` 的优先级与等式判定，用对比测试锁定 3 条 scenario。
  - `AIChunk.tool_executions` 的填充需要保持"tool_use 出现在哪个 AIChunk 里"的顺序；mitigation：pair 在 `build_chunks` 内按 chunk 切片分 batch，避免事后定位。
