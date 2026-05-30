## ADDED Requirements

### Requirement: Inline embed queued user message as SemanticStep

系统 SHALL 在 chunk-building 时，对 `is_queued_input == true` 的 user 消息：
- 不 flush assistant buffer
- 不产出 UserChunk
- 不清除 pending_slashes
- 将其记录为 pending，在下一次 flush AIChunk 时作为 `SemanticStep::UserMessage { uuid, text, timestamp }` 插入 `semantic_steps` 序列的精确时序位置

时序位置定义：在 pending 记录的 timestamp 之后、第一个 timestamp 更晚的其它 step 之前；若无更晚 step 则追加到末尾。

连续多条 queued_command SHALL 各自产独立 `UserMessage` step，按 timestamp（进而行序）排列，不合并。

末尾 flush 时仍有 pending user messages 的，追加到最后一个 AIChunk 的 semantic_steps 末尾。无 AIChunk 时丢弃（与 orphan teammate 同策略）。

#### Scenario: Queued input does not flush buffer or produce UserChunk
- **WHEN** chunk-building 主循环遇到 `category == User` AND `is_queued_input == true`
- **THEN** assistant buffer 不 flush AND 无 UserChunk 产出 AND pending_slashes 不清除

#### Scenario: UserMessage step appears at correct timeline position
- **WHEN** 用户在 tool_use A（ts=1）和 tool_use B（ts=3）之间发送 queued_command（ts=2）
- **THEN** flush 产出的 AIChunk.semantic_steps 序列中 `UserMessage(ts=2)` 位于 `ToolExecution(A)` 之后、`ToolExecution(B)` 之前

#### Scenario: Multiple queued inputs produce multiple steps
- **WHEN** 同一 AI turn 内出现 2 条 queued_command（ts=2, ts=4）
- **THEN** AIChunk.semantic_steps 含 2 条独立 `UserMessage`，按 timestamp 排序

#### Scenario: Trailing queued input attaches to last AIChunk
- **WHEN** 文件末尾有 queued_command 且无后续 assistant 消息（buffer 空）
- **THEN** 该 UserMessage step 追加到最后一个已 emit 的 AIChunk 的 semantic_steps 末尾

#### Scenario: Orphan queued input without any AIChunk is dropped
- **WHEN** 全文件仅有 queued_command 无任何 assistant 消息
- **THEN** 不产出任何 chunk（静默丢弃）
