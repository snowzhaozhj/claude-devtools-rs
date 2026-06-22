## ADDED Requirements

### Requirement: 导出对话流时序

Markdown / HTML 导出的 Assistant turn 内，thinking / 正文文本 / 工具调用 / subagent 卡片 SHALL 按时间顺序穿插渲染，与 SessionDetail 视图所见顺序一致。导出器 MUST 复用 SessionDetail 视图的同一时序合并实现（`displayItemBuilder.ts::buildDisplayItems`），SHALL NOT 在导出器内另写一套排序，也 SHALL NOT 把工具调用 / subagent 统一堆到 turn 末尾。最终 assistant 文本（`buildDisplayItems` 抽出的 `lastOutput`）SHALL 渲染在该 turn 其余 item 之后，与视图布局一致。

#### Scenario: 工具调用穿插在文本之间

- **WHEN** 一个 Assistant turn 含「文本 A → 工具调用 T → 文本 B（最终输出）」的时序
- **THEN** 导出内容 SHALL 按 文本 A、工具调用 T、文本 B 的顺序渲染
- **AND** 工具调用 T SHALL NOT 出现在文本 B 之后

#### Scenario: subagent 卡片按 spawn 时间穿插

- **WHEN** 一个 Assistant turn 在两段文本之间 spawn 了一个 subagent
- **THEN** 导出内容中 subagent 卡片 SHALL 出现在对应 spawn 时间点的位置
- **AND** SHALL NOT 被统一追加到 turn 末尾

#### Scenario: 导出顺序与视图一致

- **WHEN** 同一 session 在 SessionDetail 视图渲染并导出为 Markdown
- **THEN** 导出件中 thinking / 文本 / 工具 / subagent 的相对顺序 SHALL 与视图 DisplayItem（含末尾 `lastOutput`）顺序一致

### Requirement: 导出数据完整性

导出（Markdown / JSON / HTML）SHALL 基于保留 tool output 与 response content 的 SessionDetail 生成，SHALL NOT 复用首屏被 `outputOmitted` / `contentOmitted` 裁剪过的 payload。导出 MUST 通过导出专用数据路径（桌面端 `get_session_detail_for_export`，浏览器模式复用 HTTP `get_session_detail`）拉取 SessionDetail，使工具 output 与响应内容字段与源 JSONL 一致。当 `toolOutputMode` 为 `full` 时工具 output SHALL 完整非空。image data 与 subagent messages 在导出路径仍被裁剪以控制 payload。

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

## MODIFIED Requirements

### Requirement: 子代理内容导出

当 session 包含子代理（subagent / workflow）时，导出 SHALL 按 `includeSubagents` 选项决定是否在父 turn 内就地渲染子代理卡片。子代理卡片 MUST 按 spawn 时间穿插在对话流中（复用 `buildDisplayItems` 时序），并对齐视图行为：发起该子代理的 Task / Agent 工具调用 SHALL 被去重（由子代理卡片代表），不重复渲染为普通工具。

> 注：导出**子代理内部对话消息**（嵌套 conversation 完整展开）尚未实现，与 teammate / slash / workflow 渲染缺失一并由范围外 issue 跟踪。本 Requirement 当前只约束子代理卡片摘要（工具名 + description + 耗时）的就地渲染与 Task 去重。

#### Scenario: 子代理卡片就地渲染（includeSubagents=true）

- **WHEN** session 包含 subagent processes 且 `includeSubagents` 为 true
- **THEN** 子代理卡片摘要（工具名 + description + 耗时）SHALL 按 spawn 时间穿插渲染在父 turn 对应位置
- **AND** 发起该子代理的 Task / Agent 工具调用 SHALL 被去重不重复渲染

#### Scenario: 关闭子代理仅保留发起工具（includeSubagents=false）

- **WHEN** `includeSubagents` 为 false
- **THEN** 导出 SHALL NOT 渲染子代理卡片
- **AND** 发起该子代理的 Task / Agent 工具调用 SHALL 作为普通工具调用正常渲染（不被去重丢失）
