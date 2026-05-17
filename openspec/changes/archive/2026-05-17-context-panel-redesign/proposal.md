## Why

当前 Context Panel 把后端 6 类 `ContextInjection` 全部压扁成简陋的 `ContextEntry`（label + preview + estimatedTokens），丢失了 `turnIndex` / `aiGroupId` / `toolBreakdown` / `thinkingTextBreakdown` / `taskCoordinationBreakdown` 等关键字段。用户反馈"点开看不明白这是什么东西"——既看不到"哪个 turn 哪个 tool 贡献了多少 token"，也无法点击跳转到对应 AIChunk。TS 原版 `SessionContextPanel` 有 6 个独立 Section 模板 + turn 锚点导航 + phase 切换 + tool 级 breakdown，本 change 把这套体验移植过来。

## What Changes

- 重构前端 `contextExtractor.ts`：移除 `ContextEntry` 压扁逻辑，直接保留后端 6 类判别联合（`ClaudeMdInjection` / `MentionedFileInjection` / `ToolOutputInjection` / `ThinkingTextInjection` / `TaskCoordinationInjection` / `UserMessageInjection`）与各自 breakdown 字段
- 把 `ContextPanel.svelte` 拆出 6 个 Section 子组件（`UserMessagesSection` / `ClaudeMdFilesSection` / `MentionedFilesSection` / `ToolOutputsSection` / `TaskCoordinationSection` / `ThinkingTextSection`），每个 Section 用专属模板呈现关键字段
- `ToolOutputsSection` 展开后 SHALL 显示 `toolBreakdown` 每个 tool 一行（name + token + error 标记），点击单条 SHALL 跳转到对应 `toolUseId` 锚点
- `ThinkingTextSection` SHALL 拆开显示 thinking / text 各自 token；`TaskCoordinationSection` SHALL 拆开显示 SendMessage / TeamCreate / TaskCreate / TeammateMessage breakdown
- `ClaudeMdFilesSection` SHALL 按 `scope` 分三组 Global（enterprise + user）/ Project / Directory，组内保留现有 DirectoryTree 展示
- 新增 turn 锚点导航：`SessionDetail.svelte` 给每个 `AIChunk` / `UserChunk` 容器加 `data-chunk-id` 属性；ContextPanel 各 Section 通过 `onNavigateToChunk(chunkId)` 回调触发 `scrollIntoView`；点击 `toolUseId` SHALL 先滚到 chunk，再滚到 tool 子节点
- **BREAKING**（内部契约）：后端 `cdt-analyze::context::session::ai_chunk_id` SHALL 改为直接复用 `AIChunk.chunk_id`（形如 `ai:<uuid>:<n>`），不再用 `responses[0].uuid` 或 `ai-<turn_index>` fallback。这让 `ContextInjection.aiGroupId` 与前端 `AIChunk.chunkId` 字节级相等，前端可直接用 aiGroupId 定位 DOM
- 新增 Phase Selector：当 `ContextPhaseInfo.phases.length > 1` 时 SHALL 在 Header 显示下拉，选中某 phase 时 SHALL 只展示该 phase 范围内的 injections（默认 latest phase）
- 视图模式保留：Category（默认）/ Ranked；Ranked 视图增加 Grouped（按 category 颜色分块）/ Flat（纯按 token 排）子切换以对齐 TS 原版

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `session-display`: Context Panel 改为 6-section 分类模板；新增 turn 锚点导航行为；新增 Phase Selector；Ranked 视图新增 Grouped/Flat 子模式
- `context-tracking`: `ContextInjection.aiGroupId` 字段语义对齐 `AIChunk.chunkId`（不再是 `responses[0].uuid` 或 `ai-<turn_index>` fallback）；新增 `SessionDetail.phaseInfo` + `injectionsByPhase` IPC 字段暴露 per-phase injections
- `ipc-data-api`: `chunkId` 形态统一为 `<base>:<n>` 跨所有 chunk 类型（删除 AI 的 `ai:` 前缀 / 删除 non-AI "首次裸 uuid" 特例），collision-free 兜底逻辑保持不变

## Impact

- **后端**：`cdt-analyze::context::session::ai_chunk_id` 单点修改 + 相关测试断言更新（`context_tracking.rs` 等）；`crates/cdt-api/tests/ipc_contract.rs` 若有 aiGroupId 断言需同步
- **前端**：`ui/src/lib/contextExtractor.ts` 重写；`ui/src/components/ContextPanel.svelte` 拆 6 个 Section 文件到 `ui/src/components/contextPanel/`；`ui/src/routes/SessionDetail.svelte` 加 `data-chunk-id` 锚点 + chunk-scroll 调度；fixtures 更新含 breakdown 字段的完整样例
- **IPC**：协议字段名（`aiGroupId`）不变，只是字段值语义对齐 chunkId 格式；不新增 Tauri command
- **测试**：`crates/cdt-analyze/tests/context_tracking.rs` 更新断言；新增 Vitest 单测覆盖各 Section 渲染 + scroll 调度；fixture `multi-project-rich.ts` 加完整 6 类 injection 样例
- **风险**：aiGroupId 改字段值会让所有现存断言失败，但因为这是内部字段（前端目前还没消费它做导航），落地范围可控
