## Context

`DataApi` trait 有 20 个 async 方法。底层 crate 提供：
- `cdt-discover::ProjectScanner`：`scan()` → `Vec<Project>`，`list_sessions()` → `Vec<Session>`
- `cdt-parse::parse_file`：path → `Vec<ParsedMessage>`
- `cdt-analyze::build_chunks`：messages → `Vec<Chunk>`
- `cdt-config::ConfigManager`：load/save/update config
- `cdt-config::NotificationManager`：通知 CRUD
- `cdt-ssh::SshConnectionManager`：SSH 状态管理
- `cdt-discover::SessionSearcher`：搜索

## Goals / Non-Goals

**Goals:**
- `LocalDataApi` 实现所有 20 个 `DataApi` 方法
- `cdt-cli` 启动 HTTP server
- 验证 `cargo run -p cdt-cli` + `curl /api/projects` 能工作

**Non-Goals:**
- 完善每个方法的边界处理（先让 happy path 跑通）
- SSH 真实连接（provider 仍是 placeholder）
- 性能优化

## Decisions

### D1: `LocalDataApi` 结构

```rust
pub struct LocalDataApi {
    scanner: Mutex<ProjectScanner>,
    config_mgr: Mutex<ConfigManager>,
    notif_mgr: Mutex<NotificationManager>,
    ssh_mgr: Mutex<SshConnectionManager>,
    fs: FsHandle,
}
```

用 `tokio::sync::Mutex` 因为 lock 跨 await。

### D2: Session detail 路径

`get_session_detail(project_id, session_id)` → `scanner.list_sessions(project_id)` 找到 JSONL 路径 → `parse_file` → `build_chunks` → 包装返回。

### D3: CLI 启动流程

```
main() → tokio::main
  1. init tracing
  2. ConfigManager::new() + load()
  3. NotificationManager::new() + load()
  4. ProjectScanner::new(local_fs, projects_dir)
  5. SshConnectionManager::new()
  6. LocalDataApi::new(...)
  7. AppState::new(Arc::new(api))
  8. start_server(state, config.http_server.port)
```
