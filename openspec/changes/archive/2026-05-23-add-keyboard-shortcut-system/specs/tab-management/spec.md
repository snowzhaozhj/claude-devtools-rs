## MODIFIED Requirements

### Requirement: Pane 生命周期

系统 SHALL 维护 `paneLayout: { panes: Pane[]; focusedPaneId: string }` 作为 tab 状态的唯一真相源。初始状态 SHALL 为单 pane，MAX_PANES SHALL 为 4。用户 SHALL 可通过 tab 右键菜单 "Split Left" / "Split Right"、Sidebar 会话右键菜单 "Open in New Pane"、或 `keyboard-shortcuts` registry 的 `pane.split` 当前 binding（默认 mac `⌘\\` / 其他 `Ctrl+\\`）创建新 pane。closePane 操作 SHALL 仅在 `panes.length > 1` 时允许。

`pane.split` / `pane.focus.next`（默认 `mod+alt+ArrowRight`）/ `pane.focus.prev`（默认 `mod+alt+ArrowLeft`）/ `tab.switch.<n>`（默认 `mod+1` ~ `mod+9`，n=1..9）/ `tab.close`（默认 `mod+W`）/ `tab.next`（默认 `mod+]`）/ `tab.prev`（默认 `mod+[`）SHALL 由 `keyboard-shortcuts` registry 注册并 dispatch；用户 SHALL 可在 `Settings → Keyboard Shortcuts` 中自定义。

#### Scenario: 初始单 pane
- **WHEN** 应用启动
- **THEN** `paneLayout.panes.length` SHALL 等于 1，`focusedPaneId` SHALL 指向该唯一 pane

#### Scenario: Split 创建新 pane（向右）
- **WHEN** 用户在 tab 右键菜单选择 "Split Right" 或按下 `pane.split` 当前 binding
- **AND** `paneLayout.panes.length < 4`
- **THEN** 系统 SHALL 创建一个新 pane 插入到当前 pane 右侧，把触发 split 的 tab 移动到新 pane，新 pane SHALL 成为 focused

#### Scenario: Split 达到 MAX_PANES 上限
- **WHEN** 用户尝试 split 但 `paneLayout.panes.length === 4`
- **THEN** 系统 SHALL NOT 创建新 pane，操作 SHALL 静默忽略或展示禁用视觉

#### Scenario: 关闭 pane
- **WHEN** 用户关闭某个非唯一 pane（通过关闭该 pane 内最后一个 tab 或显式关闭 pane）
- **THEN** 该 pane SHALL 从 `paneLayout.panes` 中移除，相邻 pane 的 widthFraction SHALL 重新均分，`focusedPaneId` SHALL 切到相邻 pane

#### Scenario: 唯一 pane 不可关闭
- **WHEN** 只剩一个 pane 且用户关闭其最后一个 tab
- **THEN** 该 pane SHALL 保留（activeTabId 变为 null），`focusedPaneId` 保持指向它

#### Scenario: 切换 focused pane 快捷键
- **WHEN** PaneView 有 ≥ 2 个 pane
- **AND** 用户按下 `pane.focus.next` 当前 binding
- **AND** `document.activeElement` 不是 `<input>` / `<textarea>` / `[contenteditable="true"]`
- **THEN** registry dispatcher SHALL 命中 `pane.focus.next` spec
- **AND** `focusedPaneId` SHALL 切到下一个 pane（最后一个时循环到第一个）
- **AND** Sidebar 高亮与 IPC 作用域 SHALL 立即跟随

#### Scenario: 切换 tab 快捷键
- **WHEN** focused pane 内有 N 个 tab
- **AND** 用户按下 `tab.switch.<n>` 当前 binding（n=1..9）
- **AND** n ≤ N
- **THEN** focused pane 的 activeTab SHALL 切到第 n 个 tab
- **AND** n > N 时 SHALL 静默忽略

#### Scenario: 关闭 tab 快捷键
- **WHEN** 用户按下 `tab.close` 当前 binding
- **AND** focused pane 有 active tab
- **THEN** 该 active tab SHALL 被关闭（等价于点击 tab 上的关闭按钮）
- **AND** 关闭语义遵循 `### Requirement: 关闭 tab` 中既有 Scenario 路径

#### Scenario: 上一个 / 下一个 tab 快捷键
- **WHEN** focused pane 有 ≥ 2 个 tab
- **AND** 用户按下 `tab.next` 当前 binding
- **THEN** focused pane 的 activeTab SHALL 切到下一个 tab（最后一个时循环到第一个）
- **AND** `tab.prev` 行为对称（第一个时循环到最后一个）
