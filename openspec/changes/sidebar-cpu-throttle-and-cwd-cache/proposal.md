## Why

Workflow 运行多 subAgent 时 Claude DevTools 桌面端 CPU 飙到 54%（预算 idle < 2%，交互 < 15%）。Profiler 确认热点是 Sidebar 每 250ms 触发 `list_group_sessions` IPC → 后端对当前页所有 session（~50 个）无缓存地 open/read/close JSONL 提取 cwd（占 88.8% 热 samples）。正常时事件频率低不暴露；workflow 并发写入放大事件频率后结构性缺陷暴露。

## What Changes

- **`ProjectScanner` 新增 cwd 缓存**：`extract_session_cwd` 结果按 session 路径缓存，cache hit 直接返回，跳过 open/read/close I/O。cwd 在 JSONL 首行确定后不可变（已有测试断言），无需主动失效。
- **Sidebar file-change refresh 窗口从 250ms 提升到 1000ms**：降低 `list_group_sessions` IPC 调用频率 4 倍，减少残余 I/O（read_dir stat + metadata lookup + IPC 序列化）。结构性事件（`projectListChanged` / `sessionListChanged` / `deleted`）保持 250ms 不变。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `project-discovery`：`extract_session_cwd` 新增进程级缓存，跳过重复 head-read I/O
- `sidebar-navigation`：非结构性 file-change 事件的 session list refresh debounce 窗口从 250ms 提升到 1000ms

## Impact

- **后端**：`crates/cdt-discover/src/project_scanner.rs` — `ProjectScanner` 加 `cwd_cache` 字段 + `extract_session_cwd` 加 cache lookup
- **后端共享状态**：`crates/cdt-api/src/ipc/local.rs` — `LocalDataApi` 传递共享 cwd cache 给 `ProjectScanner`（类似 `shared_read_semaphore` 模式）
- **前端**：`ui/src/components/Sidebar.svelte` — 非结构性 session refresh 的 `scheduleRefresh` debounce 参数调整
- **性能**：预期 workflow 时 CPU 从 54% 降到 < 10%（88.8% 热 samples 被缓存消除 + 频率降 4 倍）
- **内存**：增加 ~250 KB（807 sessions × ~314 bytes/条），占当前 footprint 0.38%
