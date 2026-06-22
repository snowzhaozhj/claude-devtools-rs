## Why

会话导出（Markdown / JSON / HTML）当前有两个用户可感知的正确性 bug：

1. **时序反转**：Markdown / HTML 导出里，工具调用与 subagent 卡片全部被堆到 Agent 最终文本**之后**，而 SessionDetail 视图里它们是按时间顺序穿插的。导出件读起来像"先说完结论、再补一堆工具"，与真实对话顺序不符。
2. **工具 output 全空**：三种格式导出的工具调用都只有 input、output 永远为空。桌面端导出复用了首屏瘦身过的 IPC payload，tool output 已被 `OMIT_TOOL_OUTPUT` 裁掉。

两者都是导出件的内容保真问题，导致导出结果无法真实复盘会话。

## What Changes

- **Bug 1（前端，时序）**：导出器 `markdownExporter.ts::renderAIChunk` / `htmlExporter.ts::renderAIHtml` 改为复用 `displayItemBuilder.ts::buildDisplayItems` 的时序合并结果渲染，删除"末尾统一堆 toolExecutions / subagents"的旧逻辑。导出时序自此与 SessionDetail 视图**单一真相源**对齐，不再各写一套排序。
- **Bug 2（后端 + 前端，IPC）**：新增导出专用 IPC command `get_session_detail_for_export`，调用导出专用裁剪 `apply_export_omissions`——**保留 tool output + response content**（导出器实际消费），**仍裁剪 image + subagent messages**（导出器都不渲染，且是 payload 大头）。前端 `SessionMetaMenu.svelte::doExport` 改调新 command；浏览器模式复用既有 `get_session_detail`（HTTP 路由本就返回完整 detail）。
- 不引入按需懒拉取（Plan A 被否决）：导出是一次性用户动作，一次 IPC 全量往返优于 N 次懒拉取往返，且不碰首屏热路径。
- **为什么保留 response content**：JSON 导出是 `JSON.stringify(整个 detail)`，裁剪 response content 会让 JSON 出现 `content:""`（codex critical）。**为什么仍裁 image**：完整 image base64 inline 是 perf.md 反模式，会让 JSON 主线程 stringify OOM/卡顿（codex Q1）；image / subagent messages 在 JSON 中以 `dataOmitted` / `messagesOmitted` 自描述。

**范围外**：teammate / slash command / workflow / subagent 内部对话消息在导出中缺失（导出器从未渲染这些 DisplayItem 类型 / 嵌套 conversation）——另开 GitHub issue 单独跟踪，不进本 change。

## Capabilities

### New Capabilities
<!-- 无新增 capability -->

### Modified Capabilities
- `session-export`: 新增"导出对话流时序"与"导出数据完整性"两条 Requirement；MODIFIED"子代理内容导出"两个 Scenario（对齐视图的 Task 去重 + subagent 卡片就地渲染）。
- `ipc-data-api`: 新增 Requirement 暴露导出专用 IPC command `get_session_detail_for_export`，返回完全不裁剪的 SessionDetail。

## Impact

- **前端**：`ui/src/lib/export/markdownExporter.ts`、`ui/src/lib/export/htmlExporter.ts`（复用 buildDisplayItems 重写 AI chunk 渲染）；`ui/src/lib/export/projection.ts`（`projectSubagents` 在 `includeSubagents=false` 时丢弃 subagents）；`ui/src/components/SessionMetaMenu.svelte`（改调新 command）；`ui/src/lib/api.ts`（新 `getSessionDetailForExport` + transport 分叉）；`ui/src/lib/transport.ts`（登记新 command）；`ui/src/lib/tauriMock.ts`（KNOWN_TAURI_COMMANDS）；`ui/src/lib/__fixtures__/*`（如需）。
- **后端**：`crates/cdt-api/src/ipc/local.rs`（新增 `apply_export_omissions` = image + subagent-messages 裁剪）；`crates/cdt-api/src/ipc/types.rs`（`SessionDetailResponse::apply_export_omissions`）；`src-tauri/src/lib.rs`（新 command + `invoke_handler!`）；`crates/cdt-api/tests/ipc_contract.rs`（`EXPECTED_TAURI_COMMANDS` + contract test 断言导出保留 tool-output/response-content、首屏仍裁剪）。
- **IPC 协议**：新增一个 Tauri command，无字段语义变更；首屏 `get_session_detail` 行为不变。
- **性能**：导出路径 payload 主要由 tool-output + response-content 构成（文本/结构化、有界），裁掉 image base64 + subagent messages 规避反模式与超大 stringify；一次性用户动作、单次往返、首屏热路径零改动。
