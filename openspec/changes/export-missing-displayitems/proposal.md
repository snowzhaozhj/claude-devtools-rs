## Why

Session 导出（Markdown / HTML / CLI）目前在 render switch 里把 `slash` / `teammate_message` / `teammate_spawn` / `workflow` 四类 DisplayItem 与 subagent 内部对话静默 `return ""`，导出件缺失 SessionDetail 视图可见的内容。这是 change `fix-export-tool-order-and-output` 显式 scope-out 的遗留（见 `session-export` spec "子代理内容导出" Requirement 的范围外注释），由 issue #534 跟踪。导出的核心价值是完整快照，缺内容直接违背该承诺。

## What Changes

- Markdown / HTML 导出器为 `slash` / `teammate_message` / `teammate_spawn` / `workflow` 四类 DisplayItem 补齐渲染（数据均已在导出 payload 内：`AIChunk.slashCommands` / `teammateMessages`、`ToolExecution.teammateSpawn`、`SessionDetail.workflowItems`），对齐 SessionDetail 视图的语义。
- workflow 渲染对齐视图：带 `workflowRunId` 且命中 `workflowItems` 的工具调用 SHALL 渲染为 workflow 摘要（name + phases + agents 列表 + 状态/耗时/tokens），而非普通工具。
- **BREAKING（导出数据策略反转）**：导出路径（桌面 `get_session_detail_for_export` + 浏览器 HTTP `?export=1` + CLI in-process）SHALL 不再整体裁剪 subagent `messages`，改为递归渲染 subagent 内部对话流（复用 `buildDisplayItemsFromChunks`）。为控制 payload，导出路径填充 subagent messages 时 SHALL 施加三层封顶——嵌套深度上限（depth-cap）+ per-subagent byte cap（控单个病态 subagent）+ 全局累计 byte cap（控极端 fan-out IPC 总量兜底），超限清空并标注省略；三路共用同一 cap 函数与参数。
- 浏览器 HTTP 导出新增 `?export=1` query 分支走 `apply_export_omissions`（首屏路径不变）。
- CLI Markdown 导出器同步补齐上述四类 + subagent 内部对话递归渲染，透传 `workflow_items`。

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `session-export`: "子代理内容导出" Requirement 移除"子代理内部对话尚未实现"的范围外注释，新增内部对话递归渲染与 payload 封顶约束；"导出数据完整性" Requirement 修改 subagent messages 裁剪条款（从"仍被裁剪"改为"封顶填充"）；新增 teammate / slash / workflow 三类内容的导出渲染 Requirement。

## Impact

- 前端：`ui/src/lib/export/markdownExporter.ts`、`ui/src/lib/export/htmlExporter.ts`（render switch 补 case + workflow 关联）；可能新增渲染辅助。
- 后端：`crates/cdt-api/src/ipc/local.rs::apply_omissions_impl`（subagent messages 封顶填充而非清空）、`apply_export_omissions` 语义；`crates/cdt-api/src/ipc/types.rs`。
- CLI：`crates/cdt-cli/src/export.rs`（render_subagent_md 递归 + 四类渲染）。
- IPC payload：导出路径 payload 上升（受 byte cap 封顶约束，非 hot path）。
- 测试：`ui/src/lib/export/export.test.ts` 补四类 + subagent 内部对话；CLI export 测试；后端 omission 单测。
