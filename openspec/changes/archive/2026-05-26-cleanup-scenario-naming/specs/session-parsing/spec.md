# session-parsing Spec Delta

## MODIFIED Requirements

### Requirement: Deduplicate streaming entries by requestId

系统 SHALL NOT 在主文件解析路径上按 `requestId` 对 assistant 消息去重。Claude Code 的实际 JSONL 把 `requestId` 用作"同一次 API response 的 grouping key"：一次响应的多个 content block（`thinking` / `text` / 各 `tool_use`）被写成多条独立的 `assistant` 记录，**并非** streaming rewrite 的部分快照。在 parse 阶段按 `requestId` 合并或丢弃，会丢失带独立 `tool_use` 的记录（进而导致 subagent 匹配数变少）。

为了 metrics 计算路径仍能避开 `usage` 字段重复计数，系统 SHALL 暴露一个独立的 `dedupe_by_request_id` 辅助函数，行为是"保留同 `requestId` 的最后一条 assistant 记录"。该辅助函数 SHALL NOT 在 `parse_file` 公开入口上自动运行——主路径解析返回原始 `ParsedMessage` 序列。

#### Scenario: 解析文件时保留同 requestId 的所有记录

- **WHEN** 一个 JSONL 文件含两条或多条共享同一 `requestId` 的 assistant 记录，每条承载不同的 content block（例如独立的 `tool_use`）
- **THEN** `parse_file` SHALL 返回这些记录的全部 `ParsedMessage`，按文件顺序保留每一条

#### Scenario: 同 requestId 多条带 tool_use 的记录各自保留

- **WHEN** 同一 `requestId` 下有一条 `thinking` 记录、一条 `text` 记录、两条不同 `tool_use` 记录
- **THEN** `parse_file` 返回的 `ParsedMessage` 数 SHALL 等于记录数；所有 `tool_use` 均被保留，便于下游 `chunk-building` 与 `tool-execution-linking` 正确匹配

#### Scenario: metrics 辅助路径仍可按 requestId 去重

- **WHEN** 上层代码在计算 session metrics 时希望规避 `usage` 字段跨重复记录累加
- **THEN** 仍可调用 `dedupe_by_request_id(&messages)`；该函数行为与旧实现一致（保留同 `requestId` 的最后一条 assistant 记录），但 `parse_file` 不再自动调用它

### Requirement: Expose both a per-line and a per-file parsing API

系统 SHALL 同时暴露同步的 per-line 入口（解析单条 JSONL 记录）与异步的 per-file 入口（返回完整 `ParsedMessage` 序列）。两者 SHALL 产出相同形状的 `ParsedMessage`，并对等价输入给出一致的 `MessageCategory` 分类。

#### Scenario: Per-line parse path handles a valid assistant message
- **WHEN** 调用方把一条良构 JSONL assistant 记录传入 per-line 入口
- **THEN** 入口 SHALL 返回一条 `ParsedMessage`，其 category 反映 assistant 分类，tool calls 与 block 内容一致

#### Scenario: Per-file parse path agrees with per-line parse path
- **WHEN** 同一字节序列分别经 per-file 入口与逐行 per-line 入口解析（不计 `requestId` 去重）
- **THEN** 两组 `ParsedMessage` SHALL 字段级相等且顺序一致

### Requirement: Classify hard noise messages

系统 SHALL 把绝不应被渲染的消息标记为 hard noise，包括：`system` / `summary` / `file-history-snapshot` / `queue-operation` 记录、`model='<synthetic>'` 的 assistant 消息、内容仅由 `<local-command-caveat>` 或 `<system-reminder>` 包裹的 user 消息、空 command-output 消息。**与原版"interrupt marker 是 hard noise"约定相反**，本 port 不再把 interrupt marker 归入 hard noise——interrupt 需保留以供 chunk-building 生成语义步骤以及 session-state 检测使用（详见下一条 Requirement）。

#### Scenario: Missing assistant generates placeholder
- **WHEN** assistant 消息 `model='<synthetic>'`
- **THEN** SHALL 被分类为 hard noise，从所有下游渲染中排除

#### Scenario: Interrupt marker is NOT hard noise
- **WHEN** user 消息 content 以 `[Request interrupted by user` 起首
- **THEN** SHALL NOT 被分类为 hard noise；SHALL 按下一条 Requirement 分类为 `MessageCategory::Interruption`
