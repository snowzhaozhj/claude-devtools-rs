## Why

PR #113 用前端 `${base}:${index}` 临时规避了 SessionDetail keyed each duplicate-key 崩溃，但根因仍在：`responses[0].uuid` 不是 chunk 级全局唯一标识，compact/replay 后同一 session 可能出现多个 `AIChunk` 共享同一个 assistant response uuid。

SessionDetail 的 DOM key、展开状态、滚动位置与搜索定位需要一个由后端输出的稳定 chunk 标识；否则前端只能继续混用 response uuid 与 index，刷新、替换 tab 或插入 chunk 时仍可能污染 UI 状态。

## What Changes

- `get_session_detail` 返回的每个 `Chunk` SHALL 暴露统一字段 `chunkId`，在同一 `SessionDetail.chunks` 内稳定且唯一。
- `UserChunk` / `SystemChunk` / `CompactChunk` 的 `chunkId` SHALL 使用自身消息 uuid。
- `AIChunk` 的 `chunkId` SHALL 由后端构建阶段生成，不能只使用 `responses[0].uuid`，也不能只使用数组 index。
- SessionDetail 前端 keyed each、chunk 级展开状态、滚动保存与搜索定位 SHALL 优先使用 `chunkId`。
- 保留现有视觉与交互，不引入 breaking change；新增字段为 IPC/HTTP payload 扩展。

## Capabilities

### New Capabilities

- 无。

### Modified Capabilities

- `ipc-data-api`: `get_session_detail` 的 `SessionDetail.chunks` payload 新增稳定唯一的 `chunkId` 字段。
- `session-display`: SessionDetail 的 DOM key、展开状态、滚动保存与 openOrReplaceTab 隔离语义改为依赖 `chunkId`。
- `ui-search`: SessionDetail 搜索定位在 chunk 级定位/刷新场景中使用 `chunkId` 保持稳定。

## Impact

- Rust 类型与构建链路：`cdt-core` chunk 类型、`cdt-analyze` chunk builder、`cdt-api` detail 组装与序列化。
- Contract / 测试：`cdt-api` IPC contract test、相关 session detail 集成测试。
- 前端类型与渲染：`ui/src/lib/api.ts`、fixtures、`SessionDetail.svelte`、搜索与状态相关测试。
- 不新增外部依赖，不改变 Tauri command 名称或参数。
