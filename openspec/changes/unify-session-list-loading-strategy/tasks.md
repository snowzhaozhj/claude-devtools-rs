## 1. 后端 HTTP 路由切骨架（`cdt-api`）

- [x] 1.1 `crates/cdt-api/src/http/routes.rs::list_sessions` 把 `s.api.list_sessions_sync(&project_id, &pagination).await?` 改为 `s.api.list_sessions(&project_id, &pagination).await?`，删除原"HTTP 无 push 通道"注释，加新注释引用 `ipc-data-api` spec §"HTTP `list_sessions` 复用 IPC 骨架 + push 实现"；同步更新 `crates/cdt-api/src/ipc/traits.rs::list_sessions` / `list_sessions_sync` doc comment 反映新现实
- [x] 1.2 验证 `cdt-api::http::bridge::spawn_metadata_bridge` 已在 `spawn_event_bridge` 注册了 `metadata_rx` 订阅（已确认：`bridge.rs:106-128` 完整实现 `SessionMetadataUpdate` → `PushEvent::SessionMetadataUpdate` 转换；`mod.rs::start_server` 调 `spawn_event_bridge` 传入 `api.subscribe_session_metadata()` rx）
- [ ] 1.3 新增 `crates/cdt-api/tests/http_list_sessions_skeleton_then_sse.rs` 集成测试：
  - 启 HTTP server + 喂 1 个项目下 3 条 session 的 jsonl fixture（cache 空）
  - `GET /api/projects/{id}/sessions?pageSize=20` 拿骨架响应（`title=null`）
  - 同时订阅 `/api/events` SSE，断言收到 3 条 `session_metadata_update` 事件携带真实值
- [ ] 1.4 新增 `crates/cdt-api/tests/http_list_sessions_cache_hit_inline.rs`：cache 预热后 GET 路径直接返真值、零 SSE emit
- [ ] 1.5 `cargo test -p cdt-api --tests` 全绿；`cargo clippy --workspace --all-targets -- -D warnings` 全绿

## 2. 后端 `MetadataCache` 持久化（`cdt-api`）

- [ ] 2.1 `crates/cdt-api/src/ipc/parsed_message_cache.rs` 新增 `CacheSnapshot { schema_version: u32, entries: Vec<(FileSignature, SessionMetadata)> }` 结构（`serde::Serialize/Deserialize` 派生）
- [ ] 2.2 `MetadataCache` 新增 `pub fn dump_to_disk(&self, path: &Path) -> Result<(), CacheError>`：lock → 序列化 → 同步写 + fsync；失败 `tracing::warn!` 不阻塞
- [ ] 2.3 `MetadataCache` 新增 `pub fn load_from_disk(path: &Path) -> Result<Self, CacheError>`：路径不存在返空 cache + `tracing::info!`；schema 版本不匹配返空 cache + `tracing::info!`；JSON parse 失败返空 cache + `tracing::warn!`
- [ ] 2.4 `crates/cdt-api/src/ipc/local.rs::LocalDataApi` 新增 `new_with_metadata_cache(cache: Arc<Mutex<MetadataCache>>)` 构造器或扩展现有 `new_with_xxx`（按 CLAUDE.md "LocalDataApi 构造器扩展" 模式不改 `new()` 签名）
- [ ] 2.5 `src-tauri/src/lib.rs` `tauri::Builder::setup` 改为：先 `MetadataCache::load_from_disk(app_data_dir/session-metadata-cache-tauri.json)` hydrate → `LocalDataApi::new_with_xxx(... cache ...)` 注入（**确认** `AppState.api` 与 Tauri IPC handler 共用同一 `Arc<LocalDataApi>` 实例，避免误起两份 cache）；`on_window_event` 监听 `CloseRequested`，事件 handler **异步 spawn dump task + 5 s join timeout** 后才允许窗口销毁；timeout SHALL `tracing::warn!` 放行
- [ ] 2.6 `crates/cdt-cli/src/main.rs` 启动时 hydrate cache（路径 `session-metadata-cache-cli.json`）、`tokio::signal::ctrl_c` + Unix SIGTERM handler 收到信号 **异步 dump + 5 s join timeout** 后退出（**不**同步阻塞写避免 HDD 上卡退出）
- [ ] 2.7 新增 `crates/cdt-api/tests/metadata_cache_persistence.rs` 集成测试：
  - dump → load 往返语义保持（hit 数量、`FileSignature` 字段一致）
  - hydrate 后修改文件 mtime → 下次 lookup miss → 走原扫描路径
  - schema_version 不等 → load 返空 cache（不报错）
  - 文件不存在 → load 返空 cache（不 `warn!`）
- [ ] 2.8 `cargo test -p cdt-api --tests` + `cargo test --workspace` 全绿

## 3. 前端 transport SSE 订阅顺序（`ui`）

- [ ] 3.1 `ui/src/lib/transport.ts::BrowserTransport` 新增私有方法 `private async ensureSseReady(): Promise<void>`：按 `source.readyState` 判断 OPEN / CONNECTING / CLOSED 三态，附 1000 ms 超时放行；超时 `console.warn` 提示
- [ ] 3.2 `BrowserTransport.invokeHttp(cmd, args)` 入口检测 `cmd` 是否属于 `LIST_SESSIONS_LIKE_COMMANDS`（含 `list_sessions` / `list_repository_groups` / `list_worktree_sessions`）；属于则 `await this.ensureSseReady()` 后才发 fetch
- [ ] 3.3 `ui/src/lib/transport.test.ts` 加 test case：mock `EventSource` 处于 `CONNECTING` → invokeHttp 先 await onopen 再发 fetch；OPEN 时不阻塞；1000 ms 超时仍发 fetch
- [ ] 3.4 `pnpm --dir ui run check` + `just test-ui-unit` 全绿

## 4. 前端 sessions store SWR + cancel + debounce（`ui`）

- [ ] 4.1 新增 `ui/src/lib/sessionListStore.svelte.ts`：定义 `SessionListEntry { sessions, nextCursor, total, lastFetchedAt, generation, inflightAbort? }`；模块级 `Map<string, SessionListEntry>` + LRU 计数器（capacity=16）
- [ ] 4.2 实现 `read(projectId): SessionListEntry | undefined`、`loadFirstPage(projectId, opts: { mode: 'replace' | 'merge', silent: boolean }): Promise<SessionListEntry>`、`loadMore(projectId): Promise<void>`、`applyMetadata(projectId, update): void`、`invalidate(projectId): void`
- [ ] 4.3 `loadFirstPage` / `loadMore` 内部：`++entry.generation` → 创建 AbortController（浏览器 runtime；Tauri runtime 跳过）→ 调 transport invoke → resolve 时校验 generation；不等丢弃
- [ ] 4.4 `loadMore` 套 100 ms trailing debounce（自实现：维护 `pendingMoreTimer: Map<projectId, ReturnType<setTimeout>>`，每次调清 timer 重置；timer 触发时检查 generation 后发请求）
- [ ] 4.5 LRU 淘汰：用单调递增 `accessCounter` 给 entry 标记 `lastAccessedAt`，size > 16 时 evict 最小
- [ ] 4.6 新增 `ui/src/lib/sessionListStore.test.ts` 单测：cache hit / cache miss / generation cancel / LRU evict / debounce leading+trailing / inflight short-circuit / **applyMetadata 写入后 `read(projectId)` 命中返回的 entry 含已 patch 字段（read-after-write）** / **首页 SWR refresh ghost reconcile 删除 missing sessionId**
- [ ] 4.7 `pnpm --dir ui run check` + `just test-ui-unit` 全绿

## 5. Sidebar 接线 + 视觉渐显（`ui`）

- [ ] 5.1 `ui/src/components/Sidebar.svelte::loadSessions` 改：先 `sessionListStore.read(projectId)` 同步命中检查，命中则直接 hydrate `sessions / sessionsNextCursor / sessionsTotal`（不进 loading state）+ 后台 `loadFirstPage(projectId, { mode: 'merge', silent: true })` SWR；未命中走原非 silent 替换式路径，resolve 后 store 写入
- [ ] 5.2 `loadMoreSessions` 改：调用 `sessionListStore.loadMore(selectedProjectId)`，移除现有 `sessionsLoadingMore` flag 内的 inflight 判定（store 内已有 generation + debounce 替代）
- [ ] 5.3 `session-metadata-update` listener 在 `applyPendingMetadata` 之外**新增**调 `sessionListStore.applyMetadata(projectId, update)`，同步更新 store 缓存
- [ ] 5.4 渲染条件 class：`<button class:metadata-pending={!session.title && session.messageCount === 0 && !session.isOngoing}>` 应用到每条 session item 包裹元素
- [ ] 5.5 `ui/src/components/Sidebar.svelte` `<style>` 新增 `.metadata-pending` shimmer 动画（`@keyframes` linear-gradient 1.5 s 横移）+ `.session-item` 默认 `transition: opacity 150ms ease-out`
- [ ] 5.6 验证 metadata-pending 仅在三字段全占位时触发，避免 patch 部分到达时 shimmer 仍闪烁
- [ ] 5.7 验证 Tauri runtime + 浏览器 runtime + 冷启动（清磁盘 cache）三场景手动 smoke 通过：切 project 0 闪烁、快速翻页流畅、冷启首帧字段连贯

## 6. 测试与性能验证

- [ ] 6.1 新增 `ui/tests/e2e/session-list-loading.spec.ts` Playwright：
  - case A：快速点击切换 3 个 project，断言中间无"加载中..."文本闪现
  - case B：连续滚动到底部 5 次，断言 IPC 调用次数 ≤ 2（debounce 合并）
  - case C：模拟冷启动（mock IPC cache 空），断言骨架行有 `.metadata-pending` class 与 shimmer 动画
- [ ] 6.2 `bash scripts/run-perf-bench.sh` 跑 baseline 对比：`perf_cold_scan` + `perf_get_session_detail` 四维（wall / user / sys / RSS）无回归（user/real ratio 关键看是否跨过 0.5 阈值）
- [ ] 6.3 README / `src-tauri/CLAUDE.md` 若有"HTTP 同步返回"描述同步更新

## 7. 与 `add-server-mode` change 协调

- [ ] 7.1 监控 `add-server-mode` PR 合并状态；其 archive 进 main 后 SHALL 把本 worktree rebase main
- [ ] 7.2 rebase 后重读 `openspec/specs/http-data-api/spec.md` 与 `openspec/specs/ipc-data-api/spec.md` 主 spec 落定态，校验本 change 的 MODIFIED block 中"完整 Requirement 内容"与最新主 spec 一致；若 `add-server-mode` 改写了相关 Requirement 文字，SHALL 同步更新本 change 的 MODIFIED block
- [ ] 7.3 rebase 后**对照最新 `crates/cdt-api/src/http/bridge.rs` + `crates/cdt-api/src/http/sse.rs`** 校验：`forward_session_metadata` 函数签名与 `PushEvent::SessionMetadataUpdate` event schema 字段名 / 序列化形式与本 change spec delta 引用的 `session_metadata_update` 事件 schema 一致；`broadcast::channel` capacity 未被改小到丢消息阈值；不一致则在本 change tasks 内加补丁
- [ ] 7.4 `openspec validate unify-session-list-loading-strategy --strict` 通过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
