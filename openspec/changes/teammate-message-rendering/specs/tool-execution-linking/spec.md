## ADDED Requirements

### Requirement: Detect teammate-spawned tool results

`tool_linking::pair` 在配对 `tool_use` 与对应 user 消息的 `tool_result` 时 MUST 检测 user 消息顶层 `toolUseResult.status` 字段。当 `status == "teammate_spawned"` 时，从 `toolUseResult` 抽出 `name` 与 `color` 字段封装为 `cdt_core::TeammateSpawnInfo` 并赋给 `ToolExecution.teammate_spawn`。其它情况 `teammate_spawn` SHALL 保持 `None`。

`TeammateSpawnInfo.name` MUST 来自 `toolUseResult.name`（必填，命中即必有）。`TeammateSpawnInfo.color` MUST 来自 `toolUseResult.color`（可选，缺失时 `None`）。

UI 端按此字段决定渲染：非空时把整条 `tool_execution` displayItem 替换为 `teammate_spawn` 极简单行（圆点 + member-X badge + "Teammate spawned" 文案），对齐原版 `claude-devtools/src/renderer/components/chat/items/LinkedToolItem.tsx::isTeammateSpawned`；为空时保留普通 tool item 渲染。

序列化 SHALL 用 camelCase（`teammateSpawn`），`#[serde(skip_serializing_if = "Option::is_none")]` 让无 spawn 信息的 tool execution IPC payload 不含此字段，老前端兼容。

#### Scenario: Status teammate_spawned populates TeammateSpawnInfo
- **WHEN** user 消息 `tool_use_result` 为 `{"status":"teammate_spawned","name":"member-1","color":"blue"}`，对应 `tool_use_id` 配对到一条 `Agent` tool use
- **THEN** 配对产出的 `ToolExecution.teammate_spawn` SHALL 为 `Some(TeammateSpawnInfo { name: "member-1", color: Some("blue") })`

#### Scenario: Status teammate_spawned without color
- **WHEN** `tool_use_result` 为 `{"status":"teammate_spawned","name":"member-2"}`（无 color 字段）
- **THEN** `ToolExecution.teammate_spawn` SHALL 为 `Some(TeammateSpawnInfo { name: "member-2", color: None })`

#### Scenario: Other status values leave teammate_spawn None
- **WHEN** `tool_use_result.status` 为其它值（如 `"ok"`、缺失或非字符串）
- **THEN** `ToolExecution.teammate_spawn` SHALL 为 `None`

#### Scenario: No tool_use_result leaves teammate_spawn None
- **WHEN** user 消息无 `toolUseResult` 顶层字段
- **THEN** 配对产出的 `ToolExecution.teammate_spawn` SHALL 为 `None`

#### Scenario: Empty teammate_spawn omitted from IPC payload
- **WHEN** `ToolExecution.teammate_spawn = None`
- **THEN** 序列化 JSON SHALL 不含 `teammateSpawn` 键（`skip_serializing_if = "Option::is_none"` 控制）
