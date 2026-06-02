## Context

`list_group_sessions` IPC 被 Sidebar 每 250ms 调用一次（file-change → scheduleRefresh）。后端 `build_group_session_page` 对每个 worktree 调 `scanner.list_sessions()`，其中 `extract_session_cwd` 对每个 session JSONL 执行 open + read 20 行 + close。50 sessions × 4 次/s = 200+ syscalls/s。

当前架构中：
- `ProjectScanner` 在每次 `build_group_session_page` 调用时**新建**（无持久状态）
- `extract_session_cwd` 结果不缓存，每次从磁盘读
- cwd 由 JSONL 首行确定，后续 append 不改变（测试 `extract_session_cwd_uses_first_line_only` + `appending_does_not_change_cwd` 已断言）
- `session-metadata-update` SSE 事件唯一触发源是 `list_group_sessions` spawn 的 `scan_metadata_for_page`——不能完全跳过 `list_group_sessions`，只能降频

## Goals / Non-Goals

**Goals:**
- Workflow 时 CPU 从 54% 降到 < 15%（用户交互峰值预算）
- 零功能回归：metadata 推送链路（title / isOngoing / messageCount / gitBranch）不受影响
- 内存增量 < 1 MB

**Non-Goals:**
- `get_session_detail` 的增量 parse 优化（Layer 2，独立 change 追踪）
- 解耦 metadata scan 与 `list_group_sessions`（架构重构，未来方向）
- 修改 watcher debounce 机制

## Decisions

### D1：cwd 缓存生命周期 — 进程级共享 LRU

**选择**：`Arc<Mutex<LruCache<PathBuf, String>>>` 容量 2048，挂在 `LocalDataApi`，通过构造器传给 `ProjectScanner`。仅缓存成功提取到 cwd 的正结果（`Some(cwd)`）；head-read 失败或解析失败（返回 `None`）不写入缓存——避免瞬时 I/O 错误被永久固化。

**替代方案**：
- A) 普通 `HashMap` 满即停 — 满后新 session 永久 miss，在长生命周期进程中新活跃 session 无法受益（codex 审查 WARNING #2）
- B) Per-scanner 缓存 — scanner 每次新建导致缓存无持久化，无效
- C) 写入 sidecar 文件 — 增加复杂度且首次仍需 head-read
- D) 缓存 `Option<String>`（含负结果）— 瞬时读失败 / 半写入文件会被永久缓存为 None（codex 审查 WARNING #3）

**理由**：LRU 淘汰最久未访问的旧 session cwd（不再出现在 sidebar 当前页 → 自然淘汰），新活跃 session 始终能入缓存。不缓存 None 确保下次重试能成功读取（文件创建时半写入只是暂态）。

### D2：缓存 key 设计 — 文件路径

**选择**：key = `PathBuf`（session JSONL 的完整路径）

**理由**：路径唯一标识 session 文件。不含 FileSignature（mtime/size）因为 cwd 不随 append 变化。删除场景由 Rust Drop/重启自然清理（进程级缓存不持久化）。

### D3：Sidebar debounce 分层 — 独立 key 避免 trailing timer 冲突

**选择**：结构性事件和非结构性事件使用**不同 scheduleRefresh key**：
- 结构性事件：`scheduleRefresh("sidebar-structural:${groupId}", fn, 250)`
- 非结构性事件：`scheduleRefresh("sidebar-append:${groupId}", fn, 1000)`

两个 key 独立 trailing timer，互不阻塞。结构性事件不会被非结构性的 1000ms trailing 卡住。

**替代方案**：
- A) 同一 key + 动态缩短 timer — 需要修改 `scheduleRefresh` 公共 API（当已有 trailing 时取消重建），侵入性大
- B) 全部提到 1000ms — 新建 session 延迟感知明显
- C) 非结构性完全跳过 — 断掉 metadata 推送链路
- D) 同一 key 不分层 — codex 审查确认当前 `scheduleRefresh` 语义（line 177 `trailingTimers.has(key)` 直接 return）不支持后到事件缩短已设 timer

**理由**：`scheduleRefresh` 已有 `trailingTimers.has(key) → return` 语义（`fileChangeStore.svelte.ts:177`），同 key 混用两种窗口会导致结构性事件被卡在 1000ms trailing 后才执行。用独立 key 是最简单的修复，零侵入公共 API。两个 key 调用同一个 `loadSessions(groupId, true)` 函数——如果两个 timer 巧合同时 fire，`dedupeRefresh` 内层会合并为一次 IPC。

### D4：缓存与 SSH 远端的关系

**选择**：cwd 缓存仅用于 local filesystem（`FsKind::Local`）。SSH 场景 `extract_session_cwd` 已走顺序路径（`project_scanner.rs:336-341`），且 SSH session 数量少，不是热点。

**理由**：SSH 远端文件的 cwd 可能随 SSH 重连后路径变化，不适合进程级缓存。

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| cwd 缓存旧条目残留 | LRU 自然淘汰最久未访问条目；进程重启清空；残留 entry 仅占内存不影响正确性（list_sessions 靠 read_dir 发现文件不存在时不产出该 session） |
| 1000ms debounce → metadata 延迟 1s | workflow 期间用户看 SessionDetail（独立路径，150-300ms 自适应刷新），sidebar 仅辅助导航；1s 延迟可接受 |
| 未来 JSONL 格式变化导致 cwd 非首行 | 已有 spec + 测试（`extract_session_cwd_uses_first_line_only`）约束；且 `SESSION_HEAD_LINES=20` 本身就读前 20 行 |
| `session-metadata-update` 仍绑定在 `list_group_sessions` 链路 | 本 change 不解耦——降频（4→1 次/s）而非断开。完全解耦留待后续架构 change |
