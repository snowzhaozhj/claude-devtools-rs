## Why

`session-detail-lazy-render` 已落地，前端首屏渲染开销已被压到 ~20 ms（first-paint - IPC = 576-556 ms）。但用户实测仍卡顿，**真因是 IPC payload 7.3 MB 跨 webview 传输 ≈ 556 ms**（约 13 KB/ms = Tauri webview IPC 实际吞吐上限）。

`crates/cdt-api/tests/perf_get_session_detail.rs` payload breakdown（46a25772, 1221 msgs / 96 chunks / 14 subs，release）：

| 字段 | 大小 | 占比 |
|------|------|------|
| **subagent_messages** | **4659 KB** | **60%** |
| response_content | 1257 KB | 16% |
| other (incl. ToolResult) | 877 KB | 11% |
| tool_output | 436 KB | 6% |
| tool_input | 340 KB | 4% |
| semantic_steps | 130 KB | 2% |

最大杠杆是 **`Process.messages`（subagent 嵌套 chunks 全文）**，而 SubagentCard **默认折叠**，header 不需要 messages 全文，仅需要：
- `modelName` —— 从 messages 内最后一条 AI response.model 算（`SubagentCard.svelte:51-60`）
- `isolatedTokens` —— 从 messages 内最后一条 AI usage 算（`:62-80`）
- `isShutdownOnly` —— team-only 特例，依赖 messages 含 1 条 assistant + SendMessage shutdown_response（`:83-100`）
- `traceItems` —— 仅 `isExpanded` 时才需要，已是 lazy（`:103-105`）

把这 3 个 header 派生值预算到 `Process` 上（4 个新字段：`headerModel` / `lastIsolatedTokens` / `isShutdownOnly` / `messagesOmitted`），首屏 IPC 就能 drop 整个 messages 数组。展开时新增 IPC `get_subagent_trace(parentSessionId, subagentSessionId)` 按需拉取。

## What Changes

- **MODIFIED**：`cdt-core::Process` 加 4 个 derived header 字段：`header_model: Option<String>` / `last_isolated_tokens: u64` / `is_shutdown_only: bool` / `messages_omitted: bool`。`messages` 字段保留语义不变；`messages_omitted=true` 表示首屏被裁剪需要后续懒拉。
- **MODIFIED**：`ipc-data-api` capability 既有 `Expose project and session queries` Requirement —— `get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `Process.messages` 默认 SHALL 被裁剪为空 Vec、`messagesOmitted=true`，header 派生字段 SHALL 由后端预算填充。
- **ADDED**：`ipc-data-api` capability 新增 `Lazy load subagent trace` Requirement —— 新 IPC `get_subagent_trace(parentSessionId, subagentSessionId) -> Vec<Chunk>`，按需拉取 subagent 完整执行链。
- **MODIFIED**：`session-display` capability 既有 `Subagent 内联展开 ExecutionTrace` / `Subagent MetricsPill 多维度展示` 两条 Requirement —— SubagentCard header 显示 SHALL 用 `process.headerModel` / `process.lastIsolatedTokens` 而非 derive from `process.messages`；展开时若 `process.messagesOmitted=true` 且本地未缓存，SHALL 调 `getSubagentTrace(parentSessionId, sessionId)` 拉取并替换本地 state。

## Capabilities

### New Capabilities

无

### Modified Capabilities

- `ipc-data-api`：`get_session_detail` 返回值修改（subagent.messages 默认裁剪）；新增 `get_subagent_trace` IPC。
- `session-display`：SubagentCard 的 header derived 字段来源 + 展开懒加载流程。

## Impact

- **代码**
  - `crates/cdt-core/src/process.rs`：`Process` struct 加 4 字段，全部 `#[serde(default)]` 不破坏反序列化；测试 roundtrip 同步更新。
  - `crates/cdt-analyze/src/tool_linking/resolver.rs::candidate_to_process`：填充 4 个 header 字段（从 `candidate.messages` 算）。
  - `crates/cdt-api/src/ipc/local.rs::get_session_detail`：序列化 chunks 前遍历 `ai.subagents`，把 `messages` 替换为空 Vec、设 `messages_omitted=true`（header 字段已在 resolver 阶段填好）。
  - `crates/cdt-api/src/ipc/traits.rs` + `local.rs`：新增 `get_subagent_trace(parent_session_id, subagent_session_id) -> Vec<Chunk>` 方法（trait + LocalDataApi impl），从 disk 按需 parse subagent jsonl + `build_chunks`。
  - `src-tauri/src/lib.rs`：注册新 Tauri command `get_subagent_trace`。
  - `ui/src/lib/api.ts`：新增 `getSubagentTrace(parentSessionId, subagentSessionId): Promise<Chunk[]>`；`Process` TS 类型加 4 字段。
  - `ui/src/components/SubagentCard.svelte`：modelName/isolatedTokens/isShutdownOnly 改为优先用 `process.headerModel`/`process.lastIsolatedTokens`/`process.isShutdownOnly`；fallback 兼容老路径（迁移期）。`toggleExpanded` 时按需调 `getSubagentTrace` 加载 messages 到本地 `$state`，`traceItems` 用本地 messages 而非 process.messages。
- **依赖**：零新增。
- **HTTP API**：本期 IPC 路径优先；HTTP `/api/sessions/:id` 路径同样裁剪 messages 与新增 `/api/subagent-trace/:parent/:sub` endpoint 留作 follow-up（HTTP 当前无活跃用户）。
- **测试**：
  - Rust 单元：`Process` roundtrip 含新字段、`candidate_to_process` 填充 header 字段、`get_session_detail` 返回的 chunks 中 subagent.messages 为空 + `messagesOmitted=true`、`get_subagent_trace` 返回完整 chunks。
  - 前端：`npm run check --prefix ui` 通过；UI 行为人工验证（SubagentCard header 显示正确、展开时拉 trace + 渲染、嵌套 subagent 递归正常）。
- **回滚**：`get_session_detail` 内裁剪逻辑加一个 `OMIT_SUBAGENT_MESSAGES: bool = true` 模块常量；改为 false 即恢复完整 payload（前端 fallback 路径仍生效）。
- **预期收益**：1221 条 / 96 chunks / 14 subs session payload 7.3 MB → ~3 MB → IPC 556 ms → ~230 ms（线性外推 13 KB/ms），first-paint 可降到 250 ms 以内。
