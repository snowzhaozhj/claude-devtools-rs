# Implementation Tasks

> 说明：本 change 是 PR #80 已写完代码再补 spec 的事后补场景（详见 `design.md::D5` 与 user memory `feedback_identify_behavior_extension`）。所有 task 已在 PR #80 commit `c88e562` 完成，本文件作为"事后核对清单"——每个 checkbox 对应一条已实现的代码路径，供 reviewer 对照 spec 验证 fidelity。

## 1. 后端：lookup-only 函数与 cache fast-path

- [x] 1.1 在 `crates/cdt-api/src/ipc/session_metadata.rs` 新增 `try_lookup_cached_metadata(cache, path) -> Option<SessionMetadata>`：内部调 `tokio::fs::metadata` 拿 `FileSignature`、查 `MetadataCache::lookup`、`FileSignature` 等价校验通过则用 `is_session_stale(mtime, SystemTime::now())` 合成 `is_ongoing` 后返回 `Some(SessionMetadata)`；任一步骤失败/不命中返回 `None`，**不**触发扫描
- [x] 1.2 `crates/cdt-api/src/ipc/local.rs` 顶部 `use super::session_metadata::{..., try_lookup_cached_metadata};`
- [x] 1.3 `LocalDataApi::list_sessions_skeleton` 改写骨架构造循环：先用 `futures::future::join_all` + `Semaphore(METADATA_SCAN_CONCURRENCY)` 并发对所有 session 跑 `try_lookup_cached_metadata`；命中条 inline 填 `SessionSummary` 不入 `page_jobs`；miss 条入 `page_jobs` 同时填占位 `SessionSummary`
- [x] 1.4 `LocalDataApi::list_sessions` 的 `if !page_jobs.is_empty()` 分支不变（cache 全命中时 page_jobs 为空，跳过 spawn，不触碰 `active_scans`）

## 2. spec 同步（本 change 的核心产出）

- [x] 2.1 `openspec/changes/session-list-cache-fast-path/proposal.md` 写明 Why / What Changes / Capabilities (`ipc-data-api` MODIFIED) / Impact
- [x] 2.2 `openspec/changes/session-list-cache-fast-path/design.md` 写明 Context / Goals / Decisions D1-D5（含 D5 事后补 spec 元决策） / Risks
- [x] 2.3 `openspec/changes/session-list-cache-fast-path/specs/ipc-data-api/spec.md` MODIFIED `Requirement: Emit session metadata updates`，完整 copy 原有 5 个 Scenario（订阅接收 / Tauri emit / 同 projectId 取消旧 / 后台扫描并发限制 / 无 watcher 安全）+ 加 cache fast-path 描述段 + 5 个新 Scenario（骨架 lookup 并发限制 / cache 命中零 emit / cache 部分命中 / cache 全命中不触发 spawn / lookup stat 失败 fallback）

## 3. 测试

- [x] 3.1 `crates/cdt-api/tests/session_metadata_stream.rs` 新增 `repeated_list_sessions_returns_cached_metadata_inline`：第一次 list_sessions 走扫描收齐 N 条 emit；第二次同 project 同 page 骨架阶段直接带 title 非 None / `messageCount=2` / `isOngoing=true`；rx 在 300ms 内收不到任何新 update（cache 全命中跳过 spawn）
- [x] 3.2 现有 `list_sessions_returns_skeleton_and_emits_metadata_updates` / `repeated_list_sessions_aborts_previous_scan` / `concurrent_list_sessions_does_not_orphan_scan` / `metadata_scan_concurrency_is_eight` 4 测试保持通过（cache 在首次调用时为空，仍走原扫描路径）

## 4. 验证

- [x] 4.1 `cargo test -p cdt-api --test session_metadata_stream` 5/5 通过
- [x] 4.2 `cargo clippy -p cdt-api --all-targets -- -D warnings` 全过
- [x] 4.3 `just preflight` 全绿（fmt + workspace clippy + workspace test + svelte-check + spec-validate）
- [x] 4.4 codex 异构二审 2 轮通过（Q1/Q4/Q5 无 bug；Q3 串行 stat 性能问题已并发 + Semaphore 修；Q2 等长改写 cache stale 留待 cache 算法层单独跟进）
- [ ] 4.5 `openspec validate session-list-cache-fast-path --strict` 通过（待执行）
- [ ] 4.6 codex 审 spec delta + design.md（事后补 spec 仍需走审计）
