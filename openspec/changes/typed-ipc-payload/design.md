## Context

`crates/cdt-api/src/ipc/types.rs::SessionDetail`（141-178 行）当前 6 个高频字段以 `serde_json::Value` 持有，是历史"先把数据塞进去，spec 后补"的产物。`DataApi` trait（577 行 / 53 method）中 18 个 method 返回 `Result<serde_json::Value, ApiError>` 同源。`spec.md::ipc-data-api` 的约束已按 typed 字段名（`SessionDetail.chunks` / `AIChunk.subagents[i].messages` / `messagesOmitted`）写就，但实现层 `Value` 让序列化形状只能靠 `tests/ipc_contract.rs` 50 个测试事后断言——属于 fidelity gap。

现状关键点：
- `cdt-core::Chunk` / `cdt_core::ContextInjection` / `cdt_core::ContextPhaseInfo` 已实现 `Serialize + Deserialize`，wire 形状已被 `OMIT_*` payload 瘦身路径冻结
- `local.rs:3239-3254` 构造 SessionDetail 时调 `serde_json::to_value(&typed)` 把 typed 转 Value 再塞回 Value 字段——全程 Value 持有等于让 typed 信息在 trait 边界处丢失
- `local.rs` 的 `metrics` / `metadata` 是 hand-built `json!({"message_count": N})` / `json!({"last_modified":..,"size":..,"cwd":..})`，没有对应的 typed struct
- `ui/src/lib/api.ts::SessionDetail` 前端 interface 的 `metrics` / `metadata` / `contextInjections` / `injectionsByPhase` 都是 `Record<string, unknown>` / `unknown[]`，消费侧（`ContextPanel.svelte` / `SubagentCard.svelte`）按字段访问时缺编译期保护
- `Arc<dyn DataApi>` 仅 6 处用法，全集中在 HTTP test 文件——boundary-1 trait object 多态需求弱

约束：
- wire JSON byte-for-byte **不**得变（`OMIT_*` 路径冻结、`ipc_contract.rs` round-trip、前端按 camelCase 字段名读取）
- 不引入新依赖
- typed 化 SHALL 给编译器接管字段契约；transport（IPC + HTTP route）继续 transparent

## Goals / Non-Goals

**Goals:**

1. `SessionDetail` 6 个 `Value` 字段全部改 typed，且 wire JSON 形状不变（`ipc_contract.rs` 现有断言全部保持绿）
2. 选 5 个高频 `DataApi` 方法（`search` / `get_config` / `update_config` / `get_subagent_trace` / `get_notifications`）的 `Result<Value>` 改成 typed `Result<XxxResponse>`
3. 新建 `SessionMetrics` / `SessionMetadata` 两个 struct 取代 hand-built JSON
4. 前端 `ui/src/lib/api.ts` 的 `SessionDetail` interface 4 个弱类型字段同步 typed 化，消费侧（`ContextPanel.svelte` / 等）编译期校验通过
5. 给 `ipc-data-api` spec 加一条 implementation-binding requirement，把"实现 SHALL typed 而非 Value"写进契约

**Non-Goals:**

1. **不**拆 `DataApi` trait 成多个 sub-trait（boundary-1）—— 见 `D3`
2. **不**改 18 个 Value 方法中其余 13 个（SSH 子集 / 文件路径子集 / Trigger CRUD / `validate_path`）—— 见 `D2`
3. **不**改 wire JSON 形状任何字段名 / camelCase / enum tag（`Chunk.kind` / `ContextInjection.category`）/ `xxxOmitted` 标记
4. **不**改 HTTP route 行为、SSE event payload、`OMIT_*` 瘦身策略
5. **不**改前端 IPC 调用站点签名（`invoke<SessionDetail>("get_session_detail", ...)` 函数签名不变）
6. **不**重构 `LocalDataApi::get_session_detail` 内部组装流程（仅替换 typed wrap 点）
7. **不**做 perf 改进（typed 化是字段契约固化，序列化路径与 Value 等价）

## Decisions

### D1：选 typed struct 而非全 trait 拆分

**决策**：把 `SessionDetail` 字段类型从 `Value` 改成 typed Rust struct（用已有 `cdt-core` 类型 + 新建两个轻量 wrapper）；**不**新建 `XxxResponse` newtype 包一层。

**理由**：

- `cdt-core::Chunk` / `cdt_core::ContextInjection` / `cdt_core::ContextPhaseInfo` 已是稳定 typed，wire 形状已被瘦身路径冻结——直接用就好
- newtype wrapper（`SessionDetailChunks(Vec<Chunk>)`）只增 boilerplate 不增类型安全，与 `serde(transparent)` 等价
- `metrics` / `metadata` 没有现成 typed，新建专用 struct 反映 wire 真实形状（不复用 `cdt_core` 内部领域模型，避免领域模型变化触动 wire）

**Alternatives considered**:

- **A. 把 `DataApi` trait 整体拆 sub-trait**（`SessionApi` / `ConfigApi` / `SshApi` ...）：见 `D3`，否决
- **B. 全部 18 个 Value method 一次性改 typed**：见 `D2`，scope 太大且 14 个低频方法 typed 收益边际递减
- **C. 用 `#[serde(flatten)]` 把 `SessionDetail` 结构展平**：会破坏 `xxxOmitted` 同级字段的语义，否决

### D2：14 个 Value method 暂留的判定准则

**决策**：本次只改 5 个高频 method 为 typed；其余 13 个暂留 `Result<Value>`，每个加 `// TODO(typed-ipc-payload): typed 化 issue=<num>` 注释链向后续工作。

**判定准则**（method 该不该 typed 化）：

| 维度 | 留 Value 阈值 | 改 typed 阈值 |
|---|---|---|
| 调用频次（用户可感知路径） | 仅 Settings 配置交互或一次性诊断 | 启动 / 列表渲染 / 详情打开 |
| payload 字段数 | < 3 字段（如 `{ "ok": true }`） | ≥ 3 字段或嵌套结构 |
| 是否已有 typed 源 | 数据来源是 hand-built JSON 且无现成类型 | 来源是 typed struct（`Chunk` / `AppConfig` 等）或可低成本新建 |
| 跨 transport 复用 | 仅 IPC | IPC + HTTP route + SSE 任两个以上 |
| 前端类型化收益 | 消费侧仅读 1-2 个字段 | 多组件 / 多状态机消费 |

按本表得出 5 个 typed 化候选：

| 方法 | 高频路径 | 已有 typed 源 | 跨 transport |
|---|---|---|---|
| `search` | UI search box 高频 | `cdt_core::SearchSessionsResult` 存在 | IPC + HTTP |
| `get_config` | 启动时一次 + Settings UI | `cdt_config::AppConfig` 存在 | IPC + HTTP |
| `update_config` | Settings UI 保存 | 同上 | IPC + HTTP |
| `get_subagent_trace` | SubagentCard 展开 | `Vec<cdt_core::Chunk>` 存在 | IPC + HTTP |
| `get_notifications` | 启动 + 通知面板 | `Vec<NotificationRecord>` 已在 `cdt-config` | IPC + HTTP |

13 个暂留的方法（按 5 个分组）：

- **SSH 子集 7 个**（`ssh_connect` / `ssh_test_connection` / `ssh_get_state` / `ssh_get_config_hosts` / `resolve_ssh_host` / `ssh_save_last_connection` / `ssh_get_last_connection`）：SSH 配置流是低频用户交互；payload 形状与 `cdt-ssh` crate 内部状态强耦合，typed 化需要先稳定 `cdt-ssh` 公共类型边界；本次跳过。
- **文件 / 路径子集 3 个**（`validate_path` / `read_claude_md_files` / `read_mentioned_file` / `read_agent_configs`）：4 个，全是 ad-hoc JSON 反馈结构（成功 + 错误信息组合），消费侧仅按 `if (resp.ok)` 判读；改 typed 收益小且需新建 4 个 struct。
- **Trigger CRUD 2 个**（`add_trigger` / `remove_trigger`）：返回 `{ "ok": bool, "trigger_id"?: ... }` 简单形状，调用频次极低（仅 Settings UI），typed 化 ROI 低。

（注：上文 SSH 7 + 路径 4 + Trigger 2 = 13，与开头"13 个"一致）

后续 issue 跟踪：本 change archive 后 SHALL 在 `openspec/README.md::路线图` 加一条 "Phase 2: 13 个低频 IPC method typed 化"，按本表准则在新 capability 改动触发时机会捎带改。

**Alternatives considered**:

- **A. 全部 18 个一把改**：scope 爆炸（13 个新增 struct + 全部消费侧 TS 类型同步），CR 难审
- **B. 只改 SessionDetail 6 字段，trait 不动**：少做一半价值，且 `get_subagent_trace` / `search` 这种高频路径仍是 Value 等于本次目标没达成

### D3：不拆 `DataApi` sub-trait

**决策**：保持 `DataApi` 单一 trait，53 个 method 全部留下；本次改的 5 个 method 签名变 typed 即可。

**理由**：

1. `Arc<dyn DataApi>` 仅 6 处用法（`crates/cdt-api/tests/http/`）—— trait object 多态需求弱
2. Rust 不支持多 trait object（`dyn SessionApi + ConfigApi`），拆 sub-trait 会逼 supertrait 或 enum dispatch，复杂度爆炸
3. 唯一实现者是 `LocalDataApi`（同 `cdt-api/src/ipc/local.rs`）；拆 trait 增加 boilerplate 无真实并行扩展收益
4. 当前 trait 已按 section comment 自然分组（项目 / 会话 / 搜索 / 配置 / SSH / Trigger / ...），可读性足够

**Alternatives considered**:

- **A. 拆 6 个 sub-trait + supertrait `DataApi: SessionApi + ConfigApi + ...`**：HTTP test 的 `Arc<dyn DataApi>` 仍可用，但 `LocalDataApi` impl 块要拆 6 个；改一个 method 跨 trait 移动；trade-off 不值
- **B. 拆 sub-trait 但保留单一 `DataApi` 作为 facade**：双重维护成本翻倍

### D4：`injections_by_phase` 用 `BTreeMap<String, Vec<...>>` 而非 `HashMap`

**决策**：从 `serde_json::Map<String, ...>`（实际是 `IndexMap`）迁移到 `BTreeMap<String, Vec<ContextInjection>>`。

**理由**：

- JSON object key 序列化顺序：`serde_json::Map` 默认按 `IndexMap` 插入顺序；`BTreeMap` 按字典序；`HashMap` 顺序不稳定
- 前端 `Record<string, unknown[]>` 不依赖 key 顺序（`Object.keys()` 后按 `phase_number` 数值排序消费）
- `BTreeMap` 给出确定字典序，让 `ipc_contract.rs` round-trip 断言稳定（key 顺序 deterministic）
- key 类型保持 `String`（JSON object key 只能是 string）—— `phase_number.to_string()` 与现状一致

**Alternatives considered**:

- **A. `HashMap`**：序列化顺序不稳定，contract test 偶发抖动
- **B. `IndexMap`**：保留插入顺序但需引入新依赖（`indexmap` crate）—— 暂不引入

### D5：`SessionMetrics` / `SessionMetadata` 字段定义（**保 snake_case wire**）

**决策**：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct SessionDetailMetrics {
    pub message_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct SessionDetailMetadata {
    pub last_modified: Option<i64>, // unix epoch ms（apply 阶段从 local.rs 反推真实类型；wire 是 JSON number）
    pub size: Option<u64>,
    pub cwd: Option<String>,
}
```

**理由**（codex propose 二审 P0 修订）：

- `local.rs:3243-3247` hand-built `json!({"message_count": ...})` / `json!({"last_modified": ..., "size": ..., "cwd": ...})` 是 **snake_case** wire，**不**是 camelCase
- 测试 `ipc_contract.rs:1124` 已断言 `message_count`（snake_case wire 历史既定事实）
- typed struct 必须保 snake_case rename 与历史 wire 逐字段对齐，否则 `messageCount` / `lastModified` 是 P0 wire drift（HTTP transport / 浏览器 mode / 任何按 snake_case key 读的 consumer 立刻挂）
- 实际 audit 前端：`detail.metrics.*` 全部未消费（`SessionDetail.svelte:729-731` 是 `chunk.metrics`，跟 `SessionDetail.metrics` 无关）；`detail.metadata` 仅读 `cwd`（`SessionDetail.svelte:856`，单字母词不区分 case）—— 但即使前端不消费，wire 不能漂移（防御 HTTP / 未来 consumer）
- 三个字段全 `Option<>` —— 反映 `local.rs:3247-3252` 真实行为（fs `metadata()` 失败时 `last_modified` / `size` 为 null，jsonl 文件 `cwd` 字段缺失时 `cwd` 为 null）
- `Default` 允许 `ipc_contract.rs` 测试用 `SessionMetadata::default()` 替代 `json!({})` 字面量，构造点最小变动

**Alternatives considered**:

- **A. `#[serde(rename_all = "camelCase")]`**（最初决策）：让 wire 变 `messageCount` / `lastModified` —— P0 wire drift，否决（codex 二审命中）
- **B. 把 `last_modified` 用 `chrono::DateTime<Utc>`**：当前 wire 是 ISO 8601 string，前端按 `new Date(s)` 解析，改成 `DateTime` 需要决定序列化精度 + 时区策略，scope 蔓延
- **C. 把 `cwd` 用 `PathBuf`**：`PathBuf` 序列化在 Windows 下是 `\\` 分隔符 hellscape，避免引入跨平台风险——保持 `String` 与现状

### D7：metrics / metadata 暂不修正 camelCase IPC 契约违规

**决策**：本 change **不**把 `SessionMetrics` / `SessionMetadata` 改成 camelCase 修正历史 IPC 字段名违规；保留 snake_case 与历史 wire 一致。followup issue 单独 PR 修。

**理由**：

- `crates/CLAUDE.md::Serde / IPC 契约` 明确硬约束"面向前端的 struct 必须 `#[serde(rename_all = "camelCase")]`"
- `metrics` / `metadata` 的 hand-built `json!({"message_count": ...})` 等于历史 IPC 字段名违反 camelCase 硬约束
- 但 typed 化的核心目标是"把 Value 静态化为 typed struct，wire 不变"——把它扩成"修历史 wire 不一致"会让 scope 蔓延、codex 二审 / archive / e2e 都更复杂
- 前端实测：`SessionDetail.metrics.*` 完全未消费（`SessionDetail.svelte:729-731` 是 `chunk.metrics` 不同字段）；`SessionDetail.metadata` 仅读 `cwd`（line 856），单字母词不区分大小写 —— 修正 camelCase 在前端层面"零影响"
- 但 HTTP transport / `cdt-cli serve` 浏览器 mode / SSE bridge / 未来潜在 consumer 任何按 snake_case key 读的路径在 camelCase 切换瞬间挂，**风险面 > 修正收益**
- followup issue 候选标题："metrics / metadata IPC wire 字段名修正为 camelCase（修复 IPC 契约违规）"，PR 内容：(a) 改 `#[serde(rename_all = "camelCase")]`；(b) grep 全仓 `message_count` / `last_modified` 等 snake_case key 消费者一一确认；(c) ipc_contract.rs 加 camelCase 断言 + 删 snake_case 断言；(d) 标 BREAKING + minor version bump

**Alternatives considered**:

- **A. 在本 change 内同时修正 camelCase**：scope 蔓延 + codex 二审多一轮 + e2e 需要全平台 smoke，否决
- **B. 永远不修，把 snake_case 视为 metrics/metadata 历史既定 wire**：违反 CLAUDE.md 硬约束，且 IPC 契约一致性长期会失守，否决

### D6：5 个高频 method typed 返回类型选型

| 方法 | 选定类型 | 来源 |
|---|---|---|
| `search` | `cdt_core::SearchSessionsResult` | 已有 typed struct（`crates/cdt-core/src/search.rs:31`，camelCase）—— 详 `D8` |
| `get_config` | `cdt_config::AppConfig` | 已有（`crates/cdt-config/src/types.rs:16`，camelCase）；前端已 `Promise<AppConfig>` typed |
| `update_config` | `cdt_config::AppConfig` | 与 `get_config` 对称——返回保存后的最新值；前端已 `Promise<AppConfig>` typed |
| `get_subagent_trace` | `Vec<cdt_core::Chunk>` | 直接复用 `cdt-core` typed |
| `get_notifications` | `cdt_config::GetNotificationsResult` | 已有（`crates/cdt-config/src/notification_manager.rs:27`，camelCase）；前端已 `Promise<GetNotificationsResult>` typed —— **修订**：apply audit 发现实际类型不是 `Vec<NotificationRecord>`（不存在该类型），是 `GetNotificationsResult { notifications, total, totalCount, unreadCount, hasMore }` envelope |

**Apply audit 结果**（task 1.x 已跑完）：5 个候选类型全部已存在 + camelCase + 前端 typed 镜像。`get_notifications` 类型修订记录在 `tasks.md::A. Audit 结果`。

### D9：`get_sessions_by_ids` not-found fallback 形态（mid-apply 新增决策）

**决策**：not-found fallback path 改用 typed default 值（`chunks: vec![]` / `metrics: SessionMetrics::default()` / `metadata: SessionMetadata::default()` / `phase_info: ContextPhaseInfo::default()` / `context_injections: vec![]` / `injections_by_phase: BTreeMap::new()` / `is_ongoing: false` / `title: None` / `project_id: ""`），**移除** ad-hoc `{"status":"not_found"}` 带外标记。前端按 `result.projectId === ""` 区分 not-found（已有信号）。

**理由**：

- apply 编译期发现 `local.rs:3485-3492` / `3498-3508` 两处 `get_sessions_by_ids` not-found fallback 用 `metadata: json!({"status": "not_found"})` + `chunks: Null` / `metrics: Null` / `phase_info: Null` 的异类 wire 形态——typed 化后这些字段无法用单一 typed 表达
- 前端 grep `metadata.status` / `not_found` 全仓 0 命中——**前端不消费 status 标记**，是 dead wire
- 唯一 contract consumer 是 `tests/http_session_detail_global_lookup.rs:230` 的 `get_sessions_by_ids_handles_mixed_existence` 测试断言；前端实际靠 `result.projectId === ""`（line 228 验证 `projectId == ""`）判断 not-found
- typed default 形态：`{chunks:[], metrics:{message_count:0}, metadata:{last_modified:null,size:null,cwd:null}, contextInjections:[], injectionsByPhase:{}, phaseInfo:{phases:[],compactionCount:0,aiGroupPhaseMap:{},compactionTokenDeltas:{}}, isOngoing:false, title:null, projectId:"", sessionId:"sid-ghost"}` —— 永远"shape 完整可用"，前端无需 null guard 即可访问字段

**Wire 漂移评估**：

- 受影响 API：仅 `get_sessions_by_ids` not-found fallback 路径
- 形状变化：metadata 从 `{"status":"not_found"}` → `{"last_modified":null,"size":null,"cwd":null}`（移除 status 字段，加三个空字段）；chunks/metrics/phaseInfo 从 `null` → 各自 typed default
- 前端影响：前端不读 metadata.status（已 grep 确认 0 命中），不破坏现有读取
- HTTP test：`tests/http_session_detail_global_lookup.rs:230` 断言要改为期待 typed default
- Risk verdict：**dead wire 清理 + 前端 typed safety 提升，可接受**

**Alternatives considered**:

- **A. 保 status 字段**：在 `SessionMetadata` 加 `status: Option<String>` + `skip_serializing_if`——typed struct 字段含义不一致（status 与 last_modified/size/cwd 语义不属于同一类），可读性差，否决
- **B. `metadata: SessionMetadataOrError`** enum 改 untagged：增加前端 typed match 复杂度，得不偿失
- **C. `Option<SessionDetail>` 让 not-found 路径返回 None**：trait 签名变 `Vec<Option<SessionDetail>>`，破坏前端调用站点 + HTTP wire 形态，scope 蔓延
- **D. 保留全部 ad-hoc Value 不动**：违背本 change "typed 化" 主目标，否决

### D8：`search` empty query 短路修法（mid-apply 新增决策）

**决策**：`LocalDataApi::search` 在 `query.is_empty()` 时构造完整 `SearchSessionsResult { results: vec![], total_matches: 0, sessions_searched: 0, query: "".to_string(), is_partial: false }` 返回，**不**保留历史 hand-built 短路 `{"query": "", "results": []}` 缺字段形态。

**理由**：

- apply task 1.3 audit 发现 `LocalDataApi::search` 当前在 `query.is_empty()` 时返回 hand-built `{"query": "", "results": []}` —— 缺 3 个字段（`totalMatches` / `sessionsSearched` / `isPartial`），与 `SearchSessionsResult` typed wire 形状不一致
- typed 化后改 `Result<SearchSessionsResult>` 必须构造完整值，empty query 路径自动产出 `{"results":[],"totalMatches":0,"sessionsSearched":0,"query":"","isPartial":false}`，**wire 形状从 4 字段→7 字段**
- 这是 bug fix：前端 `CommandPalette.svelte:116` 按 `"totalMatches" in session ? session.totalMatches : ...` 判定，缺字段路径走 fallback；新增 3 字段不破坏现有读取，只让 wire 更完整
- 影响面：HTTP transport / SSE bridge / e2e fixture 需重审 empty query 响应形状（无 hidden consumer 按 4 字段 key 集合断言）

**Wire 漂移评估**（codex CR 时会问）：

- 受影响 API：仅 `search` empty query 路径
- 形状变化：`{query, results}` → `{query, results, totalMatches, sessionsSearched, isPartial}`，多 3 字段全是新增（无字段删除/重命名）
- 前端影响：`CommandPalette.svelte:116` "in" 判定 + fallback——empty query 时 results 为 [] 不渲染 entry，新字段当前路径完全不读
- HTTP / e2e：tests/ipc_contract.rs 现有测试无 empty query 短路断言（baseline 115 通过），新增字段不破坏既有断言
- Risk verdict：**bug fix，可接受**

**Alternatives considered**:

- **A. 保留缺字段 wire 形状**：在 typed 层加 `Option<Vec<...>>` / `#[serde(skip_serializing_if = "...")]` 让 empty query 输出 `{query:"", results:[]}`——会让 typed struct 字段语义不一致（明明定义为 `usize` 却序列化时跳过），否决
- **B. empty query 直接报 `ApiError::validation`**：与现有 silent fallback 行为不一致，可能破坏前端"输入框为空时不报错"用户体验，否决

## Risks / Trade-offs

[**Risk**: `metrics` / `metadata` 新建 struct 时 serde 字段名与历史 wire 形状漂移] → **Mitigation**：apply 第 1 步先 grep `local.rs` + `ipc_contract.rs` + 前端 `ContextPanel.svelte` 等位置实际读到的字段名，逐字段对照 D5 定义；构造完后跑 `cargo test -p cdt-api --test ipc_contract` 校验 round-trip；新增专门 round-trip 测试覆盖 `messageCount` / `lastModified` / `size` / `cwd` 四个 wire key 名

[**Risk**: `Chunk` / `ContextInjection` 是 internally-tagged enum（`#[serde(tag = "kind")]` / `tag = "category"`），改 typed 时若误加 `rename_all_fields` 或漏 tag 配置会破坏 wire 形状] → **Mitigation**：本 change **不**改 `cdt-core` 内任何 enum 配置；仅改 `cdt-api` 的 SessionDetail 字段类型；`ipc_contract.rs` 增加 `kind: "user" / "ai" / "system" / "compact"` 字面值断言保护

[**Risk**: 50 个 `ipc_contract.rs` 测试中 ~5 个直接用 `SessionDetail { chunks: json!([]), metrics: json!({}) }` 字面量构造，typed 化后构造代码膨胀] → **Mitigation**：把字面量构造改成 `SessionDetail { chunks: Vec::new(), metrics: SessionMetrics { message_count: 0 }, metadata: SessionMetadata::default(), context_injections: Vec::new(), injections_by_phase: BTreeMap::new(), phase_info: ContextPhaseInfo::default(), ..base }`；如 `ContextPhaseInfo` 没 `Default`，本 change 加 `#[derive(Default)]`（前提：phase_info 当前 Value::Object({}) 等价 Default）

[**Risk**: `ContextPhaseInfo::default()` 序列化后形状与历史 `json!({})` / 真实非空形状不一致] → **Mitigation**：grep `cdt-core::ContextPhaseInfo` 确认字段是否全 `Option<>` / 有 `Default`；如非 default-friendly，测试构造点用真实 fixture（不是 `json!({})`）—— 先保持现状，apply 第 1 步先 audit

[**Risk**: 前端 `ContextPanel.svelte` 等消费侧把 `Record<string, unknown>` 改成 typed interface 时，编译期发现额外字段访问位置，需逐处修] → **Mitigation**：前端改完跑 `pnpm --dir ui run check`（svelte-check）+ `pnpm --dir ui run test`（vitest）；Playwright e2e 至少跑 SessionDetail 打开 + ContextPanel 渲染场景

[**Risk**: `update_config` 返回 typed `AppConfig` 后，IPC contract test 若先前断言 wire 形状的 key 名（如 `theme` / `notifications`）正常，但 `AppConfig` 字段是否 camelCase？需逐字段确认] → **Mitigation**：apply 前先跑 `cargo test -p cdt-api --test ipc_contract -- --nocapture` 看现有断言；如 `AppConfig` 非 camelCase，本 change SHALL **不**接 `update_config` 进 typed 列表（改进 Phase 2）

[**Risk**: HTTP route 内部 `axum::Json(value)` 转发 `DataApi::xxx().await?` 返回值，改 typed 后类型推断可能要显式 turbofish] → **Mitigation**：`axum::Json<T: Serialize>` 已支持任何 typed；compile error 处显式 `Json::<TypedReturn>(...)` 即可

**Trade-off：编译时间 vs 字段契约**

typed 化让 trait method 签名变重，可能轻微影响 `cdt-api` 编译时间。本次只动 5 个 method，影响估计 < 5%；权衡下编译期字段保护远胜过 Value 失保护带来的 contract test 维护成本。

## Migration Plan

1. **现状 audit**（apply task 1）：grep `cdt-core` / `cdt-config` 确认 5 个 method typed 返回类型存在 + Serialize 实现；grep `local.rs` 确认 `metrics` / `metadata` hand-built JSON 字段名
2. **新建 struct**（apply task 2）：`SessionMetrics` / `SessionMetadata` 写入 `crates/cdt-api/src/ipc/types.rs`；如缺失 typed 类型在本 step 一并新建
3. **改 SessionDetail 字段**（apply task 3）：6 个 `Value` 字段改 typed；`local.rs:3239-3254` 构造点同步改 typed 直接构造（不再 `serde_json::to_value`）
4. **改 5 个 trait method 签名**（apply task 4）：`DataApi` trait + `LocalDataApi` impl 同步改；HTTP route handler 跟改
5. **改 `ipc_contract.rs`**（apply task 5）：~5 处 SessionDetail 构造点改 typed；新增 round-trip 断言保护新 wire key
6. **跑 cargo test**（apply task 6）：`cargo test -p cdt-api`；红了修
7. **改前端 typed**（apply task 7）：`ui/src/lib/api.ts` 4 个弱类型字段改 typed；`ContextPanel.svelte` 等消费侧编译错误处修
8. **跑 svelte-check + vitest**（apply task 8）：`pnpm --dir ui run check` + `pnpm --dir ui run test`
9. **手动 smoke**（apply task 9）：`just dev` 启动桌面端，打开大会话，检查 SessionDetail 渲染 / ContextPanel / SubagentCard 行为不变
10. **写 spec delta**（已在 specs/ 目录预先生成，apply 仅校验）

**Rollback strategy**：本 change 是单 PR 原子改动。如 archive 后发现 wire 形状漂移：

- `git revert <archive-commit>` 恢复 active state
- `git revert <implementation-commit>` 恢复实现
- 重跑 `ipc_contract.rs` 确认 wire 形状回到旧版

## Open Questions

1. `cdt-config::NotificationRecord` 是否已存在 + camelCase？apply 第 1 步 grep 确认；如缺失则本 change 新建（仍走单 PR）。
2. `cdt-core::SearchSessionsResult` wire 形状是否与 `LocalDataApi::search` 返回的 hand-built JSON 完全一致？apply 第 1 步对照 `local.rs::search` 实现 + 前端 `ui/src/lib/api.ts` 当前 typed `SearchResult` 类型；不一致就在本 change 中调整（优先改 `cdt-api` 内组装代码与 `cdt-core` typed 对齐，**不**改 `cdt-core` 公共类型）。
3. `update_config` 返回 typed `AppConfig` —— 需要前端确认是否依赖原 Value 形状。apply 前 grep `ui/` 中 `update_config` 调用站点；前端如按 `Record<string, unknown>` 处理则可平滑切；如有 hand-typed 强类型则同步对齐。

（以上 3 项均不阻塞 propose；apply 第 1 步 audit 后会变成 task 内具体行动，必要时 mid-apply 在 design.md 加 D7 / D8 修订决策。）
