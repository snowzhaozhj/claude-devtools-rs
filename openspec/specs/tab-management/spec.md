# tab-management Specification

## Purpose

定义桌面应用的多 Tab 管理行为：Tab 生命周期（打开/关闭/切换）、per-tab 状态隔离、session 数据缓存。本 spec 覆盖 MVP 行为；DnD 拖拽排序、多 Pane 分屏、Tab 持久化为后续扩展。

## Requirements

### Requirement: 打开 session tab

用户从 Sidebar 点击会话时，系统 SHALL 打开一个 session tab。若该 sessionId 已有打开的 tab，系统 SHALL 切换焦点到已有 tab 而非创建重复 tab。新 tab 的 label SHALL 为 session 标题（截断至 50 字符），id SHALL 为唯一标识符。

#### Scenario: 首次打开 session
- **WHEN** 用户点击 Sidebar 中一个尚未打开的 session
- **THEN** 系统 SHALL 创建新 tab 并设为 active，TabBar SHALL 显示该 tab

#### Scenario: 重复点击已打开的 session
- **WHEN** 用户点击 Sidebar 中一个已有 tab 的 session
- **THEN** 系统 SHALL 切换 activeTab 到已有 tab，不创建新 tab

#### Scenario: Tab label 截断
- **WHEN** session 标题超过 50 字符
- **THEN** tab label SHALL 截断到 50 字符并追加省略号

### Requirement: 关闭 tab

用户点击 tab 的关闭按钮时，系统 SHALL 移除该 tab 并清理其关联的 UI 状态和 session 缓存。

#### Scenario: 关闭非活跃 tab
- **WHEN** 用户关闭一个非当前 active 的 tab
- **THEN** 该 tab SHALL 从 TabBar 移除，activeTab 不变

#### Scenario: 关闭活跃 tab 且还有其他 tab
- **WHEN** 用户关闭当前 active tab 且 TabBar 中还有其他 tab
- **THEN** 系统 SHALL 自动激活相邻 tab（优先同位置，否则前一个）

#### Scenario: 关闭最后一个 tab
- **WHEN** 用户关闭 TabBar 中唯一的 tab
- **THEN** activeTab SHALL 变为 null，Main 区域 SHALL 显示空状态占位

#### Scenario: 关闭时清理资源
- **WHEN** 任何 tab 被关闭
- **THEN** 该 tab 的 per-tab UI 状态和 session 数据缓存 SHALL 被删除

### Requirement: 切换 tab

用户点击 TabBar 中的 tab 时，系统 SHALL 切换 active tab 并恢复目标 tab 的 UI 状态。

#### Scenario: 切换到有缓存的 tab
- **WHEN** 用户点击一个已加载过 session 数据的 tab
- **THEN** 系统 SHALL 从缓存恢复 session 数据，不发起 API 请求

#### Scenario: 切换时保存当前 tab 状态
- **WHEN** 用户从 tab A 切换到 tab B
- **THEN** tab A 的展开/折叠状态、搜索状态、Context Panel 状态和滚动位置 SHALL 被保存

### Requirement: Per-tab UI 状态隔离

每个 tab SHALL 维护独立的 UI 状态（expandedChunks、expandedItems、searchVisible、contextPanelVisible、scrollTop）。不同 tab 的 UI 操作 SHALL 互不影响。

#### Scenario: 两个 tab 打开同一 session
- **WHEN** 同一 session 通过不同路径打开在两个 tab 中（当前 MVP 通过 openTab 去重不会出现，但 spec 预留）
- **THEN** 两个 tab 的展开状态 SHALL 各自独立

#### Scenario: 滚动位置恢复
- **WHEN** 用户在 tab A 滚动到某位置，切换到 tab B 后再切回 tab A
- **THEN** tab A 的 conversation 滚动位置 SHALL 恢复到之前保存的值

### Requirement: Session 数据缓存

已加载的 session 数据 SHALL 以 tab 为粒度缓存。切换 tab 时若缓存命中 SHALL 跳过 API 调用。关闭 tab 时缓存 SHALL 被释放。

#### Scenario: 缓存命中
- **WHEN** 切换到一个之前已加载完成的 tab
- **THEN** SessionDetail 数据 SHALL 直接从缓存读取，loading 状态 SHALL 不出现

#### Scenario: 缓存未命中
- **WHEN** 切换到一个首次打开的 tab
- **THEN** 系统 SHALL 调用 getSessionDetail API 加载数据，显示 loading 状态，加载完成后存入缓存

### Requirement: TabBar 渲染

TabBar SHALL 在 Main 区域顶部渲染水平标签条。无 tab 打开时 TabBar SHALL 隐藏。

#### Scenario: 有 tab 时显示 TabBar
- **WHEN** tabs 列表非空
- **THEN** TabBar SHALL 可见，高度固定约 36px，显示所有 tab 项

#### Scenario: 无 tab 时隐藏 TabBar
- **WHEN** tabs 列表为空
- **THEN** TabBar SHALL 不渲染

#### Scenario: Active tab 视觉区分
- **WHEN** 某个 tab 为 active
- **THEN** 该 tab SHALL 有区别于非 active tab 的视觉样式（背景色、底部边框等）

### Requirement: Sidebar 与 Tab 联动

Sidebar 的会话高亮 SHALL 跟随当前 active tab 的 sessionId。切换 tab 时 Sidebar 高亮 SHALL 同步更新。

#### Scenario: 切换 tab 后 Sidebar 同步
- **WHEN** 用户切换到一个不同 session 的 tab
- **THEN** Sidebar 中对应 session 项 SHALL 高亮，之前的高亮 SHALL 移除

#### Scenario: 无 active tab 时 Sidebar 无高亮
- **WHEN** 所有 tab 已关闭
- **THEN** Sidebar 中 SHALL 无 session 项被高亮
