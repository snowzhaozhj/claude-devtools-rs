## 1. Audit / 准备（cdt-api + cdt-core + cdt-config）

- [x] 1.1 grep `crates/cdt-api/src/ipc/local.rs` `metrics` / `metadata` hand-built `json!({...})` 的实际 wire 字段名（snake_case 还是 camelCase），把结果记到 `design.md::Open Questions` 1 + 2 + 3 解答位置
- [x] 1.2 grep `crates/cdt-config/src/` 确认 `AppConfig` 是否 `#[serde(rename_all = "camelCase")]` + `NotificationRecord` 是否存在；不存在的位置 / 字段差异在本 task 列出
- [x] 1.3 grep `crates/cdt-core/src/search.rs::SearchSessionsResult` 是否与 `LocalDataApi::search` 返回的 hand-built JSON 完全一致；不一致的差异在本 task 列出
- [x] 1.4 grep `ui/src/lib/api.ts` 中 `update_config` 调用站点的预期返回类型（强 typed 还是 `Record<string, unknown>`）
- [x] 1.5 grep `crates/cdt-core/src/context.rs::ContextPhaseInfo` 字段是否全 `Option<>` 或已有 `#[derive(Default)]`；如缺失 `Default` derive 则在 task 5.x 中补
- [x] 1.6 跑 `cargo test -p cdt-api --test ipc_contract -- --nocapture` 拿现有 50 个测试 baseline 输出
- [x] 1.7 把 1.1-1.5 audit 结果写入本 task 文件下方 `## A. Audit 结果` 段（mid-apply 决策快照），如有 `D7 / D8` 需补的，去 `design.md` 加补丁段 + re-validate

## 2. 新建 typed struct（cdt-api）

- [x] 2.1 在 `crates/cdt-api/src/ipc/types.rs` 新增 `SessionDetailMetrics { message_count: usize }`，含 `#[serde(rename_all = "snake_case")]`（详 `design.md::D5` —— **不**用 camelCase，避免 P0 wire drift）+ `Debug` / `Clone` / `Serialize` / `Deserialize` / `PartialEq` / `Eq` derive
- [x] 2.2 在 `crates/cdt-api/src/ipc/types.rs` 新增 `SessionDetailMetadata { last_modified: Option<String>, size: Option<u64>, cwd: Option<String> }`，含 `#[serde(rename_all = "snake_case")]` + `Default` 等 derive
- [x] 2.3 如 1.2 / 1.3 audit 显示需要新建 `NotificationRecord` 或调整 `AppConfig` / `SearchSessionsResult`，在本 task 内补；本 change SHALL 不改 `cdt-core` 公共 API 已有字段名（仅可加 `Default` derive）
- [x] 2.4 跑 `cargo build -p cdt-api`，确认新 struct 编译通过

## 3. 改 SessionDetail 字段类型（cdt-api）

- [x] 3.1 修改 `crates/cdt-api/src/ipc/types.rs::SessionDetail` 6 个字段类型：`chunks: Vec<cdt_core::Chunk>` / `metrics: SessionDetailMetrics` / `metadata: SessionDetailMetadata` / `context_injections: Vec<cdt_core::ContextInjection>` / `injections_by_phase: BTreeMap<String, Vec<cdt_core::ContextInjection>>` / `phase_info: cdt_core::ContextPhaseInfo`
- [x] 3.2 在 `crates/cdt-api/src/ipc/local.rs:3239-3254` SessionDetail 构造点把 `serde_json::to_value(&xxx)` 改成直接传 typed 值；hand-built `json!({"message_count": N})` 改成 `SessionDetailMetrics { message_count: N }`；`metadata` 同理改 `SessionDetailMetadata { ... }`
- [x] 3.3 grep `crates/cdt-api/src/ipc/local.rs` 中所有 `SessionDetail { chunks: ..., metrics: ..., ... }` 字面量构造（含 3485 / 3498 行 `get_sessions_by_ids` not-found fallback 路径）同步改 typed default 值；按 `design.md::D9` 移除 `{"status":"not_found"}` ad-hoc 标记，改用 `SessionDetailMetadata::default()` / `Vec::new()` / `ContextPhaseInfo::default()`
- [x] 3.3b 修 `crates/cdt-api/tests/http_session_detail_global_lookup.rs:230` `get_sessions_by_ids_handles_mixed_existence` 断言：从 `metadata == json!({"status":"not_found"})` 改为按 typed default 期待（`metadata.cwd.is_none()` + `metadata.last_modified.is_none()` 等）
- [x] 3.4 跑 `cargo build -p cdt-api`，编译错误处逐处修
- [x] 3.5 跑 `cargo clippy -p cdt-api --all-targets -- -D warnings`，警告处修

## 4. 改 5 个高频 DataApi method 签名（cdt-api）

- [x] 4.1 修改 `crates/cdt-api/src/ipc/traits.rs::DataApi::search` 签名为 `Result<cdt_core::SearchSessionsResult, ApiError>`
- [x] 4.2 修改 `DataApi::get_config` / `DataApi::update_config` 签名为 `Result<cdt_config::AppConfig, ApiError>`
- [x] 4.3 修改 `DataApi::get_subagent_trace` 签名为 `Result<Vec<cdt_core::Chunk>, ApiError>`
- [x] 4.4 修改 `DataApi::get_notifications` 签名为 `Result<cdt_config::GetNotificationsResult, ApiError>`（apply audit 1.2 已确认实际类型是 `GetNotificationsResult` envelope，不是 `Vec<NotificationRecord>`）
- [x] 4.5 同步改 `crates/cdt-api/src/ipc/local.rs::LocalDataApi` 5 个 method 实现：把 `Ok(json!({...}))` 改成 `Ok(typed_struct)`
- [x] 4.5b `LocalDataApi::search` empty query 短路构造完整 `SearchSessionsResult { results: vec![], total_matches: 0, sessions_searched: 0, query: "".to_string(), is_partial: false }`（详 `design.md::D8`，wire 形状从 4 字段扩为 7 字段属于 bug fix）
- [x] 4.6 给 13 个暂留 Value 的 trait method 在 `traits.rs` 源代码处加 `// TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2` 注释（行内或 method 上方一行均可）
- [x] 4.7 跑 `cargo build -p cdt-api`，编译错误处逐处修
- [x] 4.8 跑 `cargo clippy -p cdt-api --all-targets -- -D warnings`

## 5. HTTP route handler 跟改（cdt-api）

- [x] 5.1 grep `crates/cdt-api/src/http/routes/` 中 `search` / `get_config` / `update_config` / `get_subagent_trace` / `get_notifications` 5 个 method 对应的 axum handler；如有 `axum::Json<serde_json::Value>` 显式标注，改成对应 typed
- [x] 5.2 跑 `cargo build -p cdt-api --features http`，编译错误处逐处修
- [x] 5.3 跑 `cargo clippy -p cdt-api --all-targets -- -D warnings`

## 6. 改 ipc_contract 测试（cdt-api）

- [x] 6.1 修改 `crates/cdt-api/tests/ipc_contract.rs` 中 5 处 SessionDetail 构造点（grep `SessionDetail {` 找全），把 `chunks: json!([])` / `metrics: json!({})` / 等字面量改成 typed `Vec::new()` / `SessionDetailMetrics { message_count: 0 }` / `SessionDetailMetadata::default()` / `BTreeMap::new()` / `ContextPhaseInfo::default()`
- [x] 6.2 新增测试 `session_detail_typed_metrics_metadata_round_trip`：构造 typed `SessionDetail` → `serde_json::to_value(&detail)` → 逐字段断言 wire 顶层 key 名（`metrics` / `metadata` 顶层 camelCase，**内部字段 snake_case**：`message_count` / `last_modified` / `size` / `cwd`，详 `design.md::D5` + `D7`）+ 断言反序列化回 typed `PartialEq` 通过
- [x] 6.3 新增测试 `chunk_kind_tag_value_preserved`：构造典型 `Chunk::User` / `Chunk::Ai` 等 → 序列化 → 断言 `kind` 字段取值在 `{user, ai, system, compact}` 集合内（防 enum tag 漂移）
- [x] 6.4 新增测试 `injections_by_phase_btreemap_key_is_string`：用 `phase_number.to_string()` 作 key 构造 → 序列化 → 断言 JSON 顶层 `injectionsByPhase` 的 key 集合是 `String`、且按字典序输出
- [x] 6.5 跑 `cargo test -p cdt-api --test ipc_contract`，红了修
- [x] 6.6 跑 `cargo test -p cdt-api`，5 个 high-frequency method typed 返回的相关测试全部通过

## 7. 改前端 typed（ui）

- [x] 7.1 修改 `ui/src/lib/api.ts::SessionDetail` interface：`metrics: SessionDetailMetrics`（新建 TS interface）/ `metadata: SessionDetailMetadata`（新建）/ `contextInjections: ContextInjection[]`（确认已有 ContextInjection TS 类型）/ `injectionsByPhase?: Record<string, ContextInjection[]>`
- [x] 7.2 在 `ui/src/lib/api.ts` 或 `ui/src/lib/types/` 新增 `SessionDetailMetrics` / `SessionDetailMetadata` TS interface（与 Rust 端 camelCase 字段镜像）
- [x] 7.3 grep `ui/src/components/**/*.svelte` 中读取 `detail.metrics` / `detail.metadata` / `detail.contextInjections` / `detail.injectionsByPhase` 的位置（重点 `ContextPanel.svelte` / `SubagentCard.svelte` / `SessionDetailPage.svelte`），typed 后编译错误处修
- [x] 7.4 修改 `update_config` / `get_config` / `search` / `get_subagent_trace` / `get_notifications` 5 个 IPC 调用站点的前端 typed 返回类型；如前端先前未强 typed（用 `unknown` / `any`）则同步加上
- [x] 7.5 跑 `pnpm --dir ui run check`（svelte-check），错误处修
- [x] 7.6 跑 `pnpm --dir ui run test`（vitest），红了修

## 8. 写 spec delta + validate

- [x] 8.1 `openspec validate typed-ipc-payload --strict` 通过（specs/ipc-data-api/spec.md 已在 propose 阶段写好，本 task 只校验）
- [x] 8.2 检查 `proposal.md` `Modified Capabilities` 段与 spec delta 文件目录命名一致

## 9. 手动 smoke + e2e

- [ ] 9.1 跑 `just dev` 启动桌面端，打开一个真实大会话（≥ 1000 messages），检查 SessionDetailPage / ContextPanel / SubagentCard 渲染行为不变（首屏可见、没有 Console 红字、`detail.metrics.messageCount` / `detail.metadata.lastModified` 等字段消费正确）
- [ ] 9.2 触发 5 个 typed 化 method（`search` 输入查询词 / `get_config` 启动即拿 / `update_config` 改 Settings 保存 / `get_subagent_trace` 展开 SubagentCard / `get_notifications` 启动即拿），DevTools Console 无 type error / 无网络请求形状漂移
- [ ] 9.3 如改动覆盖 HTTP transport（`server-mode` capability 走 http），用 `cdt-cli serve` + 浏览器 `?http=1` 入口跑 e2e（参照 `e2e-http-verify` skill）—— 仅当本 change 影响 HTTP route 形状时执行（按 1.x audit 决定）

## 10. 流水线（一把梭）

- [x] 10.1 跑 `just preflight`（fmt + lint + test + spec-validate）一次过
- [ ] 10.2 git commit 业务改动 + spec delta（**不**含 archive；archive 留 N.4）；commit message 引用 change slug

## A. Audit 结果

（task 1.x 落地后 mid-apply 决策快照）

- **1.1 wire 形状**：`local.rs:3243-3247` hand-built JSON 是 **snake_case**（`message_count` / `last_modified` / `size` / `cwd`）—— codex propose 二审 P0 已点出，design.md::D5 已改 `#[serde(rename_all = "snake_case")]` rename 对齐
- **1.2 `cdt-config` 类型**：`AppConfig` `#[serde(rename_all = "camelCase")]` ✅；**`NotificationRecord` 不存在**——`get_notifications` 实际返回 `cdt_config::GetNotificationsResult`（`notification_manager.rs:27`，camelCase），含 `notifications` / `total` / `totalCount` / `unreadCount` / `hasMore` 5 字段；**修订**：design.md::D6 / proposal.md / tasks.md::4.4 已同步改为 `GetNotificationsResult`
- **1.3 `cdt_core::SearchSessionsResult`**：camelCase（`search.rs:31`）含 `results` / `totalMatches` / `sessionsSearched` / `query` / `isPartial` 5 字段 ✅；但 `LocalDataApi::search:3520-3525` 在 `query.is_empty()` 时 hand-built `{"query":"","results":[]}` 缺 3 字段——**新增 D8 决策**：empty query 路径 typed 化后构造完整 `SearchSessionsResult`，wire 形状扩字段属于 bug fix
- **1.4 前端 typed 状态**：`getConfig` / `updateConfig` 已 `Promise<AppConfig>` ✅；`getNotifications` 已 `Promise<GetNotificationsResult>` ✅；本 change 不需改前端 5 个 method 的调用类型（已强 typed），仅需改 `SessionDetail` interface 4 个弱类型字段
- **1.5 `ContextPhaseInfo`**：`crates/cdt-core/src/context.rs:289` `#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]` 已含 `Default` ✅，task 5.x 不需补 derive
- **1.6 baseline test**：`cargo test -p cdt-api --test ipc_contract` 115 通过 0 失败（不是 design.md 估计的 50；估计偏低但约束不变，无须改文档）
- **1.7 design.md / proposal.md / tasks.md / spec delta 已同步更新**：D5 改 snake_case rename + D6 修订 NotificationRecord → GetNotificationsResult + D7 metrics/metadata 不修 camelCase 的 trade-off 决策 + D8 search empty query bug fix mid-apply 新增决策；spec scenario 字段名同步 snake_case；tasks.md 加 4.5b


## N. 发布

- [ ] N.1 `git push -u origin feat/typed-ipc-payload` + `gh pr create` 描述含 `Perf impact`（typed 化 wire 形状不变，预期 0 影响；列基准跑分作 evidence）+ 链回 `openspec/changes/typed-ipc-payload/proposal.md`
- [ ] N.2 `/wait-ci <pr>` 全绿（与 N.3 并行启动）
- [ ] N.3 调 `Agent({ subagent_type: "codex:codex-rescue", ... })` 跑 PR 二审，prompt 重点查：序列化形状漂移（`Chunk.kind` / `ContextInjection.category` enum tag / camelCase / `injections_by_phase` key 类型 / `xxxOmitted` 标记）/ 前端 typed match 漏改 / `ipc_contract.rs` 覆盖（`metrics` / `metadata` 新 struct 字段命名 + `Chunk` enum tag + injectionsByPhase key 类型）/ 13 个暂留 method 的 TODO 注释是否都加 / `D5` 字段命名是否与历史 wire 形状一致（含 1.1 audit 结论）—— 报 bug 修 → push → 回 N.2 重跑（可循环）
- [ ] N.4 N.2 + N.3 都通过后跑 `openspec archive typed-ipc-payload -y`（原子完成 mv + sync）→ `git add -A` + `git commit -m "chore(opsx): archive typed-ipc-payload"` + push → 再走一次 wait-ci 全绿；不 merge；最终发文本总结
- [ ] N.5 archive 后开 followup GitHub issue：标题 "metrics / metadata IPC wire 字段名修正为 camelCase（修复 IPC 契约违规）"，body 引用 `design.md::D7` + 列出修法（改 `#[serde(rename_all = "camelCase")]` + grep 全仓 snake_case key 消费者 + ipc_contract 加 camelCase 断言 + 标 BREAKING + minor version bump），label `bug` + `tech-debt`
