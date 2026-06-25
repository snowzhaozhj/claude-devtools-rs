## MODIFIED Requirements

### Requirement: Read-only tool set

MCP server SHALL 暴露以下 7 个 tools，全部标注 `readOnlyHint=true`、`destructiveHint=false`、`idempotentHint=true`：`list_projects`、`list_sessions`、`get_session`、`get_turn`、`get_step_output`、`search`、`get_stats`。

所有 tools SHALL 返回 JSON 结构化数据（不返回纯文本大段 dump）。turn/step 数据形状、截断规则、分页字段由 `[[session-turn-view]]` owner，本工具集引用其契约。CLI 与 MCP 的数据参数集 SHALL 完全一致（CLI 仅额外提供终端渲染 flags）。

#### Scenario: list_projects 返回项目列表

- **GIVEN** 用户有至少一个 Claude Code 项目
- **WHEN** AI 调用 `list_projects` tool
- **THEN** 返回 SHALL 包含 `name`、`path`、`sessions`、`lastActive` 字段的 JSON 数组

#### Scenario: list_sessions 全局跨项目查询

- **WHEN** AI 调用 `list_sessions` 不传 `project`，传 `since="yesterday"`
- **THEN** 返回 SHALL 含所有项目中时间范围内的 session 列表
- **AND** 每条 SHALL 含 `sessionId`、`projectName`、`title`、`messageCount`、`timestamp`、`gitBranch`、`isOngoing`、`filesTouched`
- **AND** 参数 SHALL 限于 `project`、`since`、`until`、`grep`、`cursor`（不含 `branch`、`is_ongoing`、`limit`、`group_by`——消费者从返回数据自行过滤/分组）

#### Scenario: get_session 返回 compact overview

- **GIVEN** 一个含多个 turn 的 session
- **WHEN** AI 调用 `get_session` 传 `session`
- **THEN** 返回 SHALL 含 session 级 `sessionId`、`model`、`totalCost`、`durationMs`、`filesTouched`、`userIntents`
- **AND** SHALL 含 `turns` 数组，每个 turn 含 `index`、`question`、`answer`、`tools`（按工具名聚合，每项 `name`/`count`/`errorCount`）、`stepsCount`、`metrics`
- **AND** SHALL 含统一分页字段 `total` 与（如有下一页）`nextCursor`
- **AND** 参数 SHALL 限于 `session`、`grep`、`cursor`（不含 `project`——服务端自动解析；不含 `include`）

#### Scenario: get_session grep 命中标注

- **WHEN** AI 调用 `get_session` 传 `grep` 且某 turn 的 steps 内容（含 thinking / tool input / output）命中
- **THEN** 返回 SHALL 只含命中的 turn 且每个命中 turn 含 `matchedIn`（标注命中位置，如 `"tool:Read"`）

#### Scenario: get_turn 返回单 turn 完整 steps

- **WHEN** AI 调用 `get_turn` 传 `session` 与 `turn`
- **THEN** 返回 SHALL 含该 turn 的 `question`、`answer`、`metrics` 与有序 `steps`（thinking/text/tool/subagent 等）
- **AND** tool step 的 output ≥5KB 时 SHALL 截断并标 `outputTruncated`/`outputBytes`
- **AND** steps 超过单页上限时 SHALL 用 `total`+`nextCursor` 分页

#### Scenario: get_step_output 取完整原文

- **WHEN** AI 对某被截断的 step 调用 `get_step_output` 传 `session`、`turn`、`step`
- **THEN** 返回 SHALL 含该 step 的完整未截断 output

#### Scenario: search 返回 turn 级命中

- **WHEN** AI 调用 `search` 传 `query`
- **THEN** 返回 SHALL 为 turn 级命中列表，每条含 `sessionId`、`turnIndex`、`question`、`matchSnippet`、`timestamp`、`projectName`
- **AND** AI SHALL 能用返回的 `turnIndex` 直接调 `get_turn` 钻取

#### Scenario: session='latest' 解析

- **WHEN** AI 调用 `get_session` 或 `get_turn` 传 `session="latest"`
- **THEN** 服务端 SHALL 解析为最近一次 session（按 timestamp 降序第一条）

## REMOVED Requirements

### Requirement: Context budget truncation

**Reason**: `get_session_chunks` 工具被删除；其按 chunk 粒度的 `max_tokens` 预算截断被 `session-turn-view` 的服务端内置截断（tool output ≥5KB）+ step 分页 + `get_step_output` 取全文替代。

**Migration**: 用 `get_turn` 取单 turn（自带 step 分页与 output 截断），用 `get_step_output` 取被截断的完整原文，不再需要 `max_tokens` 参数。

### Requirement: grep 过滤 session detail chunks

**Reason**: `get_session_chunks` 工具被删除；grep 行为迁移到 `get_session`（按 turn 粒度匹配完整 steps 内容并返回 `matchedIn`）。

**Migration**: 用 `get_session` 传 `grep`，命中以 turn 为单位返回，匹配范围覆盖 thinking / tool input / output / 文本。
