## MODIFIED Requirements

### Requirement: 侧栏折叠/展开

Sidebar SHALL 支持折叠（隐藏）与展开两种状态。折叠状态由 `sidebarStore.svelte.ts` 的模块级 runes state 管理（内存级，重启回归默认展开）。

折叠入口 SHALL 提供两条：(1) SidebarHeader 顶部右侧 `PanelLeft` icon 按钮，点击切换；(2) **通过 `keyboard-shortcuts` capability 注册的全局快捷键 `sidebar.toggle`**（默认 binding：mac `⌘B` / Win+Linux `Ctrl+B`）SHALL 切换。展开入口 SHALL 提供 (1) 折叠态下 TabBar 最左侧 `PanelLeft` icon 按钮；(2) 同一 `sidebar.toggle` 快捷键。

折叠时 sidebar SHALL 完全不渲染（不留窄轨道、不留 0 宽度占位 DOM）。展开时 sidebar SHALL 恢复折叠前的宽度（如未拖拽过则为默认宽度）。

`sidebar.toggle` 快捷键 SHALL 由用户在 `Settings → Keyboard Shortcuts` 中自定义（覆盖默认 binding）；自定义后 SHALL 立即生效，重启 SHALL 保留。

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

- **WHEN** 用户按下 `sidebar.toggle` 当前 binding（默认 mac `⌘B` / 其他 `Ctrl+B`）
- **AND** `document.activeElement` 不是 `<input>` / `<textarea>` / `[contenteditable="true"]`
- **THEN** `keyboard-shortcuts` registry dispatcher SHALL 命中 `sidebar.toggle` spec 并调用其 handler
- **AND** Sidebar 折叠状态 SHALL 切换（展开 ↔ 折叠），等价于点击 PanelLeft 按钮
- **AND** `event.preventDefault()` SHALL 被调用

#### Scenario: 快捷键在折叠态下仍生效

- **WHEN** Sidebar 当前折叠
- **AND** 用户按下 `sidebar.toggle` 当前 binding
- **THEN** Sidebar SHALL 重新展开（dispatcher 单一 listener 挂在 `document` 顶层，不依赖 Sidebar 自身渲染）

#### Scenario: 用户自定义 binding 后生效

- **WHEN** 用户在 `Settings → Keyboard Shortcuts` 把 `sidebar.toggle` 改为 `mod+shift+B`
- **AND** 保存生效
- **THEN** 后续按下 `mod+shift+B` SHALL 切换 Sidebar 折叠
- **AND** 按下原默认 `mod+B` SHALL NOT 触发折叠（除非另一 spec 占用了 `mod+B`）

#### Scenario: 重启后回归展开

- **WHEN** 用户折叠 Sidebar 后关闭应用并重新启动
- **THEN** Sidebar SHALL 处于展开状态（折叠状态不持久化，与 sidebar 宽度同维度）
