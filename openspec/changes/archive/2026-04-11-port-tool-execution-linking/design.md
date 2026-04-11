## Context

chunk-building port 把 `tool_executions: Vec<ToolExecutionPlaceholder>` 和 `subagents: Vec<SubagentPlaceholder>` 留空。TS 原版把这两件事分散在 `ToolExecutionBuilder.ts`（pair 与 record）+ `SubagentResolver.ts`（三阶段匹配）+ `ChunkFactory.ts` 的后处理里；还存在两条必须修的 impl-bug（followups.md）：
1. 重复 `tool_use_id` 被静默合并，应 warn。
2. Task 过滤从未生效。

本次 port 把 pair / record / Task 过滤 / 三阶段 resolver 全部落到 Rust 的 **纯函数** 层，端到端 candidate 装载与 Process→AIChunk 归集留给后续 port。

## Goals / Non-Goals

**Goals:**
- 在 `cdt-core` 定义真正的 `ToolExecution` / `Process` / `SubagentCandidate` 类型，替换占位符。
- 在 `cdt-analyze::tool_linking` 提供纯同步 API：pair、resolver、filter。
- `build_chunks` 默认路径自动 pair，`AIChunk.tool_executions` 不再为空。
- 三阶段 resolver 与 Task filter 在纯函数层有完整测试覆盖。
- 2 条 TS impl-bug 正式修掉。
- chunk-building 的 insta snapshot 和 spec delta 同步更新。

**Non-Goals:**
- 不从磁盘加载 subagent candidate（那是 `port-project-discovery` → 后续 port 的任务）。
- 不把 `resolve_subagents` 或 `filter_resolved_tasks` 串到 `build_chunks` 的默认路径上——那是 `port-team-coordination-metadata` 的收尾。
- 不实现 team 元数据富化（Req 4）或 team 工具摘要（Req 5），它们属 team-coordination-metadata。
- 不改 `ChunkMetrics::tool_count` 的统计语义。

## Decisions

### D1：类型放 `cdt-core`，逻辑放 `cdt-analyze`
- 新增 `crates/cdt-core/src/tool_execution.rs` 与 `crates/cdt-core/src/process.rs`（或并入 `chunk.rs`——选拆分更清晰）。
- `AIChunk.tool_executions: Vec<ToolExecution>`、`AIChunk.subagents: Vec<Process>`——占位类型删除。
- 逻辑放 `crates/cdt-analyze/src/tool_linking/{mod.rs, pair.rs, resolver.rs, filter.rs}`。

### D2：`ToolExecution` 字段
```rust
pub struct ToolExecution {
    pub tool_use_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: ToolOutput,      // Text(String) | Structured(Value) | Missing
    pub is_error: bool,
    pub start_ts: DateTime<Utc>, // assistant message timestamp
    pub end_ts: Option<DateTime<Utc>>, // None ⇒ orphan
    pub source_assistant_uuid: String, // 用于回填 chunk
}

pub enum ToolOutput {
    Text(String),
    Structured(serde_json::Value),
    Missing,
}
```
`source_assistant_uuid` 是为了把执行记录分发回正确的 `AIChunk`——见 D5。

### D3：`Process` 与 `SubagentCandidate`
```rust
pub struct Process {
    pub session_id: String,
    pub root_task_description: Option<String>,
    pub spawn_ts: DateTime<Utc>,
    pub end_ts: Option<DateTime<Utc>>,
    pub metrics: ChunkMetrics,
    pub team: Option<TeamMeta>, // 占位：port-team-coordination-metadata 填
}

pub struct SubagentCandidate {
    pub session_id: String,
    pub description_hint: Option<String>, // 从 root 消息抽取的 prompt / 描述
    pub spawn_ts: DateTime<Utc>,
    pub parent_session_id: Option<String>,
    pub metrics: ChunkMetrics,
}

pub struct TeamMeta { pub team_name: String, pub member_name: String, pub member_color: Option<String> }
```
`Process` 的 `team` 字段与 `TeamMeta` 先留空/`None`，下次 port 填。

### D4：pair 算法
`pair_tool_executions(messages) -> ToolLinkingResult`：
- 第一遍按顺序扫描，建立 `tool_use_id → (assistant_uuid, tool_call, assistant_ts)` map。
- 第二遍扫描 user 消息里的 `tool_results`，按 id 查上面的 map：找到即产 `ToolExecution { output, end_ts = user_ts, is_error }`，并从 map 里 remove。
- 重复 id：`tracing::warn!(tool_use_id = %id, "duplicate tool_result; keeping first")`，后续同 id 的 result 被丢弃。
- 扫描结束后 map 里剩下的是 orphans：产出 `ToolExecution { output: Missing, end_ts: None, is_error: false }`。
- 返回结构：
  ```rust
  pub struct ToolLinkingResult {
      pub executions: Vec<ToolExecution>,
      pub duplicates_dropped: usize, // 仅统计，方便测试
  }
  ```
- 时间复杂度 O(n + m)，一个 hashmap 足够。

### D5：`build_chunks` 如何分发 `ToolExecution` 到 chunk
方案 A：在 `build_chunks` 内第一步就跑 `pair_tool_executions(messages)`，生成总列表；flush AIChunk buffer 时按 `source_assistant_uuid ∈ {responses[i].uuid}` 过滤，分摊到对应 chunk。
方案 B：按 chunk 切片分别 pair——更少跨 chunk 边界语义，但 tool_result 经常出现在后续 chunk 里（user 消息通常在 AIChunk 之后），切片会断开配对。
**选 A**。

缺点：一次 O(n) 的后过滤；对几千条消息的会话来说可忽略。

### D6：三阶段 resolver 纯函数
```rust
pub fn resolve_subagents(
    task_calls: &[ToolCall], // 仅 is_task == true
    candidates: &[SubagentCandidate],
    executions: &[ToolExecution], // 用来读 teammate_spawned 结果
) -> Vec<ResolvedTask>;

pub struct ResolvedTask {
    pub task_use_id: String,
    pub resolution: Resolution,
}

pub enum Resolution {
    ResultBased(Process),
    DescriptionBased(Process),
    Positional(Process),
    Orphan,
}
```
阶段：
1. **Result-based**：遍历 `executions`，若对应 Task 的 `output` 是结构化 JSON 且包含 `teammate_spawned` 或 `session_id` 字段，立即从 candidates 里按 `session_id` 查 Process 返回。
2. **Description-based**：剩余 Task 与未分配 candidates 做笛卡尔积：`description ≈ candidate.description_hint` 且 `|task_ts - candidate.spawn_ts| < TIME_WINDOW`（取 TS 版 `SubagentResolver.ts:207-309` 的窗口默认 60s）。若某 task 只匹配到 1 个 candidate，即 link。
3. **Positional**：若 phase 2 后仍有未分配，且"未分配 task 数 == 未分配 candidate 数"，按 task spawn order ↔ candidate spawn order 一一配对。
4. 剩余 → `Resolution::Orphan`。
- 不强求 candidate 每个都被用到：multi-parent 场景下 candidate 可能属于别的 parent。
- 测试覆盖 4 条 scenario。

### D7：Task filter 为单独函数，不接入默认 build_chunks
```rust
pub fn filter_resolved_tasks(executions: &mut Vec<ToolExecution>, resolutions: &[ResolvedTask]);
```
- 删除 `executions` 里所有 `tool_use_id` 出现在 `ResolvedTask(Resolution != Orphan)` 里的条目。
- 本 port 的 `build_chunks` 不调用它——保持默认行为（与上次 port 的 spec 说法一致）。
- 在 `tool_linking` 的单元测试里直接验证 filter 函数行为。

### D8：错误处理
- `tool_linking` 的函数都是纯函数，不返回 `Result`。
- 警告走 `tracing::warn!`。
- 测试用 `tracing-test` 子 crate 捕获日志——或者更轻的方式：`pair_tool_executions` 返回 `duplicates_dropped: usize`，测试断言 ≥ 1 即可，避免引入新依赖。**选后者**。

### D9：snapshot 更新策略
- 3 份 fixture 的 snapshot 都会变：`multi_ai.jsonl` 里有 1 个 `Bash` tool_use，现在会产出 1 个 orphan `ToolExecution`（因为 fixture 里没有 tool_result）。
- 测试通过 `INSTA_UPDATE=always cargo test -p cdt-analyze --test chunks` 重新接受。

### D10：spec delta 的切分
- tool-execution-linking 的 delta 只 MODIFY Req 1/2/3；Req 4/5 不动。
- chunk-building 的 delta 再次 MODIFY 同样三个 requirement，把"过渡期 owner"从 `port-tool-execution-linking` 改为 `port-team-coordination-metadata`，并把"Structure slot exists even with no linking implemented" 这一 scenario 升级为"Tool executions populated by build_chunks"。

## Risks / Trade-offs

- **[Risk] 纯函数 resolver 与真实 candidate 装载逻辑脱节**：三阶段的行为锁定在单元测试里，但装载代码（下个 port）可能传错候选集合 → Mitigation：在 `resolve_subagents` 的 doc 里明确 candidate 的筛选前提（应已按 parent session id 预过滤），并在下次 port 的 tasks.md 必做项里列出。
- **[Risk] `ChunkMetrics::tool_count` 保留旧语义导致 tool_executions 与 tool_count 不自洽**：期内 tool_executions 里可能出现"Task resolved"的条目被 UI 消费，但 tool_count 依然算它 → Mitigation：chunk-building spec delta 明确这是过渡期，owner 是 port 10。
- **[Trade-off] `ToolExecution` 字段里同时存 `source_assistant_uuid` 和 `start_ts` 是冗余**：冗余换来分发时 O(1) 查找 + 可直接序列化给 UI。接受。
- **[Risk] positional fallback 误配**：TS `SubagentResolver.ts` 的等式判定只在同一 parent 作用域内合法；若我们传入的 candidate 里混入别的 parent → Mitigation：函数 doc 强制约束 caller 预过滤 candidate；单元测试加一条"candidate 里有无关 subagent → 不应 positional 匹配"反例。

## Migration Plan

1. `cdt-core`：新增 `tool_execution.rs` / `process.rs`，删除 `ToolExecutionPlaceholder` / `SubagentPlaceholder`；`chunk.rs` 的 `AIChunk` 迁移字段类型；更新 `lib.rs` re-export。
2. `cdt-analyze::tool_linking`：落地三个纯函数 + 单元测试。
3. `cdt-analyze::chunk::builder`：在 flush 前跑 pair，按 `source_assistant_uuid` 分发到 `AIChunk.tool_executions`。
4. 更新 `tests/chunks.rs` 的 snapshot（`INSTA_UPDATE=always`）。
5. 写 spec delta，跑 `openspec validate --strict`。

回滚：revert 该 commit；`cdt-core` 与 `cdt-analyze` 会回到占位符状态，上次 port 的所有测试继续 pass。

## Open Questions

- Q1：`ToolOutput::Structured` 的 shape——TS 版把 Bash 的 stdout/stderr 存成 JSON object `{stdout, stderr, exit_code}`，Read tool 存成 `{content, ...}`——Rust port 是否提前拆出具体 enum 变体？**决策：不拆**，保持 `serde_json::Value` 的原样保留，由 UI 层按需解析；spec Req 2 的 "preserve both streams" 由"tool_result 原文按原样进入 output"保证。
- Q2：tool_linking 模块是否暴露 `ToolLinkingResult` 还是直接返回 `Vec<ToolExecution>`？**决策**：返回结构体，方便未来扩字段（duplicates_dropped 已经需要暴露）。
