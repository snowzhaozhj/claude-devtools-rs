# tab-management Spec Delta

## MODIFIED Requirements

### Requirement: Pane 生命周期

系统 SHALL 维护 `paneLayout: { panes: Pane[]; focusedPaneId: string }` 作为 tab 状态的唯一真相源。初始状态 SHALL 为单 pane，MAX_PANES SHALL 为 4。用户 SHALL 可通过 tab 右键菜单 "Split Left" / "Split Right"、Sidebar 会话右键菜单 "Open in New Pane"、或 `keyboard-shortcuts` registry 的 `pane.split` 当前 binding（默认 mac `⌘\` / 其他 `Ctrl+\`）创建新 pane。closePane 操作 SHALL 仅在 `panes.length > 1` 时允许。

`keyboard-shortcuts` registry SHALL 注册以下 spec id 并在 dispatcher 命中时执行对应操作；用户 SHALL 可在 `Settings → Keyboard Shortcuts` 中自定义任一 binding：

- `pane.split`（默认 `mod+\`）：触发 split right
- `pane.focus.next`（默认 `mod+alt+ArrowRight`）/ `pane.focus.prev`（默认 `mod+alt+ArrowLeft`）：循环切换 focused pane
- `tab.switch.<n>`（n=1..9，默认 `mod+1` ~ `mod+9`）：切到 focused pane 的第 n 个 tab；n > 当前 tab 数时静默忽略
- `tab.close`（默认 `mod+W`）：关闭 focused pane 的 active tab，遵循"关闭 tab" Requirement 既有路径
- `tab.next`（默认 `mod+]`）/ `tab.prev`（默认 `mod+[`）：循环切换 focused pane 的 active tab

#### Scenario: 初始单 pane

- **WHEN** 应用启动
- **THEN** `paneLayout.panes.length` SHALL 等于 1，`focusedPaneId` SHALL 指向该唯一 pane

#### Scenario: Split 创建新 pane（向右）

- **WHEN** 用户在 tab 右键菜单选择 "Split Right" 或按下 `pane.split` 当前 binding
- **AND** `paneLayout.panes.length < 4`
- **THEN** 系统 SHALL 创建一个新 pane 插入到当前 pane 右侧，把触发 split 的 tab 移动到新 pane，新 pane SHALL 成为 focused

#### Scenario: Split 达到最大 pane 数上限

- **WHEN** 用户尝试 split 但 `paneLayout.panes.length === 4`
- **THEN** 系统 SHALL NOT 创建新 pane，操作 SHALL 静默忽略或展示禁用视觉

#### Scenario: 关闭 pane

- **WHEN** 用户关闭某个非唯一 pane（通过关闭该 pane 内最后一个 tab 或显式关闭 pane）
- **THEN** 该 pane SHALL 从 `paneLayout.panes` 中移除，相邻 pane 的 widthFraction SHALL 重新均分，`focusedPaneId` SHALL 切到相邻 pane

#### Scenario: 唯一 pane 不可关闭

- **WHEN** 只剩一个 pane 且用户关闭其最后一个 tab
- **THEN** 该 pane SHALL 保留（activeTabId 变为 null），`focusedPaneId` 保持指向它

#### Scenario: 任一 pane / tab spec id 当前 binding 命中即触发对应操作

- **WHEN** 用户按下任一 pane / tab spec id 当前 binding（白名单：`pane.split` / `pane.focus.next` / `pane.focus.prev` / `tab.switch.1..9` / `tab.close` / `tab.next` / `tab.prev`）
- **AND** `document.activeElement` 不是 `<input>` / `<textarea>` / `[contenteditable="true"]`
- **THEN** registry dispatcher SHALL 命中该 spec
- **AND** 系统 SHALL 执行对应 pane / tab 操作（focus 切换 / tab 切换 / tab 关闭 / split 创建），副作用与等效鼠标操作一致
- **AND** 当操作前提不成立时 SHALL 静默忽略，包括但不限于：`pane.focus.next` / `pane.focus.prev` 在 `panes.length < 2` 时；`tab.next` / `tab.prev` 在 focused pane 仅 1 个 tab 时；`tab.switch.<n>` 在 n 超出当前 pane tab 数时；`tab.close` 在 focused pane 无 active tab 时；`pane.split` 在 `panes.length === 4` 时

#### Scenario: 用户改自定义 binding 后生效

- **WHEN** 用户在 `Settings → Keyboard Shortcuts` 把任一 spec id 的 binding 改为新组合
- **THEN** 后续按下新组合 SHALL 触发该 spec id 对应操作
