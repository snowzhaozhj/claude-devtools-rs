# sidebar-navigation Specification

## Purpose

定义 Sidebar 的导航行为：项目选择、会话列表展示（日期分组 / 排序 / 过滤）、与 Tab 系统的联动、会话 Pin / Hide、右键菜单、宽度拖拽调整。同时覆盖骨架快速渲染、`session-metadata-update` 增量 patch、虚拟滚动等性能机制。多选 / 批量操作留作后续扩展。

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

点击会话项 SHALL 通过 Tab 系统在 focused pane 内打开或切换到对应 session。Sidebar 高亮 SHALL 跟随 focused pane 的 activeTab 的 sessionId。

#### Scenario: 点击会话打开 tab
- **WHEN** 用户点击一个会话项
- **THEN** 系统 SHALL 调用 Tab 系统的 openTab，该 openTab 的作用域 SHALL 为 `focusedPaneId` 对应 pane（具体 tab 生命周期行为由 tab-management spec 定义）

#### Scenario: 高亮跟随 focused pane 的 activeTab
- **WHEN** focused pane 的 activeTabId 变化（无论通过 Sidebar 点击、TabBar 点击、跨 pane focus 切换还是快捷键）
- **THEN** Sidebar 中对应 sessionId 的会话项 SHALL 高亮，之前的高亮 SHALL 移除

#### Scenario: 无 active tab 时无高亮
- **WHEN** focused pane 的 activeTabId 为 null
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

Sidebar 会话项 SHALL 支持右键打开上下文菜单，提供快捷操作。菜单 SHALL 包含 "Open in New Pane"（当 `paneLayout.panes.length < 4` 时可用）操作，用于在当前 focused pane 右侧创建新 pane 打开该 session。

#### Scenario: 打开右键菜单
- **WHEN** 用户在会话项上右键点击
- **THEN** SHALL 在点击位置显示浮层菜单，菜单包含：在新标签页打开、Open in New Pane、置顶/取消置顶、隐藏/取消隐藏、复制 Session ID、复制恢复命令

#### Scenario: Open in New Pane
- **WHEN** 用户在会话项右键菜单选择 "Open in New Pane"
- **AND** `paneLayout.panes.length < 4`
- **THEN** 系统 SHALL 在 focused pane 右侧创建一个新 pane，在该 pane 中打开目标 session 的 tab，新 pane SHALL 成为 focused

#### Scenario: Open in New Pane 达到上限时禁用
- **WHEN** `paneLayout.panes.length === 4`
- **THEN** 右键菜单的 "Open in New Pane" 项 SHALL 显示为禁用状态（灰色 + 不可点击）

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

### Requirement: 骨架列表快速加载

Sidebar 切换项目或初次加载时 SHALL 在骨架数据（`sessionId` / `projectId` / `timestamp`）到达后立即渲染完整列表骨架；元数据字段（`title` / `messageCount` / `isOngoing`）pending 时 SHALL 使用占位回退（fallback 到 sessionId 前 8 位 + "…" / `C` 空计数 / 无 ongoing 圆点）。元数据 pending 期间 SHALL NOT 显示"加载中..."遮罩，以避免阻挡已可交互的会话项。

#### Scenario: 骨架到达后立即渲染列表

- **WHEN** `listSessions(projectId)` 返回的 `SessionSummary[]` 已填充 `sessionId` / `timestamp` 但 `title` 为 null、`messageCount` 为 0、`isOngoing` 为 false
- **THEN** Sidebar SHALL 立即渲染会话项（按 timestamp 分组），每项标题 SHALL fallback 到 `sessionId` 前 8 位加 "…"，元数据行 SHALL 显示 `C`（无计数）+ 相对时间

#### Scenario: 骨架态不显示加载中遮罩

- **WHEN** 骨架数据已返回但元数据 patch 尚未全部到达
- **THEN** 会话列表区域 SHALL NOT 显示 "加载中..." 文字；列表 SHALL 展示已有骨架

#### Scenario: 仅骨架未返回时才显示加载中

- **WHEN** 切换项目后 `listSessions` 首次调用尚未 resolve（骨架未到）
- **THEN** 会话列表区域 SHALL 显示 "加载中..." 文字，直至骨架返回

### Requirement: 会话元数据增量 patch

Sidebar SHALL 订阅后端 `session-metadata-update` 事件，按 `sessionId` 定位列表中的 `SessionSummary` 并 in-place 替换其 `title` / `messageCount` / `isOngoing` 字段。patch 操作 SHALL 保持 `{#each}` 的稳定 key（`sessionId`）不变，避免整行 DOM 重建。非当前 `selectedProjectId` 的 event SHALL 被忽略。

#### Scenario: 元数据事件按 sessionId 匹配并 patch

- **WHEN** 当前 `selectedProjectId = "projectA"`，前端收到 payload `{ projectId: "projectA", sessionId: "s1", title: "重构 auth", messageCount: 42, isOngoing: false }`
- **THEN** Sidebar SHALL 找到 `sessions[i].sessionId === "s1"` 的条目，将其 `title` 更新为 "重构 auth"、`messageCount` 更新为 42、`isOngoing` 更新为 false；其他条目 SHALL 不变

#### Scenario: 元数据事件不改变列表顺序或重建 DOM

- **WHEN** 一条 `session-metadata-update` patch 到达
- **THEN** 被 patch 的会话项 SHALL 保持在原位置，DOM 节点 SHALL 被复用（Svelte `{#each}` 的 `(session.sessionId)` key 保障），不触发 OngoingIndicator 动画重启或 pin 图标闪烁

#### Scenario: 非当前 project 的事件被忽略

- **WHEN** 当前 `selectedProjectId = "projectA"`，收到 payload `{ projectId: "projectB", sessionId: "sX", ... }`
- **THEN** Sidebar SHALL NOT 修改本地 `sessions` 状态

#### Scenario: file-change silent 刷新保留已获取元数据

- **WHEN** file-change 触发 `loadSessions(projectId, silent=true)` 并返回新骨架（title/messageCount/isOngoing 全部重置为占位）
- **THEN** Sidebar SHALL 按 `sessionId` 将旧 `sessions` 的元数据字段 merge 进新骨架（旧有值的 session 元数据字段不被重置为占位），直到新的 `session-metadata-update` 到达再覆盖

### Requirement: 会话列表虚拟滚动承载

Sidebar 会话列表 SHALL 以 windowing 方式渲染：仅渲染视口内及上下 overscan 区间内的列表项。所有列表项（PINNED 分区头 + pinned sessions + 日期分组头 + 各日期分组内的 sessions）SHALL 摊平为单一固定行高的 flat 列表参与同一 windowing 单元。滚动位置变化时 SHALL 不引发会话项 DOM 整块重建（依赖 `{#each}` 稳定 key + 上下 spacer 占位元素）。

#### Scenario: 超出视口的会话项不渲染

- **WHEN** 当前项目有 200 个可见会话，视口高度可容纳 20 项
- **THEN** DOM 内实际渲染的 `session-item` 节点数 SHALL 为 `20 + 2 * overscan`（overscan 固定 5，即 30 个以内），其余位置 SHALL 由占位（spacer）高度填充

#### Scenario: 滚动不重建复用节点

- **WHEN** 用户滚动列表
- **THEN** 已渲染的会话项 DOM 节点 SHALL 被 Svelte `{#each (item.key)}` 复用（同 sessionId 进入视口时重用原节点），不触发 OngoingIndicator 的 `animate-ping` 动画重启

#### Scenario: 分组头滚动出视口时同步裁剪

- **WHEN** 用户向下滚动到某个分组头已离开视口
- **THEN** 该 `date-group-label` 元素 SHALL 与同步滚出的 session 项一起退出 DOM（参与 windowing 而非 sticky 渲染），spacer 高度 SHALL 正确反映被裁剪的总高度

#### Scenario: 高亮项位于视口外不触发自动滚动

- **WHEN** `activeSessionId` 对应的会话项当前不在视口内
- **THEN** Sidebar SHALL NOT 自动滚动到该项；用户滚动到该位置时 SHALL 正确显示高亮样式

#### Scenario: file-change 刷新保持滚动位置

- **WHEN** file-change 触发 `silent=true` 刷新并替换 sessions
- **THEN** 视口滚动位置 SHALL 保持不变（`scrollTop` 不重置）

