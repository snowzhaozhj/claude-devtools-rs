## 1. UI 完整历史加载

- [x] 1.1 实现 `listAllSessions` 循环扩大 `pageSize`，直到最终响应 `nextCursor = null`
- [x] 1.2 让 Sidebar 使用 `listAllSessions` 加载完整会话历史并保持 silent metadata merge
- [x] 1.3 让 Command Palette 使用 `listAllSessions`，本地 session 搜索覆盖默认第一页之后的旧会话

## 2. 测试与验证

- [x] 2.1 补齐 mockIPC 的 `list_sessions` 分页行为，支持 `pageSize` 与 `cursor`
- [x] 2.2 增加 51 条会话回归测试，覆盖 Sidebar/API 完整加载默认第一页之后的旧会话
- [x] 2.3 增加会话数量变化后仍有 `nextCursor` 时继续扩大请求的回归测试
- [x] 2.4 运行 `npm run check --prefix ui`
- [x] 2.5 运行 `npm run test:unit --prefix ui -- src/lib/api.test.ts src/lib/tauriMock.test.ts`
- [x] 2.6 运行 `openspec validate sidebar-complete-session-history --strict`
