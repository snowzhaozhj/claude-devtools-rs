## MODIFIED Requirements

### Requirement: Context Panel turn 锚点导航

Context Panel 内每条 injection SHALL 提供一个跳转动作，把 SessionDetail 主视图滚动到对应容器（按 `aiGroupId == chunkId` 匹配 `data-chunk-id` DOM 属性）。点击 `ToolOutputs` Section 内某条 tool breakdown SHALL 先确保该 chunk 展开（`expandedChunks` 含 `chunkId`），再在目标 chunk 内查找该 tool 子节点（按 `data-tool-use-id == toolUseId` 匹配）并滚动到该子节点；若目标 chunk 内找不到 tool，则退化为滚到 chunk 本身。点击 `UserMessageInjection` 的跳转 SHALL 按其 `aiGroupId` 命中的 chunk 类型分流：若命中的 chunk 本身是 `UserChunk`（被打断的 turn，`aiGroupId` 即该用户消息自身的 `chunkId`），SHALL 直接滚到该 `UserChunk`；否则（完整 turn，`aiGroupId` 是 `AIChunk` 的 `chunkId`）SHALL 滚到该 turn 紧邻前的 `UserChunk`，无前置 UserChunk 则退化为滚到该 AIChunk。

#### Scenario: 点击 injection 滚到 AIChunk

- **WHEN** 用户点击 Category 视图任一非 user-message Section 的 injection 行
- **THEN** SHALL 把对应 `chunkId` 加入 `expandedChunks`（已在则跳过）
- **AND** SHALL `await tick()` 一次后 `scrollIntoView({ block: "center", behavior: "smooth" })` 把 `[data-chunk-id="<aiGroupId>"]` 节点稳定滚动到 conversation 容器中部

#### Scenario: 点击 tool breakdown 行展开并滚到 tool

- **WHEN** 用户点击 Tool Outputs Section 内某 tool 行
- **THEN** SHALL 先把对应 AIChunk 的 `chunkId` 加入 `expandedChunks`（已在则跳过）
- **AND** SHALL `await tick()` 一次后滚动到该 chunk
- **AND** SHALL 再次 `await tick()` 后滚动到 `[data-tool-use-id="<toolUseId>"]` 节点

#### Scenario: 点击完整 turn 的 user message injection 滚到前置 UserChunk

- **WHEN** 用户点击 User Messages Section 某条 user-message injection，且其 `aiGroupId` 命中的 chunk 是一个 `AIChunk`
- **THEN** SHALL 滚到该 AIChunk 紧邻前的 `UserChunk` 容器（按时序在 `chunks` 数组中向前匹配）；若无前置 UserChunk 则 SHALL 退化为滚到该 AIChunk

#### Scenario: 点击被打断 turn 的 user message injection 直接滚到该 UserChunk

- **WHEN** 用户点击 User Messages Section 某条 user-message injection，且其 `aiGroupId` 命中的 chunk 本身是一个 `UserChunk`（被打断的 turn）
- **THEN** SHALL 直接滚到该 `UserChunk` 容器（`aiGroupId == 该 UserChunk.chunkId`）
- **AND** SHALL NOT 向前回溯到上一条用户消息

#### Scenario: SessionDetail 渲染 chunk 时挂 DOM 锚点

- **WHEN** SessionDetail 渲染任意 `Chunk`
- **THEN** chunk 容器节点 SHALL 带 `data-chunk-id={chunk.chunkId}` 属性
- **AND** AIChunk 内每个 `ToolExecution` 渲染节点 SHALL 带 `data-tool-use-id={exec.toolUseId}` 属性
