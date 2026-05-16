## Why

应用冷启时前端会并行触发多条可复用的启动查询：Sidebar 调用 `listRepositoryGroups`，DashboardView 额外调用 `listProjects`，TabBar 与 App 分别查询通知未读数。用户已观察到冷启 CPU 峰值可达 20%+，这些重复 IPC 会放大启动期扫描与反序列化压力。

## What Changes

- 前端 SHALL 以共享启动数据源加载 repository groups / projects，让 Sidebar 与 Dashboard 复用同一次项目发现结果。
- Dashboard 冷启 SHALL NOT 为项目概览额外触发 `listProjects`；若 Sidebar 已发起或完成 `listRepositoryGroups`，Dashboard 直接从共享数据派生项目卡片。
- 通知 badge 冷启 SHALL 复用单一 unread count 请求，避免 App 与 TabBar 同时调用 `getNotifications(1, 0)`。
- 保持现有 IPC command 与后端行为不变；仅在前端无法避免重复请求时才考虑后端改动。
- 不引入 breaking change。

## Capabilities

### New Capabilities

- 无。

### Modified Capabilities

- `sidebar-navigation`: 增加冷启共享项目数据与 Dashboard/Sidebar 去重查询契约。
- `notification-ui`: 增加通知未读数冷启单次请求与共享 badge 数据契约。

## Impact

- 影响 `ui/` 前端状态管理、Sidebar、DashboardView、App/TabBar 通知 badge 初始化路径。
- 不改变 `cdt-api` / Tauri IPC command 的请求与响应格式。
- 验证重点是 `npm run check --prefix ui`、相关 UI 单测，以及必要时 cold scan perf smoke。
