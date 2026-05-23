## ADDED Requirements

### Requirement: Keyboard Shortcuts Section

`Settings.svelte` SHALL 在现有 Section 导航中新增独立 tab "键盘快捷键"，与 General / Notifications / Connection 平级（与 `### Requirement: Settings Section 导航` 既有列表合并展示）。该 tab SHALL 在所有运行模式（含 standalone、SSH、HTTP server）下可见——quick fix 与 server-mode 当前不区分该 tab 的可见性。

Section 内容 SHALL 由组件 `KeyboardShortcutsPanel.svelte` 提供，按 `keyboard-shortcuts` capability 的 `ShortcutSpec.category`（`global` / `tabs` / `sidebar` / `search` / `session`）分组渲染；每个 category SHALL 用 14px medium weight 标题 + 16px 行间距形成层级（遵循 `DESIGN.md::The Tool Density Rule`）。每行（`ShortcutRow.svelte`）SHALL 展示：

- 左：`description`（自然语言中文）
- 中：`KeyRecorderInput.svelte` 显示 `formatShortcut(currentBinding)`（mono 字体，遵循 `DESIGN.md::The Machine Information Rule`）
- 右：弱化的"重置默认"图标按钮（仅当当前 binding 与 default 不一致时启用）

#### Scenario: tab 入口
- **WHEN** 用户打开 Settings 页面
- **THEN** 左侧导航 SHALL 显示 "键盘快捷键" 入口，与 General / Notifications / Connection 平级
- **AND** 默认未选中状态下入口行 SHALL 用 neutral hover bg、不引入 Focus Blue 持久彩色

#### Scenario: 列表分组
- **WHEN** 用户切到 "键盘快捷键" tab
- **THEN** SHALL 按 `global` / `tabs` / `sidebar` / `search` / `session` 顺序渲染 5 个 category 段
- **AND** 每段顶部 SHALL 显示中文 category 名（如 "全局" / "标签页" / "侧栏" / "搜索" / "会话"）
- **AND** 每段下 SHALL 列出该 category 注册的所有 ShortcutSpec 行

### Requirement: Keyboard Shortcut 录键交互

`KeyRecorderInput.svelte` SHALL 提供 idle / recording / conflict 三态切换：

- **idle**：neutral surface + 1px border + mono `formatShortcut(currentBinding)`；hover 显示 tooltip "点击修改"
- **recording**：accent 1px border + 10×10 secondary spinner（遵循 `DESIGN.md::The Static-vs-Live Shape Rule` 的 secondary spinner 缩档）+ placeholder "按下新的快捷键..."；进入 recording 时 SHALL 调用 `registry.suspend()`
- **conflict**：warning 1px border + warning bg + mono 新 binding + 文案 "与 `<other-shortcut-description>` 冲突"；保存按钮 SHALL disabled

录键状态机：

1. 用户点击 idle widget → 进 recording，焦点 trap 在录入器内
2. recording 期间 keydown SHALL 调用 `event.preventDefault()` 阻止字符落入 input
3. 当一次 keydown 含完整修饰键 + 主键（即 `event.key` 不在 `["Meta","Control","Alt","Shift"]`）→ commit binding、调用 `registry.findConflict(binding, currentId)`
4. 若 conflict 非 null → 进 conflict 态、保存 disabled
5. 若无 conflict → 切回 idle 显示新 binding，外层 panel 启用保存按钮
6. 录键期间用户按 Escape → 取消录入，恢复 idle 显示原 binding；调用 `registry.resume()`

#### Scenario: 进入 recording 态
- **WHEN** 用户点击 idle 状态的 KeyRecorderInput
- **THEN** widget SHALL 切到 recording 态（accent border + spinner）
- **AND** 焦点 SHALL trap 在该 widget 内
- **AND** `registry.suspend()` SHALL 被调用（dispatcher 引用计数 +1）

#### Scenario: 录入新 binding
- **WHEN** 用户在 recording 态按下 `mod+shift+P`（无冲突）
- **THEN** widget SHALL 切回 idle 态，显示 mono `⇧⌘P`（mac）或 `Ctrl+Shift+P`（其他）
- **AND** `registry.resume()` SHALL 被调用（引用计数 -1）
- **AND** 外层 ShortcutRow SHALL 反映新值，"重置默认"按钮 SHALL 启用

#### Scenario: 录入冲突 binding
- **WHEN** 用户在 recording 态按下 `mod+B`（已被 `sidebar.toggle` 占用）
- **THEN** widget SHALL 切到 conflict 态
- **AND** 文案 SHALL 显示 "与 切换 Sidebar 折叠 (⌘B) 冲突"
- **AND** 保存按钮 SHALL disabled

#### Scenario: Escape 取消录入
- **WHEN** 用户在 recording 态按下 Escape
- **THEN** widget SHALL 切回 idle 态、显示原 binding
- **AND** `registry.resume()` SHALL 被调用

#### Scenario: 录键期间不触发已注册快捷键
- **WHEN** 用户在 recording 态按下 `mod+B`
- **AND** `mod+B` 已被 `sidebar.toggle` 占用
- **THEN** Sidebar SHALL NOT 切换折叠状态（dispatcher 处于 suspend）
- **AND** widget SHALL 进入 conflict 态展示冲突

### Requirement: Keyboard Shortcut 持久化与恢复

修改 SHALL 通过 `Save` 按钮显式提交（不自动保存，不在录键 commit 时 debounce 自动写）；点 Save 时 SHALL 单次 `set_config` IPC 写入完整 `keyboardShortcuts` HashMap（包含本次 panel 内全部 pendingOverrides），并在 IPC resolved 后一次性把 registry 内存 keymap 切到新值。IPC 失败 SHALL 回滚 pendingOverrides 与 registry，UI 显示 inline 错误。

每行 ShortcutRow SHALL 提供"重置默认"图标按钮（仅当 currentBinding ≠ defaultBinding 时启用），点击 SHALL 把该 ID 的 override 从 `cdt-config::keyboard_shortcuts` 中移除。Panel 顶部 SHALL 提供"重置全部"按钮，点击 SHALL 弹确认对话框（"将所有快捷键恢复为默认值，已自定义的将丢失"），确认后清空整个 `keyboard_shortcuts` HashMap。

#### Scenario: 单条重置默认
- **WHEN** 用户在某 ShortcutRow 点击"重置默认"按钮
- **THEN** 该 ID 的 override SHALL 从 `cdt-config::keyboard_shortcuts` 中移除
- **AND** registry 内存 keymap 该 ID SHALL 恢复为 builtin default
- **AND** UI 该行 SHALL 显示 default binding，"重置默认"按钮 SHALL disabled

#### Scenario: 重置全部
- **WHEN** 用户点击"重置全部"按钮并确认
- **THEN** `cdt-config::keyboard_shortcuts` SHALL 被清空（HashMap empty）
- **AND** registry SHALL 重新 `bootstrap` 走纯 builtin defaults
- **AND** 所有 ShortcutRow SHALL 显示 default binding

#### Scenario: 关闭 Settings 不丢未保存改动
- **WHEN** 用户在 KeyRecorderInput 录入新 binding 但未点 Save 就切到其他 Section
- **THEN** Panel 顶部 SHALL 显示未保存提示条 + "保存" / "丢弃" 按钮
- **AND** 用户点击"丢弃"或离开 Settings SHALL 回滚所有 pending 修改

#### Scenario: 保存失败回滚
- **WHEN** 用户点击 Save，IPC `setKeyboardShortcuts` 失败
- **THEN** registry 内存 keymap SHALL 回滚到改动前状态
- **AND** UI SHALL 显示 inline 错误："保存失败：<reason>"
