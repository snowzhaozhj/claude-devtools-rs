## Why

13 个 capability 已全部 port，但 `DataApi` trait 没有具体实现，`cdt-cli` 是空壳。本 change 把底层 crate 组装成可运行的 HTTP API server，让 `cargo run -p cdt-cli` 能真正启动并返回数据。

## What Changes

- 在 `cdt-api::ipc` 新增 `LocalDataApi`：实现 `DataApi` trait，组装 `ProjectScanner` / `ConfigManager` / `NotificationManager` / `SshConnectionManager` / `parse_file` / `build_chunks` 等
- 修改 `cdt-cli/src/main.rs`：初始化各 manager → 构造 `LocalDataApi` → 启动 HTTP server
- 添加 `cdt-cli` 对底层 crate 的依赖

## Capabilities

### New Capabilities
（无——这是集成接线，不是新 capability）

### Modified Capabilities
（无）

## Impact

- **代码**：`cdt-api/src/ipc/local.rs` 新增 ~400 行；`cdt-cli/src/main.rs` 重写 ~50 行
- **依赖**：`cdt-cli` 新增 `cdt-discover`、`cdt-config`、`cdt-ssh` 直接依赖
- **效果**：`cargo run -p cdt-cli` 启动 HTTP server on `localhost:3456`
