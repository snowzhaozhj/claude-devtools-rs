## Why

会话详情页展开 Read/Edit/Write 工具详情时，几百行内容也可能阻塞主线程，导致点击目标选不中或延迟响应。当前实现对每行同步执行 `highlight.js` 与 `DOMPurify`，Edit diff 还叠加 LCS 计算与逐行高亮；这与原版轻量行级渲染策略不一致，需要把工具详情渲染从“一次性重型同步工作”改为交互优先的可控渲染。

## What Changes

- Read/Write 工具内容改为交互优先渲染：展开后 SHALL 先展示可交互结构，再分批或轻量化渲染代码行。
- Edit diff 展开 SHALL 避免每个 diff 行再跑重型语法高亮，保留统一 diff 视觉语义与行号/增删统计。
- 工具详情渲染 SHALL 保持 XSS 防护边界：任何通过 `{@html}` 注入的 HTML 仍 MUST 来自受控高亮输出或经过清洗。
- SessionDetail 展开状态更新 SHALL 尽量局部化，减少整页 display item 与 markdown/diff 的不必要重算。
- 增加覆盖较大 Read/Write/Edit 内容的前端测试或性能回归入口，验证渲染策略不会退回逐行重型同步路径。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `session-display`: 修改工具详情渲染性能契约，补充大文本 Read/Write/Edit 展开时的交互优先与 diff 轻量渲染要求。

## Impact

- 影响 `ui/src/routes/SessionDetail.svelte` 的展开状态与输出缓存路径。
- 影响 `ui/src/components/tool-viewers/ReadToolViewer.svelte`、`WriteToolViewer.svelte`、`EditToolViewer.svelte`、`DiffViewer.svelte` 与相关渲染工具。
- 影响前端测试：需要新增或扩展 Vitest/Playwright 覆盖大文本工具详情渲染策略。
- 不改 Tauri IPC 字段、不改 `cdt-api` 后端语义、不引入新的外部依赖。
