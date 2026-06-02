## 1. 后端 cwd 缓存

- [x] 1.1 `ProjectScanner` 加 `cwd_cache: Option<Arc<Mutex<LruCache<PathBuf, String>>>>` 字段，`new_with_semaphore` 构造器加 `cwd_cache` 参数
- [x] 1.2 `extract_session_cwd` 开头加 cache lookup：仅 `FsKind::Local` 且 cache 为 Some 时查询；hit 直接 return `Some(cached_cwd)`；miss 执行 head-read，成功（Some）时写入缓存，失败（None）时不写入
- [x] 1.3 `LocalDataApi` 加 `shared_cwd_cache: Arc<Mutex<LruCache<PathBuf, String>>>` 字段（容量 2048），在 `new_with_watcher` / `new` 构造器中初始化
- [x] 1.4 所有生产构造 `ProjectScanner` 的路径传入 `shared_cwd_cache.clone()`（`build_group_session_page`、`list_sessions_skeleton`、`scan_projects_cached_with` 等）

## 2. 前端 Sidebar debounce 分层

- [x] 2.1 `Sidebar.svelte` file-change handler：结构性事件（`sessionListChanged || deleted`）调 `scheduleRefresh("sidebar-structural:${currentGroupId}", fn, 250)`
- [x] 2.2 非结构性事件（三个标志全 false）调 `scheduleRefresh("sidebar-append:${currentGroupId}", fn, 1000)`（独立 key 避免 trailing timer 冲突）
- [x] 2.3 组件 onDestroy / group 切换时同时清理两个 key（`cancelScheduledRefresh`）

## 3. 测试

- [x] 3.1 `cdt-discover` 单测：验证 `extract_session_cwd` 第二次调用不执行 `read_lines_head`（mock fs 计数 I/O 调用）
- [x] 3.2 `cdt-discover` 单测：验证 head-read 失败（None）时不写入缓存，下次重试
- [x] 3.5 `cdt-discover` 单测：验证缓存满 2048 后 LRU 淘汰旧条目、新条目成功入缓存
- [x] 3.3 `cdt-discover` 单测：验证 Local FsKind 正确写入缓存
- [x] 3.4 Sidebar vitest：现有 file-change 测试通过 + svelte-check 零错误验证

## 4. 性能验证

- [x] 4.1 本地跑 `bash scripts/run-perf-bench.sh` 验证 cold scan bench 不回归（cwd cache 在 bench 无效场景等于 baseline）
- [ ] 4.2 手动 `just dev` + 活跃 workflow session 场景验证 CPU 降幅（Activity Monitor 观察）——需用户手动确认

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过
- [ ] N.4 archive change
