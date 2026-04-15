# sidebar-navigation Specification

## Purpose

定义 Sidebar 的导航行为：项目选择、会话列表展示（日期分组/排序/过滤）、与 Tab 系统的联动。Pin/Hide/多选/右键菜单/宽度调整为后续扩展。

## Requirements

### Requirement: 项目选择

Sidebar 顶部 SHALL 提供项目选择器。选择项目后 SHALL 自动加载该项目的会话列表。

#### Scenario: 初始加载
- **WHEN** 应用启动且有可用项目
- **THEN** 系统 SHALL 自动选中第一个项目并加载其会话列表

#### Scenario: 切换项目
- **WHEN** 用户从下拉选择器切换到另一个项目
- **THEN** 会话列表 SHALL 更新为新项目的会话，之前的列表 SHALL 被替换

#### Scenario: 无项目
- **WHEN** 无可用项目
- **THEN** Sidebar SHALL 显示空状态提示

### Requirement: 会话列表日期分组

会话列表 SHALL 按日期分组显示：TODAY、YESTERDAY、PREVIOUS 7 DAYS、OLDER。每个分组 SHALL 显示标签。空分组 SHALL 不显示。

#### Scenario: 今日会话分组
- **WHEN** 会话的 timestamp 在今天范围内
- **THEN** SHALL 归入 "TODAY" 分组

#### Scenario: 昨日会话分组
- **WHEN** 会话的 timestamp 在昨天范围内
- **THEN** SHALL 归入 "YESTERDAY" 分组

#### Scenario: 近 7 天分组
- **WHEN** 会话的 timestamp 在过去 2-7 天范围内
- **THEN** SHALL 归入 "PREVIOUS 7 DAYS" 分组

#### Scenario: 更早分组
- **WHEN** 会话的 timestamp 超过 7 天
- **THEN** SHALL 归入 "OLDER" 分组

#### Scenario: 空分组不显示
- **WHEN** 某个日期分组内无会话
- **THEN** 该分组标签和区域 SHALL 不渲染

### Requirement: 会话项展示

每个会话项 SHALL 显示标题和元数据（消息计数、相对时间）。标题 SHALL 优先使用后端提供的 title 字段，无 title 时 fallback 到 sessionId 前缀。

#### Scenario: 有标题的会话
- **WHEN** SessionSummary.title 非空
- **THEN** SHALL 显示 title，文本溢出时截断并显示省略号

#### Scenario: 无标题的会话
- **WHEN** SessionSummary.title 为 null
- **THEN** SHALL 显示 sessionId 前 8 字符 + "…"

#### Scenario: 元数据显示
- **WHEN** 会话项渲染
- **THEN** SHALL 显示消息计数（"C{N}" 格式）和相对时间（"刚刚"/"Nm"/"Nh"/"Nd"/日期）

### Requirement: 会话过滤

Sidebar SHALL 提供搜索输入框用于过滤会话列表。过滤 SHALL 基于 title 或 sessionId 的大小写不敏感子串匹配。

#### Scenario: 输入过滤文本
- **WHEN** 用户在搜索框中输入文本
- **THEN** 会话列表 SHALL 只显示 title 或 sessionId 包含该文本的会话（忽略大小写）

#### Scenario: 清空过滤
- **WHEN** 用户清空搜索框
- **THEN** SHALL 恢复显示所有会话

#### Scenario: 过滤计数
- **WHEN** 过滤激活时
- **THEN** SHALL 显示 "匹配数/总数" 格式的计数

#### Scenario: 无匹配结果
- **WHEN** 过滤文本无匹配
- **THEN** SHALL 显示 "无匹配会话" 提示

### Requirement: 会话选择与 Tab 联动

点击会话项 SHALL 通过 Tab 系统打开或切换到对应 session。Sidebar 高亮 SHALL 跟随 active tab 的 sessionId。

#### Scenario: 点击会话打开 tab
- **WHEN** 用户点击一个会话项
- **THEN** 系统 SHALL 调用 Tab 系统的 openTab（行为由 tab-management spec 定义）

#### Scenario: 高亮跟随 active tab
- **WHEN** active tab 切换（无论通过 Sidebar 还是 TabBar）
- **THEN** Sidebar 中对应 sessionId 的会话项 SHALL 高亮

#### Scenario: 无 active tab 时无高亮
- **WHEN** 无 active tab
- **THEN** Sidebar 中 SHALL 无会话项高亮

### Requirement: 加载状态

项目和会话加载过程中 SHALL 显示 loading 状态。加载失败 SHALL 不崩溃且显示错误提示。

#### Scenario: 项目加载中
- **WHEN** 项目列表正在加载
- **THEN** Sidebar SHALL 显示 "加载中..." 提示

#### Scenario: 会话加载中
- **WHEN** 切换项目后会话列表正在加载
- **THEN** 会话区域 SHALL 显示 "加载中..." 提示

#### Scenario: 加载失败
- **WHEN** API 调用失败
- **THEN** 会话列表 SHALL 为空且不显示崩溃界面，错误 SHALL 记录到 console
