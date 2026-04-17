## MODIFIED Requirements

### Requirement: Deduplicate streaming entries by requestId

The system SHALL NOT deduplicate assistant messages by `requestId` in the main file-parsing path. Claude Code 的实际 JSONL 里，同一个 `requestId` 被用作"同一次 API response 的 grouping key"：一次响应的多个 content block（`thinking` / `text` / 各 `tool_use`）会被写成多条独立的 `assistant` 记录，**并非** streaming rewrite 的部分快照。在 parse 阶段按 `requestId` 合并或丢弃，会丢失有独立 `tool_use` 的记录（进而导致 subagent 匹配数变少）。

`dedupe_by_request_id` 函数 MAY 保留在 `cdt-parse` 中，但 SHALL 仅在需要避免 `usage` 字段重复计数的 metrics 计算路径中被手动调用，SHALL NOT 在 `parse_file` 的公开入口上自动运行。

#### Scenario: parse_file 保留同 requestId 的所有记录
- **WHEN** 一个 JSONL 文件包含两条或多条共享同一 `requestId` 的 assistant 记录，每条承载不同的 content block（例如独立的 `tool_use`）
- **THEN** `parse_file` SHALL 返回这些记录的全部 `ParsedMessage`，按文件顺序保留每一条

#### Scenario: 同 requestId 多条带 tool_use 的记录各自保留
- **WHEN** 同一 `requestId` 下有一条 `thinking` 记录、一条 `text` 记录、两条不同 `tool_use` 记录
- **THEN** `parse_file` 返回的 `ParsedMessage` 数量 SHALL 等于记录数；所有 `tool_use` 均被保留，便于下游 `chunk-building` 和 `tool-execution-linking` 正确匹配

#### Scenario: dedupe_by_request_id 仍作为 metrics 辅助函数可用
- **WHEN** 上层代码在计算 session metrics 时希望规避 usage 字段跨重复记录累加
- **THEN** 仍可调用 `cdt_parse::dedupe_by_request_id(&messages)`；该函数行为与旧实现一致（保留同 requestId 的最后一条 assistant 记录），但 `parse_file` 不再自动调用它
