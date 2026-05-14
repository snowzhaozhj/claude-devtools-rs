## ADDED Requirements

### Requirement: Sidebar uses paginated current-project session loading

Sidebar 会话列表 SHALL 对齐原版 `claude-devtools` 的当前项目分页加载语义：首次选中项目或 worktree 时默认只请求当前 `selectedProjectId` 的第一页 sessions，默认 `pageSize` SHALL 为 20；用户滚动接近列表末尾或显式继续加载时，前端 SHALL 使用后端返回的 `nextCursor` 请求下一页。

Sidebar SHALL NOT 为了首屏渲染同步加载当前项目完整会话历史，也 SHALL NOT 在 Dashboard 首页为所有项目触发 sessions 列表加载。已加载页之间的合并 SHALL 按 `sessionId` 去重，并保持 `SessionSummary` 列表按 `timestamp` / mtime 倒序显示。

#### Scenario: 首屏只请求当前项目第一页

- **WHEN** 用户选中 `projectA`
- **THEN** Sidebar SHALL 调用 `list_sessions(projectA, { pageSize: 20, cursor: null })`
- **AND** Sidebar SHALL 在第一页返回后立即渲染已有 sessions
- **AND** Sidebar SHALL NOT 等待 `projectA` 完整历史加载完成

#### Scenario: 滚动触发下一页

- **WHEN** 第一页响应包含 `nextCursor`
- **AND** 用户滚动接近会话列表末尾
- **THEN** Sidebar SHALL 调用 `list_sessions(projectA, { pageSize: 20, cursor: nextCursor })`
- **AND** 新页 SHALL merge 到已加载列表中且按 `sessionId` 去重

#### Scenario: Dashboard 不加载所有项目 sessions

- **WHEN** 应用无 active tab 并显示 Dashboard 项目概览
- **THEN** Dashboard SHALL NOT 为每个项目调用 `list_sessions`
- **AND** sessions 列表加载 SHALL 只在用户选中或展开具体项目时发生

### Requirement: Pinned and hidden sessions reconcile outside the first page

Sidebar SHALL NOT 假设 pinned 或 hidden session 一定位于第一页。当前项目存在 pinned/hidden session ids 且这些 ids 未出现在已加载分页结果中时，Sidebar SHALL 使用按 `sessionId` 补拉的 API 获取对应 light `SessionSummary`，再与分页列表合并。不存在或不属于当前项目的 ids SHALL 被忽略。

Hidden session 的 UI 过滤语义保持既有行为；pinned session 的视觉位置保持既有行为。本 Requirement 只规定数据补齐来源，不重新定义 Pin/Hide 交互。

#### Scenario: pinned session 不在第一页时仍可显示

- **WHEN** `projectA` 的 pinned id `sid-old` 不在第一页 `list_sessions` 响应中
- **AND** `sid-old` 存在于 `projectA`
- **THEN** Sidebar SHALL 通过按 id 补拉获得 `sid-old` 的 `SessionSummary`
- **AND** pinned 区域或列表 SHALL 能显示该 session

#### Scenario: hidden session 不在第一页时仍能过滤

- **WHEN** `projectA` 的 hidden id `sid-hidden` 不在第一页 `list_sessions` 响应中
- **AND** 后续分页或按 id 补拉返回 `sid-hidden`
- **THEN** Sidebar SHALL 按既有 hidden 规则过滤该 session

## MODIFIED Requirements

### Requirement: 完整加载分页会话历史

Sidebar 默认会话列表 SHALL 使用当前项目的分页结果渐进展示 sessions，而不是为了首屏或普通浏览同步加载完整会话历史。若 `list_sessions` 响应包含 `nextCursor`，Sidebar SHALL 在用户滚动接近列表末尾或显式请求更多时继续分页；Command Palette 需要覆盖完整历史搜索时，MUST 使用 `session-search` 或显式承担逐页加载成本的专用路径，不能要求 Sidebar 首屏预先加载完整历史。

实现 SHALL NOT 使用“扩大 `pageSize` 并从头重拉直到 `nextCursor = null`”作为 Sidebar 首屏策略。实现 SHALL 保证每次分页返回页的 `session-metadata-update` 扫描不会因为后续页加载而错误覆盖或丢失已加载页的 metadata patch。

#### Scenario: Sidebar 首屏不加载默认第一页之后的旧会话

- **WHEN** 当前项目有 51 条会话，且 `list_sessions(projectId, { pageSize: 20, cursor: null })` 返回第一页并带 `nextCursor`
- **THEN** Sidebar SHALL 立即显示第一页 sessions
- **AND** Sidebar SHALL NOT 为了首屏显示第 51 条旧会话而同步加载完整 51 条

#### Scenario: Sidebar 滚动后加载默认第一页之后的旧会话

- **WHEN** 当前项目有 51 条会话，且用户持续滚动到需要更多 sessions
- **THEN** Sidebar SHALL 使用 `nextCursor` 继续请求后续页
- **AND** 第 51 条旧会话 SHALL 在其所在页加载后出现在会话列表中

#### Scenario: Command Palette 全历史搜索不依赖 Sidebar 首屏完整数组

- **WHEN** 当前项目有 51 条会话，且第 51 条旧会话的 title 匹配 Command Palette 查询文本
- **THEN** Command Palette SHALL 通过 `session-search` 或等价显式搜索路径覆盖该旧会话
- **AND** 该能力 SHALL NOT 要求 Sidebar 首屏已经加载第 51 条旧会话

#### Scenario: 会话数量变化时不扩大首屏请求直到完整

- **WHEN** 前端第一次调用 `list_sessions(projectId, { pageSize: 20, cursor: null })` 得到 `nextCursor`
- **AND** 项目在后续分页前新增会话
- **THEN** Sidebar SHALL 继续使用 cursor 分页或刷新当前页
- **AND** Sidebar SHALL NOT 基于 `total` 不断扩大 `pageSize` 从头请求直到完整