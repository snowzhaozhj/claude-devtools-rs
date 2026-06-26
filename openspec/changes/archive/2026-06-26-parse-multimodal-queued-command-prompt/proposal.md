# Accept multimodal queued_command prompts

## Why

排查 0.7.0 release CLI 跑 `stats` 刷 321 行日志时，发现其中的 `malformed JSONL line` WARN 是真 bug：带图片的排队命令（queued_command attachment）其 `attachment.prompt` 是多模态 content-block 数组（`[{type:text}, {type:image}]`），而解析层把 `prompt` 定成 `Option<String>`，导致整行 serde 解析失败、被当作 `MalformedLine` 整条丢弃 + 刷 warn。

这违反 session-parsing 现有 Requirement "Recognize queued_command attachment as user message" 的契约意图——该 Requirement 要求识别 queued_command attachment 并解析为 `ParsedMessage`，但 multimodal prompt 反而连 entry 都丢了。现有 spec bullet 把 `content` 写死为 `MessageContent::Text(attachment.prompt)`，只覆盖了字符串形态。

## What Changes

- 放宽 queued_command attachment 的 `prompt` 字段：从仅接受字符串扩展到接受字符串**或** content-block 数组。
- `content` 字段忠实反映 `prompt` 的实际形态——纯文本 → `MessageContent::Text`，多模态 → `MessageContent::Blocks`。
- 空判断同时覆盖空字符串与空 blocks 数组（仍跳过）。

行为契约影响 capability：**session-parsing**。

（同 PR 内还含不触碰 spec 的日志卫生改动——CLI 默认静默 + warn→debug 级别修正 + CLAUDE.md 纪律，详 tasks.md，不进 spec delta。）

## Impact

- Affected spec: `session-parsing`（MODIFIED 一个 Requirement）
- Affected code: `crates/cdt-parse/src/parser.rs`（`RawAttachment.prompt`、`try_parse_queued_command`）
- 向后兼容：字符串 prompt 行为不变；此前被错误丢弃的 multimodal prompt 现在正确解析，无破坏性。
