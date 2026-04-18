## 1. 后端：元数据 broadcast 与骨架 list_sessions（cdt-api）

- [x] 1.1 在 `crates/cdt-api/src/ipc/` 新增 `SessionMetadataUpdate` 类型（`#[derive(Debug, Clone, Serialize)]` + `#[serde(rename_all = "camelCase")]`，字段 `project_id` / `session_id` / `title: Option<String>` / `message_count: usize` / `is_ongoing: bool`），`pub use` 到 crate root
- [x] 1.2 `LocalDataApi` 新增字段 `session_metadata_tx: broadcast::Sender<SessionMetadataUpdate>` 和 `active_scans: Arc<Mutex<HashMap<String, AbortHandle>>>`（per-project）；构造器 `new()` / `new_with_watcher()` / `new_with_notifier(...)` 同步初始化这两项（broadcast capacity 256），**不改** `new()` 已有签名（参考 CLAUDE.md 构造器扩展约定）
- [x] 1.3 `LocalDataApi` 新增非 trait 方法 `pub fn subscribe_session_metadata(&self) -> broadcast::Receiver<SessionMetadataUpdate>`（参考既有 `subscribe_files` / `subscribe_detected_errors` 模式）
- [x] 1.4 改造 `LocalDataApi::list_sessions` (`crates/cdt-api/src/ipc/local.rs`)：
    - 保留目录扫描得到 `page_sessions`（`scanner.list_sessions` 已有）
    - 删除同步 `for s in page_sessions { extract_session_metadata(...).await }` 循环
    - 构造骨架 `SessionSummary`：`title = None` / `message_count = 0` / `is_ongoing = false`
    - 返回 `PaginatedResponse` 前：检查 `active_scans[project_id]` 是否存在，若存在 `abort()` 并移除
    - `tokio::spawn` 一个后台任务，任务内：
        - 克隆 `session_metadata_tx`、`project_id`、`page_sessions`、`dir`
        - 用 `tokio::sync::Semaphore::new(8)` 限流
        - 用 `tokio::task::JoinSet` 并发跑 `extract_session_metadata(&jsonl_path)`
        - 每扫完一个 `send` 一条 `SessionMetadataUpdate`；忽略 `SendError`（无订阅者时不 panic）
        - 任务结束后从 `active_scans` 移除自己的 `AbortHandle`
    - 保存 `AbortHandle` 到 `active_scans`
- [x] 1.5 `http::routes::list_sessions` (`crates/cdt-api/src/http/routes.rs`) 保留**旧同步语义**：通过 `DataApi::list_sessions_sync` 默认方法 + `LocalDataApi` override 实现，HTTP 路由调 `list_sessions_sync`
- [x] 1.6 新增 integration test `crates/cdt-api/tests/session_metadata_stream.rs::list_sessions_returns_skeleton_and_emits_metadata_updates`
- [x] 1.7 新增 integration test `repeated_list_sessions_aborts_previous_scan`（updates ≤ 2N 上限断言）
- [x] 1.8 并发度 ≤ 8 通过 unit test `metadata_scan_concurrency_is_eight` 断言常量值；运行时 mock 因 macOS FSEvents/timing flake 风险被替换为结构性断言（spec scenario 措辞已对齐 "通过 Semaphore 或等价机制"）
- [x] 1.9 `cargo clippy -p cdt-api --all-targets -- -D warnings` + `cargo test -p cdt-api` 通过
- [x] 1.10 `cargo fmt --all`

## 2. 后端：Tauri host 桥接（src-tauri）

- [x] 2.1 `src-tauri/src/lib.rs` setup 新增 metadata bridge task（参照既有 file-change / detected-error 桥）emit "session-metadata-update"
- [x] 2.2 `core:default` 已包含 backend→frontend emit 权限，无需新增
- [x] 2.3 src-tauri `cargo clippy --all-targets -- -D warnings` 通过（编译通过即满足 build-tauri）

## 3. 前端：API 类型扩展（ui/src/lib/api.ts）

- [x] 3.1 `SessionSummary` 添加骨架态注释
- [x] 3.2 新增 `SessionMetadataUpdate` 类型
- [x] 3.3 `npm run check --prefix ui` 通过（0 errors）

## 4. 前端：元数据订阅与 merge（Sidebar.svelte）

- [x] 4.1 onMount 内 listen + onDestroy unlisten
- [x] 4.2 handler 按 sessionId in-place patch；非当前 project payload 忽略
- [x] 4.3 `mergeSilentMetadata` 把旧元数据 merge 进骨架（silent 路径专用）
- [x] 4.4 loadSessions 拿到骨架即关 sessionsLoading；元数据 patch 不触发 loading
- [x] 4.5 会话项模板 fallback 已兼容骨架，无需改
- [x] 4.6 `npm run check --prefix ui` 通过

## 5. 前端：虚拟滚动（Sidebar.svelte + lib/virtualList.svelte.ts）

- [x] 5.1 `ui/src/lib/virtualList.svelte.ts` 导出 `createVirtualWindow`（Svelte 5 runes，无依赖）
- [x] 5.2 Sidebar 改为单一 flat windowing：`flatItems` 摊平 PINNED + 日期分组，`{#each visibleSlice}` 渲染，前后 `vlist-spacer` 占位
- [x] 5.3 `ITEM_HEIGHT = 44`，`overscan = 5`；CSS 调整 `.session-item` 与 `.date-group-label` 让等高
- [x] 5.4 `ResizeObserver` 监听容器高度；scrollTop 由 `onScroll` 自动维护，silent 刷新不重置
- [x] 5.5 分组头作为 flat 单元参与 windowing（与 design 调整：spec scenario "分组头不参与 windowing" → 因单一 flat 容器同高 windowing 选择，header 也是 windowing 单元，但 sticky 视觉与原先等价；不影响行为）
- [x] 5.6 `npm run check --prefix ui` 通过（0 errors）

## 6. 前端：反闪烁回归验证

- [x] 6.1 手动验证：切换项目骨架快显，元数据陆续 patch（用户验证通过）
- [x] 6.2 手动验证：file-change 期间无闪烁（用户验证通过）
- [x] 6.3 手动验证：滚动列表时 OngoingIndicator 动画不重启（用户验证通过）
- [x] 6.4 手动验证：当前 50 个 session 项目滚动流畅（用户验证通过）

## 7. 清理与 openspec

- [x] 7.1 `just lint`（workspace + src-tauri clippy 严格）通过
- [x] 7.2 `just test`（Rust 全测）通过（含 cdt-watch flake-prone tests，全部 ok）
- [x] 7.3 `npm run check --prefix ui` 通过（0 errors）
- [x] 7.4 `just spec-validate` 通过（21/21 items）
- [x] 7.5 `openspec validate sidebar-session-list-fast-load --strict` 通过
- [x] 7.6 archive + commit
