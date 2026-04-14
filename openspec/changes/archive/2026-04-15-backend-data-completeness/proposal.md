## Why

数据层 13 个 capability 已全部实现，但 API 层（`cdt-api`）有三个数据缺口：subagent 解析未集成、slash 命令被过滤丢失、全文搜索是 stub。前端已有对应类型定义和占位 UI，补全这三项后端数据即可激活完整功能。

## What Changes

- **Subagent 解析集成**：`get_session_detail` 中调用已有的 `resolve_subagents`，从 `cdt-discover` 获取候选 session 列表，补全 `AIChunk.subagents` 数据
- **Slash 命令提取**：在 `cdt-analyze` 中从 isMeta 消息提取 slash 命令信息，注入 chunk 数据结构；前端 summary 加 slash 计数
- **Search 全文搜索对接**：`LocalDataApi.search()` 集成已有的 `SessionSearcher`；新增 Tauri command `search_sessions`

## Capabilities

### New Capabilities

（无新增 capability）

### Modified Capabilities

- `ipc-data-api`：`get_session_detail` 需集成 subagent 解析；`search` 需对接 `SessionSearcher` 替代当前 stub
- `chunk-building`：需从 isMeta 消息中提取 slash 命令信息并附加到 AI chunk

## Impact

- **后端 crate**：`cdt-api`（主要改动）、`cdt-analyze`（slash 提取）、`cdt-core`（可能扩展类型）
- **Tauri 层**：`src-tauri/src/lib.rs` 新增 `search_sessions` command
- **前端**：`ui/src/lib/toolHelpers.ts` 的 `buildAiGroupSummary` 加 slash 计数；可选新增全局搜索 UI
- **依赖**：不引入新 crate，全部基于已有基础设施
