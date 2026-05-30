## Context

Claude Code JSONL 里用户在 AI turn 进行中输入的消息以两种形式存在：

1. `type:"queue-operation"` (enqueue/remove) — 无 uuid 的排队日志，已被 hard noise 过滤，继续如此。
2. `type:"attachment"` + `attachment.type:"queued_command"` — 有 uuid + parentUuid、在 DAG 有确定位置、`attachment.prompt` 含用户文本。当前 parser 因不识别 `"attachment"` type 而整条跳过。

真实 DAG 链证明：attachment 条目的 parentUuid 指向它被注入时 AI 正在处理的 tool_result，后续 assistant 直接 `parent=该 attachment`，说明 AI 把它纳入同一执行链继续。它不是"下一个 turn 的开头"，是 **turn 内部的 inline 事件**。

## Goals / Non-Goals

**Goals:**
- 让 queued_command 在回看时可见，恢复"用户为什么插话 → AI 为什么转向"的因果链
- 视觉上不打断 AI turn 的连续性——嵌入而非分割
- 复用现有 BaseItem disclosure 结构，零新 UI 组件/样式

**Non-Goals:**
- 不处理其它 attachment 子类型（skill_listing / auto_mode 等系统注入，继续跳过）
- 不改变 queue-operation 的 hard noise 分类
- 不改动 UserChunk 产出逻辑——queued_command 不走 `handle_user`，不 flush buffer
- 不为 queued_command 引入新颜色 / 新 icon / 新 CSS

## Decisions

**D1：parser 层识别 attachment 但不走 MessageType 路径。**

当前 `parse_message_type` 把 `"attachment"` 映射为 None → 跳过。修法：在 `parse_entry_at` 函数内，`parse_message_type` 返回 None 时追加一条检查——若 raw entry 的 `type == "attachment"` 且 `attachment.type == "queued_command"`，则从 `attachment.prompt` 构造 `ParsedMessage`（`message_type=User`、`category=User`、`is_meta=false`、`is_sidechain=false`），使用条目原有的 uuid/parentUuid/timestamp。其余 attachment 子类型仍 `Ok(None)` 跳过。

理由：不引入新 MessageType variant（attachment 不是新"消息类型"，它只是被错误跳过的一种 user 消息），复用现有 User 类型链路最简。

**D2：chunk-building 新增 `SemanticStep::UserMessage` 但不改主循环分发。**

queued_command 被 parser 识别后，以 `MessageCategory::User` 进入 chunk-building 主循环的 `handle_user`。但它不是 tool_result、不是 slash、不是 local-command-stdout、不是 teammate-message，会走到 else 分支（flush + 产 UserChunk）—— **这会打断 turn**。

修法：在 `handle_user` 内，产 UserChunk 的 else 分支之前，检查该消息是否有 `is_queued_input: true` 标记（parser 层设置）。命中时：不 flush、不产 UserChunk，而是把它记入 `pending_user_messages: Vec<PendingUserMessage>`（含 uuid + timestamp + text）。下一次 flush AIChunk 时，把 pending 按 timestamp 插入 `semantic_steps` 的精确位置（在它 timestamp 之后第一个 tool_use step 之前）。

为此 `SemanticStep` enum 新增：
```rust
UserMessage { uuid: String, text: String, timestamp: DateTime<Utc> }
```

理由：inline 嵌入 = 不打断 turn + 保留审计时序，符合 DESIGN.md "Ongoing / interruption 等状态应嵌入现有消息流或 slot"。

**D3：前端渲染复用 BaseItem，与 Output 行同结构。**

SessionDetail.svelte 在遍历 `chunk.semanticSteps` 时，对 `kind === "user_message"` 渲染：
```svelte
<BaseItem
  svgIcon={MESSAGE_SQUARE}
  label="User"
  summary={text.length > 60 ? text.slice(0, 60) + "…" : text}
  isExpanded={expandedItems.has(key)}
  onclick={() => toggle(key)}
>
  {#snippet children()}
    <div class="prose lazy-md" {@attach attachMarkdown(text, "output")}></div>
  {/snippet}
</BaseItem>
```

零新 CSS class、零新 icon、零新颜色。与 Output 行唯一差异：label 从 "Output" 变为 "User"、无 tokenCount。

**D4：`ParsedMessage` 新增 `is_queued_input: bool` 标记字段。**

parser 层设置 `true` 仅对 queued_command attachment；其余消息 `false`。chunk-building 据此区分"正常 user 消息"（产 UserChunk）和"排队插话"（inline embed）。

用 `#[serde(default, skip_serializing)]` 修饰——该字段只在 Rust 内部传递，不暴露给 IPC/前端。

## Risks / Trade-offs

- **时序精度**：attachment 的 timestamp 是出队时间（不是用户输入时间），但这是 JSONL 里唯一可用的时间戳，足以确定"在哪两个工具调用之间"。
- **多条连续 queued_command**：各自独立产 `SemanticStep::UserMessage`，按 timestamp 排序不合并。如果用户在同一秒发了两条，顺序由 JSONL 行序保证（顺序遍历）。
- **前端兼容**：老版前端遇到未知 `kind:"user_message"` step 会跳过（`{#each}` 无匹配分支则不渲染），不 crash。
