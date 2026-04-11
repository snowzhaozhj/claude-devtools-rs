## Context

TS 侧 context-tracking 的调用链是：
1. `processSessionContextWithPhases(chatItems, projectRoot, claudeMdTokenData, mentionedFileTokenData, directoryTokenData)`
2. 遍历 `ChatItem[]`（chunk pipeline 已产出）—— `UserGroup` / `AIGroup` / `CompactItem`。
3. 每个 `AIGroup` 调 `computeContextStats(params)` 聚合 6 类 injection，返回 `ContextStats`。
4. 碰到 `CompactItem`：backfill 上一个 AI group 的 accumulatedInjections、写 phase、重置状态、开新 phase。
5. 最终返回 `{ statsMap: Map<aiGroupId, ContextStats>, phaseInfo: ContextPhaseInfo }`。

聚合 6 类里：
- `claude-md` 来自 `createGlobalInjections()` + `detectClaudeMdFromFilePath()`（本 port **不做**文件 I/O，只接受注入字典）。
- `mentioned-file` 来自 `extractUserMentionPaths()` + `extractFileRefsFromResponses()`，配合外部 `mentionedFileTokenData` 查表。
- `tool-output` 来自 `aggregateToolOutputs()` 遍历 `linkedTools`，排除 `TASK_COORDINATION_TOOL_NAMES`。
- `task-coordination` 来自 `aggregateTaskCoordination()` 专门取那 7 个 task 工具 + `teammate_message` display item。
- `thinking-text` 来自 `aggregateThinkingText()` 扫 display items 的 `thinking` / `output` 类型。
- `user-message` 来自 `createUserMessageInjection()`，用 `estimateTokens(userGroup.rawText)`。

去重机制：每个 AI group 用 `previousPaths: Set<string>` 线程式累积，避免同一文件重复计 CLAUDE.md / mentioned-file injection。TS 里写得很清楚"threaded to avoid O(N) rebuild per group"。

当前 Rust 侧 `cdt-analyze` 已有 `chunk` / `tool_linking` 模块，能产出 `Vec<Chunk>`，但还没 display item / linked tool 的子结构。本 port 需要**先确认**这些结构在 Rust 侧的映射：

- **TS `AIGroup`** ↔ **Rust `AIChunk`**（已在 `cdt-core::chunk` 定义）
- **TS `ChatItem`** ↔ **Rust `Chunk`** 枚举（user/ai/system/compact 4 种）
- **TS `LinkedToolItem`** ↔ **Rust `ToolExecution`**（在 `cdt-core::tool_execution`；已有 `name` / `input` / `output: Option<ToolOutput>` / `call_tokens` / `result_tokens` 等字段——需要核对字段齐全性）
- **TS `AIGroupDisplayItem`** ↔ **Rust `SemanticStep`**（已在 `cdt-core::chunk` 定义，但字段形态需要确认是否覆盖 `thinking` / `output` / `slash` / `teammate_message` 4 种）

**关键前置问题**：`SemanticStep` / `ToolExecution` 这些类型是否已经把 TS 的 `tokenCount` / `callTokens` / `resultTokens` 等字段也带上了？如果没有，本 port 需要扩展它们（或者定义 adapter 层）。

## Goals / Non-Goals

**Goals:**
- 让 `cdt_analyze::context::process_session_context_with_phases(&[Chunk], &params) -> SessionContextResult` 的行为对齐 TS `processSessionContextWithPhases` 的 end-to-end 语义：同样的 6 类 injection、同样的 phase 切换、同样的 compaction token delta。
- `compute_context_stats` / 各 `aggregate_*` 都是纯函数 + 单测 100% 覆盖 spec scenario。
- `cdt-core::tokens::estimate_tokens` 作为所有 crate 的唯一 token 估计实现，冻结 `⌈len/4⌉` 语义。
- CLAUDE.md / mentioned-file 的 token 数据通过参数注入（`HashMap<String, ClaudeMdFileInfo>` / `HashMap<String, MentionedFileInfo>`），不在本 port 里读磁盘。
- JSON 序列化 shape 对齐 TS（`camelCase` 字段名），方便后续 `cdt-api` port 直接暴露。

**Non-Goals:**
- 不做 CLAUDE.md 文件的真实读取 / enterprise 扫描 / 目录爬行（`port-configuration-management` 负责）。
- 不做 `@mention` 路径到文件系统的 resolve（同上）。
- 不做 UI 展示（`ContextBadge` / `ContextPanel` / hover breakdown）。
- 不做 Read tool 的内容深度解析，仅用外部传入的 tokenCount 做求和。
- 不引入 teammate 身份识别（`port-team-coordination-metadata` 负责）；本 port 里 `teammate_message` 只要有 `token_count` 就归到 `task-coordination` 桶。
- 不做精确 tokenizer（tiktoken / GPT-BPE），仅实现 `⌈len/4⌉` 启发式；精确化留到未来有真实需求时再谈。

## Decisions

### 决策 1：模块边界 —— `cdt-core::context` vs `cdt-analyze::context`

- `cdt-core::context`：**API 形状**类型 —— `ContextInjection`（含 6 variant）、`TokensByCategory`、`ContextStats`、`ContextPhase`、`ContextPhaseInfo`、`CompactionTokenDelta`、`NewCountsByCategory`、`ToolTokenBreakdown`、`TaskCoordinationBreakdown`、`ThinkingTextBreakdown`、`MentionedFileInfo`、`ClaudeMdFileInfo` 等辅助。所有都是 `#[derive(Serialize, Deserialize)]` 带 `#[serde(rename_all = "camelCase")]`。
- `cdt-core::tokens`：`estimate_tokens(&str) -> usize` + `estimate_content_tokens(&serde_json::Value) -> usize`。
- `cdt-analyze::context`：**行为**层 —— 聚合 / 计算 / phase 处理函数，全部纯同步。

**替代方案**：把类型也塞进 `cdt-analyze::context` 本地。**拒绝**，因为 `cdt-api` 后续 port 要直接 `use cdt_core::ContextInjection`，不能去 `cdt-analyze` 跨 crate 引用行为模块的类型。

### 决策 2：API 入口的签名

```rust
pub struct ProcessSessionParams<'a> {
    pub project_root: &'a Path,
    pub claude_md_token_data: &'a HashMap<String, ClaudeMdFileInfo>,
    pub mentioned_file_token_data: &'a HashMap<String, MentionedFileInfo>,
    pub directory_token_data: &'a HashMap<String, ClaudeMdFileInfo>,
}

pub struct SessionContextResult {
    pub stats_map: HashMap<String, ContextStats>, // ai_group_id → stats
    pub phase_info: ContextPhaseInfo,
}

pub fn process_session_context_with_phases(
    chunks: &[Chunk],
    params: &ProcessSessionParams<'_>,
) -> SessionContextResult;
```

- 入参全部 `&` 借用，不 take ownership；返回 `SessionContextResult`。
- `ProcessSessionParams` 是 builder-friendly 的 struct，省得未来加字段时改 signature。
- 没有 `Result`，因为所有错误都在纯数据转换里，不会失败（外部注入的数据缺 key 时就默认 0 token，不报错，跟 TS 对齐）。

**替代方案**：把 `params` 做成 `trait ContextDataSource`，scanner 从 trait 取数据。**拒绝**，trait 会引入 dyn 开销并且让单测写假实现变复杂，当前 struct + HashMap 足够且显式。

### 决策 3：chunk 结构的适配

检查 `cdt-core::chunk::AIChunk` 和 `SemanticStep` 当前是否带够 TS 里的 `tokenCount` / `callTokens` / `resultTokens` 字段：

- 如果带够：直接消费。
- 如果不带：`cdt-analyze::context::adapter.rs` 里定义一层 `ContextView` 结构，从 `Chunk` 提取需要的字段；本 port **不修改** `cdt-core::chunk`。

本 port 的实施第一步就是"扫一遍 `cdt-core::chunk` / `cdt-core::tool_execution`，列出缺哪些字段"。如果缺失，**倾向加字段到 `cdt-core`**（因为这些 token count 本来就是 chunk-building / tool-execution-linking 阶段算出来的，属于数据产物），而不是写 adapter。adapter 是兜底方案。

**开放问题**：确认 `ToolExecution::call_tokens` / `result_tokens` / `skill_instructions_token_count` 是否都存在。若缺 `skill_instructions_token_count`，先在本 port 加 `Option<u32>`，默认 `None`。

### 决策 4：thinking-text 与 task-coordination 的边界

TS 里 `aggregateThinkingText` 遍历 `displayItems` 的 `thinking` / `output` 两种类型；`aggregateTaskCoordination` 遍历 linkedTools 取 7 个 task 工具，外加 `displayItems` 里的 `teammate_message`。

Rust 侧：
- `SemanticStep` 需要能区分 thinking / output / slash / teammate_message 4 种。当前是否满足，需要在实施 §1 阶段确认。
- 如果 `SemanticStep` 只有 thinking / text / tool / subagent 4 类型（按 TS `SemanticStepExtractor` 的分组），可能需要在 `cdt-core::chunk` 里补一个 `TeammateMessage { teammate_id, token_count }` variant。

**决策**：若 `SemanticStep` 不够，先在 `cdt-core::chunk` 扩枚举，但保持向后兼容（旧消费方只需忽略新 variant）。

### 决策 5：phase / compaction delta 的语义冻结

TS `processSessionContextWithPhases` 的 phase 逻辑：
1. 初始 phase = 1；无 compact 时整个 session 是 phase 1。
2. 碰到 `CompactItem`：
   - backfill 上一组 `accumulatedInjections`（O(N) 一次）。
   - 写 `{ phaseNumber, firstAIGroupId, lastAIGroupId, compactGroupId }`。
   - 清空 `accumulatedInjections` / `previousPaths` / `isFirstAiGroup = true`。
   - `currentPhaseNumber += 1`。
   - **保留** `lastAIGroupBeforeCompact`，用于下个 phase 的 first group 计算 delta。
3. 新 phase 的第一个 AI group：用 `getFirstAssistantTotalTokens(aiGroup)` vs `getLastAssistantTotalTokens(lastAIGroupBeforeCompact)` 算 `CompactionTokenDelta { preCompactionTokens, postCompactionTokens, delta }`。
4. 最后一个 phase 在循环结束后 finalize。

Rust 按同样顺序实现，**不改**语义。单测要覆盖：
- 无 compact：1 phase，delta map 为空。
- 1 次 compact：2 phase，delta map 含 1 个 `CompactionTokenDelta`。
- compact 落在最后（无新 AI group）：旧 phase 正常 finalize，delta map 不加新条目。

### 决策 6：去重 / `previousPaths` 的所有权

TS 用 `Set<string>` 线程传递。Rust 用 `HashSet<String>`，每次 `compute_context_stats` 返回一个**新** set（而不是就地改原 set），调用方把返回值喂给下一次调用。理由：纯函数 + 测试更简单，HashSet 的 clone 在实践中 set 规模 < 100，开销可忽略。

```rust
pub fn compute_context_stats(params: &ComputeStatsParams<'_>) -> ComputeStatsResult {
    // returns { stats, next_previous_paths }
}
```

### 决策 7：错误处理与容错

- token data 字典查不到 key：返回 0 token，不报错。TS 行为。
- 空 AI group（无 steps / 无 responses）：返回空 `ContextStats`，`total_estimated_tokens = 0`，category 全 0。本 port 新增 scenario 覆盖（coverage-gap）。
- serde deserialize 失败：这是上游 `cdt-parse` 的责任，本 port 不处理。
- panic：禁止。所有 `unwrap()` 只允许在 `#[cfg(test)]` 里。

### 决策 8：测试策略

- **单元测试**（每个模块底部 `#[cfg(test)] mod tests`）：
  - `aggregate_tool_outputs`：正常 + 无 tool + 只含 task 工具（应返回 None）+ 带 slash item。
  - `aggregate_task_coordination`：SendMessage + TaskCreate + teammate_message 混合。
  - `aggregate_thinking_text`：thinking + output 混合；只有 thinking；全 0。
  - `create_user_message_injection`：正常 / 空字符串 / 仅空白。
  - `previous_paths` 去重：同一 path 出现在两个连续 group 应只计一次。
- **集成测试**（`crates/cdt-analyze/tests/context_tracking.rs`）：
  - 空 session（0 chunk）→ 空 statsMap、空 phaseInfo。
  - 单 AI group + 1 tool output + 1 user message → stats 有 2 个 injection、total > 0。
  - 2 AI group + 1 compact → 2 phase、phase 2 的 first group 有 CompactionTokenDelta。
  - compact 出现在最后（后面没 AI group）→ 旧 phase finalize、delta map 不加条目。
  - 同一 file path 在两个连续 group 都被 @mention → 只产生 1 个 mentioned-file injection。
- **JSON shape 测试**：序列化一个典型 `ContextStats` 到 JSON，断言字段名与 TS `camelCase` 一致。

## Risks / Trade-offs

- **[Risk]** `cdt-core::chunk` 的字段可能不够用（尤其 `SemanticStep` / `ToolExecution` 缺 token 字段）→ **缓解**：实施第一步（§1）专门检查并列清单；若缺，在本 port 里扩 `cdt-core`，单独一小节任务，单独跑测试回归 `cdt-analyze::chunk` 的旧路径。
- **[Risk]** token 估计 `⌈len/4⌉` 是 byte-wise 还是 char-wise 会影响非 ASCII 场景。TS `str.length` 是 UTF-16 code unit 数，Rust `str::len()` 是 byte 数，两者对 ASCII 完全一致但对 Unicode 不同。TS 历史就是这个算法，但 Rust 选哪个？→ **决策**：Rust 用 `str::chars().count()`（Unicode scalar 数），**与 TS `s.length` 语义最接近**（JS string.length 是 UTF-16 code unit，多数情况下≈ scalar 数）。spec delta 里明确写"by Unicode scalar count / 4, rounded up"，避免歧义。
- **[Risk]** `ContextStats` 的 `accumulatedInjections` 只在每 phase 的最后一个 group 填满（TS 做了 O(N²) → O(N) 优化）。Rust 要复现这个优化，否则大 session 会炸内存 → **缓解**：直接按 TS 模式写，单测覆盖"中间 group 的 accumulatedInjections 是空的" + "phase 最后 group 的 accumulatedInjections 非空"。
- **[Trade-off]** 不自己读 CLAUDE.md 文件，全靠外部注入 token 字典 → **代价**：单独跑 `cdt-analyze` 没法验证 end-to-end；**收益**：`cdt-analyze` 保持同步 + 零 I/O，测试好写，`port-configuration-management` 后再在一起跑 end-to-end。选收益。
- **[Trade-off]** `SemanticStep` 加 `TeammateMessage` variant 会让 `cdt-core::chunk` 膨胀 → **代价**：破坏单一职责；**收益**：`cdt-api` 后续直接用一个 Chunk 结构覆盖全部 UI 渲染需求。权衡后仍选收益，因为 display item 本来就是 chunk 的一部分。
- **[Open Question]** `extractFileRefsFromResponses` 的解析规则（从 assistant 输出文本里抓 `@path` / `path/to/file` 路径）在 TS 里是个 250 行的正则 + 启发式模块。Rust 侧要不要原样 port？**暂定**：本 port 只实现"从 linked tool `Read` 工具的 input path 里抓路径"的简单版本，更复杂的 response 扫描留给 `port-configuration-management`（那里本来就要读文件，顺便做）。spec 里的相关 scenario（"User references a file with @ mention"）的语义是"文件内容已被加载后记录 injection"，当前测试可以用 fixture 注入"已加载的文件"的形式覆盖，不要求实现 response 扫描。
