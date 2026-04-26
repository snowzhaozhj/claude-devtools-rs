## ADDED Requirements

### Requirement: Render teammate messages embedded in AIChunk

SessionDetail 渲染 `AIChunk` 时 MUST 把 `chunk.teammateMessages` 作为 AIChunk 内部展示流的一类 DisplayItem 注入：每条 teammate message **按 `timestamp` 与其它 displayItems（thinking / text / tool / subagent / teammate_spawn）整体稳定排序穿插**——同 timestamp 保留 push 顺序。slash 命令仍排最前（与 AI turn 整体绑定，不参与时序排序）。

`replyToToolUseId` 字段 MUST 仅作为 teammate 卡片 header 的 reply chip 文本展示（"↪ reply"），**不**决定渲染位置——位置完全由 `tm.timestamp` 决定。这样即使没有 SendMessage 配对（teammate 主动发起回信、idle 通知等），卡片也按时序自然穿插，不会全部堆在 turn 末尾。

`displayItemBuilder` SHALL 把 teammate message 落点为 DisplayItem 类型 `{ type: "teammate_message", teammateMessage: TeammateMessage }`；`SessionDetail.svelte` 在 AIChunk 渲染流的 switch 内新增 `{:else if item.type === "teammate_message"}` 分支，渲染 `<TeammateMessageItem teammateMessage={item.teammateMessage} attachBody={...} rootSessionId={sessionId} />`。

`TeammateMessageItem.svelte` MUST 实现以下视觉契约：

1. **左侧 3px 彩色边**：颜色取自 `teammateMessage.color` 经 `getTeamColorSet(color)` 映射到 14 色调色板的 `border` 槽；缺失时退化到 `var(--color-border)`。
2. **Header 紧凑一行**：`color dot + teammate badge (teammateId, 同色系背景) + "Message" type label + summary 截断 (80 字符) + reply-to chip (CornerDownLeft icon + recipient/summary 简写) + token count (~Nk tokens 灰色) + chevron 折叠/展开`。
3. **默认折叠**：仅显示 header；用户点击 header 任意位置展开后渲染 markdown body（走 `attachMarkdown(body, "teammate")` 走 lazy markdown 管线）。
4. **噪声态极简**：`isNoise === true` 时 SHALL **不**渲染卡片框，仅渲染单行（`color dot + teammateId + body 单行截断`），`opacity: 0.45`，无展开/折叠。
5. **Resend 标记**：`isResend === true` 时 header 追加 RefreshCw icon + "Resent" 文案，整卡 `opacity: 0.6`。
6. **Token count 容错**：`tokenCount == null` 或 0 时 token 槽 SHALL 不渲染。
7. **Reply-to chip 容错**：`replyToToolUseId == null` 时 chip 槽 SHALL 不渲染。

`lazyMarkdown.svelte.ts` 的 `Kind` union MUST 加 `"teammate"` 分支（与 user / ai 同样走 `marked + highlight.js + DOMPurify` 管线）。

#### Scenario: Teammate messages interleave with other items by timestamp
- **WHEN** AIChunk 的 displayItems 时序为 `t=1 Read → t=2 Output(team已建) → t=3 SendMessage→alice → t=4 teammate(alice reply, replyTo=tu-send-alice) → t=5 Output(完毕)`
- **THEN** UI DisplayItem 顺序 SHALL 严格按 timestamp 升序排列：`Read → Output(team已建) → SendMessage→alice → TeammateMessageItem(alice) → Output(完毕)`——teammate 卡片**因 timestamp** 紧贴 SendMessage，不依赖 reply_to 配对

#### Scenario: Multiple teammate replies interleave by timestamp
- **WHEN** AIChunk 时序：`t=1 SendMessage→alice → t=2 SendMessage→bob → t=3 alice reply → t=4 bob reply → t=5 Output`
- **THEN** UI 顺序 SHALL 为 `SendMessage→alice → SendMessage→bob → TeammateMessageItem(alice) → TeammateMessageItem(bob) → Output`——按时序，**不**因 reply_to 把 alice reply 强行拉到 alice 的 SendMessage 之后

#### Scenario: Teammate without reply_to interleaves naturally
- **WHEN** AIChunk 含 `[t=1 Output, t=2 teammate(member-1, replyToToolUseId=null), t=3 Output]`（teammate 主动发起回信，无 SendMessage 配对）
- **THEN** UI 渲染 SHALL 为 `Output → TeammateMessageItem(member-1) → Output`——按 timestamp 穿插，**不**追加到 turn 末尾

#### Scenario: replyToToolUseId only affects chip text not position
- **WHEN** TeammateMessageItem 渲染时 `replyToToolUseId === "tu-x"`
- **THEN** 卡片 header SHALL 显示 reply chip（"↪ reply"），但卡片位置 SHALL 由 `timestamp` 决定，与 `tu-x` 在 displayItems 中的位置无关

#### Scenario: Noise teammate renders as minimal inline row
- **WHEN** teammate `isNoise === true`
- **THEN** SHALL 渲染单行（color dot + teammateId + body 截断），`opacity: 0.45`，SHALL NOT 渲染卡片框 / chevron / 展开区

#### Scenario: Resend teammate rendered with refresh badge and dimmed
- **WHEN** teammate `isResend === true` 且 `isNoise === false`
- **THEN** SHALL 渲染完整卡片，header 追加 RefreshCw icon + "Resent" 文案，整卡 `opacity: 0.6`

#### Scenario: Markdown body renders via lazy pipeline
- **WHEN** 用户首次展开一个 TeammateMessageItem（非 noise），body 含围栏代码块
- **THEN** 展开区 SHALL 通过 `attachMarkdown(body, "teammate")` 触发懒加载 markdown 渲染，含 highlight.js 语法高亮与 DOMPurify XSS 过滤；视口外的 teammate 卡片 SHALL 不消耗 markdown 渲染时间
