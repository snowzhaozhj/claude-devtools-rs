## ADDED Requirements

### Requirement: 项目 git 分支只读栏

SidebarHeader SHALL 在项目名按钮下方渲染一栏只读 git 分支信息（`GitBranch` icon + branch name），值取自 `SessionSummary.gitBranch`。展示规则：优先使用 `activeSessionId` 对应 session 的 `gitBranch`；当 `activeSessionId` 为 `null` 或未在当前 sessions 列表中时，回退到列表第一条 session 的 `gitBranch`（`sessions[0]`，列表已按 timestamp desc 排序）；两者均为 `null` 时该栏 SHALL NOT 渲染（不留空位、不显示 `--` 占位）。

该栏 MUST 跟随 `activeSessionId` 变化即时更新——用户切换不同 session 时若两 session 在不同 git 分支，分支栏 SHALL 反映 active session 的分支。session 元数据通过 `session-metadata-update` 增量 patch 时若该 session 是 active，分支栏 SHALL 同步更新。

#### Scenario: 显示 active session 的 git 分支

- **WHEN** 用户选中 session `s1`（`activeSessionId = "s1"`），`s1.gitBranch = "feat/x"`
- **THEN** SidebarHeader SHALL 在项目名下方渲染 `GitBranch` icon + `feat/x` 文本

#### Scenario: 无 active session 时回退到 sessions[0]

- **WHEN** `activeSessionId = null`
- **AND** `sessions[0].gitBranch = "main"`
- **THEN** SidebarHeader SHALL 渲染 `GitBranch` icon + `main` 文本

#### Scenario: 无任何 gitBranch 时不渲染

- **WHEN** `activeSessionId` 对应 session 与 `sessions[0]` 的 `gitBranch` 均为 `null`
- **THEN** SidebarHeader SHALL NOT 渲染 git 分支栏；项目名按钮下方 SHALL 紧接 session filter bar，不留空位

#### Scenario: 分支栏跟随 active 切换更新

- **WHEN** 用户从 session `s1`（`gitBranch="feat/x"`）切换到 `s2`（`gitBranch="feat/y"`）
- **THEN** 分支栏文本 SHALL 在切换的下一帧更新为 `feat/y`

#### Scenario: 元数据 patch 更新分支显示

- **WHEN** active session `s1` 当前 `gitBranch = null`（骨架态）
- **AND** 后端推送 `session-metadata-update` 含 `sessionId: "s1", gitBranch: "feat/x"`
- **THEN** SidebarHeader 分支栏 SHALL 在 patch 应用后渲染 `feat/x`

### Requirement: 侧栏折叠/展开

Sidebar SHALL 支持折叠（隐藏）与展开两种状态。折叠状态由 `sidebarStore.svelte.ts` 的模块级 runes state 管理（内存级，重启回归默认展开）。

折叠入口 SHALL 提供两条：(1) SidebarHeader 顶部右侧 `PanelLeft` icon 按钮，点击切换；(2) 全局 `Cmd+B`（macOS）/ `Ctrl+B`（其他平台）快捷键 SHALL 切换。展开入口 SHALL 提供 (1) 折叠态下 TabBar 最左侧 `PanelLeft` icon 按钮；(2) 同一快捷键。

折叠时 sidebar SHALL 完全不渲染（不留窄轨道、不留 0 宽度占位 DOM）。展开时 sidebar SHALL 恢复折叠前的宽度（如未拖拽过则为默认宽度）。

#### Scenario: 默认展开

- **WHEN** 应用首次启动
- **THEN** Sidebar SHALL 处于展开状态，宽度为默认值（280px）

#### Scenario: 折叠按钮隐藏 Sidebar

- **WHEN** 用户点击 SidebarHeader 顶部 `PanelLeft` 按钮
- **THEN** Sidebar 整体 DOM SHALL 不再渲染；TabBar 最左侧 SHALL 出现展开按钮

#### Scenario: 展开按钮恢复 Sidebar

- **WHEN** 折叠态下用户点击 TabBar 最左侧 `PanelLeft` 按钮
- **THEN** Sidebar SHALL 重新渲染，宽度恢复为折叠前的值

#### Scenario: 快捷键切换

- **WHEN** 用户按 `Cmd+B`（macOS）或 `Ctrl+B`（其他平台）
- **THEN** 折叠状态 SHALL 切换（展开 ↔ 折叠），等价于点击 PanelLeft 按钮

#### Scenario: 快捷键在折叠态下仍生效

- **WHEN** Sidebar 当前折叠
- **AND** 用户按 `Cmd+B`
- **THEN** Sidebar SHALL 重新展开（快捷键监听挂在 App 顶层，不依赖 Sidebar 自身渲染）

#### Scenario: 重启后回归展开

- **WHEN** 用户折叠 Sidebar 后关闭应用并重新启动
- **THEN** Sidebar SHALL 处于展开状态（折叠状态不持久化，与 sidebar 宽度同维度）
