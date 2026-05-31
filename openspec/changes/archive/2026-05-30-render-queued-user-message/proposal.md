## Why

Claude Code 2.1.x 在 AI 一轮 turn 进行中用户排队输入的消息，以 `type:"attachment" + attachment.type:"queued_command"` 形式落盘 JSONL。当前 parser 不识别 `type:"attachment"` 条目（`parse_message_type` 返回 None → 整条跳过），导致用户的中途插话在回看时完全消失——"AI 为什么突然转向"的因果链断裂，审计价值受损。TS 原版同样未处理此格式（同源 coverage gap），不是 port 回退。

## What Changes

- session-parsing：识别 `type:"attachment"` 且 `attachment.type == "queued_command"` 的条目，提取 `attachment.prompt` 作为消息内容，映射为新的 `MessageCategory::User` 消息（保留 uuid / parentUuid / timestamp）。其余 attachment 子类型（`skill_listing` / `auto_mode` 等系统注入）继续跳过。
- chunk-building：把该消息作为 `SemanticStep::UserMessage` inline 嵌入当前 AIChunk（不 flush buffer、不打断 turn），放在精确时序位。
- session-display（前端）：用 BaseItem disclosure 渲染 `UserMessage` step，`svgIcon=MESSAGE_SQUARE`、`label="User"`、`summary=消息文本截断`、可展开查看全文。与 Output 行完全同结构，无状态标记。
- `queue-operation` 条目继续按 hard noise 过滤（它是同一条消息的重复排队日志，无 uuid）。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `session-parsing`：新增 attachment/queued_command 识别规则
- `chunk-building`：新增 `SemanticStep::UserMessage` variant + inline 嵌入逻辑
- `session-display`：新增 UserMessage step 渲染行

## Impact

- `crates/cdt-parse/src/parser.rs`：扩展 `parse_entry_at` 识别 attachment 条目
- `crates/cdt-core/src/message.rs`：`SemanticStep` enum 新增 `UserMessage` variant
- `crates/cdt-analyze/src/chunk/builder.rs`：主循环处理新分类
- `ui/src/routes/SessionDetail.svelte`：渲染 UserMessage step
- IPC payload：`SemanticStep` 序列化新增 variant，前端需识别
