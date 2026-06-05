## MODIFIED Requirements

### Requirement: Exclude filtered content from search index

系统 SHALL 在搜索匹配阶段排除 hard-noise 消息和 sidechain 消息。系统 SHALL 索引以下内容类型：user 消息文本、assistant 消息文本、`tool_use` block 的 input（递归提取 JSON string leaf，单块上限 8KB）、`tool_result` block 的 content（递归提取 JSON string leaf，单块上限 8KB）。JSON object key SHALL NOT 被纳入搜索文本。

#### Scenario: Search term appears only inside a hard-noise system-reminder
- **WHEN** 唯一命中位于一条被分类为 hard noise 的消息内
- **THEN** 结果 SHALL NOT 包含该命中

#### Scenario: Search term appears in tool_use input command
- **WHEN** query 为 "mw switch" 且某 assistant 消息包含 `tool_use` block，其 input 的 command 字段含 "mw switch get carts2"
- **THEN** 结果 SHALL 包含该命中，message_type 为 "tool_use"

#### Scenario: Search term appears in tool_result content
- **WHEN** query 为 "enabled" 且某 user 消息包含 `tool_result` block，其 content 字符串含 "enabled: true"
- **THEN** 结果 SHALL 包含该命中，message_type 为 "tool_result"

#### Scenario: Search term matches JSON key but not value
- **WHEN** query 为 "command" 且 tool_use input 的 JSON 结构含 key "command" 但 value 不含 "command"
- **THEN** 结果 SHALL NOT 包含该命中（仅匹配 string leaf value，不匹配 key）

#### Scenario: Tool content exceeding 8KB is truncated in index
- **WHEN** 某 tool_result content 超过 8KB
- **THEN** 搜索索引 SHALL 只包含前 8KB 的 string leaf 文本
- **AND** 超出部分的内容 SHALL NOT 出现在搜索命中中

## ADDED Requirements

### Requirement: Search scoped to a single session by ID

系统 SHALL 支持通过 session ID 参数将搜索范围限定到单个 session。当提供 session ID 时，系统 SHALL 只在该 session 的内容中搜索，返回 hit-level 结果（同 single-session search 格式）。当 session ID 和 project ID 同时提供时优先使用 session ID 定位文件；当仅提供 session ID 时系统 SHALL 自动定位该 session 所属 project。

#### Scenario: Intra-session search with session ID
- **WHEN** 调用搜索时指定 session ID 为 "908b77f7" 且 query 为 "mw switch"
- **THEN** 系统 SHALL 只在该 session 中搜索
- **AND** 返回结果的 sessions_searched SHALL 为 1

#### Scenario: Session ID auto-resolves project
- **WHEN** 调用搜索时仅指定 session ID（不指定 project）
- **THEN** 系统 SHALL 自动定位该 session 所属 project 并在其中搜索
