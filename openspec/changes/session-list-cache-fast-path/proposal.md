## Why

PR #80 修 "sidebar 偶发显示 sessionId 8 字符前缀（如 `464d13b7…`）" bug 时，前端主修是 `Sidebar.svelte::onMount` 调换 `listen("session-metadata-update")` 与 `await loadProjects()` 的顺序，杜绝 tauri emit 在 listener 注册前 fire-and-forget 丢消息。这是纯时序 bug fix，不涉及 spec。

但同一 PR 还引入了**后端 cache fast-path 兜底**：`LocalDataApi::list_sessions_skeleton` 对每条 jsonl_path 先查 `MetadataCache`，命中条骨架阶段直接 inline 填回真实元数据、**不**入 page_jobs（即不 spawn 后台扫描、不 broadcast `SessionMetadataUpdate`）。这是 `list_sessions` 的**新行为路径**——即使 emit 链路任何原因丢消息，重复打开列表也能从 cache 拿到完整元数据。该路径被现有 spec line 23 "SHALL 允许占位" + Scenario "最多 N 条 emit" 字面允许（允许不等于必须），但 spec 没显式描述这条 fast-path 的语义，后人改 `MetadataCache` 行为时不会知道 sidebar 显示也依赖它。

本 change 把这条新行为路径固化到 `ipc-data-api` spec，让后人改 cache 时能看到 sidebar 的依赖契约。

## What Changes

- `LocalDataApi::list_sessions_skeleton` 对每条 jsonl_path 调 `try_lookup_cached_metadata`（lookup-only fast-path，不触发扫描）。命中条骨架阶段 inline 填回 `title` / `messageCount` / `isOngoing` / `gitBranch`，**不**入 `page_jobs`；未命中条（cache miss / stat 失败 / `FileSignature` 不等——mtime / size / identity 任一不等）仍入 `page_jobs` 走原后台扫描 + broadcast emit 路径
- Lookup 用 `futures::future::join_all` 并发执行，并发上限 `Semaphore(METADATA_SCAN_CONCURRENCY=8)` 限流，避免 caller 传超大 page_size 把 tokio blocking pool 打满
- 新增 `session_metadata::try_lookup_cached_metadata(cache, path) -> Option<SessionMetadata>`：lookup-only 变体，复用 `FileSignature` 等价比较 + `is_session_stale(mtime)` 实时合成 `isOngoing`；`FileSignature` 不等（mtime / size / identity 任一不等） / stat 失败 / cache miss 任一返回 `None`，让调用方 fallback 到后台扫描

非变更项：
- IPC payload 结构 / Tauri command 协议 / camelCase 命名 / `OMIT_*` 系列开关 —— 全部不动
- `subscribe_session_metadata()` 返回的 `broadcast::Receiver<SessionMetadataUpdate>` 协议不变；emit 时机 / payload 字段不变
- `active_scans` / `scan_generation` race-free 抢占逻辑不变（cache 全命中时 `page_jobs.is_empty()` 路径直接 skip spawn 分支，不改动 `active_scans`）
- `MetadataCache` 自身实现 / capacity / FileSignature 算法不变（沿用 change `multi-session-cpu-cache` archive 的产出）

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `ipc-data-api`: `Requirement: Emit session metadata updates` 的语义扩展——补充 cache fast-path 路径、emit 数量上界改写、并发限流。

## Impact

- 代码：`crates/cdt-api/src/ipc/local.rs::list_sessions_skeleton`、`crates/cdt-api/src/ipc/session_metadata.rs`（新增 `try_lookup_cached_metadata`）
- 测试：`crates/cdt-api/tests/session_metadata_stream.rs::repeated_list_sessions_returns_cached_metadata_inline`（PR #80 已加）
- 性能：cache 命中率高时 `list_sessions` 后台扫描调用次数下降；首次冷启动行为不变
- 用户体感：sidebar 重复打开列表时 title 立即出现，不再依赖 broadcast emit 到达
- spec：`openspec/specs/ipc-data-api/spec.md` 的 `Requirement: Emit session metadata updates` 整段 MODIFIED——保留原有 5 个 Scenario（订阅接收 / Tauri emit / 同 projectId 取消旧 / 后台扫描并发限制 / 无 watcher 安全），新增 5 个 Scenario（骨架 lookup 并发限制 / cache 命中零 emit / cache 部分命中 / cache 全命中不 spawn / lookup stat 失败 fallback）

## Meta：事后补 spec 声明

按 CLAUDE.md `feedback_sync_spec_after_code.md` 与新增的 `feedback_identify_behavior_extension.md`，行为契约扩展 SHALL 先 propose 再 apply。本 change 是 PR #80 已写完代码再补 spec 的"事后补"场景——是被 CLAUDE.md 否决的下策，但既已发生，spec delta 真实反映已实现的代码行为，让后人能从 spec 单一源头理解。详细元决策见 `design.md::D5`。
