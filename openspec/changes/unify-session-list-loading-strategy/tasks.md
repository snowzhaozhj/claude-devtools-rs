## 1. 后端 HTTP 路由切骨架（`cdt-api`）

- [x] 1.1 `crates/cdt-api/src/http/routes.rs::list_sessions` 把 `s.api.list_sessions_sync(&project_id, &pagination).await?` 改为 `s.api.list_sessions(&project_id, &pagination).await?`，删除原"HTTP 无 push 通道"注释，加新注释引用 `ipc-data-api` spec §"HTTP `list_sessions` 复用 IPC 骨架 + push 实现"；同步更新 `crates/cdt-api/src/ipc/traits.rs::list_sessions` / `list_sessions_sync` doc comment 反映新现实
- [x] 1.2 验证 `cdt-api::http::bridge::spawn_metadata_bridge` 已在 `spawn_event_bridge` 注册了 `metadata_rx` 订阅（已确认：`bridge.rs:106-128` 完整实现 `SessionMetadataUpdate` → `PushEvent::SessionMetadataUpdate` 转换；`mod.rs::start_server` 调 `spawn_event_bridge` 传入 `api.subscribe_session_metadata()` rx）
- [x] 1.3 新增 `crates/cdt-api/tests/http_list_sessions_skeleton_then_sse.rs` 集成测试：
  - 启 HTTP server + 喂 1 个项目下 3 条 session 的 jsonl fixture（cache 空）
  - `GET /api/projects/{id}/sessions?pageSize=20` 拿骨架响应（`title=null`）
  - 同时订阅 `/api/events` SSE，断言收到 3 条 `session_metadata_update` 事件携带真实值
- [x] 1.4 新增 `crates/cdt-api/tests/http_list_sessions_cache_hit_inline.rs`：cache 预热后 GET 路径直接返真值、零 SSE emit
- [x] 1.5 `cargo test -p cdt-api --tests` 全绿；`cargo clippy --workspace --all-targets -- -D warnings` 全绿

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

- [x] 7.1 `add-server-mode` PR #169 已 merge + PR #175 archive 已 merge 进 main；本 worktree 已 rebase origin/main
- [x] 7.2 rebase 后重读 `openspec/specs/http-data-api/spec.md` 与 `openspec/specs/ipc-data-api/spec.md` 主 spec：本 change 的 MODIFIED `Expose project and session queries`（ipc-data-api）/ `Serve projects and sessions over HTTP under /api prefix`（http-data-api）的"完整 Requirement 内容"与最新主 spec 一致；add-server-mode 没改本 change 引用的两条 Requirement
- [x] 7.3 校验 `crates/cdt-api/src/http/bridge.rs::spawn_metadata_bridge` 已订阅 `metadata_rx` 转 `PushEvent::SessionMetadataUpdate`，event 字段名 `project_id` / `session_id` / `title` / `message_count` / `is_ongoing` / `git_branch` 与本 change spec delta 引用一致；`broadcast::channel` capacity 由 `AppState::new(... capacity ...)` 调用方决定（cdt-cli / src-tauri 均传 ≥ 16，不会触发丢消息阈值）
- [x] 7.4 `openspec validate unify-session-list-loading-strategy --strict` 通过

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
