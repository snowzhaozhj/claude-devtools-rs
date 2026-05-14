## Why

当前 Sidebar 和 Command Palette 只消费 `list_sessions` 的默认第一页，项目会话超过 50 条时旧会话从列表与本地搜索入口消失。PR #63 的修复暴露出契约缺口：前端在使用分页 IPC 时必须保证当前项目的完整会话历史可见，而不是依赖默认 page size。

## What Changes

- Sidebar 初次加载、切换项目、file-change silent refresh 时 SHALL 加载当前项目完整会话历史。
- Command Palette 打开时 SHALL 为当前项目加载完整会话历史，保证本地 session 搜索覆盖旧会话。
- 前端消费 `list_sessions` 时若响应包含 `nextCursor`，MUST 继续扩大请求或采用等价方式获取完整结果，直到最终响应 `nextCursor = null`。
- 实现不得使用逐页追加导致同 project 的 `session-metadata-update` 后台扫描只覆盖最后一页。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `sidebar-navigation`: 明确 Sidebar 与 Command Palette 必须加载完整分页会话历史，而不是只展示默认第一页。

## Impact

- `ui/src/lib/api.ts`：完整会话列表加载 helper。
- `ui/src/components/Sidebar.svelte`：会话列表加载路径。
- `ui/src/components/CommandPalette.svelte`：命令面板本地会话搜索数据源。
- `ui/src/lib/tauriMock.ts` 与 UI 单测：分页 mock 与 51 条会话回归覆盖。
