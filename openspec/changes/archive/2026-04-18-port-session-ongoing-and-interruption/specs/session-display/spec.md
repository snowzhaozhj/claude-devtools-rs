## ADDED Requirements

### Requirement: Ongoing banner at session bottom

SessionDetail SHALL 在 `detail.isOngoing === true` 时于对话流底部渲染
`<OngoingBanner />`——内容为蓝色背景的胶囊区块，含 spinner 图标与
文案 "Session is in progress..."。`isOngoing` 为 false / undefined
时 SHALL NOT 渲染该横幅。横幅的出现与消失 MUST 随自动刷新
（`Auto refresh on file change` Requirement）切换，无需用户手动操作。

#### Scenario: Banner shown when ongoing
- **WHEN** 当前打开的 session `detail.isOngoing === true`
- **THEN** SessionDetail SHALL 在对话容器尾部渲染 `<OngoingBanner />`，
  图标 SHALL 有 `animate-spin` 动画，文案 SHALL 为 "Session is in
  progress..."

#### Scenario: Banner hidden when ended
- **WHEN** `detail.isOngoing === false`
- **THEN** SessionDetail SHALL NOT 渲染 `<OngoingBanner />`

#### Scenario: Banner toggled by auto refresh
- **WHEN** session 收到一个 `file-change` 事件并刷新后，后端返回的
  `detail.isOngoing` 从 `true` 变为 `false`（用户按 Esc 插入
  interrupt marker）
- **THEN** 横幅 SHALL 在该次重渲染中消失，不需要用户切 tab 或其他操作

### Requirement: Interruption semantic step rendering

AIChunk 的 `semantic_steps` 中 `kind === "interruption"` 的项 SHALL
以独立的红色 badge 块渲染——文案 "Session interrupted by user"
（或取 step.text 作 tooltip）。该块 SHALL 位于 AIChunk 语义步骤的
自然位置，不参与工具区域展开/折叠切换。

#### Scenario: Interruption step rendering position
- **WHEN** 一个 `AIChunk.semantic_steps` 末尾含一个
  `{ kind: "interruption", text: "[Request interrupted by user for tool use]",
  timestamp: "..." }` 条目
- **THEN** 该 AIChunk 正文（非工具展开区）SHALL 渲染一行红色背景的
  "Session interrupted by user" 块，位置在本 chunk 最末一个 Thinking
  / Text 步骤之后

#### Scenario: Interruption block does not depend on tools-expanded state
- **WHEN** 用户未展开 AIChunk 的工具区域
- **THEN** 中断块 SHALL 仍然可见（与 Thinking / Text 步骤同层次）

#### Scenario: No interruption step means no block
- **WHEN** AIChunk.semantic_steps 不含 `kind === "interruption"` 的条目
- **THEN** SessionDetail SHALL NOT 渲染任何中断相关的块
