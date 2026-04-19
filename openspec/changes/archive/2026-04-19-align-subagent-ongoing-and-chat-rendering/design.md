# Design

## 决策

### 1. Subagent `is_ongoing` 走 `check_messages_ongoing`，resolver 层保留 OR 兜底

**方案**：装载层 `parse_subagent_candidate` 在 `parse_file(path)` 后立即跑 `cdt_analyze::check_messages_ongoing(&msgs)`，把结果写入 `SubagentCandidate.is_ongoing`；resolver 的 `compute_is_ongoing(cand) = cand.is_ongoing || cand.end_ts.is_none()` 保留——装载层判 true 时强制 ongoing，判 false 时仍允许 `end_ts=None` 兜底（parse 失败 / 空 session 等 edge case）。

**为什么不改 resolver**：resolver 拿到的 `SubagentCandidate.messages` 已是 `Vec<Chunk>` 不是 `Vec<ParsedMessage>`，`check_messages_ongoing` 签名是 `&[ParsedMessage]`。让装载层算好传下来，resolver 就不需要重新解析；OR 逻辑也确保即便将来某个 candidate 来源没算 `is_ongoing`，`end_ts=None` 仍能兜底判 ongoing。

### 2. AI header token 只取 last response usage，不动后端 `aggregate_metrics`

**方案**：前端 `SessionDetail.svelte` 的 AI chunk 分支新增 `{@const lastUsage = [...chunk.responses].reverse().find(r => r.usage)?.usage ?? null}`，用 `lastUsage` 四项算总和显示 header；后端 `ChunkMetrics.inputTokens/outputTokens/cacheCreation/cacheRead` 仍按 `aggregate_metrics` 累加（其他地方可能依赖累加语义，如 Waterfall / dashboard）。

**为什么前端算而不改后端**：`ChunkMetrics` 的累加语义被 `Waterfall` / `context-tracking` / `http-data-api` 等跨组件使用；贸然改成 last-only 会波及未知调用方，需要更大范围 regression。前端 AI header 是独立的视觉展示，`chunk.responses` 已含逐条 usage 数据，前端取最后一条成本最低且语义精确（"turn 结束时的 context snapshot"）。

**取 last 而非取 max**：原版 AIChatGroup.tsx 行 234 注释明确"Get the LAST assistant message's usage"。Anthropic API 每次 call 的 `cache_read_input_tokens` 都是"整段历史已缓存部分"；一个 turn 里多次 tool_use 的 API call 都会含"从 session 开头到当前"的 cache size，累加会重复计数同一段历史 N 次。取 last 恰好等于 turn 结束时的完整 context snapshot。

### 3. task-notification 卡片移植原版 `UserChatGroup.tsx:484-536`

**方案**：`cleanDisplayText` 保持现状（洗掉 `<task-notification>` 整段 XML）；新增 `parseTaskNotifications(content)` 单独抽取 taskId / status / summary / outputFile 四字段；UI 渲染条件改为 `text || images.length > 0 || taskNotifications.length > 0`。

**为什么不改 `cleanDisplayText`**：task-notification XML 如果混在正文里展示出来非常丑（多行 `<task-id>`/`<tool-use-id>`/`<summary>` 等嵌套标签）；洗掉文本然后卡片化渲染是原版已验证的最佳方案。

### 4. MetricsPill / Bot / 工具 row 纯视觉对齐，不涉及行为契约

这三条改动是"对齐原版视觉"——`MetricsPill` 去 `slot-label` 改 `·`→`|`、`Bot` 单 path 换多 path/rect、`BaseItem` token 加 "tokens" 后缀。它们不是行为契约（不影响数据流 / IPC / 语义），本 change 不把它们写进 spec——CLAUDE.md 明确"样式修复单点改动"不走 openspec。

## Alternatives considered

**A. 改后端 `aggregate_metrics` 只取 last response** — 语义最精确，但影响面广（Waterfall / context-tracking / HTTP API 都读 `ChunkMetrics`）。放弃。

**B. 通过 `title=` HTML 原生 tooltip 代替自定义 popover** — 实现最小，但浏览器原生 tooltip 延迟 1s+ 且样式难控，用户反馈不直观。改走 Info icon + 自定义卡片（hover 立即显示）。

**C. 在 `cleanDisplayText` 里把 task-notification XML 转换为 markdown 表格** — 复杂度高且定制样式受限。放弃，走原版卡片组件路径。
