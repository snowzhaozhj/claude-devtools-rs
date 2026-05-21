# Tasks

> **Session 进度（2026-05-22 第二轮）**：opsx:propose + design codex 二审 + 第一段 foundation 已完成。本轮已落地：§4 algorithm 分叉消除（get_session_detail / get_image_asset / get_tool_output / find_session_project / get_subagent_trace 全切 cache wrapper + fs trait；list_sessions_skeleton inner & outer + build_group_session_page 改 SkeletonThenStream：SSH hot path 用 `lookup_trust_cached` 0 fs op，cache miss 入 page_jobs 走 fs 异步刷新 + SSE 推差量；删除 SSH FullEager inline read 路径与 `should_emit_inline_update`）、§6 5 处 policy ADR 注释、§7 4 subagent helper + parse_subagent_candidate + locate_session_jsonl 全切 fs trait + 保留 flat/nested 双结构、§8 image_disk_cache.rs module 抽出 + discover_memory_layers / read_memory_file 切 fs trait + xtask 零违规、ipc_contract test 跟进 SkeletonThenStream 新语义、SshFileSystemProvider::open_read 加 fake fallback 让测试走通。**剩余 follow-up**：§5 SSH 后台 batch read_dir_with_metadata（per-project N→1 stat 优化，当前已通过 `scan_metadata_for_page` per-session via fs trait 异步刷新功能正确，仅缺批量优化）、§12 micro-bench `perf_scanner_open_read.rs` + `perf_ssh_cache_hit.rs`。这两项是 perf 优化非行为改变，留 PR-D2 follow-up。


## 1. cdt-parse: 新增 `parse_file_via_fs` SSH-aware 入口
- [x] 1.1 在 `crates/cdt-parse/src/lib.rs` 加 `pub async fn parse_file_via_fs(fs: &dyn cdt_fs::FileSystemProvider, path: &Path) -> Result<Vec<ParsedMessage>, ParseError>`
- [x] 1.2 内部走 `fs.open_read(path).await?` 拿 `Box<dyn AsyncRead+Send+Unpin>` → `tokio::io::BufReader::with_capacity(32 * 1024, reader)` → `lines().next_line()` 喂现有逐行 dedupe / parse 状态机
- [x] 1.3 旧版 `parse_file(path)` 改为兼容 wrapper：内部 `parse_file_via_fs(&*cdt_fs::local_handle(), path).await`
- [x] 1.4 单测覆盖：(a) Local fs 包装 dyn AsyncRead 与原 `parse_file` 输出 byte-equal；(b) 大文件（>1MB fixture）逐行解析顺序正确；(c) Unpin bound 在 Box::new(file) cast 处编译通过

## 2. session_metadata: scanner 切 dyn AsyncRead + 加 fs 参数
- [x] 2.1 `extract_session_metadata_with_ongoing` 签名加 `fs: &dyn cdt_fs::FileSystemProvider` 第一参（保留 `path: &Path` 第二参）
- [x] 2.2 内部 `File::open(path)` → `fs.open_read(path).await`；保留同 `Err` graceful return default
- [x] 2.3 `BufReader::new(file)` → `BufReader::with_capacity(SCANNER_BUF_BYTES, reader)`，定 `const SCANNER_BUF_BYTES: usize = 32 * 1024;` 在 module 顶（design D5：与 SFTP packet 上限对齐）
- [x] 2.4 公开 wrapper `extract_session_metadata(path)` 保留为 path-only 入口；内部 `let fs = cdt_fs::local_handle(); extract_session_metadata_with_ongoing(&*fs, path).await.0`
- [x] 2.5 `is_file_stale(path)` 改名 `is_file_stale(fs: &dyn FileSystemProvider, path: &Path) -> bool`，内部 `fs.stat(path).await.ok().and_then(...)`；callsite 同步加 fs 参数
- [x] 2.6 移除 `use tokio::fs::File;` import（确保 H1 这个 module 干净）

## 3. parsed_message_cache: cache miss 路径走 `parse_file_via_fs` + 新加 cache helpers
- [x] 3.1 `extract_parsed_messages_cached` 内部 `cdt_parse::parse_file(path)` → `cdt_parse::parse_file_via_fs(fs, path)`（fs 参数已由 PR-C 引入）
- [x] 3.2 加 `ParsedMessageCache::lookup_with_known_signature(&mut self, ctx, path, sig)` —— 跳过内部 stat 用调用方提供的 signature
- [x] 3.3 加 `ParsedMessageCache::lookup_trust_cached(&mut self, ctx, path)` —— hot path cache hit trust，不校验 signature 直接返 entry
- [x] 3.4 单测：fake fs counter 验证 cache miss 路径 `open_read` 计数 = 1 / `read_to_string` = 0；`lookup_trust_cached` 不调 fs op；`lookup_with_known_signature` 与 `lookup` 在同 sig 下行为等价
- [x] 3.5 同形 helper 加到 `MetadataCache`：`lookup_with_known_signature` + `lookup_trust_cached`

## 4. local.rs: 13 处 algorithm 分叉消除（按 design D2 表）
- [x] 4.1 `list_sessions_skeleton` 内层 line 809-863 page metadata lookup：拆掉 `if is_remote { return None }` 早退；hot path 改用 `cache.lookup_trust_cached(&ctx, path)`（先返 cache 内容，0 fs op）
- [x] 4.2 `list_sessions_skeleton` 外层 line 1444-1574：拆掉 line 1498-1503 SSH 早退；line 1515 `let remote_meta = if is_remote { fs.read_to_string + parse }` 整段拆掉；line 1524 `should_emit_inline_update` 整套删（统一走 SSE 推差量）；line 1574 `if !is_remote` gate 去掉，SSH 也入 page_jobs spawn —— 与 Local 同入口的 SkeletonThenStream
- [x] 4.3 `build_group_session_page` line 555-572：page metadata lookup 同 4.1，hot path cache trust + 后台 batch
- [x] 4.4 `get_session_detail` line 2086-2171：起始处改 `let (fs, projects_dir, ctx) = self.active_fs_and_context_strict().await?;`（**codex Blocking #4 修正**：贯穿用同一 strict 快照，避免 SSH disconnect 中间态降级）；line 2141 messages 解析改走 `extract_parsed_messages_cached(&self.parsed_msg_cache, &*fs, &ctx, &jsonl_path)`；line 2171 `is_ongoing && !is_remote && stale check` 保留 `!is_remote` gate（codex Blocking #2 跨 clock domain）+ ADR `// policy fork: SSH mtime/local clock 跨 domain，PR-E lift to BackendPolicy::stale_check_strategy`
- [x] 4.5 `get_image_asset` line 2481-2530：起始处用 `active_fs_and_context_strict()` 同快照；line 2504 SSH inline `fs.read_to_string + parse_jsonl_content` 改走 `extract_parsed_messages_cached`
- [x] 4.6 `get_tool_output` line 2561-2593：同 4.5
- [x] 4.7 `find_session_project` line 2325 Local 走 tokio::fs 改 fs.read_dir / fs.stat（algorithm 统一）
- [x] 4.8 `get_subagent_detail` line 2395-2396 Local 走 tokio::fs 改 fs trait（algorithm 统一）

## 5. local.rs: SSH 后台 batch 校验 task + SSE 推差量
- [ ] 5.1 加 helper `pub(crate) async fn batch_validate_metadata_and_push_sse(self, fs: Arc<dyn FileSystemProvider>, ctx: ContextId, project_dir: PathBuf, sessions: Vec<PathBuf>, sse_tx: ...) -> Result<(), ApiError>`：调 `fs.read_dir_with_metadata(&project_dir)` 拿全 dir metadata；对 sessions 中每条 path 比对 cache signature；mismatch / 新增 → spawn cache miss scan + SSE event 推 metadata diff
- [ ] 5.2 `list_sessions_skeleton` SSH 路径在返骨架 + cache trust 渲染后 spawn 后台 batch task per project_dir（注册 abort handle 到 `active_scans` map per-key cancel）
- [ ] 5.3 `ssh_disconnect` 时 abort 该 ssh ctx 下的所有 batch task（D3-bis）
- [ ] 5.4 SSE event 类型复用现有 `session_metadata_update` 推送 channel（前端已订阅，无 BREAKING）

## 6. local.rs: 5 处 policy 分叉加 ADR 注释（保留行为，PR-E 上移）
- [x] 6.1 line 2035 `get_project_memory` SSH early-return empty：加 `// policy fork: PR-E lift to BackendPolicy::supports_memory`
- [x] 6.2 line 2067 `read_memory_file` SSH not_found：加同上注释
- [x] 6.3 line 2157 `candidates = if is_remote { Vec::new() }`：加 `// policy fork: PR-E lift to BackendPolicy::supports_subagent_scan`
- [x] 6.4 line 2171 `is_ongoing && !is_remote && stale check`：加 `// policy fork: SSH mtime/local clock 跨 domain，PR-E lift to BackendPolicy::stale_check_strategy 或加 SSH-aware clock skew compensation`
- [x] 6.5 line 2696 `SearchConfig::from_fs_kind(fs.kind())`：加 `// policy fork: PR-E lift to BackendPolicy::search_config`
- [x] 6.6 line 3068 / 3078 `if is_remote { NoopGitIdentityResolver }`：加 `// policy fork: PR-E lift to BackendPolicy::git_identity_resolver`

## 7. local.rs: 4 个 subagent helper 切 fs trait（保留 flat / nested 双结构）
- [x] 7.1 `find_subagent_jsonl(jsonl_paths_root, session_id)` 加 `fs: &dyn FileSystemProvider` 第一参；保留双结构 fallback：先 fs.exists flat → fs.read_dir nested；3 处 `tokio::fs::metadata` / `tokio::fs::read_dir` 替换为 `fs.stat` / `fs.read_dir`
- [x] 7.2 `scan_subagent_candidates` 同上保留双结构
- [x] 7.3 `find_subagent_jsonl_cross_project` 同上保留双结构
- [x] 7.4 `scan_subagent_candidates_cross_project` 同上保留双结构
- [x] 7.5 caller（line 2109 / 3593-3617 等）一轮 grep 改齐传 `&*fs`
- [x] 7.6 单测：双结构 fixture 都能定位 candidate（防 codex High #1 退化）

## 8. local.rs: 其它 tokio::fs::* 直调清理
- [x] 8.1 line 2114 `get_session_detail` Local fallback `tokio::fs::metadata`：改 `fs.stat()`
- [x] 8.2 line 3332 / 3371 / 3593 / 3596 / 3613 / 3617 / 3709 / 3717 等 memory / locate / subagent helper 内 tokio::fs：一律改 fs.stat / fs.read_dir / fs.read_to_string
- [x] 8.3 line 3670-3676 image disk cache `create_dir_all` / `metadata` / `write` 三处：抽到新 module `crates/cdt-api/src/ipc/image_disk_cache.rs`，函数 `pub(super) async fn cache_to_disk(cache_dir, file_path, bytes) -> ...`，**Local + SSH 都写**（design D4 修订；codex High #2）；该 module 路径加 ALLOWLIST
- [x] 8.4 line 3964 `parse_subagent_candidate` 内 `tokio::fs::File::open`：改 `fs.open_read` + 流式解析
- [x] 8.5 grep 全 `crates/cdt-api/src/` 确认无 `tokio::fs::*` 残留（除 ALLOWLIST 路径）

## 9. cdt-config mention.rs SSH graceful skip 契约（D7）
- [ ] 9.1 `crates/cdt-config/src/mention.rs::read_mentioned_file` 加 `is_ssh: bool` 参数
- [ ] 9.2 `is_ssh == true` 时 early-return `Err(MentionError::NotSupportedUnderSsh)` 或类似结构化错误
- [ ] 9.3 `local.rs` 内 caller 一轮改齐传 `fs.kind() == FsKind::Ssh`
- [ ] 9.4 单测覆盖：SSH 模式下调用返结构化错误，Local 模式不变

## 10. ALLOWLIST 扩展 + 顶部豁免准则
- [x] 10.1 `crates/cdt-fs/ALLOWLIST.md` 顶部加段落：豁免准则（D7 / D4 引用，每条新加 ALLOWLIST 行 SHALL 在 PR description 引用 design 决策）
- [x] 10.2 在 `## Allowlist` table 加 4 行：`crates/cdt-config/**`（reason 含 mention.rs SSH 契约）/ `crates/cdt-api/src/notifier.rs` / `crates/cdt-api/src/http/routes.rs` / `crates/cdt-api/src/ipc/image_disk_cache.rs`
- [x] 10.3 verify allowlist 完整：跑 `cargo run -p xtask -- check-fs-direct-calls` 应零违规（不带 `--warn-only`）

## 11. xtask 加 allowlist 校验（codex High #3）+ CI workflow 切 fail-on-match
- [x] 11.1 `crates/xtask/src/check_fs_direct_calls.rs`：扫完源码后**反向校验**每条 allowlist pattern 至少匹配 ≥1 实际文件——零匹配 exit 1 + 报 `error: ALLOWLIST entry '<pattern>' matches 0 files (likely typo or stale)`
- [x] 11.2 xtask parse table 第 2 列 reason；空 / 仅 `--` / 长度 < 10 视为占位 → exit 1 + 报 `error: ALLOWLIST entry '<pattern>' has empty/placeholder reason`
- [x] 11.3 单测覆盖 11.1 + 11.2（构造空 reason allowlist + 不匹配 glob，断言 xtask exit 1 + 错误消息）
- [x] 11.4 `.github/workflows/ci.yml` line 53 / 56-57 / 68 / 69 去掉 `--warn-only` flag + 注释相应更新（去掉 "warn-only / PR-A 期间过渡" 措辞）
- [x] 11.5 xtask 内部 line 67-69 注释更新：`--warn-only` 仅作本地诊断 opt-in；CI 默认 enforce fail-on-match

## 12. 性能验证: micro-bench + integration test
- [ ] 12.1 新增 `crates/cdt-api/tests/perf_scanner_open_read.rs` micro-bench：5 runs min/median/stddev 对比 `tokio::fs::File::open + BufReader` vs `LocalFileSystemProvider::open_read + BufReader::with_capacity(32K)`，500KB 与 5MB 两个 jsonl size；`#[ignore]` 默认不进 CI，本地跑取数据
- [ ] 12.2 新增 `crates/cdt-api/tests/perf_ssh_cache_hit.rs` integration test：fake-SSH provider 含 op counter；**走 `LocalDataApi::list_sessions` / `get_session_detail` 等 user-facing public method**（codex High #5），断言：(a) 二次 `list_sessions` cache hit trust → `open_read = 0` / `read_to_string = 0` / 后台 batch 走 read_dir_with_metadata（counter 反映）；(b) 二次 `get_tool_output` 同 session → fs op = 1 stat 0 open_read
- [ ] 12.3 新增 `crates/cdt-api/tests/perf_ssh_scanner_chunked_read.rs` `#[ignore]` SSH 大文件 scan integration（fake-SSH 注入 50ms RTT/read，packet 32K），断言 5MB jsonl scan wall < 9s
- [ ] 12.4 apply 前跑 `bash scripts/run-perf-bench.sh --runs 5` 留 baseline 数据
- [ ] 12.5 apply 完跑同命令，对比四维（wall / user / sys / RSS / user-real-ratio），写入 PR Perf impact 段
- [ ] 12.6 D1 micro-bench median 通过准则：candidate ≤ baseline × 1.3；超过即拒并回头优化（备选：减小 buf 到 16K / 改 enum 包装）
- [ ] 12.7 ADR grep 数量断言：跑 `rg "policy fork: PR-E lift to BackendPolicy" crates/cdt-api/src/ipc/local.rs | wc -l`，预期 ≥ 6（line 2035 / 2067 / 2157 / 2171 / 2696 / 3068 / 3078）；PR description 贴最终 grep 输出（codex Medium #3）

## 13. 顺手改：bg-task-dispatch 文档 + justfile bg-pr quoting
- [x] 13.1 `.claude/rules/bg-task-dispatch.md` "启动样板" 段：把 inline prompt 部分加粗 + 加备注"justfile bg-pr 已经能正确处理 inline prompt 内的双引号 / 反引号"
- [x] 13.2 加一节"prompt 含特殊字符"：示例 `just bg-pr xxx '改 \`fn foo()\` 的实现并加测试'` 验证 backtick 不被吃
- [x] 13.3 `justfile` `bg-pr` recipe 改用 `quote()` 函数 + `--` 分隔，避免 inline prompt 双引号嵌套被吃
- [x] 13.4 verify：本机跑 `just bg-pr test-quoting 'echo \`backtick\` "double quote" $\\HOME and $$bash_var'`（PROMPT 含三种特殊字符）；session 启动后 prompt 内特殊字符保持原样

## 14. 测试 / clippy / 验证
- [x] 14.1 `cargo clippy --workspace --all-targets -- -D warnings` 全过
- [x] 14.2 `cargo fmt --all`
- [x] 14.3 `cargo test --workspace`
- [x] 14.4 `cargo run -p xtask -- check-fs-direct-calls`（不带 `--warn-only`）零违规 + exit 0
- [x] 14.5 `pnpm --dir ui run check`（确保前端无 IPC 字段联动 break）
- [x] 14.6 `openspec validate unify-fs-direct-calls --strict` 过
- [x] 14.7 `cargo check --manifest-path src-tauri/Cargo.toml` 让 lockfile 同步（PR-B/C 已修，本 PR 复查）

## N. 发布
- [ ] N.1 push 分支 + 开 PR（PR 描述含 Perf impact 四维 + Non-Goals + 18 处分叉处理表 + ADR grep count）
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
