## ADDED Requirements

### Requirement: SessionDetail 与高频 DataApi 方法 SHALL 用 typed Rust struct 暴露字段

`crates/cdt-api/src/ipc/types.rs::SessionDetail` 的 6 个字段（`chunks` / `metrics` / `metadata` / `context_injections` / `injections_by_phase` / `phase_info`）SHALL 用 typed Rust struct（含本 capability 新增的 `SessionDetailMetrics` / `SessionDetailMetadata` 与 `cdt-core` 已有的 `Chunk` / `ContextInjection` / `ContextPhaseInfo`）持有；`DataApi` trait 中至少以下 5 个高频方法的返回类型 SHALL 是 typed `Result<XxxResponse, ApiError>` 而非 `Result<serde_json::Value, ApiError>`：`search` / `get_config` / `update_config` / `get_subagent_trace` / `get_notifications`。typed 化 SHALL **不**改变任何 wire JSON 形状——所有 typed struct 的 serde 字段名、camelCase 命名、enum tag（`Chunk.kind` / `ContextInjection.category`）、`xxxOmitted` 标记 SHALL 与本要求被引入之前的 wire 形状逐字节一致。其余 13 个 `Result<serde_json::Value, ApiError>` 方法（SSH 子集 / 文件路径子集 / Trigger CRUD / `validate_path`）SHALL 暂留 `Value`，由后续 change 按本 capability 提供的判定准则（`design.md::D2`）逐批 typed 化。

#### Scenario: SessionDetail 6 个字段编译期为 typed

- **WHEN** 调用方在 Rust 代码中按 `let detail: SessionDetail = local_data_api.get_session_detail(...).await?;` 取得 `SessionDetail`
- **THEN** `detail.chunks` SHALL 直接是 `Vec<cdt_core::Chunk>`，`detail.metrics` SHALL 是 `SessionDetailMetrics`，`detail.metadata` SHALL 是 `SessionDetailMetadata`，`detail.context_injections` SHALL 是 `Vec<cdt_core::ContextInjection>`，`detail.injections_by_phase` SHALL 是 `BTreeMap<String, Vec<cdt_core::ContextInjection>>`，`detail.phase_info` SHALL 是 `cdt_core::ContextPhaseInfo`
- **AND** 上述任一字段 SHALL **不**是 `serde_json::Value`
- **AND** 调用方按 `detail.metrics.message_count` 直接访问字段 SHALL 编译通过（不需 `serde_json::from_value` / `as_object()` 之类 runtime 解构）

#### Scenario: SessionDetail 序列化 wire 形状不变

- **WHEN** 同样的输入数据分别走 typed 化前与 typed 化后的 `LocalDataApi::get_session_detail`，并各自 `serde_json::to_value(&detail)?` 序列化
- **THEN** 两次序列化产物 SHALL 在所有 key 名 / value 形状 / 嵌套层次上逐字段一致——具体含 `chunks[*].kind`（`"user"` / `"ai"` / `"system"` / `"compact"`）、`chunks[*].subagents[*].messages` / `messagesOmitted`、`chunks[*].toolExecutions[*].output` / `outputOmitted`、`chunks[*].responses[*].content` / `contentOmitted`、`metrics.message_count`（snake_case 历史 wire，**不**是 `messageCount`，详 `design.md::D5` + `D7`）、`metadata.last_modified` / `metadata.size` / `metadata.cwd`（snake_case 历史 wire）、`contextInjections[*].category`、`injectionsByPhase` 的 key 形状（`String`，由 `phase_number.to_string()` 得出）、`phaseInfo` 内字段
- **AND** `crates/cdt-api/tests/ipc_contract.rs` 现有覆盖 SessionDetail 的所有断言（含 `session_detail_single_phase_injections_by_phase_equals_context_injections` / `session_detail_multi_phase_preserves_phase1_injections` / `session_detail_title_field_round_trip`）SHALL 保持绿

#### Scenario: 5 个高频 DataApi 方法返回 typed

- **WHEN** 调用方在 Rust 代码中按 `let cfg: AppConfig = local_data_api.get_config().await?;`（或 `update_config` / `search` / `get_subagent_trace` / `get_notifications` 同形）取得返回值
- **THEN** 返回类型 SHALL 是 typed struct（`cdt_config::AppConfig` / `cdt_core::SearchSessionsResult` / `Vec<cdt_core::Chunk>` / `cdt_config::GetNotificationsResult`）而非 `serde_json::Value`
- **AND** 编译期访问字段（如 `cfg.theme` / `result.results[0].sessionId`）SHALL 通过类型检查
- **AND** `serde_json::to_value(&typed_return)?` 产物 SHALL 与 typed 化前的 hand-built JSON 形状逐字段一致；以下两处 EXCEPTION：
  - `search` empty query 路径：typed 化后形状从 `{query, results}` 扩为 `{query, results, totalMatches, sessionsSearched, isPartial}`（`SearchSessionsResult` 完整字段），属于 bug fix（详 `design.md::D8`），新增字段全部为 `0` / `[]` / `false` 默认值，不破坏前端 `CommandPalette.svelte:116` 现有 `"totalMatches" in session ? ... : ...` "in" 判定路径
  - `get_sessions_by_ids` not-found fallback 路径：typed 化后 `metadata` 从 `{"status":"not_found"}` 改为 typed default `{"last_modified":null,"size":null,"cwd":null}`（移除 ad-hoc status 带外标记），`chunks` / `phase_info` / `metrics` 从 `null` 改为各自 typed default；前端按 `result.projectId === ""` 判定 not-found（已有信号），详 `design.md::D9`

#### Scenario: 13 个低频方法暂留 Value 是 spec-allowed

- **WHEN** 调用方按 `let resp: serde_json::Value = local_data_api.ssh_connect(...).await?;`（或其他 13 个低频方法之一）取得返回值
- **THEN** 实现 SHALL 仍允许返回 `Result<serde_json::Value, ApiError>`，不要求本 change 必须 typed 化
- **AND** 该方法源码处 SHALL 含 `// TODO(typed-ipc-payload): typed 化判定准则见 design.md::D2` 形式的注释链向后续 change

#### Scenario: 前端 SessionDetail TS interface 同步 typed

- **WHEN** 前端 `ui/src/lib/api.ts` 中定义 `SessionDetail` interface
- **THEN** `metrics` / `metadata` / `contextInjections` / `injectionsByPhase` 四个字段 SHALL **不**是 `Record<string, unknown>` / `unknown[]`
- **AND** 上述字段 SHALL 引用与 Rust 端 `SessionDetailMetrics` / `SessionDetailMetadata` / `ContextInjection` / `Record<string, ContextInjection[]>` 镜像的 typed TS interface
- **AND** `pnpm --dir ui run check`（svelte-check）SHALL 在引入本 typed 后通过

### Requirement: SessionDetailMetrics 与 SessionDetailMetadata 字段定义 SHALL 与历史 snake_case wire 逐字段对齐

新增 typed struct `SessionDetailMetrics` SHALL 含 `message_count: usize` 单字段（serde **snake_case** rename，与 `local.rs:3243` 历史 hand-built `json!({"message_count": ...})` wire 一致）；`SessionDetailMetadata` SHALL 含 `last_modified: Option<String>` / `size: Option<u64>` / `cwd: Option<String>` 三字段（serde **snake_case** rename，与 `local.rs:3244-3247` 历史 wire 一致，全部 nullable）。两个 struct 序列化产物 SHALL 与 `crates/cdt-api/src/ipc/local.rs` 历史 hand-built JSON 在所有可能输入下逐字段一致——typed 化 SHALL **不**修正 camelCase IPC 契约违规（详 `design.md::D7`，留 followup issue）。

#### Scenario: SessionDetailMetrics 序列化 wire 形状

- **WHEN** 实现按 `serde_json::to_value(&SessionDetailMetrics { message_count: 42 })?` 序列化
- **THEN** 产物 SHALL 是 `{"message_count": 42}`（snake_case，**不**是 `{"messageCount": 42}`）
- **AND** 与 `local.rs:3243` 历史 `json!({"message_count": 42})` 形状逐字节一致

#### Scenario: SessionDetailMetadata 字段全 nullable + snake_case wire

- **WHEN** 文件系统 `metadata()` 调用失败 / jsonl 中 `cwd` 字段缺失
- **THEN** `SessionDetailMetadata { last_modified: None, size: None, cwd: None }` 序列化 SHALL 产出 `{"last_modified": null, "size": null, "cwd": null}`（snake_case，**不**是 `lastModified`）
- **AND** 与 `local.rs:3244-3247` 历史 `json!({"last_modified": null, "size": null, "cwd": null})` 形状逐字节一致
- **AND** 前端 `SessionDetail.svelte:856` 按 `detail.metadata.cwd` 消费 SHALL 与改动前行为一致（其余 `last_modified` / `size` 当前前端未消费但 wire 形状仍 SHALL 保留以兼容 HTTP transport / 未来 consumer）

### Requirement: ipc_contract 测试 SHALL 覆盖 typed 字段命名 round-trip

`crates/cdt-api/tests/ipc_contract.rs` SHALL 在本 change 后含至少一个新测试（例如 `session_detail_typed_metrics_metadata_round_trip`）覆盖 `SessionDetail` typed 化后的 wire 形状：从 typed struct 出发 `serde_json::to_value` 再 `serde_json::from_value::<SessionDetail>` 反序列化回 typed，断言所有字段值不变。

#### Scenario: SessionDetail typed round-trip

- **WHEN** 测试构造 `SessionDetail { chunks: Vec::new(), metrics: SessionDetailMetrics { message_count: 0 }, metadata: SessionDetailMetadata::default(), context_injections: Vec::new(), injections_by_phase: BTreeMap::new(), phase_info: ContextPhaseInfo::default(), is_ongoing: false, title: None, session_id: "s".into(), project_id: "p".into() }`，序列化为 `Value`，再反序列化回 typed
- **THEN** 反序列化产物 SHALL 与原始 `SessionDetail` 字段逐一相等（`PartialEq`）
- **AND** 序列化产物的顶层 key 集合 SHALL 是 `{sessionId, projectId, chunks, metrics, metadata, contextInjections, injectionsByPhase, phaseInfo, isOngoing, title}`（顶层 SessionDetail 是 camelCase）
- **AND** `metrics` / `metadata` 内部字段 SHALL 仍是 snake_case（`message_count` / `last_modified` / `size` / `cwd`），与 `local.rs:3243-3247` 历史 hand-built wire 一致（详 `design.md::D5` + `D7`）
