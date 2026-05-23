# ipc-data-api Specification Delta

## ADDED Requirements

### Requirement: SessionDetail 暴露与 SessionSummary 同源派生的 title

`get_session_detail` 返回的 `SessionDetail` MUST 暴露字段 `title: Option<String>`（camelCase 序列化），其值 SHALL 与同一 sessionId 的 `SessionSummary.title` 派生字节级一致——即调用 `extract_session_metadata_from_parsed(&messages, is_stale)` 一次产出，与 `list_sessions` 后台扫描路径共用同一派生函数。

派生 SHALL 在 `get_session_detail` 已持有的 `messages: Vec<ParsedMessage>` slice 上执行，**不**得重读 JSONL 文件。`is_stale` 入参 SHALL 与同上下文计算 `isOngoing` 时使用的同义值保持一致（即 `!is_ongoing` 的同源 stale 状态）；title 派生本身不依赖 `is_stale`，传值仅为 API 同形。

派生失败 / messages 为空时 `title` SHALL 为 `None`。前端在 `None` 时 SHALL fallback 到 `sessionId.slice(0, 8)`，与 sidebar 显示规则一致（不**得** slice(0, 12) 或其它长度）。

HTTP `GET /api/sessions/{sid}` 与 IPC `get_session_detail` 共用同一 `LocalDataApi::get_session_detail` 实现，自动适用本 Requirement。

#### Scenario: detail.title 与 SessionSummary.title 一致

- **GIVEN** session JSONL 首条 user 消息文本为 `"修复登录页样式"`
- **WHEN** 前端先调 `list_sessions(projectId)` 拿 `SessionSummary[]`，再调 `get_session_detail(projectId, sessionId)` 拿 `SessionDetail`
- **THEN** `SessionDetail.title` SHALL 与对应 `SessionSummary.title` 字节级相等（均为 `"修复登录页样式"`）

#### Scenario: detail.title 跳过 [Request interrupted by user 起首消息

- **GIVEN** session JSONL 首条 user 消息文本以 `"[Request interrupted by user"` 起首，第二条 user 消息文本为 `"重试一次"`
- **WHEN** `get_session_detail` 返回
- **THEN** `SessionDetail.title` SHALL 为 `Some("重试一次")`（与 sidebar 规则一致——不**得**返回中断字面量）

#### Scenario: detail.title 处理 slash with args

- **GIVEN** session JSONL 首条非系统输出 user 消息为 `<command-name>/model</command-name><command-args>sonnet</command-args>` 形态
- **WHEN** `get_session_detail` 返回
- **THEN** `SessionDetail.title` SHALL 为 `Some("/model sonnet")`（与 sidebar 同口径——不**得**跳过 slash 消息）

#### Scenario: detail.title 提取 teammate-message summary

- **GIVEN** session JSONL 首条 user 消息含 `<teammate-message teammate_id="m1" summary="审查 PR 137">body</teammate-message>`
- **WHEN** `get_session_detail` 返回
- **THEN** `SessionDetail.title` SHALL 为 `Some("审查 PR 137")`（取 summary 属性，不**得**是 body 或空字符串）

#### Scenario: detail.title 跳过 local-command-stdout 内容

- **GIVEN** session JSONL 首条 user 消息文本以 `<local-command-stdout>` 起首包裹命令输出，第二条 user 消息文本为 `"继续"`
- **WHEN** `get_session_detail` 返回
- **THEN** `SessionDetail.title` SHALL 为 `Some("继续")`（不**得**把 stdout 内容当作 title）

#### Scenario: detail.title 截断到 TITLE_MAX_CHARS

- **GIVEN** session JSONL 首条 user 消息文本是 600 个汉字
- **WHEN** `get_session_detail` 返回
- **THEN** `SessionDetail.title.unwrap().chars().count()` SHALL `== 500`（与 sidebar TITLE_MAX_CHARS 同值）

#### Scenario: detail.title 在 messages 为空时为 None

- **GIVEN** session JSONL 解析后 `messages.is_empty()` 为 true
- **WHEN** `get_session_detail` 返回
- **THEN** `SessionDetail.title` SHALL 为 `None`（前端 fallback 到 `sessionId.slice(0, 8)`，由前端契约保证）

#### Scenario: detail.title 处理 slash 无 args 走 command_fallback

- **GIVEN** session JSONL 首条非系统输出 user 消息为 `<command-name>/clear</command-name><command-args></command-args>` 形态（`<command-args>` 为空），后续 user 消息均为系统输出 / 中断 / 空文本
- **WHEN** `get_session_detail` 返回
- **THEN** `SessionDetail.title` SHALL 为 `Some("/clear")`（与 sidebar 一致 —— 走 `command_fallback` 路径而非空标题）

#### Scenario: detail.title 跳过 is_meta 标记的 user 消息

- **GIVEN** session JSONL 首条 user 消息文本为 `"内部追问"` 但其 `is_meta` 字段为 `true`，第二条 user 消息文本为 `"用户实际输入"` 且 `is_meta=false`
- **WHEN** `get_session_detail` 返回
- **THEN** `SessionDetail.title` SHALL 为 `Some("用户实际输入")`（`is_meta=true` 的消息不**得**被取作 title）

#### Scenario: detail.title 在 sanitize 后为空时继续寻找下一条

- **GIVEN** session JSONL 首条 user 消息文本完全由 `<system-reminder>...</system-reminder>` 等 system tag 包裹（`sanitize_for_title` 移除全部内容后为空字符串），第二条 user 消息文本为 `"实际请求"`
- **WHEN** `get_session_detail` 返回
- **THEN** `SessionDetail.title` SHALL 为 `Some("实际请求")`（sanitize 后空 SHALL 触发"取下一条"循环行为）
