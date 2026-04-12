## 1. TypeScript 类型定义

- [x] 1.1 在 `ui/src/lib/api.ts` 中补充 `Chunk`、`SemanticStep`、`ToolExecution`、`ToolOutput`、`ChunkMetrics` 等类型定义，与 Rust `serde(rename_all = "camelCase", tag = "kind")` 对齐
- [x] 1.2 更新 `SessionDetail` 接口，将 `chunks` 从 `any` 改为 `Chunk[]`，`metrics` / `metadata` 补充类型

## 2. 路由与导航

- [x] 2.1 修改 `App.svelte`：新增 `"detail"` 视图状态，传递 `projectId` + `sessionId` 给 `SessionDetail` 组件
- [x] 2.2 修改 `SessionList.svelte`：session card 添加 `onclick` 回调，通过 `onSelect(sessionId)` 向上通知

## 3. SessionDetail 页面

- [x] 3.1 创建 `ui/src/routes/SessionDetail.svelte`：调用 `getSessionDetail` 加载数据，处理 loading / error 状态
- [x] 3.2 实现 metrics 汇总栏：显示总 chunk 数、总 input/output tokens、总工具调用数
- [x] 3.3 实现 chunk 列表渲染：按 `chunk.kind` 区分 `user` / `ai` / `system` / `compact` 四种样式
- [x] 3.4 实现 AI chunk 展开/折叠：默认折叠显示摘要，展开显示 semantic steps 和 tool executions
- [x] 3.5 实现 tool execution 详情渲染：工具名、input（截断）、output（截断 + 展开按钮）、error 高亮

## 4. Bug 修复

- [x] 4.1 修复 NaN：给 `cdt-core` 的 `ChunkMetrics`/`AIChunk`/`UserChunk`/`SystemChunk`/`CompactChunk`/`AssistantResponse`/`SemanticStep` 加 `#[serde(rename_all = "camelCase")]`
- [x] 4.2 修复 NaN：给 `cdt-core` 的 `ToolExecution`/`ToolOutput`/`ToolCall`/`ToolResult` 加 `#[serde(rename_all = "camelCase")]`
- [x] 4.3 修复 session ID 缩略：移除 `formatSessionId` 的截断逻辑
- [x] 4.4 适配 `TokenUsage` 前端类型为 snake_case（与 Anthropic API 原始格式一致）

- [x] 4.5 修复误分类：`build_chunks` 跳过 `is_meta` 用户消息（skill prompt / system-reminder 注入），抽取 `chunk_loop` 共用函数
- [x] 4.6 新增 2 个测试：`meta_messages_are_skipped` + `meta_tool_result_still_merges_into_buffer`

## 5. 验证

- [x] 5.1 `npm run check`（svelte-check + tsc）通过
- [x] 5.2 `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [x] 5.3 `cargo test --workspace` 全部 301 tests 通过
- [x] 5.4 `cargo tauri dev` 编译运行成功
