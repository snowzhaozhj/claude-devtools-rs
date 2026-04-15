# ui-search Specification (Delta — command-palette-dashboard)

> Delta spec：新增 Command Palette 相关 Requirements。

## ADDED Requirements

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
