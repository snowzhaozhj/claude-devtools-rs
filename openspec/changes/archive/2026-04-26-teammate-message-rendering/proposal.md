## Why

当前 multi-agent / team session 在 UI 上有两个体验断层：

1. **Sidebar 标题污染**：teammate session 的标题直接吐出原始 XML（如 `<teammate-message teammate_id="...">`），不可读。根因：`cdt-api::session_metadata::sanitize_for_title` 的噪声 tag 白名单里没有 `teammate-message`。
2. **主面板看不到队友发言**：`cdt-analyze::chunk::builder` 在 `MessageCategory::User` 分支检测到 teammate 消息后直接 `continue` 整条丢弃，IPC 层根本不暴露这部分数据，前端无法呈现"队友 → 主线"的回复内容。原版 TS（`TeammateMessageItem.tsx` + `displayItemBuilder.ts::linkTeammateMessagesToSendMessage`）把 teammate 消息嵌入到触发它的 AI turn 内部展示流，能让用户一眼看出"哪条 SendMessage 收到什么回复"——这部分行为缺失。

## What Changes

- **BREAKING**（IPC 字段新增）：`AIChunk` 新增 `teammateMessages: TeammateMessage[]` 字段，承载嵌入到该 turn 的队友回复。每条携带 `teammateId / color / summary / body / timestamp / replyToToolUseId / tokenCount / isNoise / isResend`。前端按 `timestamp` 把卡片与其它 displayItems 整体稳定排序穿插（详见下方"渲染流"）。
- **BREAKING**（IPC 字段新增）：`ToolExecution` 新增 `teammateSpawn?: TeammateSpawnInfo` 字段。pair 阶段从 `tool_result.toolUseResult.status == "teammate_spawned"` 抽 `name` / `color`，前端检测到非空时把整条 tool item 替换为 `teammate_spawn` 极简单行（圆点 + member-X badge + "Teammate spawned"），对齐原版 `LinkedToolItem.tsx::isTeammateSpawned`。
- **chunk-building 行为契约修订**：`is_teammate_message` 不再无条件 `continue` 丢弃；改为转化为 `TeammateMessage` 注入到下一个 flush 出的 `AIChunk.teammateMessages`。一条 user 消息含 N 个 `<teammate-message>` 块时（如 team 启动阶段把多个 idle_notification 拼到同一条消息），用 `parse_all_teammate_attrs` global regex 各自产独立卡片（uuid 加 `-{idx}` 后缀去重）。teammate 消息**仍不**产 `UserChunk`。`reply_to_tool_use_id` 仍按"向前找最近未配对 `SendMessage`，跨 AIChunk 回溯上限 3 个"配对，但**仅作为 chip 显示**，不决定渲染位置。
- **session 标题清洗修复**：`sanitize_for_title` 把 `teammate-message` 加入剥离标签集合，并优先取 `summary=` 属性作为标题候选；都没有时回退到 body 前 100 字（不再吐 XML）。
- **前端新增 `TeammateMessageItem.svelte`**：左侧 3px 彩色边卡片，头部 `color dot + teammate badge + Message label + summary 截断 + reply-to chip + token 估算 + timestamp`，默认折叠，展开走现有 lazyMarkdown 管线（新增 `kind: "teammate"`）。运维噪声 (`idle_notification` / `shutdown_request` / `shutdown_approved` / `teammate_terminated`) 渲染成灰色单行不开卡。检测到 resend 关键词加 RefreshCw 图标 + 透明度 0.6。
- **`SessionDetail.svelte` 渲染流改造**：AIChunk 内的 displayItemBuilder 把 `teammate_message` / `teammate_spawn` 与其它 displayItems（thinking / text / tool / subagent）按各自 `timestamp` 整体稳定排序穿插，slash 仍排最前。`replyToToolUseId` 仅控制 chip 文本，不决定位置（对齐原版 `displayItemBuilder.ts::sortDisplayItemsChronologically`）。这样即便 teammate 主动发言无 SendMessage 配对，卡片也按时序自然穿插，不会全部堆在 turn 末尾。`buildSummary` 同步对齐：`process.team` 非空的 subagent 与 `teammate_spawn` 按 unique `memberName` 计入 "N teammates"，`teammate_message` 计为 "N teammate messages"。

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `chunk-building`: 新增 Requirement"Teammate messages embed into AIChunk"——teammate user 消息不再丢弃，转化为 AIChunk 子项；同时 MODIFY 既有的"Filter teammate messages from user chunks"语义为"不产 UserChunk 但保留为 AIChunk 子项"。
- `team-coordination-metadata`: MODIFY"Render teammate messages as dedicated items"——细化 reply-to 配对规则、噪声/resend 检测契约；ADD"Parse all teammate-message blocks from one user message"——多 block 解析（global regex）。
- `tool-execution-linking`: ADD"Detect teammate-spawned tool results"——pair 阶段从 user msg `toolUseResult.status` 抽 `teammate_spawned` 的 name/color 落到 `ToolExecution.teammate_spawn`，对齐原版 `LinkedToolItem.tsx::isTeammateSpawned`。
- `ipc-data-api`: MODIFY `AIChunk` 字段集，新增 `teammateMessages` 数组及 `TeammateMessage` 子类型契约；新增 `ToolExecution.teammateSpawn` 字段。
- `session-display`: 新增 Requirement"Render teammate messages embedded in AIChunk"——前端把 teammate 卡片按 timestamp 与其它 displayItems 整体排序穿插（不依赖 reply_to 配对决定位置）；teammate_spawn 渲染极简单行替代普通 tool item。

## Impact

- **代码**：
  - `crates/cdt-analyze/src/chunk/builder.rs`：teammate 消息处理分支重写；新增 `pending_teammates` 状态机。
  - `crates/cdt-analyze/src/team/`：新增 `link_teammates_to_send_message` reply-to 配对函数；新增 noise / resend 检测函数。
  - `crates/cdt-core/src/chunk.rs`（或同等位置）：`AIChunk` 结构新增 `teammate_messages` 字段；新增 `TeammateMessage` 结构。
  - `crates/cdt-api/src/ipc/session_metadata.rs`：`sanitize_for_title` 修复 + 优先取 `summary=` 属性。
  - `crates/cdt-api/src/ipc/types.rs` / 相关序列化：`AIChunk` IPC schema 透传新字段。
  - `ui/src/lib/api.ts`：`AIChunk` 类型加 `teammateMessages`；`Chunk` union 不变。
  - `ui/src/components/TeammateMessageItem.svelte`（新建）。
  - `ui/src/routes/SessionDetail.svelte`：AIChunk 渲染流插入 teammate 卡片。
  - `ui/src/lib/lazyMarkdown.svelte.ts`：`Kind` 加 `"teammate"`。
- **测试**：
  - `cdt-analyze` 单元测：teammate 不产 UserChunk + 注入 AIChunk + reply-to 配对 + orphan 处理 + noise 不进 markdown body。
  - `cdt-api` 集成测：`get_session_detail` 对 fixture 输出 `teammateMessages` 数组结构正确；`session_metadata` title 清洗 teammate 标签。
- **依赖**：无新增 crate 依赖（regex 已在用）。
- **回滚开关**：在 `chunk::builder` 顶部加 `const EMBED_TEAMMATES: bool = true;` 一行开关，关掉等价于回退到旧"丢弃"行为，便于单边回退。
