# notification-triggers Specification

## Purpose

定义工具错误检测、通知 trigger 评估、regex 安全校验、历史回放预览、通知持久化的全流程规则，以及订阅 `file-watching` 的后台 pipeline 在文件追加时自动产 `DetectedError` 并去重持久化的契约。本 capability 由系统托盘 / dock badge / 通知中心 UI 共同消费。

## Requirements

### Requirement: Detect errors from tool executions

系统 SHALL 通过检查 `tool_result` 块的 `is_error=true` 标记以及把配置的 error pattern 与工具输出做匹配两种方式来识别工具执行错误。`is_error` 标记检查 MUST 优先于 content pattern 匹配。每条产出的 `DetectedError` SHALL 携带由 `(session_id, file_path, line_number, tool_use_id, trigger_id, message)` 元组派生出的确定性 id，确保对同一次发生的重复检测得到相同 id。

#### Scenario: Tool result flagged `is_error`
- **WHEN** 一个 `tool_result` 块带 `is_error=true`，且 trigger 模式为 `error_status` 配 `require_error=true`
- **THEN** SHALL 产出一条 `DetectedError`，附工具名、消息 uuid、输出预览、trigger id、trigger color
- **AND** 若错误消息命中任意 ignore pattern，则该错误 SHALL 被压制

#### Scenario: Tool output matches configured error pattern
- **WHEN** 工具输出含命中已配置 regex error pattern 的子串，且 trigger 模式为 `content_match`
- **THEN** SHALL 产出一条 `DetectedError`

#### Scenario: Token threshold exceeded
- **WHEN** 一个 trigger 模式为 `token_threshold`，且某次工具执行的估算 token 数超过配置阈值
- **THEN** SHALL 为每个超限的 tool_use 块产出一条 `DetectedError`，附 token 数详情

#### Scenario: Deterministic id across rescans
- **WHEN** 用相同参数（同一 session、line、tool_use_id、trigger_id、message）两次调用 `create_detected_error`
- **THEN** 两次返回记录的 `id` 字段 SHALL 字节级相等

### Requirement: Evaluate notification triggers against new messages

系统 SHALL 对每条新摄入的消息评估所有用户配置的已启用 trigger，命中时产出 `DetectedError` 记录。

#### Scenario: Trigger with literal keyword
- **WHEN** 一个 trigger 配 `content_match` 模式 `"ERROR"`，且新到达的 assistant 消息含 `"ERROR"`
- **THEN** SHALL 产出一条 `DetectedError`，附 trigger id、session id、命中预览

#### Scenario: Trigger with regex pattern
- **WHEN** 一个 trigger 配 regex 模式
- **THEN** 系统 SHALL 对消息内容应用该 regex（大小写不敏感），命中时产出 `DetectedError`

#### Scenario: Trigger scoped to specific tool names
- **WHEN** 一个 trigger 指定 `tool_name = "Bash"`，且出现匹配的 Bash `tool_result`
- **THEN** `DetectedError` SHALL 触发；其它工具的命中 SHALL NOT 触发该 trigger

#### Scenario: Ignore patterns suppress matches
- **WHEN** trigger 命中但命中内容也匹配该 trigger 的 `ignore_patterns` 之一
- **THEN** 该匹配 SHALL 被压制，不产 `DetectedError`

### Requirement: Validate regex patterns safely

系统 SHALL 在使用前校验用户提交的 regex 模式，对在固定时间预算内已知会引发 catastrophic backtracking 的模式予以拒绝。

#### Scenario: Pathological regex submitted
- **WHEN** 用户提交的 regex 在测试字符串上超过校验时间预算
- **THEN** 系统 SHALL 拒绝该 regex 并返回 validation error，不应用它

#### Scenario: Regex cache bounded
- **WHEN** 编译过的不同 regex 模式数超过 500
- **THEN** 最旧的缓存条目 SHALL 被淘汰（LRU 策略）

### Requirement: Test triggers against historical sessions

系统 SHALL 允许调用方拿一个 trigger 配置在已有 session 数据上做回放测试，返回历史上会被命中的消息列表，且不持久化任何通知。

#### Scenario: Preview a new trigger
- **WHEN** 用户对一个新 trigger 在过去 30 天的 session 上做预览
- **THEN** 系统 SHALL 返回所有 would-have-matched 消息，附 session id、timestamp、命中预览

### Requirement: Persist and expose notifications

系统 SHALL 把已触发的通知持久化到 `~/.claude/claude-devtools-notifications.json`，附读 / 未读状态，并支持分页与 mark-as-read 操作。持久化层 SHALL 按 `id` 去重：同 id 的二次写入视为 no-op，不改动已存状态与计数。

#### Scenario: Mark notification as read
- **WHEN** 调用方把某 notification id 标为已读
- **THEN** 通知状态 SHALL 更新、未读数 SHALL 减 1，新状态 SHALL 跨进程重启保留

#### Scenario: Auto-prune on startup
- **WHEN** 已存通知数超过 100
- **THEN** 系统 SHALL 删除最旧的若干条，使总数不超过 100

#### Scenario: Paged retrieval
- **WHEN** 调用方按 limit 与 offset 请求通知
- **THEN** 系统 SHALL 返回该页通知 + total 数 + 未读数 + `has_more` 标记

#### Scenario: Same-id submission is idempotent
- **WHEN** `add_notification` 用两条同 `id` 的 `DetectedError` 各调用一次
- **THEN** 存储 SHALL 恰好保留一条，未读数 SHALL 至多增加 1，第二次调用 SHALL 返回表示重复写入的信号（例如 `Ok(false)`）

### Requirement: Automatic background notification pipeline

系统 SHALL 运行一个后台 pipeline：订阅 `file-watching` 的变更事件、重新解析受影响的 session 文件、对解析得到的消息评估所有已启用 trigger、把新检测到的 `DetectedError` 经 `NotificationManager` 持久化，全程 SHALL NOT 依赖任何 UI 操作。

#### Scenario: New JSONL line with tool error triggers detection
- **WHEN** 一个 `.jsonl` session 文件追加了一条新 assistant 消息，含 `is_error=true` 的 `tool_result`
- **AND** 用户启用了 `error_status` trigger 配 `require_error=true`
- **THEN** pipeline SHALL 产出一条 `DetectedError`、经 `NotificationManager::add_notification` 持久化、并把它发到 pipeline 的 `DetectedError` broadcast 通道

#### Scenario: Duplicate detection across rescans is suppressed
- **WHEN** 同一次 session 文件变更触发了多次检测（例如另一次无关 append 引发的重扫）
- **AND** 同一 `(session_id, line_number, tool_use_id, trigger_id, message)` 元组对应的 `DetectedError` 又被产出一次
- **THEN** `NotificationManager` SHALL 识别已存在的 id 并跳过持久化，pipeline SHALL NOT 在 `DetectedError` 通道上重发该重复事件

#### Scenario: Deleted file events are ignored
- **WHEN** `FileChangeEvent` 携带 `deleted: true`
- **THEN** pipeline SHALL NOT 尝试解析已不存在的文件，SHALL NOT 产出任何 `DetectedError`

#### Scenario: Empty trigger set is a no-op
- **WHEN** 用户没有任何已启用 trigger
- **THEN** pipeline SHALL 仍接收 file change 事件，但 SHALL NOT 调用 `detect_errors`，SHALL NOT 写入通知
