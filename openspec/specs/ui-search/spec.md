# ui-search Specification

## Purpose

定义两种搜索模式的行为契约：Cmd+F 会话内文本搜索（已实现）和 Cmd+K 全局搜索 Command Palette（待实现）。

## Requirements

### Requirement: Cmd+F 激活会话内搜索

用户在 SessionDetail 视图中按 Cmd+F（或 Ctrl+F）SHALL 显示搜索栏。搜索栏 SHALL 出现在会话内容上方，输入框 SHALL 自动获得焦点。

#### Scenario: 快捷键激活
- **WHEN** 用户在 SessionDetail 视图中按 Cmd+F 或 Ctrl+F
- **THEN** SearchBar SHALL 变为可见，输入框 SHALL 自动 focus 并 select 已有文本

#### Scenario: 重复按 Cmd+F
- **WHEN** SearchBar 已可见时用户再次按 Cmd+F
- **THEN** 输入框 SHALL 重新获得 focus 并 select 全部文本

### Requirement: 会话内文本搜索与高亮

输入搜索文本后，系统 SHALL 在 conversation 容器的可见文本节点中查找所有匹配项，并通过 `<mark>` 元素高亮显示。搜索 SHALL 大小写不敏感。

#### Scenario: 输入搜索文本
- **WHEN** 用户在搜索框中输入文本（300ms debounce 后）
- **THEN** conversation 容器中所有匹配的文本片段 SHALL 被 `<mark>` 元素包裹高亮

#### Scenario: 大小写不敏感
- **WHEN** 搜索文本为 "error" 且内容中包含 "Error"、"ERROR"
- **THEN** 所有变体 SHALL 均被高亮

#### Scenario: 跳过代码块
- **WHEN** 搜索执行时
- **THEN** `<pre>`、`<code>`、`<style>`、`<script>` 标签内的文本 SHALL 不参与匹配

#### Scenario: 无匹配结果
- **WHEN** 搜索文本在 conversation 中无匹配
- **THEN** 搜索栏 SHALL 显示 "无结果"

### Requirement: 搜索结果导航

用户 SHALL 能通过 Enter/Shift+Enter 或导航按钮在匹配项间循环移动。当前匹配项 SHALL 有视觉区分，并自动滚动到视口中心。

#### Scenario: Enter 跳到下一个匹配
- **WHEN** 用户按 Enter
- **THEN** 当前索引 SHALL 前进到下一个匹配，该匹配项 SHALL 滚动到视口中心

#### Scenario: Shift+Enter 跳到上一个匹配
- **WHEN** 用户按 Shift+Enter
- **THEN** 当前索引 SHALL 回退到上一个匹配

#### Scenario: 循环导航
- **WHEN** 当前在最后一个匹配按 Enter
- **THEN** SHALL 回到第一个匹配（循环）

#### Scenario: 搜索计数显示
- **WHEN** 存在匹配结果
- **THEN** 搜索栏 SHALL 显示 "当前索引 / 总数" 格式（如 "3 / 12"）

#### Scenario: 当前匹配项高亮
- **WHEN** 导航到某个匹配项
- **THEN** 该 `<mark>` 元素 SHALL 带有 `data-search-current` 属性以区别于其他匹配

### Requirement: 关闭搜索栏

用户按 Esc 或点击关闭按钮 SHALL 关闭搜索栏，清除所有高亮，恢复原始文本。

#### Scenario: Esc 关闭
- **WHEN** 搜索栏可见时用户按 Esc
- **THEN** 搜索栏 SHALL 隐藏，所有 `<mark>` 高亮 SHALL 被移除并恢复原始文本节点，搜索查询 SHALL 被清空

#### Scenario: 点击关闭按钮
- **WHEN** 用户点击搜索栏的关闭按钮
- **THEN** 行为 SHALL 与按 Esc 相同

### Requirement: 搜索状态 per-tab 隔离

搜索可见性 SHALL 作为 per-tab UI 状态的一部分。切换 tab 时当前 tab 的搜索状态 SHALL 保存，切回时 SHALL 恢复。

#### Scenario: 切换 tab 时清理搜索
- **WHEN** tab A 有激活的搜索，用户切换到 tab B
- **THEN** tab A 的 searchVisible 状态 SHALL 保存，tab B SHALL 使用自己的搜索状态

### Requirement: Command Palette 触发

用户 SHALL 可以通过 Cmd+K（macOS）/ Ctrl+K 快捷键在任意界面打开 Command Palette 模态面板。

#### Scenario: Cmd+K 打开
- **WHEN** 用户按下 Cmd+K（或 Ctrl+K）
- **THEN** SHALL 弹出模态面板，搜索框自动聚焦

#### Scenario: Esc 关闭
- **WHEN** Command Palette 打开时用户按 Escape 或点击遮罩
- **THEN** 面板 SHALL 关闭，焦点回到之前的内容

#### Scenario: 重复打开
- **WHEN** Command Palette 已打开时再次按 Cmd+K
- **THEN** SHALL 关闭面板（toggle 行为）

### Requirement: Command Palette 搜索模式

Command Palette SHALL 以组合视图展示搜索结果：项目区 + 会话区。搜索为本地过滤。

#### Scenario: 项目过滤
- **WHEN** 用户输入文本
- **THEN** 项目区 SHALL 显示 displayName 或 path 包含查询文本的项目（大小写不敏感），最多 5 条

#### Scenario: 会话过滤
- **WHEN** 用户输入文本且已选中项目
- **THEN** 会话区 SHALL 显示 title 或 sessionId 包含查询文本的会话（大小写不敏感），最多 20 条

#### Scenario: 无选中项目时隐藏会话区
- **WHEN** 无选中项目
- **THEN** 会话区 SHALL 不显示

#### Scenario: 空查询
- **WHEN** 搜索框为空
- **THEN** SHALL 显示全部项目和全部会话（受数量限制）

### Requirement: Command Palette 键盘导航

Command Palette 结果列表 SHALL 支持完整键盘导航。

#### Scenario: 上下键选择
- **WHEN** 用户按 ↓/↑
- **THEN** 选中高亮 SHALL 在结果列表中移动，跨越项目/会话两个区域

#### Scenario: Enter 选择项目
- **WHEN** 高亮项为项目时用户按 Enter
- **THEN** SHALL 选中该项目（Sidebar 切换）并关闭面板

#### Scenario: Enter 选择会话
- **WHEN** 高亮项为会话时用户按 Enter
- **THEN** SHALL 通过 Tab 系统打开该会话并关闭面板

#### Scenario: 查询变化重置选中
- **WHEN** 搜索文本变化
- **THEN** 选中索引 SHALL 重置为 0
