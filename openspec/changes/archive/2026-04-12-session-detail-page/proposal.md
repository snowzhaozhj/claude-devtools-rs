## Why

13 个数据层 capability 已全部 port 完成，Tauri + Svelte 5 骨架已搭好（`tauri-svelte-scaffold`），目前有 ProjectList 和 SessionList 两个页面。用户点击某个 session 后没有 detail 视图——需要实现 session detail 页面来展示 chunk 列表、语义步骤、工具执行和 session 指标，这是整个 DevTools 的核心可视化功能。

## What Changes

- 新建 `SessionDetail.svelte` 组件：渲染从 `get_session_detail` API 返回的 chunks 列表
  - `UserChunk`：显示用户消息内容
  - `AIChunk`：可展开/折叠，显示 semantic steps（thinking / text / tool execution / subagent spawn）和 tool executions 详情
  - `SystemChunk`：显示系统消息
  - `CompactChunk`：显示 compaction 摘要
- 顶部 metrics 汇总栏：总 token 用量、工具调用次数、chunk 数量
- 修改 `SessionList.svelte`：session card 点击触发导航
- 修改 `App.svelte`：新增 `"detail"` 视图路由，支持三级导航（projects → sessions → detail）
- 补充 `api.ts` 的 TypeScript 类型定义（`Chunk`、`SemanticStep`、`ToolExecution` 等）

## Capabilities

### New Capabilities

（无——这是纯 UI 层改动，不新增数据层 capability）

### Modified Capabilities

（无——后端 API 已就绪，不改变 spec 行为）

## Impact

- **前端文件**：`ui/src/` 下新增 `SessionDetail.svelte` 及若干子组件，修改 `App.svelte`、`SessionList.svelte`、`api.ts`
- **后端**：无改动，复用已有的 `get_session_detail` Tauri command
- **依赖**：无新增
