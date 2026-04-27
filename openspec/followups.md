# Baseline Cross-Check Findings

本文件汇总 baseline spec 与当前 TS 实现对齐过程中发现的偏差、疑似 bug、spec 未覆盖的真实行为，以及无法写入 spec 的 UI 隐式契约。供 Rust 重写时决策"复刻 vs 修正 vs 补 spec"。

图例：
- **spec-gap**：spec 描述不准确或缺失，应在 baseline 归档前或 Rust port 前更新 spec
- **impl-bug?**：疑似实现 bug，Rust port 时应修正而非复刻
- **coverage-gap**：spec 未覆盖但实现里有真实行为，需要补 scenario 或拆新 capability
- **implicit**：无法写进 baseline 的隐式契约（UI 交互、状态动画、键盘绑定等）

---

## session-parsing

### [impl-bug?] requestId 去重函数存在但未被调用 ✅ 已在 `port-session-parsing` 修正
- Spec: `Deduplicate streaming entries by requestId` requirement
- 代码：`src/main/utils/jsonl.ts` 定义了 `deduplicateByRequestId`，但 `src/main/services/parsing/SessionParser.ts:77` 附近的 `processMessages()` 未调用它
- 现状：流式 rewrite 场景下可能计入多条同 `requestId` 的 assistant 消息
- Rust port 决策：实现去重（按 spec），不复刻这个 miss
- **Rust 实现**：`crates/cdt-parse/src/dedupe.rs::dedupe_by_request_id` 由 `parse_file` 在收集完所有 `ParsedMessage` 后自动调用；`crates/cdt-parse/tests/dedupe.rs::parse_file_invokes_dedup_automatically` 是 wire-in 回归测试。

### [coverage-gap] 缺 JSONL 解析恶意输入的测试 ✅ 已在 `port-session-parsing` 补齐
- `test/main/services/parsing/` 没有对单行 malformed JSON 的用例
- Rust port 时应配套加 scenario-level test
- **Rust 实现**：`crates/cdt-parse/tests/parse_file.rs::{malformed_line_in_middle_is_skipped, two_adjacent_malformed_lines_both_skipped, empty_file_returns_empty_vec}` 覆盖全部三种异常路径；malformed 行通过 `tracing::warn!` 报告并跳过。

---

## chunk-building

### [impl-bug?] Task tool 过滤未在 AIChunk 构建阶段生效 ✅ 已在 Rust port 修复
- Spec: `Filter Task tool uses when subagent data is available`
- 代码：`ToolExecutionBuilder` 构建所有 tool execution，随后的 `ChunkFactory.buildAIChunkFromBuffer` 未在 subagent 已 resolve 的情况下移除对应 Task tool_use
- 可能结果：UI 里同一个 Task 既作为工具项展示，也作为 subagent 展示
- Rust port 决策：按 spec 过滤
- **Rust 实现**：后端 `build_chunks_with_subagents` 已调用 `filter_resolved_tasks`；前端新增 `displayItemBuilder.ts` 的 `buildDisplayItems` 从 `semanticSteps` 构建统一 `DisplayItem[]`，header summary 由 `buildSummary(items)` 统计（对齐原版 `displaySummary.ts`），subagent 用独立 `SubagentCard` 组件渲染。

### [coverage-gap] 多 tool 链接 / orphan tool_result / Task 过滤没有测试
- `test/main/services/analysis/ChunkBuilder.test.ts` 只覆盖基础 chunk 创建和 sidechain 过滤
- 补 vitest + Rust 实现 scenario

### [implicit] SemanticStepGrouper 的分组粒度未进 spec
- `SemanticStepExtractor` 提取 thinking/text/tool/subagent 步骤；`SemanticStepGrouper` 把相邻同类步骤合并展示
- baseline 只冻结"按时间顺序提取"，没有冻结合并策略（实现细节可能演进）
- Rust port 决策：自行设计分组策略，不约束

---

## tool-execution-linking

### [spec-gap] 重复 tool_use_id 的处理没被实现
- Spec 写了"WHEN 两个 tool_result 共享同一 id THEN 记录第一个并 log warning"
- 代码：`ToolExecutionBuilder` 没有 duplicate-id 检测与告警分支
- 决策：要么删掉 spec 的该 scenario（未实现），要么 Rust port 补实现。倾向 **保留 spec + Rust 补实现**（正确行为）

### [spec-gap] SendMessage summary 格式细节与实现不一致
- Spec: "SendMessage 摘要应含 recipient 和 truncated message preview"
- 代码 `src/renderer/utils/toolRendering/toolSummaryHelpers.ts:237` 使用 `type` 与 `to` 字段，不一定包含正文 preview
- 决策：Rust port 时按 spec 写；baseline spec 可保留不动

### [coverage-gap] Task→subagent 的三阶段匹配（result-based / description-based / positional）没写进 spec
- `SubagentResolver.ts:207-309` 实现了三级 fallback 匹配
- spec 只说"match task descriptions and spawn timestamps"过于笼统
- 建议：归档前给 `tool-execution-linking` 补一条 `Match Task calls to subagents by three fallbacks` 的 requirement

### [impl-bug?] Phase 1 subagent 匹配路径错：应读 JSONL 顶层 `toolUseResult.agentId` 而非 content block text ✅ 已在 2026-04-16 subagent 三连修复中修正
- **背景**：2026-04-16 `fix-ai-header-tool-count` 修复过程中定位到此问题，已部分缓解（添加了从 content block 文本抽取 `agentId:` 的兜底），但根本问题未修。
- **原版实现**（`../claude-devtools/src/main/services/analysis/SubagentResolver.ts`）：从 JSONL 条目的**顶层** `toolUseResult.agentId` 字段直接读取 subagent session id。此字段是 Claude Code 写 JSONL 时独立于 `message.content` 输出的结构化结果。
- **Rust 现状**（`crates/cdt-analyze/src/tool_linking/resolver.rs::extract_session_id`）：只看 `ToolExecution.output`，该字段来自 content block 里的 `tool_result`。当前从 text 里 regex 抽 `agentId:` 仅是兜底。
- **影响**：14 个 Task/Agent tool_use 中只有 4 个成功匹配到 subagent（`chunk[1]` ×2 / `chunk[15]` / `chunk[25]`），其余 10 个因 content block 中不含 agentId 文本而 fallback 到 description/positional 匹配失败。
- **修复路径**：
  1. `cdt-parse` 需在 `ParsedMessage` 上保留 JSONL 顶层 `toolUseResult` 字段（当前被丢弃）
  2. `cdt-analyze` 的 `ToolExecution` 增加 `result_agent_id: Option<String>` 字段
  3. `extract_session_id` Phase 1 优先读该字段
- **风险**：`cdt-parse` 结构体扩展会触发 serde 兼容性检查；需同步更新 `session-parsing` spec。
- **Rust 实现**：`ParsedMessage.tool_use_result: Option<Value>` 在 `cdt-parse/src/parser.rs` 通过 `#[serde(rename = "toolUseResult")]` 保留；`ToolExecution.result_agent_id: Option<String>` 在 `cdt-analyze/src/tool_linking/pair.rs` 从 user 消息顶层 `toolUseResult.agentId` 抽取；`resolver.rs::extract_session_id(exec)` 优先读 `result_agent_id`；新增单测 `phase1_prefers_result_agent_id_over_output`。

### [impl-bug?] `dedupe_by_request_id` 丢弃含 Agent tool_use 的 assistant 消息 ✅ 已在 `align-subagent-ui-with-original` 修正
- **实际根因**（2026-04-17 定位）：Claude Code 新 JSONL 格式里，同 `requestId` 表示"同一次 API response 的 grouping key"——一次响应的多个 content block（`thinking` / `text` / 每个独立 `tool_use`）被写成多条独立 assistant 记录共享同一 `requestId`。盲目 dedupe 会把含不同 `tool_use` 的记录误判为 streaming rewrite 而丢弃。
- **验证数据**：session `46a25772-b57c-43bb-9ca6-f0292f9ca912` 下 `requestId=req_011Ca5q9ggoStFzstiaLR5Y1` 有 4 条记录（1 thinking + 1 text + 2 独立 tool_use），dedupe 后仅剩 1 条，丢失 2 个 Agent 调用。
- **修复**：移除 `parse_file` 主路径上的 `dedupe_by_request_id` 调用；`dedupe_by_request_id` 函数保留在 `crates/cdt-parse/src/dedupe.rs` 供 metrics 计算按需使用。session-parsing spec 的 "Deduplicate streaming entries by requestId" requirement 已反转：SHALL NOT 在 parse_file 自动去重。回归测试 `crates/cdt-parse/tests/dedupe.rs::parse_file_does_not_dedupe_by_request_id`。

### [coverage-gap] Rust 匹配 `Task || Agent` 两个工具名，原版只匹配 `Task`
- `resolver.rs` 的 Task filter 包含 `name == "Task" || name == "Agent"`
- 原版只有 `name === "Task"`
- **原因**：Claude Code 新版本把 Task 工具改名为 Agent，Rust port 做了兼容；非 bug，但需在 spec 里显式声明"Task/Agent 同义词"。

---

## team-coordination-metadata

### [spec-gap] teammate vs subagent 分开计数不在实现里
- Spec: "count distinct teammates separately from regular subagents"
- 代码：`SubagentResolver` 把 team 信息塞进 `Process.team`，但没有独立的 teammate 计数 API
- 决策：倾向 **修改 spec**，把该 scenario 改写为"能从 Process.team 区分 teammate 与普通 subagent，调用方自行计数"

### [coverage-gap] 缺 teammate detection / team enrichment 测试
- 现有测试没有覆盖 `isParsedTeammateMessage` 分支与 `Process.team` 富化链路
- Rust port 时应补

---

## project-discovery

### [spec-gap] 路径解码"最接近的存在路径"歧义消解没实现 ✅ 已在 `port-project-discovery` 修正
- Spec: `Path containing legitimate hyphens` → "resolving to the closest existing filesystem path when ambiguous"
- 代码：`src/main/utils/pathDecoder.ts:40-64` 是 best-effort 简单替换，注释明确说不能歧义消解；歧义靠 `ProjectPathResolver.ts:76-86` 通过读 JSONL 里的 `cwd` 补救
- 决策：**改 spec**，把机制写清楚：解码是 best-effort；真实路径由 session 文件中的 cwd 字段最终确定
- **Rust 实现**：`crates/cdt-discover/src/path_decoder.rs::decode_path` 保持 best-effort；`crates/cdt-discover/src/project_path_resolver.rs::ProjectPathResolver::resolve` 的解析顺序为 composite registry → cache → 绝对路径 hint → `read_lines_head` 抽 session `cwd` 字段 → `decode_path` fallback。集成测试 `cwd_field_overrides_decode` / `decode_path_fallback_used_when_no_cwd_in_sessions` 覆盖两条主路径。同时 port 在 `FileSystemProvider` 上新增 `read_lines_head`，修正 TS 侧 SSH 模式必须拉完整 JSONL 的隐性性能 bug。

---

## configuration-management

### [impl-bug?] 损坏 config 不会自动备份 ✅ 已在 `port-configuration-management` 修复
- Spec: "back up the corrupted file, load defaults, log the error, and continue"
- 代码：`ConfigManager.ts:379-396 loadConfig()` 只 log + 加载默认，没有备份
- 决策：Rust port 已按 spec 实现备份行为（`cdt-config::manager::ConfigManager::backup_corrupted_file`）

---

## context-tracking

### [spec-gap] Compaction 边界检测机制描述与实现不一致（行为一致）
- Spec 说"检测 compact summary messages"
- 代码：`contextTracker.ts:998` 通过 display item `type === 'compact'` 检测
- 两者行为等价（CompactChunk 总是对应 compact summary message），但机制描述需要对齐
- 决策：**微调 spec 措辞**为"context phase boundaries derived from compact items / compact summary messages"

### [spec-gap] notification-triggers spec 里的 `is_error` 检测路径可能偏离实现 ✅ 已在 `port-notification-triggers` 确认并实现
- Spec: "detect by inspecting tool_result for is_error=true"
- 代码：TS `ErrorTriggerChecker.ts:170` 的 `requireError` 分支实际检查了 `result.isError`，行为与 spec 一致
- Rust port 已实现 `is_error` flag 检查（`error_trigger_checker.rs` `check_tool_result_trigger`）

### [coverage-gap] computeContextStats / processSessionContextWithPhases 无单元测试 ✅ 已在 `port-context-tracking` 补齐
- `test/renderer/utils/` 下只有 `claudeMdTracker.test.ts`
- Rust port 时应补这两个核心函数的测试
- **Rust 实现**：`crates/cdt-analyze/src/context/stats.rs::compute_context_stats`（3 单测覆盖 empty / 聚合 / CLAUDE.md 去重）+ `session.rs::process_session_context_with_phases`（入口）；`crates/cdt-analyze/tests/context_tracking.rs` 7 个集成测试覆盖 spec 5 条 Requirement 与本 port 新增的 ADDED / MODIFIED scenario（empty slice、compaction delta `{pre=1000, post=600, delta=-400}`、末尾 compact 不产生 delta、路径去重、missing token fallback、camelCase JSON shape）。

---

## ipc-data-api

Spec 覆盖了 9 大操作集合，但 preload 真实暴露的 API 超出 spec 列表。**spec 未覆盖的真实 API**：

- `readAgentConfigs`（`src/preload/index.ts:180`）
- `getSessionsByIds`（`:157`）
- `getSessionGroups`（`:155`）
- `getRepositoryGroups` / `getWorktreeSessions`（`:161-163`）
- `readClaudeMdFiles` / `readDirectoryClaudeMd` / `readMentionedFile`（`:172-177`）
- `session.scrollToLine`（`:327`，UI 定位 deep link）

决策：**归档前给 ipc-data-api 补一条 requirement**，列出这些 API 的用途，或者显式把 CLAUDE.md 相关操作从 `configuration-management` 中移过来。`session.scrollToLine` 是 UI 定位，属于 UI 层隐式契约 → 放 implicit 区。

---

## http-data-api

### [spec-gap] 路由前缀与错误码全部与实现偏差
- Spec 示例用 `GET /projects`、`POST /search/sessions`；实现用 `/api/projects`、`/api/projects/:projectId/sessions-paginated` 等 `/api/*` 前缀
- Spec 约定 400/404/409/500；实现大量返回空数组/空对象/null，没有显式 HTTP 状态码区分
- 决策：
  1. **改 spec**：把前缀写成 `/api`，把路由形态贴近实现
  2. **Rust port 时修正错误处理**：按 spec 的 status code 约定实现

### [coverage-gap] 实现里存在但 spec 没列的路由
- `src/main/http/utility.ts`、`validation.ts` 等 12 个路由文件全部覆盖到，spec 只点名了一半
- 建议：归档前为 http-data-api 补一个"完整路由清单"附录，或拆出 `http-routes` 能力

### [coverage-gap] http session detail 集成测试缺 CLAUDE.md 真实场景

- 来源：codex 二审 PR #33（2026-04-28）提到 `tests/http_session_detail_global_lookup.rs` 现有 fixture 无 `CLAUDE.md` 文件，触发不到 `build_claude_md_from_filesystem` + `process_session_context_with_phases` 的真实 `context_injections` 数据
- 现状：测试只断言 `context_injections.is_array()`（无 CLAUDE.md 为空数组）+ chunks 数量；不能证明含 inj 时形态正确
- 落地建议：tmpdir 写最小 `CLAUDE.md`（global / project / project-local 任一），跑 `get_session_detail` 后断言 `context_injections` 含 `category: "claude-md"` 条目；代码无需改动，仅扩测试 fixture
- 影响面：`process_session_context_with_phases` 路径已有 `cdt-analyze` 单测覆盖；本 followup 是 IPC 端到端形态加固，非阻塞

### [coverage-gap] SSE 增量补全 ssh-status / updater 事件源

- 来源：change `fix-http-session-detail-and-event-bridge`（2026-04-28 落地 file/todo/notification 三类 SSE producer 后的遗留）
- 现状：`PushEvent` enum 已有 `SshStatusChange { context_id, state }` / 客户端期望的 `updater` 事件 variant；`spawn_event_bridge` 只接通 `FileChangeEvent` / `TodoChangeEvent` / `DetectedError` 三个 producer
- 缺口：(a) `cdt-ssh::SshConnectionManager` 未暴露 `broadcast::Sender<SshStatusChange>`；(b) updater 事件目前仅 Tauri host 走 `app.emit("updater://available")` 推送，没有可订阅的 broadcast 源
- 落地建议：
  1. `cdt-ssh` 在 `SshConnectionManager` 加 `pub fn subscribe_status_changes() -> broadcast::Receiver<SshStatusChange>`，connect / disconnect / 心跳失败时 broadcast；同步给 `cdt-api::http::spawn_event_bridge` 加第 4 个 producer
  2. updater：在 `cdt-api` 内为 `check_for_update` / 启动时静默检查路径加 `broadcast::Sender<UpdaterEvent>`，Tauri host 与 SSE bridge 共享同一订阅源
  3. spec `http-data-api` §"Push events via Server-Sent Events" 在两条 producer 接通后把 SSH / updater 移出"暂不强制"段
- 影响面：headless / 浏览器客户端目前收不到 SSH 状态与升级进度；Tauri 客户端走前端 emit 路径不受影响

---

## file-watching

✅ 完全匹配：100ms 去抖常量 `FileWatcher.ts:35 DEBOUNCE_MS = 100`，事件 payload 字段对齐，多订阅者分发 OK。无 followup。

---

## session-search

✅ 行为全对：scope、case-insensitive、noise 排除、cache by mtime。

### [coverage-gap] SSH stage-limit 快速搜索未进 spec
- `SessionSearcher.ts:29-31 SSH_FAST_SEARCH_STAGE_LIMITS` 在 SSH 模式下限制扫描阶段
- 决策：Rust port 时保留，spec 归档前加一条"SSH 模式下支持分阶段限制以避免长延迟"

---

## ssh-remote-context

✅ 完全匹配：`LocalFileSystemProvider` / `SshFileSystemProvider` 都实现同一 `FileSystemProvider` 接口；`ServiceContextRegistry.switch()` 支持切换；状态枚举齐全。无 followup。

---

## notification-triggers

见 context-tracking 区块下 `is_error` 那条；其它条目与实现匹配。

---

## notification-triggers pipeline（UI 已知遗留的首项）

### [coverage-gap] 后台自动通知管线缺失 ✅ 已在 `2026-04-17-auto-notification-pipeline` 修复
- 原 CLAUDE.md 的 "UI 已知遗留问题" 第一条：`cdt-watch` 可监听文件变更，但尚未接入 trigger 匹配→自动创建通知的扫描管道。
- Rust 实现：新增 `cdt-api::notifier::NotificationPipeline`，订阅 `FileWatcher::subscribe_files()`，对每个 `FileChangeEvent` 全量 `parse_file` → `detect_errors` → `NotificationManager::add_notification` → `broadcast::Sender<DetectedError>`；`src-tauri` 在 `tauri::Builder::setup` 里 spawn watcher + bridge 到前端 `notification-added` 事件。
- 去重：`DetectedError.id` 改为 SHA-256(`session_id|file_path|line_number|tool_use_id|trigger_id|message`) 前 16 字节 hex（确定性），`NotificationManager::add_notification` 返回 `Result<bool>`，重复 id 不写入不 broadcast。
- 覆盖：`crates/cdt-api/tests/notifier_pipeline.rs::pipeline_emits_detected_error_on_new_jsonl_line` 端到端集成测试（真实 FileWatcher + tmp 目录 + 写 JSONL + subscribe）。

---

## 实时会话刷新（UI 已知遗留）

### [coverage-gap] `file-change` 事件未桥到前端，打开的会话不会自动刷新 ✅ 已在 `2026-04-18-realtime-session-refresh` 修复
- **原版行为**（`src/main/index.ts:127-135` + `src/renderer/store/index.ts:230-275`）：
  - `FileWatcher.on('file-change')` → `mainWindow.webContents.send('file-change', event)`
  - renderer store 订阅，命中当前打开的 session → 重拉 `getSessionDetail` 并替换 store；命中当前 project 的新 session → 刷新 sidebar session list
  - `sessionDetailSlice` 做 in-flight dedupe（多个 file-change 合并成一次 refresh）
- **Rust 实现**：
  - 后端：`src-tauri/src/lib.rs` 在 `tauri::Builder::setup` 内 spawn 第三个 task，订阅 `watcher.subscribe_files()` 并 `emit("file-change", &event)` 到前端；`FileChangeEvent` / `TodoChangeEvent` 加 `#[serde(rename_all = "camelCase")]` 与项目 IPC 命名约定一致
  - 前端：`ui/src/lib/fileChangeStore.svelte.ts` 模块级 `listen("file-change")` 单例 + `registerHandler/unregisterHandler` + `dedupeRefresh(key, fn)` 合并同 key 的并发刷新
  - SessionDetail：`onMount` 注册 `session-detail-${tabId}` handler，命中当前 `(projectId, sessionId)` 时通过 `dedupeRefresh` 调 `getSessionDetail` 替换 `detail` + `tabStore` 缓存；刷新前判断 `scrollTop + clientHeight >= scrollHeight - 16` 是否 pinned-to-bottom，若是则 `tick()` 后 restore `scrollTop = scrollHeight`
  - Sidebar：`$effect` 依赖 `selectedProjectId` 重注册 `sidebar` handler，命中当前 project 时 `dedupeRefresh` 调 `loadSessions(currentProjectId)`（含 `untrack` 防 effect 自激）；effect cleanup + `onDestroy` 双重 unregister
- **openspec deltas**：`ipc-data-api` MODIFIED `Emit push events for file changes and notifications` 加 3 个 file-change 桥 Scenario；`session-display` ADDED `Auto refresh on file change` 6 个 Scenario；`sidebar-navigation` ADDED `Auto refresh session list on file change` 6 个 Scenario

### [impl-bug] isOngoing 缺 stale check（已结束 session 仍显绿点）✅ 已修复（2026-04-25）
- **用户报告**（2026-04-25）：sidebar 多日前会话仍显示 ongoing 绿点（如 4月12日的 session 在 4月25日仍显示运行中）。
- **根因**：`cdt-api::session_metadata::extract_session_metadata` 与 `LocalDataApi::get_session_detail` 都仅调 `cdt_analyze::check_messages_ongoing(&messages)` 做纯结构性判定（五信号活动栈），缺原版 `claude-devtools/src/main/services/discovery/ProjectScanner.ts:753-755` 的 `STALE_SESSION_THRESHOLD_MS = 5 * 60 * 1000` 兜底——session 异常退出（用户 Ctrl+C / kill cli / 关机）时最后一条消息可能是 `tool_result` 之类 AI 活动，无 ending 信号，被算法误判 ongoing；原版用文件 mtime 5 分钟无写入兜底标 dead，对齐 issue #94。
- **修复**：`cdt-api::ipc::session_metadata` 新增 `STALE_SESSION_THRESHOLD: Duration = 5 min`、`is_session_stale(file_modified, now)` 纯函数与 `is_file_stale(path)` async wrapper；`extract_session_metadata` 在 `messages_ongoing == true` 时再调 `is_file_stale(path)` 兜底；`local.rs::get_session_detail` 同样补上 stale check。`messages_ongoing == false` 时直接 `is_ongoing = false`，stat 失败保守保留 messages_ongoing 判定，时钟回拨（mtime 在未来）保守判 not stale。
- **测试**：`cdt-api::ipc::session_metadata::tests` 5 个纯函数 case（fresh / 4m59s / 5m exact / 7d / future-mtime）。集成 fixture 文件均刚创建 mtime=now，自动 pass stale check 不影响既有用例。
- **是否补 spec**：未走 openspec change（属于 port-fidelity 修复，与原版 TS 行为对齐，spec 未明确 stale 阈值；待下次 spec sync 时把 5min 阈值入 `ipc-data-api` `Expose project and session queries` Requirement）。

### [coverage-gap] Session "in progress" 状态检测与 ongoing/interruption UI ✅ 已在 `port-session-ongoing-and-interruption` 修复
- **最终实现**：
  - `cdt-core::MessageCategory::Interruption` 独立分类（不再是 hard noise），`HardNoiseReason::InterruptMarker` 删除
  - `cdt-core::SemanticStep::Interruption { text, timestamp }` 新增，`chunk-building` 在遇到 Interruption 消息时 flush buffer 并追加到最后一个 `AIChunk.semantic_steps`
  - `cdt-analyze::check_messages_ongoing(&[ParsedMessage]) -> bool` 端口 TS 活动栈算法，覆盖五种 ending 信号（text / interrupt / ExitPlanMode / tool rejection / SendMessage shutdown_response）
  - `cdt-api::SessionSummary` + `SessionDetail` 新增 `isOngoing`；`session_metadata` 扫描时累积全 `ParsedMessage` 一并计算 ongoing
  - UI：`OngoingIndicator.svelte` + `OngoingBanner.svelte` 两组件；Sidebar PINNED + 日期分组 session title 前渲染绿点；SessionDetail 对话容器尾部按 `detail.isOngoing` 渲染蓝色横幅；AIChunk body 内渲染 "Session interrupted by user" 红色块
- **附带 impl-bug** `crates/cdt-parse/src/noise.rs:13` 把 interrupt marker 归 hard noise ✅ 一并修复：`classify_hard_noise` 不再识别 interrupt；新增 `is_interrupt_marker` 独立判定，`parser::classify_category` 在 hard-noise 为 None 且为 user 消息时赋 `MessageCategory::Interruption`
- **openspec deltas**：`session-parsing` MODIFIED + ADDED（interrupt 独立 category）；`chunk-building` MODIFIED + ADDED `Emit interruption semantic step`；`ipc-data-api` MODIFIED `Expose project and session queries`（+isOngoing 两 Scenario）；`session-display` ADDED 两 Requirement（banner + interruption step 渲染）；`sidebar-navigation` ADDED `Ongoing indicator on session item`

### 建议实现顺序
1. ~~先做实时 `file-change` 桥~~ ✅ `2026-04-18-realtime-session-refresh`
2. ~~再做 ongoing + interruption~~ ✅ `port-session-ongoing-and-interruption`
3. Execution Trace / 多 Pane 分屏 / 虚拟滚动等按原路线图继续

---

## 性能 / 首次打开大会话卡顿

### [perf] 首次点开大会话明显延迟 ✅ 已在 `session-detail-lazy-render` 修复（前端首屏部分）
- **用户报告**（2026-04-19）：一个 976 条消息的会话，首次点开时首屏渲染有明显延迟；切回同 session 时不慢。
- **定位结果**（2026-04-19）：
  - 后端 release 实测（`crates/cdt-api/tests/perf_get_session_detail.rs`）：1221 条 / 96 chunk session 总耗时仅 **45 ms**（parse 18ms + scan_subagents 14ms + build 4ms + serde 8ms），后端**不是**瓶颈。
  - 真正大头在前端：(a) IPC payload **7.7 MB** 跨 webview 传输 + JS `JSON.parse` 50–150 ms 量级；(b) 96 个 chunk 全量同步 `marked + highlight.js + DOMPurify` 渲染，~200–500 ms；(c) 首屏 `processMermaidBlocks` 全树扫描 + mermaid 动态 import 30+ KB。
- **本次修复**（change `session-detail-lazy-render`，行为契约入主 spec：`openspec/specs/session-display/spec.md` `Lazy markdown rendering for first paint performance` + `Skeleton placeholder while loading`）：
  - 新增 `ui/src/lib/lazyMarkdown.svelte.ts`：用 `IntersectionObserver`（`rootMargin: 200px`）把所有 markdown 渲染推迟到 chunk 进入视口才触发；占位高度按文本长度估算。
  - 新增 `ui/src/components/SessionDetailSkeleton.svelte`：首屏 loading 期间 5 条静态骨架卡片替代纯文本"加载中..."。
  - `processMermaidBlocks` 从首屏全树扫描迁移到 lazy observer 的 onRendered 回调（仅扫该 chunk 子树）。
  - 后端 `LocalDataApi::get_session_detail` 与 `scan_subagent_candidates` 加 `tracing::info!(target: "cdt_api::perf", ...)` 探针；前端 `SessionDetail.svelte` `[perf]` console.info 探针——均保留供未来回归监测。
- **后续**：
  - **Phase 2 已落地**（change `subagent-messages-lazy-load`，行为契约入主 spec：`openspec/specs/ipc-data-api/spec.md` `Lazy load subagent trace` + `openspec/specs/session-display/spec.md` `Subagent 内联展开 ExecutionTrace` / `Subagent MetricsPill 多维度展示`）：实测 IPC 仍占首屏 97%（556ms / 576ms），用户 console 数据证实前端 lazy markdown 单独不能解决跨进程 7.3 MB payload。breakdown 显示 `subagent_messages` 占 60%（46a25772 case：4659 KB / 7702 KB）。`Process` 上加 4 个 derived header 字段（`headerModel` / `lastIsolatedTokens` / `isShutdownOnly` / `messagesOmitted`），`get_session_detail` 默认裁 `subagent.messages` 为空 + 设 `messagesOmitted=true`；新增 `get_subagent_trace(rootSessionId, subagentSessionId)` IPC，`SubagentCard` 展开时按需懒拉。实测：46a25772 payload 7702 KB → 3070 KB（砍 60%），按 13 KB/ms 推算 IPC 556 ms → ~230 ms。回滚开关 `OMIT_SUBAGENT_MESSAGES: bool` 一行切回。
  - **Phase 3 已落地**（change `session-detail-image-asset-cache`，行为契约入主 spec：`openspec/specs/ipc-data-api/spec.md` `Lazy load inline image asset` + `openspec/specs/session-display/spec.md` `Inline image lazy load via asset protocol`）：phase 2 后用升级版 perf bench（commit `0c8a7a6` 加 chunk 类型 / AI 子树 / user content block 三层细分）发现新瓶颈是 user message 内联截图的 base64 data——7826d1b8 case 7 张截图占 RAW 82%（4220 KB / 5161 KB），phase 2 OMIT 完全没覆盖（image 在 user chunk 而非 subagent.messages）。`ImageSource` 加 `data_omitted: bool`（camelCase `dataOmitted`）；`get_session_detail` 默认把所有 ImageBlock `source.data` 替换为空 + 设 flag，回滚开关 `OMIT_IMAGE_DATA: bool`。新增 `get_image_asset(rootSessionId, sessionId, blockId) -> String` IPC：SHA256 内容寻址（前 8 字节 hex）→ 落盘 `<dirs::cache_dir>/claude-devtools-rs/cdt-images/<hash>.<ext>` → 返回 `asset://localhost/<path>` URL（Tauri `tauri.conf.json::assetProtocol.scope` 配三平台 cache 路径，cargo 自动加 `protocol-asset` feature）。前端新建 `ImageBlock.svelte` 组件（IntersectionObserver rootMargin=200px），顺带补齐前端原本完全没渲染 image 的 coverage gap（`utext()` 只取 text block）。失败 fallback 返回 `data:` URI 保活。**实测**（cargo test --release perf_get_session_detail）：
    - 4cdfdf06 (172 msgs / 2 image): IPC payload 1768 → **515 KB**（砍 71%），est ipc 136 → ~40 ms
    - 7826d1b8 (250 msgs / 7 image): IPC payload 4840 → **620 KB**（砍 88%），est ipc 372 → ~48 ms
    - 46a25772 (1221 msgs / 0 image): IPC payload 3070 KB 不变（无 image，符合预期）
  - **Phase 4 已落地**（change `session-detail-response-content-omit`，行为契约入主 spec：`openspec/specs/ipc-data-api/spec.md` `Expose project and session queries` Requirement 内的 "AssistantResponse content omitted by default" 三条 Scenario）：phase 3 后 46a25772（无 image）case 仍 IPC 427ms / 2799 KB，breakdown 显示 `responses[].content` 占 41% (1257 KB)。前端代码审查（`grep -rn responses ui/src/`）确认**全 UI 6 处 `.responses` 用法均不读 `.content`**（chunk 显示文本走 `semanticSteps`），故采用最小改动路径：`AssistantResponse` 加 `#[serde(rename = "contentOmitted", default)] pub content_omitted: bool`；`get_session_detail` 序列化前 `apply_response_content_omit(chunks)` 把所有 `AIChunk.responses[].content` 替换为 `MessageContent::Text("")` + 设 flag；OMIT 顺序 image → response.content → subagent，覆盖回滚组合下嵌套 subagent.messages 层。**不需要新 IPC、不需要前端改动**。回滚开关 `OMIT_RESPONSE_CONTENT: bool` 一行切回。**实测**（cargo test --release perf_get_session_detail）：
    - 4cdfdf06 (172 msgs / 2 image / 5 sub): IPC payload 515 → **248 KB**（砍 52%），est ipc 40 → ~38 ms（image-heavy case 已被 phase 3 砍走大头，本期增量小）
    - 7826d1b8 (250 msgs / 7 image / 1 sub): IPC payload 620 → **334 KB**（砍 46%），est ipc 48 → ~51 ms
    - 46a25772 (1221 msgs / 0 image / 14 sub): IPC payload 3070 → **1829 KB**（砍 40%），est ipc 230 → ~51 ms（首屏纯文本大 session 主战场，符合预期）
  - **Phase 5 已落地**（change `session-detail-tool-output-lazy-load`，行为契约入主 spec：`openspec/specs/ipc-data-api/spec.md` 内的 "ToolExecution output omitted by default" 三条 Scenario + `Lazy load tool output` Requirement / `openspec/specs/session-display/spec.md` `Lazy load tool output on expand` Requirement）：phase 4 后用户实测 46a25772 IPC 369 ms / 1668 KB / first-paint 395 ms；breakdown 显示 `tool_exec.output` 占剩余 26%（436 KB）。**关键复盘**：phase 4 实测证明 IPC time 不是 payload-bound 线性函数（payload 砍 40% IPC 只砍 13%）—— V8 `JSON.parse` 大对象 + Tauri webview baseline 是固定/非线性开销，**chunk 数量没变** object graph 拓扑没动。phase 5 按"非线性折扣"保守预估 -12% 到 -16% IPC time（不是质变）。`ToolExecution` 加 `outputOmitted: bool` + `ToolOutput::trim()` helper（Text/Structured 清空 inner、Missing 不变）；`get_session_detail` 序列化前 `apply_tool_output_omit(chunks)`；新 IPC `get_tool_output(rootSessionId, sessionId, toolUseId) -> ToolOutput`，前端 `ExecutionTrace` + `SessionDetail` 内联 tool 渲染分支各自加 `outputCache: Map<toolUseId, ToolOutput>`，`toggle(key, exec?)` 时若 `exec.outputOmitted=true` fire-and-forget 调 IPC，`effectiveExec(exec)` cache 命中替换 output 透传给 ToolViewer。回滚开关 `OMIT_TOOL_OUTPUT: bool`。**实测**（cargo test --release perf_get_session_detail）：
    - 4cdfdf06 (172 msgs / 5 sub): IPC payload 248 → **163 KB**（砍 34%）
    - 7826d1b8 (250 msgs / 1 sub): IPC payload 334 → **202 KB**（砍 40%）
    - 46a25772 (1221 msgs / 14 sub): IPC payload 1829 → **1412 KB**（砍 23%），按 4.5 KB/ms 实测吞吐推算 IPC 369 → ~314 ms，first-paint 395 → ~340 ms（命中"渐进改良"档位）
  - **下下轮 follow-up**（phase 5 后仍不够时）：当前剩余 1412 KB 内 `responses[].meta` 588 KB（35%）+ `tool_exec.input` 340 KB（24%）已是高频访问字段难裁。继续追 first-paint < 200 ms 必须**换思路**：(a) 后端虚拟分页（首屏 N=20 chunks 完整 + 剩余 76 chunks 只返 metadata 占位，前端 IntersectionObserver 滚到尾部按需 `get_session_chunks(offset, limit)`）—— 直接砍 object graph 节点数 80%，预计 IPC 砍 60% 量级；(b) Tauri Channel + binary 序列化（bincode/postcard 替代 JSON）—— 砍 V8 `JSON.parse` 开销，吞吐 4.5 → 20+ KB/ms；(c) `tool_exec.input` 字段级裁剪（per-tool whitelist 保留 file_path/command 小字段，OMIT old_string/new_string/content 大字段）—— ROI 中等。优先级排序前需要先实测 phase 5 落地后的真实 first-paint 数字与用户痛感再定。
  - 跨视口搜索高亮 / 浏览器原生 Cmd+F：见下方两条独立条目。

### [coverage-gap] lazy markdown 副作用：搜索高亮无法命中未渲染 chunk
- **背景**：`session-detail-lazy-render` 把 chunk 内 markdown 改为视口懒渲染。`ui/src/lib/searchHighlight.ts` 通过 textNode walk 在 conversation 容器内高亮——视口外的占位 div 没有 markdown 文本节点，搜不到。
- **影响**：用户在 SearchBar（Cmd+F）输入查询时，匹配项若位于未渲染的 chunk 内，无法定位也不参与 next/prev 导航。
- **修复路径**（待复现痛点后再做）：
  1. SearchBar 触发时，把所有 lazy 占位强制 fire（observer.observe → renderMarkdown 全部）
  2. 或在 search 路径上单独跑 raw text match（不依赖 DOM 节点），找到目标 chunk 后 `scrollIntoView` + 触发 observer
- **不做**：立即修。先观察用户是否在大 session 频繁用搜索，再定优先级。

### [coverage-gap] lazy markdown 副作用：浏览器原生 Cmd+F 不命中未渲染 chunk
- **背景**：同上根因。Chrome / WebView 内置 Find-in-Page 也只搜可见 DOM 文本，未渲染的占位 div 不命中。
- **修复路径**：理论上需要在 `keydown` 监听 `Cmd+F`（系统原生 Find）触发前临时切到 `LAZY_MARKDOWN_ENABLED=false` 全量渲染——但首帧成本会回到改造前。
- **不做**：punt 到下一轮。Tauri 窗口里浏览器 Cmd+F 用得很少（被应用内 SearchBar 接管）。

---

## Subagent 状态判定 / UI 显示

### [impl-bug?] Subagent 实际未完成但 UI 卡片显示"已完成" ✅ 已修复（2026-04-19）
- **用户报告**（2026-04-19）：观察到 subagent 任务并未真正完成，但 Rust 版 SubagentCard 右上角图标仍显示 ✓（`sa-status-done`）而不是旋转的 loading（`sa-status-running`）。独立于刚刚 archive 的 `fix-subagent-display-order-and-styling`。
- **根因**：装载层 `crates/cdt-api/src/ipc/local.rs::parse_subagent_candidate` 用 `is_ongoing = end_ts.is_none()` 简化判定——只要 JSONL 末行 timestamp > 首行 timestamp 就视为已完成。而主 session 走的是 `cdt_analyze::check_messages_ongoing` 五信号活动栈（text / interrupt / ExitPlanMode / tool rejection / SendMessage shutdown_response）。两套判定算法**不一致**：subagent 中断后无 assistant 收尾的情况（如 `5a3a23b2.../agent-aee63780244f1f959.jsonl`：末尾 user/tool_result 无 assistant 回复）按 spec 应判 ongoing，但被装载层误判 done。Resolver 层 `compute_is_ongoing(cand) = cand.is_ongoing || cand.end_ts.is_none()` 是 OR 兜底，无法纠正装载层的错误。UI 层字段对齐无问题（`SubagentCard.svelte::process.isOngoing` ↔ `Process.is_ongoing`，serde camelCase 自动转）。
- **修复**：`parse_subagent_candidate` 在已 `parse_file(path)` 拿到 `Vec<ParsedMessage>` 后，先跑 `cdt_analyze::check_messages_ongoing(&msgs)` 算 `is_ongoing`，再清 sidechain → `build_chunks`。`end_ts` 仍按时间戳填充供 `compute_duration_ms` 使用，与 ongoing 判定独立。Resolver 的 OR 兜底保留——装载层为 true 时强制 ongoing；为 false 时仍允许 `end_ts.is_none()` 兜底（如 parse 失败的 case）。
- **测试**：`crates/cdt-api/src/ipc/local.rs::tests` 三个 `#[tokio::test]`：
  - `parse_subagent_candidate_marks_unfinished_tool_use_as_ongoing`（末尾 assistant tool_use 无 tool_result）
  - `parse_subagent_candidate_marks_text_ending_as_done`（末尾 assistant text 是 ending）
  - `parse_subagent_candidate_marks_orphan_tool_result_as_ongoing`（复刻真实 case：tool_use → tool_result 但无后续 assistant）
- **性能**：perf bench 三个 sample（4cdfdf06 / 7826d1b8 / 46a25772）`scan_subs` / `IPC` / `TOTAL` 时间均与改前同档（46a25772：scan_subs 21ms / IPC 47ms / TOTAL 55ms），`check_messages_ongoing` 在已 parse 的 msgs 上 O(n) 单次遍历开销可忽略。

---

## Windows 平台（change `windows-platform-support` 后的遗留跟进）

### [coverage-gap] WSL distro 扫描与 UNC 路径候选

- TS 原版 `src/main/services/config.ts:901-939` 的 `handleFindWslClaudeRoots` 在 `process.platform === 'win32'` 时扫描 WSL 发行版、解析 WSL 主目录、生成 `\\wsl$\<distro>\home\<user>\.claude` 风格的 UNC 路径候选，让 Windows host 上的应用能直接发现 WSL 内的 Claude Code 数据。
- Rust port 当前未实现（见 change `windows-platform-support` proposal 的 "Not in scope"）。Windows 用户若只在 WSL 里跑 Claude Code CLI，当前版本 Windows native 应用找不到数据（`~/.claude/projects` 解析到 `C:\Users\<user>\.claude\projects`，不在 WSL 命名空间内）。
- 落地建议：新 change `windows-wsl-distro-scan`，新增 `cdt-discover::wsl` 模块（仅 `#[cfg(target_os = "windows")]`）；调 `wsl.exe -l -v` 枚举 distro，拼 UNC 路径；IPC 层 `list_claude_roots_on_windows()` 返候选给前端选择。
- 依赖：`cdt-discover` 已依赖 `tokio::process`，可复用。UNC 路径需验证 `Path::is_absolute()` 在 Windows 上识别 `\\server\share\...`（应该是的）；前面已有 `looks_like_absolute_path` 识别 UNC。

### [coverage-gap] Windows Authenticode 签名 / `bundle.windows` 配置

- `src-tauri/tauri.conf.json::bundle` 当前只有 `macOS.signingIdentity = "-"`，Windows 无签名配置 → release bundle（`.msi` / NSIS `.exe`）不带 Authenticode 签名，Windows SmartScreen 会弹 "未知发布者" 警告并默认阻止运行。
- `README.md` 已告知用户手动放行（"更多信息" → "仍要运行"），可接受但不专业；正式发版建议购买 EV 证书或走 Azure Trusted Signing。
- 落地建议：
  1. 购证/申请 Azure Trusted Signing → 拿到证书指纹或 Signing Account 名字
  2. `tauri.conf.json::bundle` 加 `"windows": { "certificateThumbprint": "...", "digestAlgorithm": "sha256", "timestampUrl": "http://timestamp.digicert.com" }` 或 `azureSignTool` 配置
  3. `.github/workflows/release.yml` Windows job 注入 `WINDOWS_CERTIFICATE` / `WINDOWS_CERTIFICATE_PASSWORD` secrets；或 OIDC 连 Azure
  4. 新 change `windows-code-signing`，proposal 重点说明证书来源 + 成本 + 风险（私钥外泄）

### [coverage-gap] Windows 路径大小写不敏感比较

- TS 原版 `pathValidation.ts:65-72` 的 `normalizeForCompare(s, isWindows)` 在 `process.platform === 'win32'` 时 `path.normalize().toLowerCase()`，让 `C:\Users\Alice` 和 `c:\users\ALICE` 比较相等。
- Rust port 当前字符串级比较（`starts_with` / `eq`）大小写敏感。Windows 用户若 JSONL 里 `cwd` 是 `C:\Users\Alice` 但配置里是 `c:\users\alice`，可能匹配失败。
- 落地建议：小 change 或随手 fix。在 `cdt-discover::path_decoder` 加 `pub fn paths_equal_cross_platform(a: &str, b: &str) -> bool`，Windows 分支先 `to_lowercase()` 再比。
- 影响面：`ProjectPathResolver::resolve` 的 cache key、`FileWatcher::route_event` 的 `starts_with` 判定。实际触发概率不高（Claude Code CLI 通常写一致的 cwd），但会是 Windows 用户的隐藏 bug。

---

## Implicit contracts（baseline 外，UI 层）

下列行为无法冻结进 baseline specs，Rust 重写选 UI 技术栈时需要单独决策是否复刻：

- **滚动编排**（`useTabNavigationController`, auto-scroll bottom, scroll restore）
- **搜索高亮跨会话定位**（`SessionSearcher` + 滚动联动 + 高亮持久化）
- **Tab 导航与关闭历史**（`tabSlice` + `tabUISlice`，每 tab 独立 UI 状态隔离）
- **键盘快捷键**（`keyboardUtils`，Tab 切换、搜索焦点、复制）
- **Markdown 渲染细节**（`react-markdown` + `remark-gfm` + `mermaid` + 代码块 syntax highlight）
- **主题切换与 CSS 变量级联**（`useTheme`，dark/light）
- **Dashboard 水瀑图渲染策略**（`waterfall` 数据 → 渲染形态）
- **虚拟滚动 / 大会话渲染性能**（decision on list virtualization 策略）
- **Notification 桌面提醒 / 系统托盘** 行为

这些条目在 Rust port 里属于 **UI 技术栈决策域**，可以按新栈习惯重做，不强制 1:1。
