## Context

`list_sessions` IPC 支持分页，Tauri command 默认 `pageSize = 50`。Sidebar 与 Command Palette 原先只消费默认第一页，导致当前项目会话数超过 50 时旧会话从列表与本地搜索入口消失。后端每次 `list_sessions(projectId)` 会取消同 project 上一轮元数据扫描，因此前端不能通过逐页追加的方式加载完整历史，否则最终只会扫描最后一页元数据。

## Goals / Non-Goals

**Goals:**

- Sidebar 展示当前项目完整会话历史，包括默认第一页之后的旧会话。
- Command Palette 的本地 session 搜索覆盖当前项目完整会话历史。
- 保持后端 `session-metadata-update` 扫描覆盖最终展示的完整列表。
- 覆盖两次请求之间会话数量变化的边界。

**Non-Goals:**

- 不修改 `list_sessions` IPC 字段、分页协议或 Tauri command 签名。
- 不改变后端元数据扫描的同 project 取消策略。
- 不引入无限滚动 UI 或远程搜索行为。

## Decisions

### D1: 前端用扩大 `pageSize` 的从头重取策略

选择：新增前端 helper 消费 `list_sessions`，若响应仍有 `nextCursor`，按最新响应的 `total` 扩大 `pageSize` 并从头重取，直到 `nextCursor = null`。

候选方案：
- 逐页用 `cursor` 追加：请求更少重复数据，但每次调用都会取消上一轮同 project 元数据扫描，最终只扫描最后一页，和 Sidebar 元数据增量 patch 机制冲突。
- 后端新增 `list_all_sessions`：协议更直接，但本次问题是前端未完整消费既有分页能力，新增 IPC 会扩大行为契约和测试面。

### D2: Sidebar 与 Command Palette 共用同一个完整加载 helper

选择：`Sidebar.svelte` 和 `CommandPalette.svelte` 均调用同一 helper，避免两个入口对会话历史覆盖范围不一致。

候选方案：仅修 Sidebar。该方案会让列表可见但 Command Palette 搜索仍漏旧会话，用户仍会感知历史不完整。

## Risks / Trade-offs

- 完整加载大项目会话会比只取 50 条多一次或多次目录扫描 → 通过虚拟滚动限制 DOM 渲染量；后端仍只返回骨架，元数据异步限流扫描。
- 两次请求之间新增会话可能让旧 `total` 不足 → helper 以最新响应为准循环扩大 `pageSize`，直到没有 `nextCursor`。
- 若项目在高频新增会话时 `total` 持续增长，helper 可能多重试几次 → 实际只在打开列表或刷新时触发，且每轮使用最新 `total` 快速收敛。
