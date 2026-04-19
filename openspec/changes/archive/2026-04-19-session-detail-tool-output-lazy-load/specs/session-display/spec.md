# session-display Spec Delta

## ADDED Requirements

### Requirement: Lazy load tool output on expand

ExecutionTrace 渲染的每个 tool item SHALL 在 `toggle(key)` 展开时检查对应 `exec.outputOmitted`：若为 `true` 且本地 `outputCache` 未命中，SHALL 调 IPC `getToolOutput(rootSessionId, sessionId, toolUseId)` 拉取 `ToolOutput` 并写入本地缓存；ToolViewer 渲染 SHALL 优先用本地 `outputCache.get(toolUseId)`，fallback 到 `exec.output`（兼容 `outputOmitted=false` 老后端 / 回滚路径）。本机制对主 SessionDetail 与 SubagentCard 内嵌套 ExecutionTrace 同等适用，sessionId 参数 SHALL 由所在 trace 的 sessionId 提供（嵌套 subagent trace 用 subagent 的 sessionId）。

#### Scenario: 折叠状态不触发 IPC

- **WHEN** SessionDetail 首屏渲染含 N 个 tool execution，且 `outputOmitted=true`
- **THEN** 前端 SHALL NOT 调 `getToolOutput`，仅渲染 BaseItem header（label / summary / status）
- **AND** Network 面板 SHALL 显示 0 次 `get_tool_output` 调用

#### Scenario: 展开时按需拉

- **WHEN** 用户点击某个 tool item 触发 `toggle(key)`
- **AND** 对应 `exec.outputOmitted=true` 且本地 `outputCache` 未命中
- **THEN** 前端 SHALL 调 `getToolOutput(rootSessionId, sessionId, exec.toolUseId)` 一次
- **AND** 拉取成功后 SHALL 把结果写入本地 `outputCache.set(toolUseId, output)`，触发 ToolViewer 用新 output 重渲染

#### Scenario: 重复展开复用本地缓存

- **WHEN** 用户先展开后折叠再展开同一 tool item
- **THEN** 第二次展开 SHALL NOT 触发 `getToolOutput` IPC（直接用 `outputCache.get(toolUseId)`）

#### Scenario: 老后端 / 回滚开关 fallback

- **WHEN** 后端响应中 `outputOmitted=false` 或字段缺失（老后端）
- **THEN** 前端 SHALL 直接渲染 `exec.output`，SHALL NOT 调 `getToolOutput`

#### Scenario: 嵌套 subagent 内 tool 用 subagent sessionId 拉

- **WHEN** SubagentCard 展开后渲染嵌套 ExecutionTrace，用户点击其中某 tool
- **THEN** 前端 SHALL 调 `getToolOutput(rootSessionId, subagent.sessionId, toolUseId)`，sessionId 用 subagent 的，不复用 root 的

#### Scenario: IPC 失败不阻塞 UI

- **WHEN** `getToolOutput` IPC 抛错或返回 `ToolOutput::Missing`
- **THEN** 前端 SHALL 渲染 fallback 显示（如"output 加载失败"或空状态），SHALL NOT 阻塞其它 tool item 的展开
