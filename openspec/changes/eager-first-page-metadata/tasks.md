## 1. cdt-api: list_sessions cursor=None 路径同步等真值

- [x] 1.1 `crates/cdt-api/src/ipc/local.rs` 顶层加 const `EAGER_FIRST_PAGE_LIMIT: usize = 20` 与 `EAGER_PER_SESSION_TIMEOUT: Duration = Duration::from_millis(500)`
- [x] 1.2 `LocalDataApi::list_sessions` 内识别 `pagination.cursor.is_none()` 分支：调 `list_sessions_skeleton` 拿 items → 取前 `min(items.len(), EAGER_FIRST_PAGE_LIMIT)` 条进 eager 同步等：`futures::future::join_all` + 共享 `Arc<Semaphore>` 并发；每条包 `tokio::time::timeout(EAGER_PER_SESSION_TIMEOUT, extract_session_metadata_cached(...))` 限时；超时 / 字段全占位条 SHALL 保留骨架占位
- [x] 1.3 eager 路径 SHALL NOT broadcast emit 已同步等到真值的 items；超时 / 失败条 SHALL spawn deferred 单条 retry（500ms 延迟后无 timeout 重新调，最多 retry 1 次；成功后通过 `broadcast::Sender::send` emit；失败保留占位）
- [x] 1.4 `page_size > EAGER_FIRST_PAGE_LIMIT`：剩余 `page_size - 20` 条 SHALL 走骨架 + spawn `scan_metadata_for_page` + broadcast 路径（与翻页路径同模型）
- [x] 1.5 (移到 1.7b 统一) — 切 project 路径 abort 旧扫描的实现详见 1.7b（D4b 修订：按 `key.split('|').next()` 解析 projectId 比较，不限于单一旧 project）
- [x] 1.6 翻页路径（`pagination.cursor.is_some()`）保留原行为：lookup-fast-path + spawn scan + broadcast push 完全不动；对其它 project 翻页路径之间互不 abort
- [x] 1.7 `crates/cdt-api/src/ipc/session_metadata.rs::extract_session_metadata_cached`：加 `bypass_negative: bool` 参数（默认 false）。解析返回字段全占位（`title=None && message_count=0 && !is_ongoing && git_branch=None`）时 SHALL NOT 写入正向 `MetadataCache`（D6b）；同时维护 `negative_results: Map<sessionId, (FileSignature, Instant)>` 写入失败 sessionId + signature + 时刻；`bypass_negative=false` 路径 60s `NEGATIVE_TTL` 内对同 sessionId（FileSignature 等价）跳过解析直接返占位（D6c），避免永久损坏 jsonl 持续 spike；**deferred retry 路径** SHALL 用 `bypass_negative=true` 调用绕过 negative TTL 强制重新解析（codex v3 复审 issue 3 修复：避免自我阻断）
- [x] 1.7b 切 project 时 `LocalDataApi::list_sessions(projectB, cursor=None)` eager 路径开始前遍历 `active_scans`，按 `key.split('|').next()` 解析 projectId，abort 所有 `projectId != "projectB"` 的 entry（D4b 修订：不限于 projectA 单一来源，所有非当前 project 的旧 scan 都 abort 让出 permits）
- [x] 1.7c remainder scan 的 `active_scans` key 编码为 `format!("{project_id}|None|remainder")`（D11）；同 key 的新 spawn 走既有 generation token cleanup abort 旧 entry，避免高频 silent refresh 反复 spawn 形成永远跑不完循环
- [x] 1.8 `crates/cdt-api/src/ipc/traits.rs::list_sessions` doc comment 显式刻入 cursor 分叉契约（D8）：cursor=None 含真值；cursor=Some 可骨架
- [x] 1.9 `crates/cdt-api/src/http/routes.rs::list_sessions` HTTP handler 不动——继续调 `DataApi::list_sessions(...)`，由 trait 实现内部按 `cursor` 分叉
- [x] 1.10 `crates/cdt-api/src/ipc/traits.rs::list_sessions_sync` 保留作为 trait fallback 不动；axum HTTP route 仍不允许调用

## 2. 测试覆盖（后端）

- [x] 2.1 `crates/cdt-api/tests/session_metadata_stream.rs` 新增 scenario：
  - 首页 `cursor=None` 路径调用 `list_sessions` 后 `subscribe_session_metadata` receiver 在 300ms 内收到 0 条 update（验证 eager 路径前 20 条不 emit broadcast）；同时响应 items 含真值字段
  - 首页 eager 单条 metadata 解析超时（mock 一个 jsonl 让 `extract_session_metadata_cached` 阻塞 > 500ms）：响应该条占位 + receiver 后续收到 deferred retry emit 的 update
  - 首页 eager 单条解析失败（mock 损坏 jsonl）：响应该条占位 + 正向 `MetadataCache` 不含该 sessionId entry（D6b 不写 cache）+ receiver 收到 deferred retry emit
  - **D6c**: 永久损坏 jsonl 60s negative TTL backoff——连续多次 list_sessions 同 sessionId 仅第一次调 `extract_session_metadata_with_ongoing`；60s 内重复正常请求（`bypass_negative=false`）`negative_results` 命中直接返占位（不再调解析）；60s 后或 `FileSignature` 不等时重新尝试
  - **D6c retry bypass**（codex v3 issue 3）：deferred retry 调 `extract_session_metadata_cached(.., bypass_negative=true)` 跳过 negative TTL 强制重新解析；如果 jsonl 已恢复 retry 成功 → 移除 negative_results + 写正向 cache + emit broadcast；如果仍失败 → 重写 negative_results 续 60s
  - **D4b 修订**: projectA 翻页扫描 + projectC 翻页扫描同时进行中，调 `list_sessions("projectB", cursor=null)` 进入 eager 时遍历 active_scans 按 projectId 解析 abort 所有 != "projectB" 的 entry（projectA + projectC 两个都被 abort）；不限于单一旧 project
  - **D11**: 高频 silent refresh + `pageSize > 20` remainder scan 同 `format!("{project_id}|None|remainder")` key dedupe——active_scans 中始终至多 1 个 entry；最后一次 spawn 完成；中间被 abort 的 task 不泄漏
- [x] 2.2 `crates/cdt-api/tests/http_list_sessions_skeleton_then_sse.rs` 现有 scenarios 改写：`cursor=None` 路径直接断 items 含真值；新增 `cursor=Some` 翻页路径用例覆盖原骨架+SSE 行为
- [x] 2.3 `crates/cdt-api/tests/http_list_sessions_cache_hit_inline.rs` 现行测试若依赖"骨架 + cache 命中 inline"行为，需更新——首页 cursor=None 路径无骨架阶段（直接全是真值），cache 命中 / miss 都同步 await
- [x] 2.4 `crates/cdt-api/tests/ipc_contract.rs::list_sessions` 形状不变，但**值含义** assertion 加：首页 `cursor=None` 响应前 20 条中 `title` / `messageCount` / `isOngoing` / `gitBranch` 至少有一个非占位（除非真无 user message）
- [x] 2.5 新建 `crates/cdt-api/tests/perf_eager_first_page.rs`：模仿 `perf_cold_scan.rs` 结构，用 `perf_fixture.rs` 生成 30 project × 50 session corpus；分别量测：
  - `list_sessions(project, { page_size: 20, cursor: None })` wall-clock + user/sys + RSS（基线 page_size=20）
  - `list_sessions(project, { page_size: 50, cursor: None })` wall-clock（验证 D5b 切割：前 20 条 eager + 后 30 条骨架，wall < 300ms）
  - 复合场景：projectA 翻页扫描进行中 + projectB 首页 eager 启动（验证 D4b abort + permit 立即可用，wall 未被 projectA 拖慢）
  - 输出 `[perf]` 行；建立 baseline 写入 `tests/perf-baseline.json`
- [x] 2.6 `scripts/run-perf-bench.sh` 加 `perf_eager_first_page` 到 bench 列表（与 `perf_cold_scan` / `perf_get_session_detail` 并列），CI gate `+20% wall / +50% user / RSS +30%` 阈值

## 3. 前端实现 + 测试

- [x] 3.1 `ui/src/lib/transport.ts::BrowserTransport.invokeHttp(cmd, args)`：D9 + D9b cursor 分叉 ensureSseReady gate——`cmd === 'list_sessions' && (!args.cursor || args.cursor === null)` 不 await `ensureSseReady()` 但 fire-and-forget `void this.ensureSseReady()`（异步触发 SSE 订阅 + sse-recovered 兜底）；其它命令保留原行为
- [x] 3.2 `ui/src/lib/transport.test.ts` 新增用例：
  - `list_sessions(cursor=null)` 不 await `ensureSseReady`（不阻塞 fetch）但 fire-and-forget 触发 ensureSseReady（spy 看到调用）
  - **fast-open 竞态**（codex v3 issue 1）：`list_sessions(cursor=null)` 调用时 EventSource 处于 `CONNECTING`，立即设 `sseRecoveryPending=true`；EventSource 在 fetch 后 < 1000ms 内成功 OPEN，emit `sse-recovered` 给 handler（不依赖 timeout 路径）
  - timeout-then-open 路径：EventSource 1000ms 内未 OPEN，timeout 后 reconnect 成功 OPEN，emit `sse-recovered`（既有路径）
  - `list_sessions(cursor='20')` 调用 `ensureSseReady` 1000ms gate
  - `list_repository_groups` / `list_worktree_sessions` 仍 await `ensureSseReady`
- [x] 3.3 `ui/src/lib/sessionMerge.ts::applySilentRefresh`：`mergeSessions(prev, firstPageItems, true)` 改成 `mergeRecoveryResponse(prev, firstPageItems)`（沿用现有函数，不需要新增；D3b）
- [x] 3.4 `ui/src/lib/sessionMerge.test.ts` 新增 / 修改 `applySilentRefresh` 用例：
  - silent refresh response 含真值时覆盖 prev stale（关键回归测试，验证不再被压住）
  - silent refresh response 是占位（极端 deferred retry 失败）时保留 prev 真值
  - silent refresh 仍保留 prev 尾部超出第一页的 sessionId（既有 Scenario 不破坏）
- [x] 3.5 `ui/src/components/Sidebar.svelte` 不需要 markup / CSS 改动（首页路径下 items 含真值，`.metadata-pending` class 自然不触发；翻页路径上仍按既有行为）
- [x] 3.6 `pnpm --dir ui run check` + `pnpm --dir ui exec vitest run` 跑过

## 4. 验证与文档

- [x] 4.1 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 4.2 `cargo fmt --all`
- [x] 4.3 `cargo test --workspace`
- [x] 4.4 `bash scripts/run-perf-bench.sh --bench perf_eager_first_page --runs 5` 跑完写入 baseline；首页 `cursor=None` wall 分档（codex v2 复审 issue 6）：**p50 < 300ms / p95 < 500ms / worst < 1500ms**（D7 timeout 保护）；page_size=20 与 page_size=50（D5b 切割）两组都满足；user/real ≤ 0.5
- [x] 4.5 `openspec validate eager-first-page-metadata --strict`

## N. 发布

- [x] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
