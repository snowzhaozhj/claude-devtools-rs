## Why

当前 Rust 版桌面应用只支持单会话模式——Sidebar 点击 session 直接替换 Main 区域内容，无法同时保持多个会话的浏览状态。原版 claude-devtools 的 Tab 系统是核心交互，用户需要在多个 session 间快速切换对比。这是第二批 UI 对齐中优先级最高的功能。

## What Changes

- 新增 `TabBar` 组件：水平标签条，显示已打开的 session tab，支持切换、关闭、新建
- 新增 Tab 状态管理：Svelte 5 runes 风格的响应式 store，管理 tab 列表、活跃 tab、per-tab UI 状态
- 新增 per-tab session 数据缓存：切换 tab 时无需重新加载 session 数据，zero-latency 切换
- 改造 `App.svelte` 布局：从 Sidebar + Main 双栏改为 Sidebar + (TabBar + Main) 三层
- 改造 `Sidebar` 交互：点击 session 改为 openTab（已打开则切换焦点，否则新建 tab）
- per-tab 展开/折叠状态隔离：同一 session 在不同 tab 中可有独立的展开状态
- **不含**：DnD 拖拽排序、多 Pane 分屏、右键菜单、Tab 持久化（留给后续迭代）

## Capabilities

### New Capabilities

（无——本次改动为纯前端 UI 功能，不涉及数据层 capability spec）

### Modified Capabilities

（无——后端 API 不变，前端消费方式不变）

## Impact

- **前端文件**：新增 `TabBar.svelte`、`tabStore.ts`；改造 `App.svelte`、`Sidebar.svelte`、`SessionDetail.svelte`
- **后端**：无改动
- **依赖**：无新增依赖
- **布局变更**：Main 区域顶部新增 TabBar，高度约 36px，Session 内容区相应减少
