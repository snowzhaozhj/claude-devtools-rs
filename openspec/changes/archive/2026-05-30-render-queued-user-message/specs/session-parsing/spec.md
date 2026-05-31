## ADDED Requirements

### Requirement: Recognize queued_command attachment as user message

系统 SHALL 识别 `type:"attachment"` 且 `attachment.type == "queued_command"` 的 JSONL 条目，将其解析为 `ParsedMessage`：
- `message_type` = `User`
- `category` = `User`
- `content` = `MessageContent::Text(attachment.prompt)`
- `uuid` / `parent_uuid` / `timestamp` 取条目原有字段
- `is_queued_input` = `true`
- `is_meta` = `false`
- `is_sidechain` = `false`

其余 attachment 子类型（`attachment.type` 非 `"queued_command"`）SHALL 继续返回 `Ok(None)` 跳过。

`queue-operation` 条目的 hard noise 分类不变。

#### Scenario: Attachment with queued_command is parsed as user message
- **WHEN** JSONL 条目 `type == "attachment"` AND `attachment.type == "queued_command"` AND `attachment.prompt` 非空
- **THEN** `parse_entry_at` 返回 `Ok(Some(ParsedMessage))` with `category == User` AND `is_queued_input == true` AND content 为 prompt 文本

#### Scenario: Attachment with non-queued_command type is skipped
- **WHEN** JSONL 条目 `type == "attachment"` AND `attachment.type != "queued_command"`（如 `skill_listing` / `auto_mode`）
- **THEN** `parse_entry_at` 返回 `Ok(None)`

#### Scenario: Attachment without prompt is skipped
- **WHEN** JSONL 条目 `type == "attachment"` AND `attachment.type == "queued_command"` AND `attachment.prompt` 为空或缺失
- **THEN** `parse_entry_at` 返回 `Ok(None)`

#### Scenario: queue-operation remains hard noise
- **WHEN** JSONL 条目 `type == "queue-operation"`
- **THEN** 分类不变，仍为 `HardNoise(NonConversationalEntry)`
