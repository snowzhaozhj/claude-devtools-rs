## MODIFIED Requirements

### Requirement: Context Panel 视图模式

Context Panel SHALL 支持 Category（按类别分组）和 Ranked（按 token 排序）两种主视图模式；Ranked 模式下 SHALL 提供 Grouped（按 category 颜色块分组）与 Flat（纯 token 排序平铺）子模式切换。Category 视图 SHALL 把 injections 拆为 6 个独立 Section（User Messages / CLAUDE.md Files / Mentioned Files / Tool Outputs / Task Coordination / Thinking + Text），每个 Section 用专属模板呈现关键字段而非通用 `label + preview`。所有 Section 默认 SHALL 处于展开状态；空 Section（无对应 injection）SHALL NOT 渲染。

#### Scenario: 默认 Category 视图 + 6 Section 全展开

- **WHEN** Context Panel 打开
- **THEN** SHALL 默认显示 Category 视图
- **AND** 6 个 Section 中所有非空 Section SHALL 默认展开
- **AND** 空 Section SHALL NOT 出现在 DOM 中

#### Scenario: 切换到 Ranked 视图

- **WHEN** 用户点击 "Ranked" 按钮
- **THEN** SHALL 切到 Ranked 视图并默认 Grouped 子模式
- **AND** Ranked 视图顶部 SHALL 出现 "Grouped" / "Flat" 子切换按钮

#### Scenario: Ranked Grouped 子模式

- **WHEN** Ranked 视图选中 Grouped 子模式
- **THEN** SHALL 按 category 分块，块内按 `estimatedTokens` 降序，每块顶部带 category 颜色 chip

#### Scenario: Ranked Flat 子模式

- **WHEN** Ranked 视图选中 Flat 子模式
- **THEN** SHALL 把所有 injection 平铺，按 `estimatedTokens` 降序排列，每行左侧带 category 颜色 chip

#### Scenario: 分类颜色系统

- **WHEN** Ranked 视图中渲染注入项
- **THEN** 各类别 SHALL 使用对应颜色标签：`claude-md` 紫蓝、`mentioned-file` 绿、`tool-output` 黄、`thinking-text` 紫、`task-coordination` 橙、`user-message` 蓝

#### Scenario: ToolOutputs Section 展示 tool breakdown

- **WHEN** Category 视图的 Tool Outputs Section 展开
- **THEN** 每条 `ToolOutputInjection` SHALL 展示其 `toolBreakdown` 中每个 tool 的名字、token 数、`isError` 标记
- **AND** 每个 tool 行 SHALL 是可点击的，触发 `onNavigateToTool(aiGroupId, toolUseId)`

#### Scenario: ThinkingText Section 拆分 thinking / text

- **WHEN** Category 视图的 Thinking + Text Section 展开
- **THEN** 每条 `ThinkingTextInjection` SHALL 拆开显示 `breakdown` 中 `thinking` 与 `text` 各自 token 数

#### Scenario: TaskCoordination Section 拆分各 kind

- **WHEN** Category 视图的 Task Coordination Section 展开
- **THEN** 每条 `TaskCoordinationInjection` SHALL 拆开显示 `breakdown` 中 `send-message` / `task-tool` / `teammate-message` 各 item 的 `label` + `tokenCount`

#### Scenario: UserMessages Section 显示 turn 序号

- **WHEN** Category 视图的 User Messages Section 渲染一条 `UserMessageInjection`
- **THEN** SHALL 显示 `Turn <turnIndex>` 标识 + `textPreview` + `estimatedTokens`

### Requirement: CLAUDE.md DirectoryTree

Category 视图中的 CLAUDE.md Files Section SHALL 按 `scope` 把文件分为 Global（含 `enterprise` + `user`）/ Project / Directory 三组，每组内 SHALL 以递归目录树形式展示文件路径；空组 SHALL NOT 渲染。Mentioned Files SHALL 拆到独立的 Mentioned Files Section 而非附在 CLAUDE.md Section 下。

#### Scenario: 三组分组渲染

- **WHEN** CLAUDE.md Files Section 渲染
- **THEN** SHALL 按 `scope` 分为 Global / Project / Directory 三组
- **AND** Global 组 SHALL 聚合 `scope == "enterprise"` 与 `scope == "user"` 的所有文件
- **AND** Project 组 SHALL 包含 `scope == "project"` 的文件
- **AND** Directory 组 SHALL 包含 `scope == "directory"` 的文件
- **AND** 任一组无文件时 SHALL NOT 渲染该组的 header 与 tree

#### Scenario: 目录树渲染

- **WHEN** 某一分组下有多个文件
- **THEN** SHALL 构建目录树，按路径层级递归渲染，目录可折叠/展开

#### Scenario: 文件节点信息

- **WHEN** 目录树中的文件节点渲染
- **THEN** SHALL 显示文件名和估计 token 数

#### Scenario: 目录排序

- **WHEN** 同级目录和文件排列
- **THEN** 文件 SHALL 排在目录之前，同类按名称字母排序

## ADDED Requirements

### Requirement: Context Panel turn 锚点导航

Context Panel 内每条 injection SHALL 提供一个跳转动作，把 SessionDetail 主视图滚动到对应 `AIChunk` 容器（按 `aiGroupId == chunkId` 匹配 `data-chunk-id` DOM 属性）。点击 `ToolOutputs` Section 内某条 tool breakdown SHALL 先确保该 chunk 展开（`expandedChunks` 含 `chunkId`），再滚到 chunk，再滚到该 tool 子节点（按 `data-tool-use-id == toolUseId` 匹配）。点击 `UserMessageInjection` SHALL 滚到该 turn 紧邻前的 `UserChunk`。

#### Scenario: 点击 injection 滚到 AIChunk

- **WHEN** 用户点击 Category 视图任一非 user-message Section 的 injection 行
- **THEN** SHALL 把对应 `chunkId` 加入 `expandedChunks`（已在则跳过）
- **AND** SHALL `await tick()` 一次后 `scrollIntoView({ block: "center", behavior: "smooth" })` 滚动到 `[data-chunk-id="<aiGroupId>"]` 节点

#### Scenario: 点击 tool breakdown 行展开并滚到 tool

- **WHEN** 用户点击 Tool Outputs Section 内某 tool 行
- **THEN** SHALL 先把对应 AIChunk 的 `chunkId` 加入 `expandedChunks`（已在则跳过）
- **AND** SHALL `await tick()` 一次后滚动到该 chunk
- **AND** SHALL 再次 `await tick()` 后滚动到 `[data-tool-use-id="<toolUseId>"]` 节点

#### Scenario: 点击 user message injection 滚到 UserChunk

- **WHEN** 用户点击 User Messages Section 某条 user-message injection
- **THEN** SHALL 滚到与该 `aiGroupId` 相邻、紧贴其前的 `UserChunk` 容器（按时序在 `chunks` 数组中匹配）；若无前置 UserChunk 则 SHALL 退化为滚到该 AIChunk

#### Scenario: SessionDetail 渲染 chunk 时挂 DOM 锚点

- **WHEN** SessionDetail 渲染任意 `Chunk`
- **THEN** chunk 容器节点 SHALL 带 `data-chunk-id={chunk.chunkId}` 属性
- **AND** AIChunk 内每个 `ToolExecution` 渲染节点 SHALL 带 `data-tool-use-id={exec.toolUseId}` 属性

### Requirement: Context Panel Phase Selector

Context Panel Header SHALL 在 `SessionDetail.phaseInfo.phases.length > 1` 时显示 Phase 切换下拉控件；下拉项为 `Latest` + `Phase 1` + `Phase 2` + ... + `Phase N`（N = phases.length）。默认选中 `Latest`（对应内部 `selectedPhase = null`）。选中具体 phase `N` 时，Context Panel SHALL 从 `SessionDetail.injectionsByPhase[N]` 直接读取该 phase 的完整 accumulated injections；选中 `Latest` 时 SHALL 显示 `injectionsByPhase[最大 phaseNumber]`（即原 `contextInjections` 字段内容）。Context Panel Header `Visible: ~Xk tokens` SHALL 按当前过滤后的 injections 计算（Latest 即原行为）。

#### Scenario: 单 phase 会话不显示 selector

- **WHEN** `SessionDetail.phaseInfo` 缺失或 `phaseInfo.phases.length <= 1`
- **THEN** Context Panel Header SHALL NOT 渲染 Phase Selector

#### Scenario: 多 phase 会话默认 Latest

- **WHEN** `phaseInfo.phases.length > 1` 且 panel 首次打开
- **THEN** Phase Selector SHALL 显示并默认选中 `Latest`
- **AND** Context Panel SHALL 展示 `injectionsByPhase[最大 phaseNumber]`

#### Scenario: 切换到具体 phase 仅展示该 phase injections

- **WHEN** 用户从 Phase Selector 选中 `Phase 2`（`phaseNumber == 2`）
- **THEN** Context Panel SHALL 直接取 `SessionDetail.injectionsByPhase["2"]` 作为输入
- **AND** 不在该 phase 的 injections SHALL 不出现在任何 Section 中
- **AND** Header `Visible: ~Xk tokens` SHALL 计算该 phase injections 的 token 总和

#### Scenario: 选中的 phase 无 injections

- **WHEN** 选中某 phase 后 `injectionsByPhase[N]` 为空数组或 undefined
- **THEN** Context Panel body SHALL 显示占位文案"本 phase 无 injection"
- **AND** 所有 Section SHALL NOT 渲染
- **AND** Header `Visible` SHALL 显示 `~0`
