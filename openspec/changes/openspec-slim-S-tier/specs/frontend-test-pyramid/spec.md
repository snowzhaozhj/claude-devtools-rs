## MODIFIED Requirements

### Requirement: mockIPC 必须覆盖所有 Tauri command 与 listen event

`ui/src/lib/tauriMock.ts` SHALL 通过 `@tauri-apps/api/mocks` 的 `mockIPC()` 注入**全部**已注册到 `src-tauri/src/lib.rs::invoke_handler!` 的 Tauri command 与已注册到前端的 listen event。覆盖率 SHALL 由 `tauriMock.test.ts` 自动断言：mockIPC 已知 command 列表 SHALL 与 `invoke_handler!` 列表逐项对齐，缺一则用例 fail。未覆盖的 command 被前端 invoke 时 SHALL 返回明确的 `[mockIPC] command "<name>" not implemented` 错误而非静默 undefined。

`LocalDataApi` 内部公开方法但**未**注册为 Tauri command（仅供 HTTP server 调）的方法 SHALL NOT 在 mockIPC 覆盖范围内。

#### Scenario: 注入完整性回归

- **WHEN** vitest 跑 `ui/src/lib/tauriMock.test.ts`
- **THEN** 用例 SHALL 把 mockIPC 已知 command 列表与 `invoke_handler!` 列表逐项对齐断言
- **AND** 任一缺失 command（mockIPC 漏注入或 invoke_handler! 漏注册）SHALL 导致测试 fail

#### Scenario: 未实现 command 的明确报错

- **WHEN** 前端调用 mockIPC 未实现的命令（如新加的后端 IPC 还未同步 mock）
- **THEN** 控制台 SHALL 输出 `[mockIPC] command "<name>" not implemented`，包含 command 名
- **AND** 调用方 invoke 的 Promise MUST reject 而非 resolve undefined
