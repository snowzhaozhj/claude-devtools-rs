## Why

`crates/cdt-api/src/ipc/types.rs::SessionDetail` 当前 6 个字段（`chunks` / `metrics` / `metadata` / `context_injections` / `injections_by_phase` / `phase_info`）以 `serde_json::Value` 持有，`DataApi` trait 53 个方法中有 18 个返回 `Result<serde_json::Value, ApiError>`。这两层弱类型让 `ipc-data-api` 的 wire 形状失去编译期保护——序列化字段命名（camelCase / `xxxOmitted`）/ enum tag（`Chunk.kind` / `ContextInjection.category`）/ 字段缺漏只能靠 `tests/ipc_contract.rs` 50 个测试事后兜底，跨 transport（Tauri IPC + HTTP route + 未来 SSH bridge）容易漂移。spec 已按 typed 字段名写约束（`SessionDetail.chunks` / `AIChunk.subagents[i].messages` / `messagesOmitted=true`），实现层却仍是 `Value` —— 这是 spec 与实现的 fidelity gap。

现在改的契机：`cdt-core::Chunk` / `cdt_core::ContextInjection` / `cdt_core::ContextPhaseInfo` 已实现 `Serialize + Deserialize` 且字段命名稳定，wire 形状已被 `OMIT_*` payload 瘦身路径冻结；本次重构 SHALL **不改任何 JSON 形状**，只把 `Value` 静态化为 typed struct 让编译器接管字段契约。

## What Changes

- **MODIFY** `crates/cdt-api/src/ipc/types.rs::SessionDetail` 的 6 个 `serde_json::Value` 字段改成 typed：
  - `chunks: Vec<cdt_core::Chunk>`
  - `metrics: SessionMetrics`（**新建**）
  - `metadata: SessionMetadata`（**新建**）
  - `context_injections: Vec<cdt_core::ContextInjection>`
  - `injections_by_phase: BTreeMap<String, Vec<cdt_core::ContextInjection>>`（key 保持 `String`，JSON object key 类型不变）
  - `phase_info: cdt_core::ContextPhaseInfo`
- **NEW** 在 `crates/cdt-api/src/ipc/types.rs` 中新增两个轻量 struct（取代 `local.rs` 内 hand-built `json!({...})`）：
  - `SessionDetailMetrics { message_count: usize }`（**保 snake_case wire**：用 `#[serde(rename_all = "snake_case")]`，与 `local.rs:3243` hand-built `json!({"message_count": ...})` 历史 wire 逐字段对齐 —— 详 `design.md::D5`；命名加 `SessionDetail` 前缀避免与 `cdt_discover::SessionMetadata` 撞名）
  - `SessionDetailMetadata { last_modified: Option<i64>, size: Option<u64>, cwd: Option<String> }`（**保 snake_case wire**：apply 阶段从 `local.rs` 反推真实类型——`last_modified` 是 unix epoch ms 整数；与历史 `last_modified` / `size` / `cwd` wire key + JSON number/string 类型一致）
- **MODIFY** **BREAKING** `crates/cdt-api/src/ipc/traits.rs::DataApi` 选 5 个高频方法的 `Result<serde_json::Value, ApiError>` 改成 typed `Result<XxxResponse, ApiError>`（trait 公共 method 签名变更，对外部 crate 的 `impl DataApi` / `Arc<dyn DataApi>` consumer **source-incompatible**；本仓内唯一实现者是 `LocalDataApi`，`Arc<dyn DataApi>` 用法集中在 `crates/cdt-api/tests/http/` 6 处，本 PR 同步跟改）：
  - `search` → `SearchResult`（参照 `cdt_core::SearchSessionsResult`）
  - `get_config` / `update_config` → `cdt_config::AppConfig`
  - `get_subagent_trace` → `Vec<cdt_core::Chunk>`
  - `get_notifications` → `cdt_config::GetNotificationsResult`（apply audit 修订：实际类型是已存在的 `GetNotificationsResult` envelope，不是 `Vec<NotificationRecord>`——后者不存在）
- **MODIFY** `LocalDataApi::search` 在 `query.is_empty()` 时构造完整 `SearchSessionsResult` 返回（含 `totalMatches=0` / `sessionsSearched=0` / `isPartial=false` 三个新字段）—— 修历史 hand-built 短路 `{"query":"","results":[]}` 缺字段 bug；详 `design.md::D8`
- **MODIFY** `LocalDataApi::get_sessions_by_ids` not-found fallback 路径改用 typed default 值（`SessionMetadata::default()` / `chunks: vec![]` 等），**移除** ad-hoc `{"status":"not_found"}` 带外标记；前端按 `result.projectId === ""` 信号判定 not-found（已有信号）；详 `design.md::D9`
- **PRESERVE** 其余 13 个返回 `Value` 的 trait 方法暂不改（SSH 子集 7 + 文件路径子集 3 + Trigger CRUD 2 + `validate_path` 1）—— 见 `design.md::D2` 判定准则
- **PRESERVE** wire JSON 形状 byte-for-byte 不变：所有新 typed struct 的 serde 字段名 / camelCase / enum tag / `xxxOmitted` 标记 SHALL 与现有 `tests/ipc_contract.rs` round-trip 断言一致
- **MODIFY** 前端 `ui/src/lib/api.ts::SessionDetail` interface：将 `metrics: Record<string, unknown>` / `metadata: Record<string, unknown>` / `contextInjections: unknown[]` / `injectionsByPhase?: Record<string, unknown[]>` 替换为对应 typed TS interface（与 Rust struct 镜像）
- **MODIFY** `crates/cdt-api/tests/ipc_contract.rs` 的 SessionDetail 构造点（共 ~5 处直接 `json!([])` / `json!({})` 字面量）改用 typed 构造函数；新增字段命名 round-trip 断言
- **NOT-CHANGED**：`http-data-api` 的 wire 形状、HTTP route 行为、SSE event payload、`OMIT_*` 瘦身策略、所有前端 IPC 调用站点签名

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `ipc-data-api`：把当前 spec 中以 typed 字段名引用的 `SessionDetail.chunks` / `AIChunk.subagents` / `injectionsByPhase` 等约束补一条 implementation-binding requirement——`SessionDetail` 与 5 个高频 DataApi 方法的实现层 SHALL 用 typed Rust struct 暴露字段，**不**得用 `serde_json::Value` 持有；wire 形状 SHALL 与本次 change 之前完全一致

### Unchanged Capabilities

- `http-data-api`：HTTP route 内部仍可调用 typed 化后的 `DataApi` 方法（trait 契约 transport agnostic），wire JSON 形状不变；该 spec **不**需要 delta

## Impact

### Affected code

- `crates/cdt-api/src/ipc/types.rs`（核心改动：SessionDetail 字段类型 + 两个新 struct）
- `crates/cdt-api/src/ipc/traits.rs`（5 个 trait method 签名改 typed）
- `crates/cdt-api/src/ipc/local.rs`（构造点 `json!({...})` → typed struct；2750 行附近 `LocalDataApi` impl 跟进）
- `crates/cdt-api/src/ipc/mod.rs`（如需 re-export 新 struct）
- `crates/cdt-api/tests/ipc_contract.rs`（~5 处构造点 + round-trip 断言；50 个测试中其余只读字段名的不需改）
- `crates/cdt-api/src/http/routes/*.rs`（如有 `axum::Json(serde_json::Value)` route handler 直接转发 trait 返回值的位置 → 改 typed 后等价 `axum::Json(typed)`，wire 形状不变）
- `ui/src/lib/api.ts`（SessionDetail interface 4 个 `unknown` 字段 typed 化）
- `ui/src/lib/types/`（如新增 mirror 类型文件）
- `ui/src/components/**`（消费侧 `Record<string, unknown>` 取字段处编译错误处修——`ContextPanel.svelte` / `SessionDetailPage.svelte` 等）

### Affected APIs

- 仅编译期 Rust signature / TS interface 变；wire JSON byte-for-byte 不变
- **BREAKING**：`DataApi` trait 公共签名变更（5 个方法）—— 对外部 crate 的 `impl DataApi` 与 `Arc<dyn DataApi>` consumer source-incompatible；本仓内唯一实现者是 `LocalDataApi`，trait object 用法仅 6 处全在 HTTP test 文件，本 PR 同步跟改；外部 crate 当前无依赖（cdt-api 不是 published crate，仅本仓内消费）

### Affected dependencies

- 无新 crate 依赖
- `BTreeMap` 比 `HashMap` 多一个稳定排序保证（`injections_by_phase` 序列化时 key 顺序确定，前端按 `phase_number.to_string()` 字典序读取，与原 `serde_json::Value::Object` 默认形状一致）

### Risks

详见 `design.md::Risks`，关键三条：(1) `metrics` / `metadata` 之前是 `local.rs` 手拼 ad-hoc snake_case JSON，新建 struct 时 serde 字段名 SHALL 用 snake_case rename 与历史 wire 逐字段对齐（**不**改 camelCase，避免 P0 wire drift；camelCase 修正留 followup issue 单独 PR 跟踪）；(2) `Chunk` / `ContextInjection` 是 internally-tagged enum，wire 形状对 `kind` / `category` tag 字段名敏感；(3) ipc_contract test 50 个里 ~5 个构造点要从 `Value::Array(vec![])` 改成 `Vec::new()` 等真实 typed 默认值。
