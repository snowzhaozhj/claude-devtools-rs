## Why

`OMIT_TOOL_OUTPUT=true` 走 lazy load 后，前端只对 Read 工具实现了"等 output 拉到才展开"，其他用 output 渲染的工具（Bash / 默认查看器）仍走"先展开后异步注入"路径——展开瞬间因 `outputOmitted=true` 渲染空 OUTPUT 区域，IPC 返回后内容跳进来，体感是明显的闪烁（用户在大会话里反馈 Bash 工具展开仍然闪）。Edit（任意 `isError`）与成功态 Write 仅渲染 input 字段，不闪；失败态 Write 走 DefaultToolViewer 渲染错误详情，依赖 output，与 Bash / Default 同类。

## What Changes

- 把"`outputOmitted=true` 时先 `getToolOutput` 拉到再展开"的契约从只覆盖 Read，扩展到所有"viewer 用到 `exec.output` 渲染"的工具：Bash、DefaultToolViewer 路径（含 `isError=true` 的 Read 与 Write 这两种 fallback 到 DefaultToolViewer 渲染错误详情的场景）。
- Edit（任意 `isError`）与 `isError=false` 的 Write 展开行为保持不变（它们走 EditToolViewer / WriteToolViewer 仅渲染 input，等 output 没意义且会让用户误以为按钮无响应）。
- 仅扩展 toggle 单点延迟语义；**不**新增"展开 AIChunk 时主动 prefetch Bash / Default" —— 避免一个 chunk 含多 Bash 时一次性触发并发 IPC 把 IPC 队列堵住，劣化"展开工具列表"交互。Read 现有的 prefetchReadOutputs 路径不动。
- 主 SessionDetail toggle 与嵌套 SubagentCard 的 ExecutionTrace toggle 行为对齐。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `session-display`: `Requirement: 大文本工具详情交互优先渲染` 下两个 "omitted Read 输出 ready 后再展开" Scenario 扩展为覆盖 Bash / Default 等所有用 output 渲染的工具。

## Impact

- `ui/src/routes/SessionDetail.svelte::toggle` —— 把 `isReadTool(exec)` gate 替换为"viewer 用 output"判断
- `ui/src/components/ExecutionTrace.svelte::toggle` —— 同上对齐
- `ui/src/lib/__tests__/`（vitest）—— 覆盖 Bash / Default 走 ready-before-expand 路径
- 不影响后端 IPC 协议、不改 `OMIT_TOOL_OUTPUT` 常量、不动 prefetch 范围
