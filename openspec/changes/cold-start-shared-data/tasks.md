## 1. 前端启动数据共享

- [x] 1.1 定位 Sidebar、DashboardView、App、TabBar 的冷启 IPC 调用点和现有 store 结构
- [x] 1.2 新增或扩展前端共享 project/repositoryGroups store，提供 in-flight 去重与缓存结果复用
- [x] 1.3 改造 Sidebar 使用共享 project/repositoryGroups 数据加载入口
- [x] 1.4 改造 DashboardView 从共享 project/repositoryGroups 数据派生项目卡片，移除冷启 `listProjects` 调用

## 2. 通知 badge 请求去重

- [x] 2.1 新增或扩展 notification unread count 共享刷新入口，合并 in-flight `getNotifications(1, 0)` 请求
- [x] 2.2 改造 App 事件刷新、TabBar 初始化和 30 秒轮询使用同一共享入口
- [x] 2.3 确认 NotificationsView 操作后的 badge 更新仍保持实时

## 3. 验证

- [x] 3.1 补充或更新相关 vitest / mockIPC 测试，覆盖 Dashboard 不再冷启调用 `listProjects` 与 unread count 去重
- [x] 3.2 运行 `npm run check --prefix ui`
- [x] 3.3 运行相关 UI 单测
- [x] 3.4 运行 `openspec validate cold-start-shared-data --strict`

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
