# sidebar-navigation Specification

## Purpose

定义 Sidebar 的导航行为：项目选择、会话列表展示（日期分组/排序/过滤）、与 Tab 系统的联动、会话 Pin/Hide、右键菜单、宽度拖拽调整。多选/批量操作为后续扩展。
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

### Requirement: Auto refresh session list on file change

当后端 `file-change` 事件命中**当前选中的项目**时，Sidebar SHALL 重拉
`listSessions` 刷新会话列表，无论命中事件中的 `sessionId` 是否已存在于现有
列表（覆盖"新会话首次写入"场景）。同一 project 短时间内多次事件 SHALL 合并
为一次 `listSessions` 调用。

#### Scenario: 当前 project 命中时刷新列表
- **WHEN** 用户当前选中 `selectedProjectId = "projectA"`
- **AND** 收到 `file-change` payload `{ projectId: "projectA",
  sessionId: <任意>, deleted: false }`
- **THEN** Sidebar SHALL 调用 `listSessions("projectA")` 并替换 `sessions`
  状态

#### Scenario: 非当前 project 命中时不刷新
- **WHEN** 用户当前选中 `selectedProjectId = "projectA"`
- **AND** 收到 `file-change` payload `{ projectId: "projectB", ... }`
- **THEN** Sidebar SHALL NOT 触发 `listSessions`

#### Scenario: 新 session 写入时出现在列表
- **WHEN** `~/.claude/projects/projectA/` 下首次创建一个新 session 文件
  `<newSid>.jsonl` 并写入第一行
- **AND** 用户当前选中 `selectedProjectId = "projectA"`
- **THEN** 该 `newSid` 对应的 SessionSummary SHALL 出现在 Sidebar 列表中
  （根据 timestamp 落到对应日期分组）

#### Scenario: 同 project 多次 file-change 合并刷新
- **WHEN** 同一 project 在 < 200 ms 内连续收到 3 次 `file-change` 事件
- **THEN** Sidebar SHALL 只发起 1 次 `listSessions` IPC 调用

#### Scenario: 删除事件也触发刷新
- **WHEN** 收到 `file-change` payload `{ projectId: "projectA",
  sessionId: "sessionX", deleted: true }`，且 `selectedProjectId = "projectA"`
- **THEN** Sidebar SHALL 触发 `listSessions("projectA")` 让 `sessionX` 从
  列表中消失

#### Scenario: 切换 project 后旧 project 的事件不再刷新
- **WHEN** 用户已经从 `projectA` 切到 `projectB`
- **AND** 此时延迟到达一条 `projectA` 的 `file-change` 事件
- **THEN** Sidebar SHALL NOT 调用 `listSessions("projectA")`（handler 在
  `selectedProjectId` 变化时已经按新值重新注册）

### Requirement: Ongoing indicator on session item

Sidebar 的每一行 session item SHALL 在 `session.isOngoing === true`
时于标题前渲染一枚绿色脉冲圆点 `<OngoingIndicator size="sm" />`。
圆点 SHALL 出现在 pin 图标（如有）之前；`isOngoing` 为 false /
undefined 时 SHALL NOT 占位。该渲染规则 MUST 同时作用于 PINNED
分区与日期分组（TODAY / YESTERDAY / …）两处 session 列表。

#### Scenario: Ongoing session shows pulsing dot
- **WHEN** `SessionSummary.isOngoing === true` 且该 session 出现在
  日期分组内
- **THEN** sidebar 对应一行 SHALL 在标题文本前渲染一枚绿色圆点，
  圆点 SHALL 带脉冲动画（`animate-ping` 或等价 CSS）

#### Scenario: Finished session shows no dot
- **WHEN** `SessionSummary.isOngoing === false`
- **THEN** sidebar 行 SHALL NOT 渲染圆点，其他视觉元素（pin 图标、
  标题、元数据行）位置 SHALL 与当前表现一致

#### Scenario: Dot appears in PINNED section too
- **WHEN** 一个被 pin 的 session 同时 `isOngoing === true`
- **THEN** PINNED 分区内该条目 SHALL 同时显示绿色圆点与蓝色 pin 图标，
  圆点 SHALL 位于 pin 图标之前

#### Scenario: Indicator updates after auto refresh
- **WHEN** 某个 session 的 `isOngoing` 因 `listSessions` 自动刷新
  从 `true` 变为 `false`
- **THEN** 对应 sidebar 行的绿点 SHALL 在该次刷新的下一帧被移除

