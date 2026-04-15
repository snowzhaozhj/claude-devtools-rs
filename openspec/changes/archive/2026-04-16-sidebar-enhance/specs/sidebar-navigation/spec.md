# sidebar-navigation Specification (Delta — sidebar-enhance)

> 本文件为 delta spec，仅包含新增/修改的 Requirements。
> 保留原 spec 的所有既有 Requirements 不变。

## ADDED Requirements

### Requirement: 会话置顶（Pin）

用户 SHALL 可以将会话置顶。置顶的会话 SHALL 在日期分组之前以独立 "PINNED" 分区显示。Pin 状态为 per-project 内存级。

#### Scenario: 置顶会话
- **WHEN** 用户通过右键菜单选择"置顶会话"
- **THEN** 该会话 SHALL 出现在 PINNED 分区顶部，带蓝色 pin 图标

#### Scenario: 取消置顶
- **WHEN** 用户通过右键菜单选择"取消置顶"
- **THEN** 该会话 SHALL 从 PINNED 分区移除，回到日期分组中对应位置

#### Scenario: Pin 排序
- **WHEN** 多个会话被置顶
- **THEN** 最新置顶的 SHALL 排在最前面（prepend 顺序）

#### Scenario: 无置顶时不显示分区
- **WHEN** 当前项目无置顶会话
- **THEN** PINNED 分区标签和区域 SHALL 不渲染

### Requirement: 会话隐藏（Hide）

用户 SHALL 可以隐藏会话。隐藏的会话默认不显示在列表中。用户 SHALL 可以通过切换按钮临时查看隐藏会话。Hide 状态为 per-project 内存级。

#### Scenario: 隐藏会话
- **WHEN** 用户通过右键菜单选择"隐藏会话"
- **THEN** 该会话 SHALL 从列表中消失（默认模式下）

#### Scenario: 取消隐藏
- **WHEN** 用户在 showHidden 模式下通过右键菜单选择"取消隐藏"
- **THEN** 该会话 SHALL 恢复正常显示

#### Scenario: 显示隐藏会话切换
- **WHEN** hiddenCount > 0 时
- **THEN** filter bar SHALL 显示眼睛图标按钮，点击 SHALL 切换 showHidden 模式

#### Scenario: 隐藏会话视觉
- **WHEN** showHidden 模式开启
- **THEN** 被隐藏的会话 SHALL 以 50% opacity 显示在列表中

### Requirement: 右键菜单

Sidebar 会话项 SHALL 支持右键打开上下文菜单，提供快捷操作。

#### Scenario: 打开右键菜单
- **WHEN** 用户在会话项上右键点击
- **THEN** SHALL 在点击位置显示浮层菜单，菜单包含：在新标签页打开、置顶/取消置顶、隐藏/取消隐藏、复制 Session ID、复制恢复命令

#### Scenario: 菜单定位 clamping
- **WHEN** 右键位置靠近视口右边界或下边界
- **THEN** 菜单位置 SHALL clamp 到视口内（8px 内边距）

#### Scenario: 菜单关闭
- **WHEN** 用户点击菜单外区域或按 Escape
- **THEN** 菜单 SHALL 关闭

#### Scenario: 复制操作反馈
- **WHEN** 用户选择复制 Session ID 或复制恢复命令
- **THEN** SHALL 复制到剪贴板，菜单项文本 SHALL 变为 "已复制!" 并在 600ms 后自动关闭菜单

### Requirement: 宽度拖拽调整

Sidebar 右边缘 SHALL 可拖拽调整宽度。

#### Scenario: 拖拽调整宽度
- **WHEN** 用户在 Sidebar 右边缘按下并拖动鼠标
- **THEN** Sidebar 宽度 SHALL 跟随鼠标 X 坐标实时变化

#### Scenario: 宽度范围限制
- **WHEN** 拖拽时鼠标超出范围
- **THEN** 宽度 SHALL 限制在 200px ~ 500px 之间

#### Scenario: 拖拽视觉提示
- **WHEN** 鼠标悬停或拖拽 resize handle
- **THEN** handle SHALL 显示蓝色半透明高亮，鼠标光标 SHALL 变为 col-resize
