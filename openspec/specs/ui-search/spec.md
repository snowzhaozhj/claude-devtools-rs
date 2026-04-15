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

### Requirement: Command Palette 全局搜索（待实现）

Cmd+K SHALL 打开 Command Palette 浮层，支持跨项目/跨会话的全文搜索。搜索 SHALL 委托后端 `search` API 执行。

#### Scenario: Cmd+K 打开 Command Palette
- **WHEN** 用户按 Cmd+K 或 Ctrl+K
- **THEN** Command Palette 浮层 SHALL 在屏幕中央显示，输入框 SHALL 自动 focus

#### Scenario: 无选中项目时搜索项目
- **WHEN** Command Palette 打开且无选中项目
- **THEN** 搜索范围 SHALL 为项目列表（按名称/路径匹配）

#### Scenario: 有选中项目时搜索会话内容
- **WHEN** Command Palette 打开且已选中项目
- **THEN** 搜索范围 SHALL 为该项目下所有会话的用户消息和 AI 最终输出文本

#### Scenario: 搜索结果导航到会话
- **WHEN** 用户点击 Command Palette 中的搜索结果
- **THEN** 系统 SHALL 打开对应会话的 tab 并导航到匹配位置

#### Scenario: Esc 关闭 Command Palette
- **WHEN** Command Palette 可见时用户按 Esc
- **THEN** Command Palette SHALL 关闭
