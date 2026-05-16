## Context

当前冷启路径由多个组件各自拉取启动数据：Sidebar 加载 repository groups 并选中项目，DashboardView 在无 active tab 时再调用 `listProjects` 生成项目卡片，App 与 TabBar 分别初始化通知未读数。虽然后端已有 list/session 扫描优化，但前端重复 IPC 会在冷启窗口叠加扫描、JSON 序列化和 webview 反序列化成本，放大 CPU 峰值。

## Goals / Non-Goals

**Goals:**

- 用前端共享 store 统一 repository groups / projects 的冷启请求生命周期。
- 让 Sidebar 与 Dashboard 复用同一份项目发现结果，消除 Dashboard 冷启 `listProjects`。
- 统一通知 unread count 初始化与刷新入口，消除 App/TabBar 启动时的重复 `getNotifications(1, 0)`。
- 保持现有 UI 行为、IPC command 和后端行为不变。

**Non-Goals:**

- 不新增后端聚合 IPC。
- 不改变 `listRepositoryGroups`、`listProjects`、`listSessions`、`getNotifications` 的响应结构。
- 不调整 session metadata 后台扫描算法。

## Decisions

### D1: 在前端共享启动数据，而不是新增后端 startup endpoint

选择在 `ui/` 中维护共享 project/repositoryGroups 状态，Dashboard 从 repository groups 派生项目卡片。候选方案是新增后端 `get_startup_data` 聚合 IPC，但这会扩大 IPC contract、HTTP mirror 和 Tauri command 同步面；当前重复请求来自前端生命周期，前端共享即可解决。

### D2: 以 `listRepositoryGroups` 作为项目发现的权威冷启请求

Sidebar 已依赖 repository groups 支持 main/worktree 结构，且 group 内包含 Dashboard 需要的项目摘要信息。Dashboard 复用该结果可避免同时维护 `listProjects` 与 `listRepositoryGroups` 两条冷启链路。若 repository groups 为空，Dashboard 显示同等空态；若请求失败，复用同一错误状态。

### D3: 保留现有 selected project 与 session loading 边界

共享项目数据只覆盖 project/repositoryGroups，不把 session list 提升为全局 startup payload。选中项目后仍只加载当前项目第一页 sessions，metadata scan 仍由现有后端机制异步推送，避免为了降低重复 IPC 而引入更大的首屏 payload。

### D4: 通知 unread count 用单一前端刷新函数去重

TabBar、App 事件监听和 NotificationsView 操作后的 badge 更新 SHALL 走同一 store action。该 action 负责 in-flight 去重与状态更新，避免组件 mount 时各自发起 `getNotifications(1, 0)`。30 秒轮询与 `notification-update` 事件仍保留，只是共享同一请求入口。

## Risks / Trade-offs

- Dashboard 从 repository groups 派生项目卡片可能暴露与 `listProjects` 字段命名不同的问题 → 先阅读现有接口类型与 fixture，必要时只在前端做显式映射，不改后端。
- 共享 store 若处理不当可能导致 Dashboard/Sidebar 互相影响 loading/error 显示 → store 只共享数据和请求生命周期，组件保留各自展示逻辑。
- unread count in-flight 去重可能吞掉事件后的刷新 → 去重只合并同时进行的请求；已有请求完成后新的事件可再次刷新。
