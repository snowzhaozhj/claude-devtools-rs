## Why

Phase 4 (`session-detail-response-content-omit`) 落地后实测：46a25772 case IPC 369 ms / 1668 KB / first-paint 395 ms。剩余 payload 分布（perf bench release）：

| 字段 | 大小 | 占剩余 |
|------|------|--------|
| **`tool_exec.output`** | **436 KB** | **26%** |
| `tool_exec.input` | 340 KB | 20% |
| `responses[].meta` | 588 KB | 35% |
| `tool_exec.meta` | 108 KB | 6% |
| 其他 | ~196 KB | 13% |

**`tool_exec.output`** 是当前最大单一可懒拉字段：
- ExecutionTrace 默认折叠（`expandedKeys = $state(new Set())`，`ExecutionTrace.svelte:27`），用户点击展开才渲染对应 ToolViewer
- 折叠状态下只用 `getToolSummary(toolName, input)` 和 `getToolStatus(exec)`——**不读 `output` 字段**
- 与 SubagentCard 的 messages 懒拉同模式

`input` 暂不动：`getToolSummary` 需要 input 内 `file_path` / `command` 等小字段做 header 摘要；要做必须按 toolName 字段级裁剪（per-tool whitelist），复杂度高，留下一轮。

## 收益预期（按非线性折扣校准）

phase 4 实测显示 IPC 时间不是 payload-bound 线性函数（payload 砍 40% IPC 只砍 13%）——因为 V8 `JSON.parse` 大对象 + Tauri webview baseline 都是固定开销。

按非线性折扣保守估：
- payload 1668 → ~1232 KB（-26%）
- IPC 369 → ~310-330 ms（-12% 到 -16%）
- first-paint 395 → ~340-360 ms

**这是渐进改良不是质变。** 如要追 < 200 ms first-paint，需要后端虚拟分页（砍 chunk 数）或换 Tauri Channel + binary（砍 V8 parse 开销）——本期不做，先把 ROI 最高的字段砍掉。

## What Changes

- **MODIFIED**：`cdt-core::ToolExecution` 加 `#[serde(rename = "outputOmitted", default)] pub output_omitted: bool` 字段（与 phase 2/3/4 同模式）。`output` 字段保留 enum 形状不变（`Text { text: "" }` / `Structured { value: Null }` / `Missing` 三 variant 不增减）；OMIT 时清空内部 text/value 但保留 variant kind，保证前端 viewer 类型路由不破。
- **MODIFIED**：`ipc-data-api` 既有 `Expose project and session queries` Requirement —— `get_session_detail` 返回的 `SessionDetail.chunks` 中所有 `AIChunk.tool_executions[i].output` 内 `text` / `value` 默认 SHALL 被替换为空，且 `outputOmitted=true`。回滚开关 `OMIT_TOOL_OUTPUT: bool` 一行切回。
- **ADDED**：`ipc-data-api` 新增 `Lazy load tool output` Requirement —— 新 IPC `get_tool_output(rootSessionId, sessionId, toolUseId) -> ToolOutput`，按需拉取被 OMIT 的单条 tool output。
- **MODIFIED**：`session-display` 既有 `Subagent 内联展开 ExecutionTrace` Requirement（或新增 `Lazy load tool output on expand`）—— ExecutionTrace 在 `toggle(key)` 展开时若对应 `exec.outputOmitted=true` 且本地未缓存，SHALL 调 `getToolOutput(rootSessionId, sessionId, toolUseId)` 拉取并替换本地 state；`outputOmitted=false` 直接用现有 `output` 字段不发额外 IPC。

## Capabilities

### New Capabilities

无

### Modified Capabilities

- `ipc-data-api`：`get_session_detail` 返回值修改（tool_exec.output 默认裁剪）；新增 `get_tool_output` IPC。
- `session-display`：ExecutionTrace 展开懒拉行为。

## Impact

- **代码**
  - `crates/cdt-core/src/tool_execution.rs`：`ToolExecution` 加 `output_omitted: bool`（`#[serde(default)]` 不破坏反序列化）；roundtrip 测试同步更新；新加 helper `ToolOutput::trim()` 把 inner text/value 清空。
  - `crates/cdt-api/src/ipc/local.rs`：新增 `apply_tool_output_omit(chunks)` 函数 + 顶部 `const OMIT_TOOL_OUTPUT: bool = true` 回滚开关；`get_session_detail` 序列化前调用顺序：image OMIT → response.content OMIT → tool_output OMIT → subagent OMIT。
  - `crates/cdt-api/src/ipc/traits.rs` + `local.rs`：新增 `get_tool_output(root_session_id, session_id, tool_use_id) -> Result<ToolOutput, ApiError>`。LocalDataApi 实现：parse 目标 jsonl → 找到 tool_use_id 对应的 `ToolExecution.output` → 直接返回。失败（jsonl 不存在 / id 找不到）SHALL 返回 `ToolOutput::Missing` 不报错。
  - `src-tauri/src/lib.rs`：注册新 Tauri command `get_tool_output`。
  - `ui/src/lib/api.ts`：新增 `getToolOutput(rootSessionId, sessionId, toolUseId): Promise<ToolOutput>`；TS `ToolExecution` 加 `outputOmitted` 字段。
  - `ui/src/components/ExecutionTrace.svelte`：`toggle(key)` 时若 `exec.outputOmitted=true`，按需调 `getToolOutput` 替换 local map；ToolViewer props 从 local map 读 output（fallback 到 exec.output）。新增 `outputCache: Map<string, ToolOutput>` $state。
- **依赖**：零新增。
- **HTTP API**：与 phase 2/3/4 同分叉——HTTP path 不应用 OMIT。
- **测试**：
  - Rust 单元：`ToolExecution` roundtrip 含新字段；`ToolOutput::trim` 三 variant 行为；`apply_tool_output_omit` 覆盖顶层 + 嵌套 subagent.messages；`get_tool_output` 命中 / 未命中两路径。
  - perf bench 重跑确认 payload 减少。
  - 前端 `npm run check --prefix ui` 通过。
- **回滚**：`OMIT_TOOL_OUTPUT: bool = false` 即恢复完整 payload；前端 fallback 路径自动接管。
- **预期收益**：46a25772 IPC 369 → ~310-330 ms（按 phase 4 非线性折扣推算），first-paint 395 → ~340-360 ms。**这是渐进改良不是质变**——根治需要虚拟分页 / Channel binary 序列化（下下轮）。
