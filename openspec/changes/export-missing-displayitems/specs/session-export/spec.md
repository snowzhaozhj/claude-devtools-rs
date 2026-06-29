# session-export Specification Delta

## ADDED Requirements

### Requirement: teammate / slash / workflow 内容导出渲染

Markdown / HTML / CLI Markdown 导出 SHALL 渲染 SessionDetail 视图可见的 `slash` / `teammate_message` / `teammate_spawn` 三类 DisplayItem 与 `workflow` 内容，不再静默跳过。这些内容的数据均已存在于导出 payload（`AIChunk.slashCommands` / `teammateMessages`、`ToolExecution.teammateSpawn`、`SessionDetail.workflowItems`），导出器 MUST 复用 `buildDisplayItems` 产出的 DisplayItem 并对齐视图语义渲染。

workflow 渲染 SHALL 对齐视图关联策略：带 `workflowRunId` 且命中 `SessionDetail.workflowItems` 的工具调用 SHALL 渲染为 workflow 摘要（name + 状态 + phases + agents 列表 + tokens/duration），SHALL NOT 重复渲染为普通工具调用。同一 `workflowRunId` 在单次导出内 SHALL 只渲染一次 workflow 摘要，该 runId 的后续工具调用 SHALL 被跳过（既不重复渲染 workflow 也不降级为普通工具），对齐视图 summary 层 `seenWorkflowIds` 去重语义。workflow agent 的内部 trace（视图层 `getWorkflowAgentTrace` 懒拉取）不在静态导出范围内。

#### Scenario: slash 命令导出渲染

- **WHEN** 一个 Assistant turn 含 slash 命令（`AIChunk.slashCommands` 非空）
- **THEN** 导出内容 SHALL 渲染 slash 命令名（`/{name}`）
- **AND** 存在 args / message 时 SHALL 一并渲染
- **AND** 存在 instructions 时 SHALL 渲染其内容（Markdown 作 blockquote / HTML 作可折叠块）

#### Scenario: teammate message 导出渲染

- **WHEN** 一个 Assistant turn 含 teammate message（`AIChunk.teammateMessages` 非空）
- **THEN** 导出内容 SHALL 渲染队友标识（`teammateId`）与消息 body
- **AND** teammate message SHALL 按 timestamp 穿插在对话流对应位置（复用 `buildDisplayItems` 时序）

#### Scenario: teammate spawn 导出渲染

- **WHEN** 一个工具调用被 `buildDisplayItems` 转化为 `teammate_spawn` DisplayItem（`ToolExecution.teammateSpawn` 非空）
- **THEN** 导出内容 SHALL 渲染单行 spawn 标识（含队友名）
- **AND** SHALL NOT 再把该工具渲染为普通工具调用

#### Scenario: workflow 摘要导出渲染

- **WHEN** 一个工具调用带 `workflowRunId` 且命中 `SessionDetail.workflowItems`
- **THEN** 导出内容 SHALL 渲染该 workflow 的摘要（name + 状态 + phases + agents 列表）
- **AND** SHALL NOT 把该工具渲染为普通工具调用

#### Scenario: 同一 workflow runId 去重

- **WHEN** 同一 `workflowRunId` 关联多个工具调用出现在导出范围内
- **THEN** 导出内容 SHALL 只渲染一次该 workflow 摘要
- **AND** 该 runId 的后续工具调用 SHALL 被跳过，不重复渲染 workflow 也不降级为普通工具

## MODIFIED Requirements

### Requirement: 子代理内容导出

当 session 包含子代理（subagent / workflow）时，导出 SHALL 按 `includeSubagents` 选项决定是否在父 turn 内就地渲染子代理卡片。子代理卡片 MUST 按 spawn 时间穿插在对话流中（复用 `buildDisplayItems` 时序），并对齐视图行为：发起该子代理的 Task / Agent 工具调用 SHALL 被去重（由子代理卡片代表），不重复渲染为普通工具。

当 `includeSubagents` 为 true 时，导出 SHALL 在子代理卡片摘要之后递归渲染子代理内部对话消息（`SubagentProcess.messages`）。递归渲染前导出 MUST 对 `messages` 应用与外层相同的导出选项投影（thinking 过滤 / 工具详略 / `includeSubagents` 去重），使导出选项在内部对话层一致生效，且投影 MUST 先于 `buildDisplayItemsFromChunks`——`includeSubagents=false` 时投影先移除该层 subagent 列表，避免 builder 按 `parentTaskId` 去重吞掉 Task/Agent 工具；随后经 `buildDisplayItemsFromChunks` 平铺。为控制 payload，导出路径填充 subagent messages 时 SHALL 施加三层封顶：嵌套深度上限（depth-cap）、per-subagent byte cap、全局累计 byte cap 兜底。深度超上限的嵌套子代理、单个 subagent messages 序列化字节超 per-subagent 上限的、或累计字节超全局上限之后的 subagent，其 messages SHALL 被清空并标记 `messagesOmitted=true`，导出渲染 SHALL 在该处标注内部对话已省略。三闸门顺序为：先按深度清空超限嵌套子代理，再按清空后形态计量 per-subagent 字节，最后按 chunks 顺序累计未清空者的字节施加全局上限。

#### Scenario: 子代理卡片就地渲染（includeSubagents=true）

- **WHEN** session 包含 subagent processes 且 `includeSubagents` 为 true
- **THEN** 子代理卡片摘要（工具名 + description + 耗时）SHALL 按 spawn 时间穿插渲染在父 turn 对应位置
- **AND** 发起该子代理的 Task / Agent 工具调用 SHALL 被去重不重复渲染

#### Scenario: 子代理内部对话递归渲染（includeSubagents=true）

- **WHEN** session 包含 subagent 且其 `messages` 在导出路径被封顶填充（非空）
- **THEN** 导出 SHALL 在该子代理卡片摘要之后渲染其内部对话流（thinking / 文本 / 工具 / 嵌套 subagent 卡片）
- **AND** 内部对话 SHALL 复用 `buildDisplayItemsFromChunks` 的同一时序合并实现

#### Scenario: 递归层应用导出选项投影

- **WHEN** 以 `includeThinking=false` 或 `toolOutputMode=name-only` 导出含 subagent 内部对话的会话
- **THEN** subagent 内部对话中的 thinking SHALL NOT 出现（`includeThinking=false`），工具 input/output SHALL 按外层同一详略模式渲染
- **AND** 内部对话层 SHALL NOT 因绕过投影而泄漏被外层过滤的内容

#### Scenario: 子代理内部对话封顶省略

- **WHEN** 子代理 messages 的嵌套深度超出上限，或单个 subagent messages 序列化字节超出 per-subagent 上限
- **THEN** 该子代理 messages SHALL 被清空且 `messagesOmitted` SHALL 为 true
- **AND** 导出渲染 SHALL 标注该处内部对话已省略（超出导出上限），而非静默为空

#### Scenario: subagent messages 全局导出上限兜底

- **WHEN** 多个 subagent 各自未超 per-subagent 上限，但累计序列化字节超出全局上限
- **THEN** 累计上限内的 subagent messages SHALL 保留
- **AND** 超出后的 subagent messages SHALL 被清空且 `messagesOmitted` 标记为 true
- **AND** 导出渲染 SHALL 在对应 subagent 处标注内部对话已省略

#### Scenario: 关闭子代理仅保留发起工具（includeSubagents=false）

- **WHEN** `includeSubagents` 为 false
- **THEN** 导出 SHALL NOT 渲染子代理卡片
- **AND** 发起该子代理的 Task / Agent 工具调用 SHALL 作为普通工具调用正常渲染（不被去重丢失）

### Requirement: 导出数据完整性

导出（Markdown / JSON / HTML）SHALL 基于保留 tool output 与 response content 的 SessionDetail 生成，SHALL NOT 复用首屏被 `outputOmitted` / `contentOmitted` 裁剪过的 payload。导出 MUST 通过导出专用数据路径（桌面端 `get_session_detail_for_export`，浏览器模式经 HTTP `GET /api/sessions/{id}?export=1` 走导出裁剪）拉取 SessionDetail，使工具 output 与响应内容字段与源 JSONL 一致。当 `toolOutputMode` 为 `full` 时工具 output SHALL 完整非空。image data 在导出路径仍被裁剪以控制 payload。

subagent messages 在导出路径 SHALL 按嵌套深度上限（depth-cap）、per-subagent byte cap、全局累计 byte cap 三层封顶填充（而非整体清空）：上限内的 subagent messages SHALL 保留供内部对话渲染，超限的 SHALL 被清空并标记 `messagesOmitted=true`。桌面 IPC、浏览器 HTTP（`?export=1`）、CLI in-process 三条导出路径 SHALL 共用同一 cap 函数与同一参数，行为一致。`messagesOmitted` 在导出语境表示"封顶省略、静态导出不可补取"，在首屏 display 语境表示"全清空、可懒拉"，渲染层统一按布尔标注省略，不新增区分原因的字段。

#### Scenario: full 模式工具 output 非空

- **WHEN** 用户以 `toolOutputMode = full` 导出含工具调用（且源数据有 output）的会话
- **THEN** 导出件中每个工具调用 SHALL 渲染其真实 output 内容
- **AND** output SHALL NOT 为空（除非源 JSONL 本就无 output）

#### Scenario: JSON 导出保留工具 output 与 response content

- **WHEN** 用户导出为 JSON
- **THEN** 序列化结果中 `tool_executions[].output` SHALL 含真实内容
- **AND** `responses[].content` SHALL NOT 为空串且 `contentOmitted` SHALL NOT 为 true

#### Scenario: 桌面与浏览器导出 tool output 一致

- **WHEN** 同一会话分别在 Tauri 桌面端与浏览器 HTTP 模式导出
- **THEN** 两者工具调用的 output 与 response content SHALL 一致非空
- **AND** 两者 subagent messages 封顶行为 SHALL 一致（同 depth-cap + 同 per-subagent byte cap）

#### Scenario: subagent messages 封顶填充

- **WHEN** 用户以 `includeSubagents = true` 导出含 subagent 的会话
- **THEN** 导出路径 SHALL 保留 depth-cap 与 per-subagent byte cap 内的 subagent messages（非空）
- **AND** 超出上限的 subagent messages SHALL 被清空且 `messagesOmitted` 标记为 true
