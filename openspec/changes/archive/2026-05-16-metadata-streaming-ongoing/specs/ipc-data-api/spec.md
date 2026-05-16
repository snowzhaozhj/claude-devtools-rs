## ADDED Requirements

### Requirement: `extract_session_metadata` 流式判定 isOngoing 不收集全量消息向量

`extract_session_metadata_with_ongoing` SHALL 流式判定 `messages_ongoing`：在 JSONL 逐行解析的 loop 内，将每条 `ParsedMessage` 即时喂给 `cdt_analyze::IsOngoingStateMachine` 的 `feed(&msg)` 接口，并在文件读取完毕后调用 `state_machine.finalize()` 得到最终 `messages_ongoing` 值。该函数 MUST NOT 在内存中保留 `Vec<ParsedMessage>` —— 即 `messages_ongoing` 的计算路径上不得 collect 全量解析结果到容器。

`cdt_analyze::IsOngoingStateMachine` SHALL 提供以下公开接口：
- `pub fn new() -> Self`：构造空状态机（`ongoing = false`，shutdown_tool_ids 为空集）
- `pub fn feed(&mut self, msg: &ParsedMessage)`：吃一条消息，按 `MessageType::Assistant` / `MessageType::User` 分发并更新内部状态
- `pub fn finalize(self) -> bool`：消费状态机得到最终 `is_ongoing` 判定

`IsOngoingStateMachine` 流式喂入的最终结果 SHALL 与既有 `cdt_analyze::check_messages_ongoing(&messages)` 在任意有限消息序列上完全等价。`check_messages_ongoing` MAY 内部委托给 `IsOngoingStateMachine`（thin wrapper：`for msg in messages { sm.feed(msg); } sm.finalize()`），公开签名保持 `pub fn check_messages_ongoing(messages: &[ParsedMessage]) -> bool`。

#### Scenario: 流式状态机不在内存保留全量 ParsedMessage

- **WHEN** 调用 `extract_session_metadata_with_ongoing` 处理一个含 N 条消息的 JSONL 文件
- **THEN** 函数实现路径 SHALL NOT 创建 `Vec<ParsedMessage>` 或等价容器以累积全部解析结果用于 `is_ongoing` 计算
- **AND** 实际驻留内存峰值 SHALL 不随 N 线性增长（仅 `IsOngoingStateMachine` 自身字段 + 当前正解析的单行消息）

#### Scenario: 状态机与切片版 check_messages_ongoing 结果等价

- **GIVEN** 一组覆盖 normal completed / ongoing tool-use / interrupted / teammate-message / shutdown_response / resumed-after-interrupt 六类典型场景的 fixture 消息序列
- **WHEN** 用 `IsOngoingStateMachine.feed(...).finalize()` 流式处理
- **AND** 用 `check_messages_ongoing(&[..])` 切片处理同一序列
- **THEN** 两种处理方式 SHALL 在每个 fixture 上返回相同 `bool` 结果

#### Scenario: 空消息序列返回 false

- **WHEN** 在新建的 `IsOngoingStateMachine` 上不调用任何 `feed`，直接 `finalize()`
- **THEN** SHALL 返回 `false`（与 `check_messages_ongoing(&[])` 一致）

#### Scenario: SHUTDOWN_RESPONSE tool 跨消息追踪

- **GIVEN** 序列：assistant 消息含 `tool_use { id: "tu-shutdown", name: "SendMessage", input: { type: "shutdown_response", approve: true } }`，紧随 user 消息含 `tool_result { tool_use_id: "tu-shutdown", ... }`
- **WHEN** 依次 `sm.feed(assistant_msg); sm.feed(user_msg); sm.finalize()`
- **THEN** 状态机内部 `shutdown_tool_ids` SHALL 在 feed assistant 时插入 `"tu-shutdown"`
- **AND** feed user 时识别匹配的 `tool_use_id`，将对应事件归类为 Interruption（ending），最终 `finalize()` SHALL 返回 `false`

#### Scenario: extract_session_metadata 公开签名保持纯函数语义

- **WHEN** 现有调用方直接调用 `extract_session_metadata(path)`
- **THEN** 该函数签名 SHALL 保持 `pub async fn extract_session_metadata(path: &Path) -> SessionMetadata` 不变
- **AND** 行为 SHALL 与本 change 之前完全一致（含 `is_ongoing` 取值，仅内部实现改流式）
