## ADDED Requirements

### Requirement: list_sessions 活动摘要字段

MCP `list_sessions` tool 返回的 session 对象 SHALL 包含活动摘要字段（`userIntents`、`lastActive`、`durationMs`、`totalCost`、`toolErrorCount`、`filesTouched`、`gitSummary`），与 CLI `sessions list --format json` 输出的字段一致。

新增字段通过 `SessionSummary` 的 serde 序列化自动透传。

#### Scenario: list_sessions 返回活动摘要

- **WHEN** MCP client 调用 `list_sessions` tool
- **THEN** 返回的每个 session 对象 SHALL 包含 `userIntents` 数组和 `filesTouched` 数组

#### Scenario: 新字段为空时不序列化

- **WHEN** 某会话的 `filesTouched` 为空数组
- **THEN** MCP 返回的 JSON 中该字段 SHALL NOT 出现（`skip_serializing_if` 生效）
