# Design

## Context

`try_parse_queued_command`（`crates/cdt-parse/src/parser.rs`）把 `type:"attachment"` 且 `attachment.type=="queued_command"` 的条目解析成 user `ParsedMessage`。`RawAttachment.prompt` 原为 `Option<String>`，直接 `MessageContent::Text(prompt)`。真实数据中带图片的排队命令 prompt 是 content-block 数组，整行 serde 失败 → `MalformedLine` 丢弃。

`MessageContent`（cdt-core）本身是 `#[serde(untagged)]` 的 `Text(String) | Blocks(Vec<ContentBlock>)`，天然能从裸字符串或 block 数组反序列化。

## Decisions

### D1：复用 `MessageContent` 吃下两态，而非新建 enum 或 `serde_json::Value`

`prompt: Option<MessageContent>`。untagged 自动按 JSON 形态选 `Text`/`Blocks`，零额外类型，且 `content` 字段可直接赋值（同源类型）。

- 否决 `Option<serde_json::Value>` + 手动分支：要在 parser 里重写一遍 string/blocks 判别，等于复刻 `MessageContent` 的反序列化逻辑。
- 否决新建 `StringOrBlocks` untagged enum：与 `MessageContent` 完全同构，纯重复。

### D2：空判断按变体分流

`message_content_is_empty`：`Text` 看字符串 `is_empty()`，`Blocks` 看数组 `is_empty()`。保持原"prompt 为空则跳过"语义，覆盖空字符串与 `[]` 两种空。

### D3：`try_parse_queued_command` 返回类型 `Result<Option<_>>` → `Option<_>`

该函数从不产生 `ParseError`（全部是"识别不了就跳过"的 `None`）。改 schema 后 clippy `unnecessary_wraps` 触发，顺势收敛为 `Option`，调用点 `parse_entry_at` 包一层 `Ok(...)`。语义更准（识别失败 ≠ 解析错误）。

## Risks

- 风险：`content` 现在可能是 `Blocks`，下游消费 queued_command 的代码若假设永远 `Text` 会漏处理 block。缓解：queued_command 产出的 `ParsedMessage` 与普通 user 消息走同一渲染/分析路径，`MessageContent::Blocks` 已是全链路一等公民，无特殊假设。
- 风险：极少数 prompt 为其他 JSON 形态（如 object）仍会 serde 失败。可接受——此时确属异常数据，落到 `MalformedLine`（现已降级 debug）跳过。
