# Design: cleanup-purity-discovery-telemetry-server

## Decisions

### D1: 纯文本清理不改契约语义

重写规则：
- 内部模块/函数/类型名（`cdt_parse::ParsedMessage`、`ProjectScanner`、`WorktreeGrouper`）→ 用行为描述替代
- 源码路径（`src-tauri/src/lib.rs`、`crates/cdt-api/tests/ipc_contract.rs`）→ 删除或改为"IPC contract test"等通用描述
- 框架引用（`tokio`、`tracing`、`axum`、`broadcast`、`tauri::Builder`）→ 用"异步运行时"、"日志系统"、"HTTP 框架"、"广播通道"、"桌面应用框架启动阶段"替代
- 实测 metric（`~10-50 μs`、`~95 ms`、`200 KB 量级`）→ 删除观测值保留阈值约束
- 保留：`get_telemetry_snapshot` / `record_correctness_events` / `http_server_start` / `http_server_stop` / `http_server_status` / camelCase 字段名 / HTTP path / SSE event 名

### D2: application-telemetry metric 分流

- `< 0.2%` / `< 0.5%` / `< 1 MB` → 保留（用户感知性能阈值约束）
- `~10-50 μs / list_sessions(50) ~95 ms` → 删除（实测观测值，属 perf.md 管辖）
- `≤ 10 ns` / `≤ 50 ns` / `< 200 ns` / `≤ 5 μs` → 保留（单操作开销上限约束，属行为契约）
- `100ms 内` → 保留（用户感知响应约束）
- `200 KB 量级` → 删除（观测值注释）

### D3: server-mode 源码路径处理

`src-tauri/src/lib.rs::invoke_handler!` / `src-tauri/src/server_mode.rs` 等引用全部改为行为描述："桌面应用入口注册"、"server-mode 实现模块"。保留 contract test 描述为"IPC contract test"、保留 `ui/src/lib/tauriMock.ts::KNOWN_TAURI_COMMANDS` 与 `ui/src/lib/api.ts` 同步要求为行为约束。
