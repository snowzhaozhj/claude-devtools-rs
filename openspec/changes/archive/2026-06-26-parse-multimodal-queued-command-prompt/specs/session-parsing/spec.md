# session-parsing spec delta

## MODIFIED Requirements

### Requirement: Recognize queued_command attachment as user message

系统 SHALL 识别 `type:"attachment"` 且 `attachment.type == "queued_command"` 的 JSONL 条目，将其解析为 `ParsedMessage`：
- `message_type` = `User`
- `category` = `User`
- `content` 取自 `attachment.prompt`，SHALL 同时支持两种形态：纯文本字符串解析为 `MessageContent::Text`，多模态 content-block 数组（如 `[{type:text}, {type:image}]`，带图片的排队命令）解析为 `MessageContent::Blocks`
- `uuid` / `parent_uuid` / `timestamp` 取条目原有字段
- `is_queued_input` = `true`
- `is_meta` = `false`
- `is_sidechain` = `false`

其余 attachment 子类型（`attachment.type` 非 `"queued_command"`）SHALL 继续返回 `Ok(None)` 跳过。

`attachment.prompt` 为空（空字符串 **或** 空 content-block 数组）或缺失时 SHALL 返回 `Ok(None)` 跳过。

`queue-operation` 条目的 hard noise 分类不变。

#### Scenario: Attachment with queued_command (text prompt) is parsed as user message
- **WHEN** JSONL 条目 `type == "attachment"` AND `attachment.type == "queued_command"` AND `attachment.prompt` 为非空字符串
- **THEN** `parse_entry_at` 返回 `Ok(Some(ParsedMessage))` with `category == User` AND `is_queued_input == true` AND `content == MessageContent::Text(prompt)`

#### Scenario: Attachment with queued_command (multimodal prompt) is parsed as blocks
- **WHEN** JSONL 条目 `type == "attachment"` AND `attachment.type == "queued_command"` AND `attachment.prompt` 为非空 content-block 数组（含 image 块）
- **THEN** `parse_entry_at` 返回 `Ok(Some(ParsedMessage))` with `category == User` AND `is_queued_input == true` AND `content == MessageContent::Blocks(...)` 保留全部块（不丢弃、不报 `MalformedLine`）

#### Scenario: Attachment with non-queued_command type is skipped
- **WHEN** JSONL 条目 `type == "attachment"` AND `attachment.type != "queued_command"`（如 `skill_listing` / `auto_mode`）
- **THEN** `parse_entry_at` 返回 `Ok(None)`

#### Scenario: Attachment with empty or missing prompt is skipped
- **WHEN** JSONL 条目 `type == "attachment"` AND `attachment.type == "queued_command"` AND `attachment.prompt` 为空字符串、空数组 `[]`、或缺失
- **THEN** `parse_entry_at` 返回 `Ok(None)`

#### Scenario: queue-operation remains hard noise
- **WHEN** JSONL 条目 `type == "queue-operation"`
- **THEN** 分类不变，仍为 `HardNoise(NonConversationalEntry)`
