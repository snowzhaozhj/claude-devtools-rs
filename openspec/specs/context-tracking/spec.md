# context-tracking Specification

## Purpose

把会话中所有进入 Claude 上下文窗口的内容分类为 6 大类（`claude-md` / `mentioned-file` / `tool-output` / `thinking-text` / `team-coordination` / `user-message`），按 turn 累计 token 统计，并在 compact 边界处重置累计、产出 phase 历史与 compaction token delta。本 capability 是纯同步算法（无 I/O、无 runtime 依赖），由 UI 的 context panel、context badge 与 hover breakdown 消费。

## Requirements

### Requirement: Classify context injections into six categories

系统 SHALL 把消耗 Claude 上下文窗口的每段内容分类为以下六类中的恰好一类：`claude-md`、`mentioned-file`、`tool-output`、`thinking-text`、`team-coordination`、`user-message`。

#### Scenario: CLAUDE.md content injected at session start
- **WHEN** 会话加载 CLAUDE.md 内容（global、project、directory 三类作用域之一）
- **THEN** 系统 SHALL 记录一条 `claude-md` injection，附文件路径、作用域、token 数

#### Scenario: User references a file with @ mention
- **WHEN** user 消息以 `@path/to/file` 引用一个文件
- **THEN** 系统 SHALL 在文件内容加载后记录一条 `mentioned-file` injection

#### Scenario: Read tool returns file content
- **WHEN** 一次 Read 工具调用产生输出
- **THEN** 系统 SHALL 按输出 token 数记录一条 `tool-output` injection

#### Scenario: Extended thinking block appears
- **WHEN** assistant 响应含 thinking 块
- **THEN** 系统 SHALL 同时记录该 thinking 块及其后 text 的 token 为一条 `thinking-text` injection

#### Scenario: TeamCreate or SendMessage invocation
- **WHEN** 一次团队协作工具调用发生
- **THEN** 系统 SHALL 把该调用的参数和结果 token 记录为一条 `team-coordination` injection

#### Scenario: Real user prompt in a new turn
- **WHEN** 一条真实用户消息开启新 turn
- **THEN** 系统 SHALL 按其 token 数记录一条 `user-message` injection

### Requirement: Compute cumulative context statistics per turn

系统 SHALL 为每个 turn 计算上下文窗口当前可见的 token 总数，按六类分项细分。即使 AI group 为空（无 step、无 response、无前置 user group），SHALL 仍产出一条 `ContextStats` 记录：六类 token 全为 0，total 为 0，而非跳过该 turn。

#### Scenario: Turn with CLAUDE.md + two tool outputs + user message

- **WHEN** 一个 turn 含上述四条 injection
- **THEN** 该 turn 的统计 SHALL 把对应 token 数累加到匹配类别字段，并暴露一个 total

#### Scenario: Empty AI group still produces a zeroed stats record

- **WHEN** 一个 turn 的 AI group 没有任何 step、response，也没有前置 user 消息
- **THEN** 该 turn 的统计 SHALL 仍被产出：`tokens_by_category.*` 全部等于 `0`，`total_estimated_tokens == 0`，`new_injections` 为空数组，而非从 stats map 中缺失

### Requirement: Reset accumulated context on compaction boundaries

系统 SHALL 把 compact 项（来自 chunk pipeline 的 compact summary 边界消息）视为上下文 phase 边界，每条边界后重启 injection 累计，同时保留前一 phase 的记录。当 compact 边界后至少存在一个 AI group 时，系统 SHALL 额外计算 `CompactionTokenDelta`，记录边界前最后一个 AI group 与边界后第一个 AI group 的 assistant `usage` 总 token 数。

#### Scenario: Session with one compaction mid-way

- **WHEN** 会话中部发生一次 compaction
- **THEN** 边界后的 injection SHALL 从零开始累计，且 SHALL 产出一条 `ContextPhaseInfo` 记录捕获已结束的 phase

#### Scenario: First AI group after compaction records a compaction token delta

- **WHEN** 会话序列为 `[AI_1, compact, AI_2]`，且 `AI_1.last_assistant.usage.total == 1000`、`AI_2.first_assistant.usage.total == 600`
- **THEN** `phase_info.compaction_token_deltas` SHALL 恰好含一条以 compact chunk id 为 key 的条目，`pre_compaction_tokens == 1000`、`post_compaction_tokens == 600`、`delta == -400`

#### Scenario: Compaction at the very end of a session does not produce a delta

- **WHEN** 会话序列为 `[AI_1, compact]`，compact 之后无任何 AI group
- **THEN** `phase_info.compaction_token_deltas` SHALL NOT 含针对该 compact chunk 的条目，但已结束的 phase SHALL 仍在 `phase_info.phases` 中被定形

### Requirement: Expose context stats to display surfaces

系统 SHALL 通过稳定的数据结构暴露每 turn context 统计、按类累计 token、phase 历史，使 UI badge、hover 细分、完整 context panel 可消费。

#### Scenario: Query context stats for a specific turn
- **WHEN** 调用方请求第 N 个 turn 的 context 统计
- **THEN** 结果 SHALL 包含 `tokensByCategory`、total token、当前活跃 phase id、该 turn 的底层 injection 列表

### Requirement: Expose a pure synchronous API driven by chunk output

系统 SHALL 以纯同步 API 形式提供 context tracking：消费内存中的 chunk 序列以及外部注入的 per-file token 字典；SHALL NOT 在 context 计算过程中执行文件 I/O、网络调用或其它副作用。该 API SHALL 可在非 async 代码路径调用，SHALL NOT 依赖 `tokio` 等 runtime。

#### Scenario: Library consumer calls the API from a sync context

- **WHEN** 非 async 线程调用 `process_session_context_with_phases(chunks, params)`，传入借用的 chunk slice 与已填好 token 字典的 `ProcessSessionParams`
- **THEN** 该函数 SHALL 在不 spawn task、不 await future、不访问文件系统的前提下返回 `SessionContextResult`

#### Scenario: Missing token data falls back to zero without error

- **WHEN** 注入的 `claude_md_token_data` / `mentioned_file_token_data` / `directory_token_data` map 不包含某 chunk 引用的 key
- **THEN** 对应 injection SHALL 仍被产出（`estimated_tokens = 0`），且函数 SHALL NOT 返回错误、panic 或写出高于 `debug` 级别的日志

#### Scenario: Empty chunk slice yields empty result

- **WHEN** 传入的 chunk slice 为空
- **THEN** 返回的 `SessionContextResult` SHALL 含空 `stats_map`、空 `phase_info.phases`、且 `phase_info.compaction_count == 0`

### Requirement: Estimate token counts with a Unicode-scalar heuristic

系统 SHALL 提供一个全局唯一的 token 估算函数 `estimate_tokens(text)`，其结果等于 `⌈scalar_count(text) / 4⌉`，其中 `scalar_count` 计的是 Unicode scalar value 数（**不**是 UTF-8 字节数，**不**是 grapheme cluster 数）。空输入 / 缺失输入 SHALL 返回 `0`。所有 context tracking 路径——以及任何其它需要粗略 token 估算的 crate——SHALL 使用该函数，不重复实现自家 heuristic。

#### Scenario: ASCII text of length 16 estimates to 4 tokens

- **WHEN** 调用 `estimate_tokens("abcdefghijklmnop")`
- **THEN** 结果 SHALL 为 `4`

#### Scenario: Empty and whitespace-only inputs

- **WHEN** 输入为空字符串
- **THEN** 结果 SHALL 为 `0`

- **WHEN** 输入为 `"   "`（三个空格）
- **THEN** 结果 SHALL 为 `1`（⌈3/4⌉）

#### Scenario: Multi-byte scalar counts by scalar, not byte

- **WHEN** 调用 `estimate_tokens("你好世界")`（4 个汉字）
- **THEN** 结果 SHALL 为 `1`（⌈4/4⌉），而非 `3`（⌈12/4⌉）

#### Scenario: JSON-valued content is stringified before estimating

- **WHEN** 用 JSON 数组 `[1, 2, 3]` 调用 `estimate_content_tokens(value)`
- **THEN** 函数 SHALL 把值序列化为字符串后调用 `estimate_tokens("[1,2,3]")`，结果为 `2`
