## 1. `ipc-data-api` / 前端分页调用

- [x] 1.1 为 `ui/src/lib/api.ts::listAllSessions` 增加单元测试或 mockIPC 测试，覆盖多页 cursor 累加且不发起 `pageSize = total` 的重拉请求。
- [x] 1.2 修改 `listAllSessions()`，使用 cursor 分页累加会话列表，保持返回类型和调用方不变。
- [x] 1.3 确认 `Sidebar.svelte` 仍在骨架列表返回后立即关闭 loading，并继续通过 `session-metadata-update` patch metadata。

## 2. `project-discovery` / 后端枚举优化

- [x] 2.1 阅读 `ProjectScanner::list_sessions` 的现有调用方和测试，确认排序、cursor、total 语义边界。
- [x] 2.2 优化 `ProjectScanner::list_sessions` 单次目录枚举中的逐文件 metadata/stat 开销，保持公共 API 不变。
- [x] 2.3 补充或更新 `cdt-discover` 测试，覆盖多会话排序与分页结果一致性。

## 3. 验证与收尾

- [x] 3.1 运行 `cargo test -p cdt-discover` 和相关 `cdt-api` IPC/metadata 测试。
- [x] 3.2 运行 `npm run check --prefix ui` 及相关 UI 单测。
- [x] 3.3 运行 `openspec validate session-list-loading-perf --strict`，确保 change 可 apply / archive。
