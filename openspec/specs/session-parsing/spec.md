# session-parsing Specification

## Purpose

把 Claude Code 的 JSONL 会话文件以流式（按行）方式解析为统一的 `ParsedMessage` 结构，覆盖 hard noise 分类、interrupt marker 识别、`tool_use` / `tool_result` 块抽取、新旧 content 格式兼容、parse warning 上报。本 capability 是数据 pipeline 的入口，输出供 `chunk-building`、`tool-execution-linking`、`context-tracking` 等下游消费。

## Requirements

### Requirement: Stream JSONL session files line by line

系统 SHALL 把 Claude Code 会话文件作为换行分隔 JSON 流式解析，每次只处理一条记录，不把整个文件载入内存。

#### Scenario: Large session file
- **WHEN** 会话文件大于 100 MB
- **THEN** 解析 SHALL 在不把整文件载入内存的前提下完成，并按文件顺序产出 `ParsedMessage`

#### Scenario: Malformed line in middle of file
- **WHEN** 单行包含非法 JSON
- **THEN** 系统 SHALL 跳过该行、记录带行号的 parse warning、继续解析后续行

#### Scenario: Empty file
- **WHEN** 文件存在但为空
- **THEN** 系统 SHALL 返回空的解析结果，不抛错

### Requirement: Produce ParsedMessage records

系统 SHALL 把每条 JSONL 记录转为一条 `ParsedMessage`，至少包含：`uuid`、`parentUuid`、`type`、`timestamp`、`content`（字符串或 block 数组）、`usage`（如有）、`model`（如有）、`cwd`、`gitBranch`、`isSidechain`、`isMeta`，以及抽取出的 `toolCalls` / `toolResults`。

#### Scenario: Assistant message with tool_use blocks
- **WHEN** 一条 JSONL 记录为 `type=assistant`，content 含 `tool_use` 块
- **THEN** `ParsedMessage` SHALL 暴露每次 tool 调用的 `id`、`name`、`input`，且仅当 tool name 为 `Task` 时 `isTask=true`

#### Scenario: User message with tool_result blocks
- **WHEN** 一条 JSONL 记录为 `type=user`，content 含 `tool_result` 块
- **THEN** `ParsedMessage` SHALL 把 `toolResults` 填上 `toolUseId`、`content`、`isError` 字段，并归类为 internal user（适用 `isMeta` 语义）

#### Scenario: Compact summary boundary
- **WHEN** 一条 JSONL 记录是 compact-summary 边界消息
- **THEN** `ParsedMessage` SHALL 设 `isCompactSummary=true`

### Requirement: Support both legacy and current content formats

系统 SHALL 接受 user 消息 content 为纯字符串（旧版 session）或 content block 数组（新版 session），并以同一字段 `ParsedMessage.content` 暴露。

#### Scenario: Legacy string content
- **WHEN** user 记录 content 为纯字符串
- **THEN** `ParsedMessage.content` SHALL 是该字符串原样

#### Scenario: Current block array content
- **WHEN** user 记录 content 是含 text / image 块的数组
- **THEN** `ParsedMessage.content` SHALL 是该数组，保留块类型与顺序

### Requirement: Deduplicate streaming entries by requestId

系统 SHALL NOT 在主文件解析路径上按 `requestId` 对 assistant 消息去重。Claude Code 的实际 JSONL 把 `requestId` 用作"同一次 API response 的 grouping key"：一次响应的多个 content block（`thinking` / `text` / 各 `tool_use`）被写成多条独立的 `assistant` 记录，**并非** streaming rewrite 的部分快照。在 parse 阶段按 `requestId` 合并或丢弃，会丢失带独立 `tool_use` 的记录（进而导致 subagent 匹配数变少）。

`dedupe_by_request_id` 函数 MAY 保留在 `cdt-parse` 中，但 SHALL 仅在需要避免 `usage` 字段重复计数的 metrics 计算路径中被手动调用，SHALL NOT 在 `parse_file` 公开入口上自动运行。

#### Scenario: parse_file 保留同 requestId 的所有记录
- **WHEN** 一个 JSONL 文件含两条或多条共享同一 `requestId` 的 assistant 记录，每条承载不同的 content block（例如独立的 `tool_use`）
- **THEN** `parse_file` SHALL 返回这些记录的全部 `ParsedMessage`，按文件顺序保留每一条

#### Scenario: 同 requestId 多条带 tool_use 的记录各自保留
- **WHEN** 同一 `requestId` 下有一条 `thinking` 记录、一条 `text` 记录、两条不同 `tool_use` 记录
- **THEN** `parse_file` 返回的 `ParsedMessage` 数 SHALL 等于记录数；所有 `tool_use` 均被保留，便于下游 `chunk-building` 与 `tool-execution-linking` 正确匹配

#### Scenario: dedupe_by_request_id 仍作为 metrics 辅助函数可用
- **WHEN** 上层代码在计算 session metrics 时希望规避 `usage` 字段跨重复记录累加
- **THEN** 仍可调用 `cdt_parse::dedupe_by_request_id(&messages)`；该函数行为与旧实现一致（保留同 `requestId` 的最后一条 assistant 记录），但 `parse_file` 不再自动调用它

### Requirement: Emit parse warnings with line numbers on malformed input

系统 SHALL 在遇到无法 parse 的行时，发出一条带文件路径（如可获取）与 1-based 行号的 warning，然后继续处理后续行。已发出的 `ParsedMessage` 流 SHALL NOT 含针对该坏行的占位条目。

#### Scenario: Single malformed line in the middle of a file
- **WHEN** JSONL 文件第 N 行为 malformed，前后皆为合法行
- **THEN** parser SHALL 输出指向第 N 行的 warning、SHALL 仅跳过该行、SHALL 按原文件顺序为其它每一行产出 `ParsedMessage`

#### Scenario: Two adjacent malformed lines
- **WHEN** 第 N 与第 N+1 行均为 malformed
- **THEN** 两行 SHALL 各自一条 warning、各自被跳过；两侧合法行 SHALL 仍被产出

### Requirement: Expose both a per-line and a per-file parsing API

系统 SHALL 同时暴露同步的 per-line 入口（解析单条 JSONL 记录）与异步的 per-file 入口（返回完整 `ParsedMessage` 序列）。两者 SHALL 产出相同形状的 `ParsedMessage`，并对等价输入给出一致的 `MessageCategory` 分类。

#### Scenario: Per-line entry point parses a valid assistant message
- **WHEN** 调用方把一条良构 JSONL assistant 记录传入 per-line 入口
- **THEN** 入口 SHALL 返回一条 `ParsedMessage`，其 category 反映 assistant 分类，tool calls 与 block 内容一致

#### Scenario: Per-file entry point agrees with per-line entry point
- **WHEN** 同一字节序列分别经 per-file 入口与逐行 per-line 入口解析（不计 `requestId` 去重）
- **THEN** 两组 `ParsedMessage` SHALL 字段级相等且顺序一致

### Requirement: Classify hard noise messages

系统 SHALL 把绝不应被渲染的消息标记为 hard noise，包括：`system` / `summary` / `file-history-snapshot` / `queue-operation` 记录、`model='<synthetic>'` 的 assistant 消息、内容仅由 `<local-command-caveat>` 或 `<system-reminder>` 包裹的 user 消息、空 command-output 消息。**与原版"interrupt marker 是 hard noise"约定相反**，本 port 不再把 interrupt marker 归入 hard noise——interrupt 需保留以供 chunk-building 生成语义步骤以及 session-state 检测使用（详见下一条 Requirement）。

#### Scenario: Synthetic assistant placeholder
- **WHEN** assistant 消息 `model='<synthetic>'`
- **THEN** SHALL 被分类为 hard noise，从所有下游渲染中排除

#### Scenario: Interrupt marker is NOT hard noise
- **WHEN** user 消息 content 以 `[Request interrupted by user` 起首
- **THEN** SHALL NOT 被分类为 hard noise；SHALL 按下一条 Requirement 分类为 `MessageCategory::Interruption`

### Requirement: Classify interrupt marker messages

系统 SHALL 把 visible text 以 `[Request interrupted by user` 起首的任意 user 消息分类为 `MessageCategory::Interruption`。Interrupt 消息与 hard noise 不同：MUST 保留在 `ParsedMessage` 流中，下游 chunk-building 据此往 `AIChunk.semantic_steps` 追加 `SemanticStep::Interruption`，session-state 检测也据其存在把会话标记为已结束。

#### Scenario: Interrupt marker in plain text content
- **WHEN** 一条 user JSONL 记录 content 为字符串 `[Request interrupted by user for tool use]`
- **THEN** 产出的 `ParsedMessage.category` SHALL 等于 `MessageCategory::Interruption`，且该消息 SHALL NOT 在 chunk-building 之前被丢弃

#### Scenario: Interrupt marker in block content
- **WHEN** 一条 user JSONL 记录 content 为含单个 text 块的数组，且块文本以 `[Request interrupted by user` 起首
- **THEN** 产出的 `ParsedMessage.category` SHALL 等于 `MessageCategory::Interruption`

#### Scenario: Non-interrupt user text is unaffected
- **WHEN** 一条 user JSONL 记录 content 为不带 interrupt 前缀的普通文本（例如 `hello`）
- **THEN** 产出的 `ParsedMessage.category` SHALL 等于 `MessageCategory::User`，分类与现行行为一致
