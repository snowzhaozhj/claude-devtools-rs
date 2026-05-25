## MODIFIED Requirements

### Requirement: Diagnostics tab 暴露 telemetry 快照

Settings 页面 SHALL 在 section 导航中新增 `Diagnostics` tab，与 `General` / `Notifications` 同级。Tab 挂载时 SHALL 调用一次 `getTelemetrySnapshot()` IPC 拿当前快照。

Tab 内容 SHALL 包含四个区域：

1. **顶部仪表盘卡片**（4 个）：cache hit rate、IPC error rate、panic count、SSH 重连次数；数值取自 telemetry snapshot 的 counter（`metadata.cache.hit` / `cdt_api.error` / `cdt_api.warn` / `panic.recovered` / `cdt_ssh.reconnect`）。
2. **延迟分布柱状图**：渲染 `histograms["ipc.list_sessions.duration_ns"]` 与 `histograms["ipc.get_session_detail.duration_ns"]` 的 32 个 power-of-2 bucket；图下方文字标 p50 / p95 / p99 数值，**MUST** 在数值旁注明"power-of-2 bucket upper bound（实际值 ≤ 此值，最坏 2x 偏差）"，避免用户误读为精确测量。
3. **最近 events 列表**：表格渲染 `recentEvents[]`（最多 100 条），列为 timestamp / kind / fields，按 timestamp 倒序。
4. **复制完整 snapshot 按钮**：点击 SHALL 把 `JSON.stringify(snapshot, null, 2)` 写入系统剪贴板并显示 toast"已复制"。

数据获取策略：

- Tab 首次 mount 时拉一次 snapshot；可显示 loading 中间态。
- 提供"刷新"按钮触发再拉一次；按钮按下到数据返回期间 SHALL 保留旧数据展示，避免闪屏。
- SHALL NOT 实现轮询 / 自动刷新——避免抢主线程；用户主动 pull 即可。

Tab 仅读不写，SHALL NOT 暴露任何修改 telemetry 状态的操作。

#### Scenario: 用户打开 Diagnostics tab

- **WHEN** 用户在 Settings 页 sidebar 点击 `Diagnostics` 项
- **THEN** 系统 SHALL 切换到 Diagnostics tab 并调一次 `getTelemetrySnapshot()` IPC
- **AND** SHALL 渲染 4 个仪表盘卡片 + 2 个延迟分布柱状图 + 最近 events 表格 + 复制按钮
- **AND** SHALL 在 1 秒内显示数据（loading 中间态可接受）

#### Scenario: 用户点击复制按钮

- **WHEN** 用户在 Diagnostics tab 顶部点击"复制完整 snapshot"按钮
- **THEN** 系统 SHALL 把 `JSON.stringify(snapshot, null, 2)` 写入系统剪贴板
- **AND** SHALL 显示 toast"已复制"持续 2 秒
- **AND** snapshot JSON SHALL 包含完整 schemaVersion / counters / histograms / recentEvents 字段

#### Scenario: 用户点击刷新按钮

- **WHEN** 用户在 Diagnostics tab 点击刷新按钮
- **THEN** 系统 SHALL 重新调 `getTelemetrySnapshot()` 拿新数据
- **AND** 在新数据返回前 SHALL 保持旧仪表盘 / 柱状图 / events 列表的渲染
- **AND** 新数据到达后 SHALL in-place 替换数值（不经"loading..."中间态）

#### Scenario: tab 仅读不写

- **WHEN** 用户在 Diagnostics tab 任意操作（除复制 / 刷新外）
- **THEN** 系统 SHALL 不提供"重置 counter"或"清空 events"按钮
- **AND** SHALL 不调用任何修改 telemetry 状态的 IPC

### Requirement: Keyboard Shortcuts Section

Settings 页面 SHALL 在 Section 导航中新增独立 tab "键盘快捷键"，与 General / Notifications / Connection 平级。该 tab SHALL 在所有运行模式（含 standalone、SSH、HTTP server）下可见。

Tab 内容 SHALL 按 `keyboard-shortcuts` capability 的 `ShortcutSpec.category`（`global` / `tabs` / `sidebar` / `search` / `session`）分组渲染；每个 category SHALL 用统一的视觉层级（标题字号 / 行间距遵循 `DESIGN.md::The Tool Density Rule`）。每行 SHALL 展示：

- 左：`description`（自然语言中文）
- 中：当前 binding 的可视化表达（mono 字体，遵循 `DESIGN.md::The Machine Information Rule`）
- 右：弱化的"重置默认"按钮（仅当当前 binding 与 default 不一致时启用）

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

录键控件 SHALL 提供 idle / recording / conflict 三态切换：

- **idle**：neutral surface + 1px border + mono 当前 binding；hover 显示 tooltip "点击修改"
- **recording**：accent 1px border + secondary spinner + placeholder "按下新的快捷键..."；进入 recording 时 SHALL 把全局快捷键 dispatcher suspend，使录键期间已注册的快捷键 SHALL NOT 被触发
- **conflict**：warning 1px border + warning bg + mono 新 binding + 文案 "与 `<other-shortcut-description>` 冲突"；保存按钮 SHALL disabled

录键状态机：

- 进入 recording 后焦点 SHALL trap 在录入器内
- recording 期间 keydown 事件 SHALL `event.preventDefault()` 阻止字符落入 input
- 当一次 keydown 含完整修饰键 + 主键时 SHALL commit binding 并查冲突；冲突非 null → 进 conflict 态、保存 disabled；无冲突 → 切回 idle 显示新 binding，外层 panel 启用保存按钮
- 录键期间用户按 Escape SHALL 取消录入、恢复 idle 显示原 binding，并恢复全局 dispatcher

#### Scenario: 进入 recording 态

- **WHEN** 用户点击 idle 状态的录键控件
- **THEN** 控件 SHALL 切到 recording 态（accent border + spinner）
- **AND** 焦点 SHALL trap 在该控件内
- **AND** 全局快捷键 dispatcher SHALL 被 suspend

#### Scenario: 录入新 binding

- **WHEN** 用户在 recording 态按下 `mod+shift+P`（无冲突）
- **THEN** 控件 SHALL 切回 idle 态，显示 mono `⇧⌘P`（mac）或 `Ctrl+Shift+P`（其他）
- **AND** 全局快捷键 dispatcher SHALL 恢复
- **AND** 外层 ShortcutRow SHALL 反映新值，"重置默认"按钮 SHALL 启用

#### Scenario: 录入冲突 binding

- **WHEN** 用户在 recording 态按下 `mod+B`（已被 `sidebar.toggle` 占用）
- **THEN** 控件 SHALL 切到 conflict 态
- **AND** 文案 SHALL 显示 "与 切换 Sidebar 折叠 (⌘B) 冲突"
- **AND** 保存按钮 SHALL disabled

#### Scenario: Escape 取消录入

- **WHEN** 用户在 recording 态按下 Escape
- **THEN** 控件 SHALL 切回 idle 态、显示原 binding
- **AND** 全局快捷键 dispatcher SHALL 恢复

#### Scenario: 录键期间不触发已注册快捷键

- **WHEN** 用户在 recording 态按下 `mod+B`
- **AND** `mod+B` 已被 `sidebar.toggle` 占用
- **THEN** Sidebar SHALL NOT 切换折叠状态（dispatcher 处于 suspend）
- **AND** 控件 SHALL 进入 conflict 态展示冲突

### Requirement: Keyboard Shortcut 持久化与恢复

修改 SHALL 通过 `Save` 按钮显式提交（不自动保存，不在录键 commit 时 debounce 自动写）；点 Save 时 SHALL 单次 IPC 写入完整 `keyboardShortcuts` HashMap（包含本次 panel 内全部 pendingOverrides），并在 IPC resolved 后一次性把 registry 内存 keymap 切到新值。IPC 失败 SHALL 回滚 pendingOverrides 与 registry，UI 显示 inline 错误。

每行 SHALL 提供"重置默认"按钮（仅当 currentBinding ≠ defaultBinding 时启用），点击 SHALL 把该 ID 的 override 从 `keyboardShortcuts` 配置中移除。Panel 顶部 SHALL 提供"重置全部"按钮，点击 SHALL 弹确认对话框（"将所有快捷键恢复为默认值，已自定义的将丢失"），确认后清空整个 `keyboardShortcuts` HashMap。

#### Scenario: 单条重置默认

- **WHEN** 用户在某行点击"重置默认"按钮
- **THEN** 该 ID 的 override SHALL 从 `keyboardShortcuts` 配置中移除
- **AND** registry 内存 keymap 该 ID SHALL 恢复为 builtin default
- **AND** UI 该行 SHALL 显示 default binding，"重置默认"按钮 SHALL disabled

#### Scenario: 重置全部

- **WHEN** 用户点击"重置全部"按钮并确认
- **THEN** `keyboardShortcuts` 配置 SHALL 被清空
- **AND** registry SHALL 重新走纯 builtin defaults
- **AND** 所有行 SHALL 显示 default binding

#### Scenario: 关闭 Settings 不丢未保存改动

- **WHEN** 用户在录键控件录入新 binding 但未点 Save 就切到其他 Section
- **THEN** Panel 顶部 SHALL 显示未保存提示条 + "保存" / "丢弃" 按钮
- **AND** 用户点击"丢弃"或离开 Settings SHALL 回滚所有 pending 修改

#### Scenario: 保存失败回滚

- **WHEN** 用户点击 Save，IPC 失败
- **THEN** registry 内存 keymap SHALL 回滚到改动前状态
- **AND** UI SHALL 显示 inline 错误："保存失败：<reason>"
