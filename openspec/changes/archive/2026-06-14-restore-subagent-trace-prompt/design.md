## Context

SubAgent 的 ExecutionTrace 由 `buildDisplayItemsFromChunks(chunks: Chunk[])`（`ui/src/lib/displayItemBuilder.ts`）从 subagent 的 `Process.messages: Chunk[]` 构建 DisplayItem 流。数据链路：

1. 后端 `cdt-api::local::candidate_to_process` 把 subagent JSONL 解析为 `ParsedMessage[]`，清掉 `is_sidechain` 后调 `cdt_analyze::build_chunks`。
2. subagent 首条消息是父会话给它的 prompt（纯文本、非 meta、非 tool_result、非 slash、非 teammate），在 `chunk/builder.rs::handle_user` 末尾 else 分支产出一个独立 `Chunk::User(UserChunk)`。
3. 前端 `buildDisplayItemsFromChunks` 遍历 chunks，当前实现 `if (c.kind !== "ai") continue` —— 把这个 UserChunk 丢弃，prompt 永远不进 trace。

注释声称"subagent 内部的 user 消息通常是 tool_result，已由 tool item 覆盖"。该前提在 chunk 模型下不成立：`is_tool_result_only` 的 user 消息在 builder 已被 merge 进前一个 assistant buffer（`builder.rs` 308-313），**不会**产出 UserChunk。因此残留在 subagent chunk 流里的 UserChunk 一定是真实用户输入。

原版 TS `displayItemBuilder.ts::buildDisplayItemsFromMessages`（426-438）对非 meta、无 tool_result 的 user 消息产 `type: 'subagent_input'` DisplayItem，port 时该路径整体缺失。

## Goals / Non-Goals

**Goals:**
- subagent ExecutionTrace 展开时显示父会话给它的 prompt（及任何真实 user 输入）。
- 不影响主会话视图、不引入重复渲染、不改后端 / IPC。

**Non-Goals:**
- 不改后端 chunk-building / UserChunk 产出逻辑（数据已正确，问题纯在前端展示）。
- 不展示 `SystemChunk`（local-command-stdout）/ `CompactChunk`——超出本次范围，subagent 场景几乎不出现。
- 不新建 `subagent_input` 专用 DisplayItem 类型——复用已有 `user_message`。

## Decisions

### D1：复用 `user_message` DisplayItem，不新建 `subagent_input`
`user_message` 类型（`displayItemBuilder.ts:75-79`）已存在、已被主视图 `SessionDetail.svelte:1279` 渲染，语义就是"用户文本输入"。复用它避免引入新类型 + 新渲染样式 + 新 summary 分支。

**Alternative（拒绝）**：照搬 TS 新建 `subagent_input` 类型——port 已统一用 `user_message` 承载 queued_command 的 `SemanticStep::UserMessage`，再加一个语义重叠类型徒增维护面。

### D2：slash UserChunk 显式跳过，避免重复渲染
后端 `handle_user` 对 slash 消息（`<command-name>/x</command-name>`）**同时**产 UserChunk **和**把 slash 信息挂到下一个 AIChunk 的 `slash_commands`（`builder.rs` 276-295）。若无差别把 UserChunk 转 user_message，slash 会既渲染成 user_message 又渲染成 slash item → 重复。

修法：`c.kind === "user"` 分支用已导出的 `extractSlashInfo(raw) === null` 判定——命中 slash 则跳过（交给 AIChunk 的 slash item 渲染）。

`cleanDisplayText` 对 slash 返回 `/name args`（非空），所以单纯"清洗后非空"的 guard **不足以**跳过 slash，必须显式判定。codex 二审确认此为必修硬伤。

**Alternative（拒绝）**：检查"下一个 AIChunk 是否有 slash_commands"——需跨 chunk 配对，比直接判 UserChunk 内容脆弱。

### D3：文本提取走 string | ContentBlock[] 双形态
`UserChunk.content` 是 `string | ContentBlock[]`。提取首个 text（string 直接取、Blocks 取第一个 `type==="text"` 的 `.text`），再过 `cleanDisplayText` 清洗 system-reminder / 不可见控制符噪声。清洗后为空则不产 item（对齐 `session-display` spec 既有的 "UserChunk 文本清洗后为空不渲染" 语义）。

### D4：system / compact chunk 仍跳过
保持现状——本次只补 UserChunk。SystemChunk（local-command-stdout）/ CompactChunk 在 subagent 场景几乎不出现，纳入会扩大 scope 与回归面，留待需要时单独处理。

## Risks / Trade-offs

- [多个 UserChunk 全部渲染] → 对齐 TS 原版行为（多轮真实 user 输入都该显示）；subagent 实际几乎只有首条 prompt 一个 UserChunk，风险极低。
- [`buildSummary` 不计 user_message] → switch 无 default 也无 user_message case，user_message 不计入摘要但不 crash；prompt 不计入 "N tools · M messages" 摘要可接受（TS 也未把 subagent_input 计入摘要）。
- [性能] → `buildDisplayItemsFromChunks` 仅在 `isExpanded` 时 `$derived` 重算，多一个 item 可忽略，无 hot path 影响。
- [import 环] → 从 `./toolHelpers` 引入 `extractSlashInfo` / `cleanDisplayText`；toolHelpers 不反向依赖 displayItemBuilder，无循环。
