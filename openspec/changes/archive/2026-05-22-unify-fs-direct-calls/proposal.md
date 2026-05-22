## Why

PR-A（change `unify-fs-abstraction`）落地了 `cdt-fs` crate（trait + ContextId + InstrumentedFs + xtask warn-only）。PR-B（`metadata-cache-context-prefix`）切了 `MetadataCache` 到 `(ContextId, PathBuf)` key + Local stat 路径走 fs trait。PR-C（`parsed-message-cache-context-prefix`）切了 `ParsedMessageCache` 到同形态 + 引入 `active_fs_and_context_strict()` helper。但 PR-A 的 H1 契约（"业务路径禁直调 `tokio::fs::*`"）和 PR-A D6 分类表（"23 处 `is_remote` 算法分叉 SHALL 消除"）至今未落地——`xtask check-fs-direct-calls` 仍 `--warn-only` 兜底，业务路径里仍有 ~35 处 `tokio::fs::*` 直调和 ~18 处 `is_remote` / `fs.kind()` 分叉。

更关键的：PR-B/C scope 边界（D8 / D6）显式声明"SSH callsite 仍走 inline 不查 cache，等 PR-D 把 scanner 切 `fs.open_read` 后才能让 SSH 真正享受 cache 命中"。所以**用户报"SSH 列表卡顿 5-10s"现象的最后一公里在本 PR**：

1. `extract_session_metadata_with_ongoing` 内部的 `tokio::fs::File::open + BufReader::lines` 切 `fs.open_read` → `Box<dyn AsyncRead + Send + Unpin>` + `BufReader`，让 cache miss 后扫描路径 SSH-aware
2. `list_sessions_skeleton` / `build_group_session_page` SSH 分支 cache lookup 早退（PR-B D8 的 `if is_remote { return (None) }` 三处）拆掉，让 SSH 路径真正接入 metadata cache
3. `get_session_detail` / `get_image_asset` / `get_tool_output` 的 SSH 分支统一走 fs trait + cache wrapper，把"算法分叉"按 PR-A D6 分类清掉
4. `xtask check-fs-direct-calls` 切 fail-on-match，配合 `crates/cdt-fs/ALLOWLIST.md` 显式列出 Local-only 路径（image disk cache / config persistence / notification history / subagent scan helpers）

**重点：让 SSH 列表用户感知"卡顿消失"**（用户决策 2026-05-22）。朴素 per-session stat 串行 50×50ms=2.5s 超 sidebar 500ms 预算（codex 二审 Blocking #1）。**全落 G + D + E 三件套**：

- **G. cache hit trust + 后台异步刷 + SSE 推差量**：用户切回已访问过的 SSH host → 立刻渲染 in-memory cache（0 RTT），后台启 batch task 走 `fs.read_dir_with_metadata` per project 拉新 metadata 后通过 SSE 推差量给 UI
- **D. SSH 改走 SkeletonThenStream**：line 855/1515/1524/1574 SSH FullEager policy 分叉**本 PR 提前实施**（与 PR-E BackendPolicy struct 上移解耦——本 PR 实施 SSH 同走 Local 入口的算法逻辑，PR-E 后续把字段值塞 struct 即可）
- **E. read_dir_with_metadata per-parent-dir 批量**：后台校验路径 SFTP READDIR reply 自带 entry attrs，1 RTT/parent

PR-F 真消除冷启动卡顿走 **方案 C SFTP message-id pipeline**（解 `Arc<Mutex<SftpSession>>` 全锁串行）——架构合理性优先：cdt-devtools-rs 与 VS Code Remote-SSH 的关键差异是"无远端 shell / binary 依赖"，方案 B 远端 mass-stat 命令需 spec change 放开远端命令清单破此架构假设。详 design.md Context 段 PR-F 路径分析。

不在本 PR 范围：BackendPolicy struct wire 到 LocalDataApi 字段持有（PR-E；本 PR 仅实施算法层 SSH 同入口 + ADR 注释）、SSH SFTP 真 message-id pipeline（PR-F）、HTTP transport 抽象（远期）。

## What Changes

- **fs-abstraction**: `xtask check-fs-direct-calls` 默认 fail-on-match（`--warn-only` 仍可显式开启，但 CI default off）。`crates/cdt-fs/ALLOWLIST.md` 新增 Local-only 业务豁免行（image disk cache / cdt-config 全部模块 / cdt-api 内 subagent helpers / notifier / http file serve），并明示"豁免仅当路径在 design.md 已分类为 Local-only 业务且 SSH 路径有显式 graceful skip"。
- **ipc-data-api**: `extract_session_metadata_with_ongoing` 签名加 `fs: &dyn FileSystemProvider`（来自 callsite 的 `active_fs_and_context_strict()` 返回三元组），内部 `File::open` → `fs.open_read(path)`；`is_file_stale` 同形态切 fs trait。`list_sessions_skeleton` / `build_group_session_page` page metadata lookup 拆掉 SSH 早退，SSH ctx 走同一 cache wrapper（cache hit 路径不调 fs，cache miss 路径走 `fs.open_read`）。`get_session_detail` SSH 分支 messages 解析路径走统一 cache wrapper（取代 inline `fs.read_to_string + parse_jsonl_content`）。`get_image_asset` / `get_tool_output` 的 SSH 分支同步收敛。8 处算法分叉消除；6 处策略分叉（list_sessions skeleton vs eager / memory not-supported / search tuning / GitIdentityResolver / subagent skip）保留并加 `// policy fork: PR-E lift to BackendPolicy::xxx` ADR 注释。
- **ssh-remote-context**: 加 Scenario "SSH 列表 cache hit 路径不调 SFTP stat / open"（验证 PR-A H2 契约在 SSH 上真正生效）。
- **session-parsing**: 加 Scenario "scanner 接受 `Box<dyn AsyncRead>` 不破 streaming 状态机性能"（D1 micro-bench dyn ≤ direct × 1.3 median）。

## Impact

- Affected specs: `fs-abstraction` / `ipc-data-api` / `ssh-remote-context` / `session-parsing`
- Affected code:
  - `crates/cdt-api/src/ipc/session_metadata.rs`（scanner 切 dyn AsyncRead + signature 加 fs/ctx）
  - `crates/cdt-api/src/ipc/local.rs`（~26 处 tokio::fs 替换 + 8 处算法分叉消除 + 6 处策略分叉 ADR 注释 + 4 个 subagent helper 签名加 fs 参数）
  - `crates/cdt-api/src/ipc/parsed_message_cache.rs`（cache wrapper 内部 scanner 切 fs.open_read）
  - `crates/cdt-api/src/notifier.rs`（poll_session 切 fs.stat 或 ALLOWLIST，本 change 选 ALLOWLIST 因为 notifier 永远 Local 视角）
  - `crates/cdt-api/src/http/routes.rs`（HTTP file serve / image data-URI 走 ALLOWLIST，HTTP 总是 Local context）
  - `crates/xtask/src/check_fs_direct_calls.rs`（去 `--warn-only` 默认）
  - `crates/cdt-fs/ALLOWLIST.md`（新增 9 行豁免）
  - `tests/perf-baseline.json`（新增 `perf_scanner_open_read_overhead` bench 基线）
  - `crates/cdt-api/tests/perf_scanner_open_read.rs`（新 D1 micro-bench：dyn vs direct）
  - `crates/cdt-api/tests/perf_ssh_cache_hit.rs`（新 SSH cache hit 路径计数 bench：fake-SSH provider，hit 路径 stat=N / open_read=0 / read_to_string=0）
  - `.claude/rules/bg-task-dispatch.md`（inline prompt 段措辞加粗 + quoting 风险提示）
  - `justfile`（bg-pr recipe 切 `quote()` + here-string 防 inline prompt 双引号反引号被吃）
- BREAKING: 否（cache 行为是回归补全；前端 IPC 字段不变；trait 公开签名不变）
