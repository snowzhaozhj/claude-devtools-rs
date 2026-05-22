## Context

PR-A 落 `cdt-fs` crate（trait + ContextId + InstrumentedFs + xtask warn-only）；PR-B 切 `MetadataCache` 到 `(ContextId, PathBuf)` key + Local stat 路径走 fs trait + 加 `active_fs_and_context()` helper（relaxed）；PR-C 切 `ParsedMessageCache` 同形 + 加 `active_fs_and_context_strict()` helper（SSH disconnect 中间态返 `not_found` 而非 silently degrade）。但 PR-A H1 契约「业务路径禁直调 `tokio::fs::*`」与 D6 分类表「23 处分叉 SHALL 消除算法分叉」未落地。

**当前底层数据**（grep + file:line 在工作目录 `/Users/zhaohejie/RustroverProjects/Project/claude-devtools-rs/.claude/worktrees/pr-d-unify-fs-direct-calls`）：

```
非 test tokio::fs::* 直调:           ~35 处
  cdt-api:                          26 处（local.rs 23 / session_metadata.rs 2 / notifier.rs 1 / http/routes.rs 2）
  cdt-config:                       12 处（claude_md 5 / manager 8 / mention 2 / notification_manager 3）
is_remote / fs.kind() 分叉:        18 处（local.rs）
受影响 cache:                        2（MetadataCache + ParsedMessageCache）
```

PR-B/C 显式声明：「SSH callsite 仍走 inline 不查 cache，等 PR-D 把 scanner 切 `fs.open_read` 后才能享受 cache 命中」。本 PR 是把这条最后一公里跑完 + 把 H1 / H3 / 30+ 处直调 + 18 处分叉一次清完，**且**让 SSH 列表用户感知"卡顿消失"——不只是"不再每次重 read 全文"。

**关于 SSH 列表性能的根本约束**（codex 二审 2026-05-22 + 用户决策）：

朴素"per-session stat 验 cache signature"在 SSH 上线性 RTT × `Arc<Mutex<SftpSession>>` 全锁串行（PR-A D3 钉死的 SFTP 已知假 batch）= 50 sessions × 50ms = 2.5s wall，**直接超** sidebar 首屏 < 500ms 预算 5 倍。读完社区主流 SSH 文件浏览器后筛选出三条**不破 spec、不依赖 PR-F**的路径，本 PR 全部落地：

- **G. cache hit trust + 后台异步刷 + SSE 推差量**：用户切回已访问过的 SSH host → 立刻渲染 in-memory cache 内容（0 RTT），后台启异步 batch 校验拉新 metadata 后通过 SSE 推差量给 UI——这才是"卡顿消失"的真正机制
- **D. SSH 改走 SkeletonThenStream**（取代 FullEager）：列表先返 file_name 骨架（一次 read_dir 1 RTT 即得，不带 metadata），metadata 走 SSE event 后续推；本 PR 顺手把原 PR-A D6 标"PR-E 上移"的 line 855/1574/1524 policy 分叉**提前到本 PR**实施
- **E. read_dir_with_metadata per-parent-dir 批量**：后台校验路径用 SFTP READDIR reply 自带 entry attrs 一次拿全 dir 内容 + 所有 entry 的 mtime/size/identity，M projects × 1 RTT；典型 5-10 projects/page = 250-500ms，重 user 30+ projects = 1.5s（**仍是 follow-up 真消除目标**）

**未来 PR-F 路径选择**：方案 C（SFTP message-id pipeline 解 `Arc<Mutex<SftpSession>>` 全锁）vs 方案 B（远端 mass-stat 命令需 spec change 放开 `find / ls / stat` 等只读 SSH exec）。**架构合理性偏向方案 C**：cdt-devtools-rs 选 SFTP 而非 SSH exec 的核心架构差异是"无远端 shell / binary 依赖"——alpine BusyBox / restricted shell / 跨发行版命令语法差异都能跑；这是与 VS Code Remote-SSH（装 server binary）的关键区别。SFTP 协议原生支持多 outstanding message id 并发，用其原生能力符合"用对工具"；方案 B 一旦放开远端命令清单会边界滑坡（find → cat → grep 持续诱惑）。性能上方案 C 也等价（N concurrent requests 1 RTT）。本 PR follow-up 段标 PR-F 走方案 C。

性能基线（`tests/perf-baseline.json` + `.claude/rules/perf.md`）：
- `perf_cold_scan` wall ≤ 500ms / user/real ≤ 0.6 / RSS ≤ 50000kb
- `perf_get_session_detail` wall ≤ 500ms / user/real ≤ 0.7 / RSS ≤ 140000kb

scanner 切 dyn AsyncRead 是潜在退化点；若 vtable + heap allocation 影响 jsonl streaming 速度，会反向打回 PR-B/C 已建立的 cache 命中收益。本 change SHALL 通过 D1 micro-bench 量化保护。

## Goals / Non-Goals

**Goals:**
- `extract_session_metadata_with_ongoing` 内部 `File::open + tokio BufReader::lines` → `fs.open_read + BufReader<Box<dyn AsyncRead+Send+Unpin>>::lines`
- `extract_session_metadata_cached` / `try_lookup_cached_metadata` / `is_file_stale` 等 cache wrapper 函数签名 fs/ctx 参数已就位（PR-B 完成），本 change 让 SSH callsite 真正调用它们；scanner 内部走 fs trait
- `extract_parsed_messages_cached` 内部 cache miss 后的 `parse_file` 调 `cdt_parse::parse_file_via_fs(fs, path)`（新增包装）切 fs.open_read
- `list_sessions_skeleton` / `build_group_session_page` SSH 分支走 SkeletonThenStream（与 Local 同入口）；首屏拿 cache 内容（hit trust），后台 batch 校验 + SSE 推差量
- `get_session_detail` / `get_image_asset` / `get_tool_output` SSH 分支统一走 fs trait + cache wrapper（消除 algorithm 分叉），用 `active_fs_and_context_strict()` 拿三元组同快照
- 18 处分叉按 D6 分类逐行处理：~13 处 algorithm 消除（含原 PR-A D6 标 policy 但本 PR 决定提前 wire 的 4 处）；~5 处 policy 加 ADR 注释保留（PR-E 上移 BackendPolicy）
- 4 个 subagent scan helper 签名加 `fs: &dyn FileSystemProvider`，保留现有 flat + nested 双结构支持，统一走 fs trait
- `notifier.rs::poll_session` / `http/routes.rs` / `cdt-config/**` 永远 Local context 路径走 ALLOWLIST 而非 fs trait
- `xtask check-fs-direct-calls`：xtask 默认行为已是 fail-on-match（PR-A 实现），本 PR **去掉 CI workflow 里的 `--warn-only` flag** + 加 allowlist 校验（每条 pattern 至少匹配 ≥1 文件 + reason 列非空）
- 顺手改 `.claude/rules/bg-task-dispatch.md` inline prompt 措辞 + 修 `justfile bg-pr` quoting bug
- 全程不破 perf 基线（wall +20% / user +50% / RSS +30% / user-real-ratio cross 0.5 任一即拒）

**Non-Goals:**
- **不**实现 SFTP message-id pipeline（PR-F；本 PR 后台 batch 校验仍受 SFTP `Arc<Mutex<SftpSession>>` 串行约束，但 hot path 已 cache trust 无 fs op 故卡顿消失）
- **不**改 BackendPolicy 业务字段定义（PR-A 已定义；line 855/1574/1524/1515 等本 PR 内联实现"SSH 走 SkeletonThenStream"，PR-E 才把这条策略上移到 BackendPolicy struct 字段并让 LocalDataApi 持有 BackendPolicy 而非 inline if）
- **不**改 IPC 字段 / 前端契约
- **不**重构 cache LRU 数据结构（VecDeque O(N) bump 留给 follow-up）
- **不**让 cdt-config / notifier / http 走 fs trait（这些是 Local-only 业务路径，走 ALLOWLIST 更符合"trait 是 Local-vs-SSH 抽象"语义）
- **不**支持 SSH 远端 @mention 文件解析（D7 钉死 SSH 下 mention 走 graceful skip 返结构化错误）
- **不**新建 fs-abstraction Requirement（复用 PR-A H1-H6 + 加 enforce 层 Scenario）

## Decisions

### D1: scanner 切 `Box<dyn AsyncRead + Send + Unpin>` + BufReader 32 KiB 重新包装

**问题**：`extract_session_metadata_with_ongoing` 当前用 `tokio::fs::File::open(path)` + `BufReader::new(file)` + `reader.lines()`，3 行流式状态机喂 ongoing_sm + count + title + git_branch。切 fs trait 需要返抽象 reader——选关联类型还是 dyn box？

**修法**：选 `Box<dyn AsyncRead + Send + Unpin>`（与 PR-A `FileSystemProvider::open_read` 已有签名一致）：

```rust
let mut reader = match fs.open_read(path).await {
    Ok(r) => BufReader::with_capacity(SCANNER_BUF_BYTES, r),
    Err(_) => return (default_metadata, false),
};
let mut lines = reader.lines();
while let Ok(Some(line)) = lines.next_line().await { ... }
```

`BufReader::with_capacity` buffer size 钉 **32 KiB**——codex 二审 Blocking #3 指出：SSH/SFTP packet 上限默认 32768，`SSH_FXP_READ` reply 单消息上限 32 KiB；`russh-sftp::client::fs::File` 的 `AsyncRead::poll_read` 内部对每个 BufReader fill 拆成 `request_size = min(buf_len, max_packet)` 个 SFTP READ message。64 KiB BufReader 强制每次 fill 跑 2 次底层 SFTP READ message——无收益，反而 BufReader 内部双 read 多一层 alloc。32 KiB 是 SFTP packet 最优值（单 BufReader fill = 单 SFTP READ）。Local NVMe 端 32K 也合理（page size 4K × 8 pages，单 syscall 读完 + 缓冲）。

**dyn dispatch overhead**：vtable lookup 每次 `poll_read` 调用几 ns，相对 syscall（Local stat/read 几十 µs）/ 网络 RTT（SSH 50ms）完全可忽略。**关键风险在 BufReader heap 分配 + 每次 `poll_read` vtable lookup**：~5MB jsonl 在 Local 走 BufReader 32K = 160 次 poll_read，160 × vtable lookup = ~80 ns 累计，0.001% wall 影响——可忽略。

**性能量化要求**：本 change 新增 micro-bench `crates/cdt-api/tests/perf_scanner_open_read.rs`，跑 5 次 min/median/stddev 对比：

- baseline: `tokio::fs::File::open(path).await` + `BufReader::new(file)` + 全文读+计数（~500KB jsonl + ~5MB jsonl）
- candidate: `LocalFileSystemProvider::open_read(path).await` + `BufReader::with_capacity(32 * 1024, reader)` + 全文读+计数

**通过准则**：candidate median ≤ baseline median × 1.3（vtable + heap alloc + buffer ≤ 30% 退化）；超过即本 change 拒合并。

**Unpin bound 满足性**（codex 二审 Low #2）：`tokio::fs::File: Unpin`（tokio doc 明示）；`russh_sftp::client::fs::File: Unpin`（russh-sftp 0.6+ 已实现，本仓 Cargo.lock 锁的版本符合；编译会在 `cdt-fs/src/local.rs` / `cdt-ssh/src/provider.rs` 的 `Box::new(file) as Box<dyn AsyncRead + Send + Unpin>` 处校验）。

**为何不用关联类型 `type Reader: AsyncRead`**：会让 trait 失去 object-safety，无法 `Arc<dyn FileSystemProvider>` 注入；PR-A trait 设计基础就是 dyn dispatch。

**替代方案**：(a) 关联类型 → 否决（破 dyn safety）；(b) `Pin<Box<dyn AsyncRead>>` → 否决（pin 噪音 vs Unpin 没成本差异）；(c) BufReader 64 KiB → 否决（codex Blocking #3 SFTP packet 32K 限制）；(d) BufReader 8 KiB（默认）→ 否决（SSH 5MB jsonl 需 ~640 RTTs）

### D2: 18 处 `is_remote` / `fs.kind() == Ssh` 分叉逐行分类落地

**分类标准**（PR-A H3 钉死）：
- **algorithm 分叉**：同一逻辑（解析 / 排序 / 过滤 / dir 遍历 / cache 写入）在 `if is_remote / else` 两路径写两遍 → 拒，统一走 fs trait
- **policy 分叉**：选 `BackendPolicy` 字段值 → 允许 inline，PR-E 上移到 BackendPolicy struct 字段

**用户决策（2026-05-22）**：原 PR-A D6 标"PR-E 上移"的 SSH list FullEager 4 处分叉（line 855 / 1515 / 1524 / 1574）**本 PR 提前实施**——因为"SSH 列表卡顿消失"承诺需要 SSH 改走 SkeletonThenStream + cache hit trust，而 SkeletonThenStream 实施本身就是把 SSH 路径与 Local 路径 align 走同一入口；PR-E 后续把这条决策从 inline 抽到 BackendPolicy 字段时就是简单 grep `// policy fork: PR-E lift` 注释 + 上移，无新算法。

**18 处最终分类**（grep `crates/cdt-api/src/ipc/local.rs` @ main HEAD 560c1f5）：

| line | 函数 / 上下文 | 分类 | 处理 |
|---|---|---|---|
| 809, 827 | `list_sessions_skeleton` page metadata lookup SSH 早退（PR-B D8） | **algorithm**（cache lookup 旁路） | **本 PR 拆**：让 SSH ctx 走同一 cache wrapper（cache hit trust，0 fs op）+ 后台 batch read_dir_with_metadata 异步刷 |
| 855 | `list_sessions_skeleton` SSH 不入 page_jobs spawn | **algorithm**（FullEager → SkeletonThenStream 改造）| **本 PR 拆**：SSH 改走 page_jobs 与 Local 同入口；先返骨架 + 后台异步 metadata fill；line 855 整个早退条件去掉 |
| 1444, 1498-1503 | `list_sessions_skeleton` outer cache lookup SSH 跳过 | **algorithm**（同 cache 旁路） | 拆 |
| 1515 | `let remote_meta = if is_remote { fs.read_to_string + parse }` | **algorithm**（SSH 全文 parse vs Local cache wrapper） | **本 PR 拆**：去掉整段 inline read_to_string + parse；改走 cache wrapper（与 Local 同走 `extract_session_metadata_cached`）；cache miss → page_jobs spawn 异步重 parse via `fs.open_read` |
| 1524 | `should_emit_inline_update = is_remote && remote_meta.is_some()` | **algorithm**（SSH 走 SkeletonThenStream，inline emit 改 SSE 推） | **本 PR 拆**：删 `should_emit_inline_update` 整套 inline emit 逻辑；统一改走 SSE event 后续推差量（与 Local 现有"先骨架后增量"一致） |
| 1574 | `if !is_remote { 入 page_jobs spawn }` | **algorithm**（与 855 / 1515 / 1524 同决策） | **本 PR 拆**：去掉 `!is_remote` gate，SSH 也入 page_jobs spawn |
| 2035 | `get_project_memory` SSH early-return empty | **policy**（SSH 不支持 memory） | 保留 + ADR `// policy fork: PR-E lift to BackendPolicy::supports_memory` |
| 2067 | `read_memory_file` SSH not_found | **policy**（同 2035） | 保留 + ADR |
| 2102 | `get_session_detail` 顶层 `let is_remote = fs.kind() == Ssh` | **transitive** | 顶层一次性算（`active_fs_and_context_strict()` 拿三元组），下游分支按下表分类 |
| 2109 | `Err(_) if !is_remote => find_subagent_jsonl (tokio::fs)` | **algorithm**（Local 走 tokio::fs vs SSH 走 fs trait） | 让 4 个 subagent helper 全切 fs trait（D6），caller 一律传 `&*fs`；`!is_remote` gate 仍保留作 policy（SSH 不跑 subagent scan）但内部走 fs trait 不分叉 |
| 2141 | `messages = if is_remote { fs.read_to_string → parse_jsonl_content }` | **algorithm**（SSH inline 全文 vs Local parse_file/cache） | 统一走 cache wrapper（Local 与 SSH 同走 `extract_parsed_messages_cached`） |
| 2157 | `candidates = if is_remote { Vec::new() }` | **policy**（SSH 不跑 subagent scan） | 保留 + ADR `// policy fork: PR-E lift to BackendPolicy::supports_subagent_scan` |
| 2171 | `is_ongoing = ... && !is_remote && stale check` | **policy**（codex Blocking #2：SSH 远端 mtime 与本机 SystemTime::now() 跨 clock domain）| 保留 + ADR `// policy fork: SSH mtime/local clock 跨 domain，5min 阈值不可比；PR-E lift to BackendPolicy::stale_check_strategy 或加 SSH-aware clock skew compensation` |
| 2325 | `find_session_project` if Local 走 tokio::fs::read_dir | **algorithm**（dir 遍历分叉） | 统一走 fs.read_dir |
| 2395-2396 | `get_subagent_detail` Local 走 tokio::fs vs SSH fs trait | **algorithm** | 统一走 fs trait |
| 2504 | `get_image_asset` Local cache wrapper vs SSH inline read_to_string | **algorithm** | 统一走 cache wrapper |
| 2572 | `get_tool_output` 同 2504 | **algorithm** | 同 |
| 2696 | `SearchConfig::from_fs_kind(fs.kind())` | **policy**（SSH 搜索参数 tuning） | 保留 + ADR `// policy fork: PR-E lift to BackendPolicy::search_config` |
| 3068 | `list_repository_groups` `if is_remote { NoopGitIdentityResolver }` | **policy**（SSH 不读本地 .git） | 保留 + ADR `// policy fork: PR-E lift to BackendPolicy::git_identity_resolver` |
| 3078 | `groups = if is_remote { Noop } else { Local }` | **policy**（同 3068） | 保留 + ADR |

**汇总**（更新后）：
- ~13 处 algorithm 分叉 → 本 PR 消除（原 8 处 + 提前实施的 4 处 SSH FullEager 改造 + 1 处 stale 重新分类为 policy 后回退）
- ~5 处 policy 分叉 → 本 PR 加 ADR 注释保留（PR-E 上移）
- 1 处 transitive 派生 → 顶层赋值，下游按上表分类

**为何 line 2171 stale 仍是 policy**（codex Blocking #2 修订）：朴素草案把 stale 收回为 algorithm 不成立——SSH 远端拿到的 mtime 是远端 clock domain，与本机 `SystemTime::now()` 的 5min 阈值比对在远端时钟回拨 / 时差场景下产生 false positive（远端时钟落后 5min → 刚写的 session 误判 stale）/ false negative（远端时钟超前 → `now.duration_since(file_modified)` Err → 永远不 stale）。**正确分类是 policy**：SSH 跳过 stale check 是 BackendPolicy 决策。PR-E lift 时同时考虑 SSH-aware clock skew compensation（连接时跑 `printf %s "$EPOCHSECONDS"` 测 offset？或继续 SSH skip stale）作为单独 PR 决策。

**为何 line 855 / 1515 / 1524 / 1574 本 PR 提前实施而非留 PR-E**：用户决策——"SSH 列表卡顿消失"承诺需要 SSH 改走 SkeletonThenStream + cache hit trust。SkeletonThenStream 本身是 SSH 与 Local 走同一入口的实现细节，与 BackendPolicy 字段定义无关；PR-E 的 BackendPolicy wire 是把"哪些字段值用哪个 backend"从 inline `if is_remote` 抽到 struct 字段——本 PR 实施的 SSH-同走-Local-入口逻辑在 PR-E 时只需把 `// policy fork: PR-E` 注释 grep + 把字段值塞入 BackendPolicy struct 即可，不重构算法。

**替代方案**：(a) 全部一刀切走 fs trait → 否决（policy 分叉真在 SSH 上需要不同行为，PR-E 才正确处理 BackendPolicy struct）；(b) policy 分叉本 PR 全消除 → 否决（要本 PR 同时 wire BackendPolicy 字段到 LocalDataApi 持有，scope 爆炸违反 PR-D 边界）；(c) line 2171 stale 收 algorithm → 否决（codex Blocking #2 跨 clock domain 不可比对）

### D3: SSH list 路径 cache hit trust + 后台 batch 校验 + SSE 推差量

**问题**：朴素 per-session stat 验 cache signature 在 SSH 上 `Arc<Mutex<SftpSession>>` 全锁串行 = 50 sessions × 50ms = 2.5s wall，**直接超** sidebar 首屏 < 500ms 预算。

**核心修法（用户决策"全落 G + D + E"）**：分两条路径

**Hot path（用户感知）**：cache hit trust（方案 G）
- 用户切回已访问过的 SSH host 时，UI 立刻拿 in-memory cache 中所有 `(ContextId::ssh(...), path)` entry 渲染列表（**0 fs op**）
- list_sessions_skeleton 的 SSH 路径**先**返 cache 内容（基于上次 metadata），**不**等 fs op 完成
- UI 拿到结果立刻渲染——这才是"卡顿消失"

**后台校验（保新鲜度）**：read_dir_with_metadata 批量 + SSE 推差量（方案 E）
- 渲染完 cache 后，spawn 一个**后台 task**走 `fs.read_dir_with_metadata(project_dir)` per project（典型 5-10 projects/page，5-10 RTTs ≈ 250-500ms）
- 拿到批量 metadata 后**逐条比对** cache signature；mismatch / 新增 → 走 cache miss + scanner 重 parse；所有结果通过 SSE event 推给前端 UI
- UI 收到 SSE event 后增量更新对应 session 行（与 Local 现有"先骨架后增量"语义完全一致）
- 这条路径仍是 O(M projects) 串行（受 SFTP Mutex 影响），但**在后台**——用户感知无延迟

**SkeletonThenStream（方案 D）**：SSH 改与 Local 同入口
- list_sessions 返回 ListSessionsResponse 的 SSE channel；首屏从 cache 取（hit trust），SSE 推 metadata diff
- 把 PR-A D6 标"PR-E 上移"的 line 855 / 1515 / 1524 / 1574 SSH FullEager 路径**本 PR 提前实施**——SSH 与 Local 同走 page_jobs spawn 模型（详 D2）
- PR-E 后续只把策略字段值上移到 BackendPolicy struct，不改算法

**冷启动首次（无 cache entry）**：
- 此时 hot path 没东西可 trust → 走方案 E 后台 batch fetch + 边拿边推 SSE 增量
- 用户首次连 SSH 仍要等 batch 拿完 metadata（~250-500ms 典型 / 1.5s 重 user 30 projects），但比"per-session stat 串行 2.5s"或"全文 read 5-30s"显著改善
- 真消除冷启动卡顿留 PR-F SFTP message-id pipeline（让 batch read_dir_with_metadata 真并发，M projects → 1 RTT 总）

**新 cache helper（本 PR 加）**：

```rust
impl MetadataCache {
    /// 用调用方提供的 FileSignature 直接查 cache —— 跳过内部 stat。
    /// 用于 list 后台 batch 校验：调用方先 read_dir_with_metadata 拿全
    /// dir metadata 后批量 lookup，避免 N 次串行 stat。
    pub fn lookup_with_known_signature(
        &mut self,
        ctx: &ContextId,
        path: &Path,
        signature: &FileSignature,
    ) -> Option<&MetadataCacheEntry> {
        let key = (ctx.clone(), path.to_path_buf());
        let entry = self.map.get(&key)?;
        (entry.signature == *signature).then_some(entry)
    }

    /// hot path cache hit trust —— 不校验 signature，直接返当前 entry。
    /// 调用方语义：信任 cache 内容，校验由后台 batch task 异步跑。
    pub fn lookup_trust_cached(
        &mut self,
        ctx: &ContextId,
        path: &Path,
    ) -> Option<&MetadataCacheEntry> {
        let key = (ctx.clone(), path.to_path_buf());
        self.map.get(&key).map(|e| {
            // bump LRU
            ...
            e
        })
    }
}
```

`ParsedMessageCache` 同形（PR-C 已有 `lookup` 走 path 取 signature 比对，本 PR 拆出 `lookup_with_known_signature` + `lookup_trust_cached` 入口）。

**watcher 失效语义**：`cdt-watch::FileWatcher` 仍 Tauri 本地 fs 包装，对 SSH ctx entry 不主动 invalidate（PR-C D4 钉死）。SSH cache 失效完全靠后台 batch 校验路径——下次列表 read_dir_with_metadata 拿的 metadata 就是最新远端状态，外部进程改文件 → mtime 变 → signature mismatch → cache miss → scan。

**spec scenario 显式**：在 `ssh-remote-context` MODIFIED Requirement "Read sessions and files over SSH with same contract" 加 Scenarios：
- "list 路径 hot path cache hit trust：用户切回 SSH host 时 UI 立刻拿 cache 渲染（fs op = 0）"
- "list 路径后台 batch 校验：spawn task 走 read_dir_with_metadata per project，SSE 推 metadata diff"
- "冷启动 list_sessions：cache 无 entry 时 SHALL 通过 SSE event 推骨架 + metadata 增量给 UI"

**替代方案**：
- (a) per-session stat 串行 → 否决（codex Blocking #1：50×50ms=2.5s）
- (b) 等 PR-F SFTP pipeline → 否决（PR-D 是用户感知卡顿消失核心 PR）
- (c) 远端 mass-stat 命令（方案 B）→ 否决（spec change，破"无远端 shell 依赖"架构）
- (d) cache hit trust + 后台 batch + SkeletonThenStream（本节版本）→ 选中

### D3-bis: 后台校验 task 与 user-facing IPC handler 的 fs / ctx 快照一致性

**问题**：D3 的"渲染完 cache 后 spawn 后台 task"涉及跨 await 持有 `Arc<dyn FileSystemProvider>` 与 `ContextId`。如果后台 task 启动后用户切了 SSH host，task 仍持有旧 fs/ctx——会写入旧 ContextId namespace 的 cache，无串扰但浪费工作。

**修法**：后台 task 在 spawn 时 clone 当时的 `(fs, ctx)` 快照（`Arc<dyn>` 是 `Arc::clone`，`ContextId` 是 cheap clone）；task 内部不再调 `active_fs_and_context()`——若用户中途切 host，task 仍跑完旧快照下的 batch 校验，写入旧 ContextId 的 cache entry（无串扰，且 reconnect 同 host 时复用）。

**取消语义**：spawn task 时把 abort handle 注册到 LocalDataApi 的 `active_scans` map（PR #38 引入的 per-key cancel 模式，详 crates/CLAUDE.md "后台任务 per-key 取消"段）；用户切 SSH host 触发 ssh_disconnect 时 abort 旧 task 避免浪费。

**spec scenario**：加 "后台 batch 校验 task 在 ssh_disconnect 时 abort"。

### D4: xtask CI workflow 切 fail-on-match + allowlist 校验扩展

**问题**：xtask 默认行为已是 fail-on-match（代码 line 67-74）；`--warn-only` 是 opt-in 不是 opt-out。但 CI workflow `.github/workflows/ci.yml:69` 仍传 `--warn-only` —— PR-A 期间设的过渡逻辑，PR-D 后应去掉。同时 codex 二审 High #3 指出 ALLOWLIST 校验薄弱：glob 拼错匹配 0 文件不报错、reason 列完全不读。

**修法**（多步）：

1. **去掉 CI workflow `--warn-only` flag**：改 `.github/workflows/ci.yml:69` `cargo xtask check-fs-direct-calls`（不带 `--warn-only`）；line 53 / 56-57 注释相应更新（去掉"warn-only / PR-A 期间"措辞）
2. **xtask 加 allowlist 校验**：扫完源码后**反向校验**每条 allowlist pattern 至少匹配 ≥1 实际文件——零匹配的 pattern 是死规则（拼错 / 未来文件挪走后未清理），exit 1 + 报 `error: ALLOWLIST entry '<pattern>' matches 0 files (likely typo or stale)`
3. **xtask 加 reason 列校验**：parse markdown table 第 2 列；reason 空字符串 / 仅 `--` / 长度 < 10 的视为占位（不严肃），exit 1 + 报 `error: ALLOWLIST entry '<pattern>' has empty/placeholder reason`
4. **ALLOWLIST.md 顶部加豁免准则段**：明示豁免准则
   - 路径在 design.md 已分类为 Local-only 业务（用户配置 / 系统通知历史 / Local-only disk cache）
   - 或 SSH 路径有显式 graceful skip / 该路径永远不接 SSH context（HTTP routes / notifier）
   - 或测试 fixture / 测试 setup 写文件（覆盖 `**/tests/**`）
   - 任何新加 ALLOWLIST 行的 PR SHALL 在 PR description 引用对应 design 决策

**ALLOWLIST 扩展行**（cdt-config / cdt-api Local-only）：

```markdown
| `crates/cdt-config/**` | 用户配置 / 通知历史 / 内存文件 — 永远 Local context（用户机本地配置），不参与 SSH cache；SSH context 下 @mention 走 graceful skip（详 design D7） |
| `crates/cdt-api/src/notifier.rs` | poll_session metadata 检测仅对 Local Tauri sessions 生效（SSH session 走前端心跳） |
| `crates/cdt-api/src/http/routes.rs` | HTTP file serve / image data-URI — HTTP context 当前不接 SSH（remote backend 通过 IPC 路径接入），future 若开 server-mode SSH 再扩 |
| `crates/cdt-api/src/ipc/image_disk_cache.rs` | image disk cache 永远本地 ~/.cache/，与 SSH context 无关 —— Local 与 SSH 的 image asset 都写本地 cache_dir（SSH 端 image 由 fs trait 拉到 Local 后 cache 在本地复用，详 design D7-image） |
```

注：image_disk_cache.rs 当前不存在；本 change 把 `local.rs:3670-3676` 的 disk cache 写入逻辑抽到独立 module（既消除一处直调豁免冲突，也提升可测性）—— 这是 ALLOWLIST 扩展引发的轻量重构，**不**算 scope 扩散因为只是 file split。

**为何 image disk cache Local + SSH 都写**（codex 二审 High #2 修订）：disk cache 路径是 Local fs（`~/.cache/`），与 SSH source 是否远端无关——SSH 端的 image asset 拉到 Local 后 cache 在本地复用是合理的（避免每次显示都 SFTP 拉一次）。task 7.4 草案误标"仅 Local kind 才写"会让 SSH image 退回 inline data-URI（IPC payload 膨胀），违反 IPC payload 瘦身原则。修订：**module 路径走 ALLOWLIST，Local + SSH 都写本地 cache**。

**为何不全切 fs trait**：cdt-config / notifier / http 这些路径 Local context 永久绑定，引入 dyn dispatch + Arc<dyn> 注入是无收益 indirection；走 ALLOWLIST + 加豁免准则段比拒绝直调更符合实际语义（design D7 决策）。

**替代方案**：(a) 保留 `--warn-only` 到 PR-E → 否决（H1 enforce 是 PR-D 终点）；(b) cdt-config 全切 fs trait → 否决（前述）；(c) 不加 allowlist 校验 → 否决（codex High #3）

### D5: 32 KiB BufReader（修订自原 64 KiB）

**问题**：scanner 切 `Box<dyn AsyncRead>` 后，SSH 端 `russh_sftp::client::fs::File` 的 read 是 SFTP message 单 RTT。`BufReader::new(file)` 默认 8 KiB buffer → 5 MB jsonl 需 ~640 RTTs ≈ 32 秒，**不可接受**。

**修法**：scanner 用 `BufReader::with_capacity(SCANNER_BUF_BYTES, reader)`，**`SCANNER_BUF_BYTES = 32 * 1024` (32 KiB)**：

- SFTP `SSH_FXP_READ` reply 单消息上限 32 KiB（codex 二审 Blocking #3 指出）
- 5 MB jsonl → ~160 个 BufReader fill = ~160 RTTs ≈ 8 秒（vs 默认 8K 32 秒）
- BufReader 32K 与 SFTP packet 同 size → 单 BufReader fill = 单 SFTP READ message，不浪费
- 64 KiB BufReader 强制每次 fill 跑 2 次底层 SFTP READ message → 无收益反而多一层 alloc
- Local NVMe 端 32K 也合理（page size 4K × 8 pages 单 syscall 读完）；vs 默认 8K，Local 几乎无感（NVMe 单 read 几十 µs）

**真消除大文件 SSH scan 卡顿留 PR-F**：方案 C SFTP message-id pipeline 让单文件内多个 SFTP READ 并发拉，~160 个 message 1 RTT → 50ms wall 即可；本 PR 单文件仍 sequential（160 RTTs ≈ 8s），但**冷启动一次性代价**（cache miss 后全 hit，二次 access 0 RTT）。

**性能量化要求**：本 PR 加 SSH 大文件 scan integration test（fake-SSH 模拟 50ms RTT/read，packet limit 32K），断言 5MB jsonl scan wall < 9s（含 BufReader 抖动 buffer）；perf bench 入口 `crates/cdt-api/tests/perf_ssh_scanner_chunked_read.rs` `#[ignore]`。

**替代方案**：(a) 64 KiB → 否决（codex Blocking #3 SFTP packet 32K 限制）；(b) 8 KiB（默认）→ 否决（SSH 5MB jsonl 32 秒不可接受）；(c) 256 KiB 大 buffer → 否决（内存压力 + SFTP packet 强制拆 8 次）

### D6: subagent JSONL helpers 切 fs trait + 保留 flat / nested 双结构

**问题**（codex 二审 High #1）：现有 `find_subagent_jsonl(jsonl_paths_root, session_id)` 支持两种 layout：
- 旧 flat: `<project_dir>/agent-<id>.jsonl`
- 新 nested: `<project_dir>/<sid>/subagents/agent-<id>.jsonl`

design 草案 task 6 写"4 个 subagent helper 全切 fs trait"未说明结构兼容；codex High #1 提醒：把 `find_subagent_jsonl_via_fs`（现有但只支持新结构）替代旧 helper 会丢失 flat 结构，让历史 session 找不到 subagent candidate。

**修法**：4 个 subagent helper 切 fs trait 时**保留双结构 fallback 逻辑**——helper 内部先试 flat、后试 nested、再走 cross-project；只把 fs op 入口换成 `fs.stat / fs.read_dir / fs.exists`，**不**改 layout 探测逻辑。

具体：

```rust
async fn find_subagent_jsonl(
    fs: &dyn FileSystemProvider,
    project_dir: &Path,
    session_id: &str,
) -> Option<PathBuf> {
    // 1. 旧 flat：<project_dir>/agent-<id>.jsonl（保留）
    let flat = project_dir.join(format!("agent-{session_id}.jsonl"));
    if fs.exists(&flat).await {
        return Some(flat);
    }
    // 2. 新 nested：<project_dir>/<sid>/subagents/agent-<id>.jsonl（保留）
    let nested_dir = project_dir.join(session_id).join("subagents");
    if fs.exists(&nested_dir).await {
        let entries = fs.read_dir(&nested_dir).await.ok()?;
        // ... 现有 entry filter / 选 first matching
    }
    None
}
```

跨 project_dir 关联（worktree / EnterWorktree 切 cwd 场景）当前不支持，crates/CLAUDE.md 已记录此 deviation；本 PR 不收口（D6 边界）。

**为何不复用 `find_subagent_jsonl_via_fs` 窄语义**：该 helper 当前仅支持新 nested layout（PR-B 引入时仅供 SSH 走的部分路径，未覆盖历史 session）；caller 接 `find_subagent_jsonl_via_fs` 会丢 flat 兼容。

**替代方案**：(a) 复用 `find_subagent_jsonl_via_fs` → 否决（codex High #1 丢 flat 结构）；(b) 保留两 helper 并存 → 否决（重复代码 + caller 困惑）；(c) 4 helper 切 fs trait + 双结构保留（本节版本） → 选中

### D7: cdt-config 全 ALLOWLIST + mention.rs SSH graceful skip 契约

**问题**：cdt-config 12 处 tokio::fs 直调（claude_md / manager / mention / notification_manager），按 PR-A "推到底"原则似应走 fs trait。但实际语义上这些路径是否真需要 SSH-aware？同时 codex 二审 High #4 指出：`mention.rs` 在 SSH context 下读 @mention 文件是当前已存在 bug（读 Local 路径必失败），ALLOWLIST 全放行会让 xtask 不抓此 risk。

**修法**：**cdt-config 全部 ALLOWLIST**（保留 tokio::fs 直调）+ **mention.rs SSH graceful skip 契约钉死**

**Local-only 业务路径分析**：
- `manager.rs`：用户配置持久化（`~/.claude/cdt-config.json`）—— **永远 Local 用户机**
- `notification_manager.rs`：通知历史（`~/.claude/cdt-notifications.json`）—— 同 manager
- `claude_md.rs`：CLAUDE.md / memory 文件读取——Local context 暴露给 IPC（`local.rs:2035 get_project_memory` SSH 路径已 early-return empty）
- `mention.rs`：@-mentioned 文件读取——当前 Local-only

**mention.rs SSH 行为契约**（codex 二审 High #4 修订）：
- 当前 `read_mentioned_file(path)` 在 SSH context 下直接读 Local 路径 → IO error → IPC 返 `ApiError::not_found`——**已存在但未文档化**
- 本 PR 钉死契约：SSH context 下 `read_mentioned_file(path)` SHALL 返 `Err(ApiError::not_supported_under_ssh)` 显式错误（含 i18n key），UI 据此提示"SSH context 下不支持 @mention 文件预览"——比 not_found 更准确
- 实施：在 `mention.rs::read_mentioned_file` 入口加一行 caller 传入的 `is_ssh: bool` 参数（caller 侧 `local.rs` 拿 `fs.kind()`），ssh true 时 early-return 错；其它逻辑不动
- 未来 PR-G 若实现 SSH @mention 解析（远端 `fs.read_to_string`）再开 spec change 上移

**ALLOWLIST 注释扩展**：`crates/cdt-config/**` 行的 reason 列**显式点名** mention.rs SSH 行为：

```
"用户配置 / 通知历史 / 内存文件 — 永远 Local context（用户机本地配置），不参与 SSH cache；
SSH context 下 mention.rs::read_mentioned_file SHALL 返 not_supported_under_ssh 而非读 Local 路径串扰（详 design D7）"
```

**为何不让 cdt-config 走 fs trait**：
- 引入 dyn dispatch / `Arc<dyn>` 注入到 cdt-config 模块违反 cdt-config crate 的"配置管理"职责边界
- cdt-config 内部目前不接 `Arc<dyn FileSystemProvider>`，让 caller（cdt-api）每次 wrap 才能调，污染 caller signature
- Local-only 路径走 trait = 无收益的 indirection
- ALLOWLIST + 显式 graceful skip 契约比强行 fs trait 包装更符合"trait 是 Local-vs-SSH 抽象"语义

**未来 PR-G 若引入"SSH 远端 memory 编辑" 或"远端 mention 文件解析"等行为**，再开 spec change 上移 cdt-config 到 fs trait。

**替代方案**：(a) cdt-config 全切 fs trait → 否决（前述）；(b) 部分切（claude_md 切，manager/notification 不切） → 否决（碎片化依据不一致）；(c) 全 ALLOWLIST + mention.rs 加 SSH 契约（本节版本） → 选中

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| scanner dyn dispatch + heap alloc 让 ~5MB jsonl scan 慢 30%+，本 change 反而打回 PR-B/C cache 命中收益 | D1 micro-bench 5 runs min/median/stddev 量化，dyn ≤ direct × 1.3 否则拒；BufReader 容量 32 KiB 与 SFTP packet 对齐 |
| ~13 处 algorithm 分叉 + 4 处提前 SSH FullEager 改造一次范围大，漏改 → 编译或行为退化 | tasks 按 line 号一处一处勾；clippy / cargo test --workspace 全过；新增测试覆盖 SSH cache hit trust 路径（counter bench 走 LocalDataApi public method） |
| ~5 处 policy 分叉加 ADR 注释，PR-E 落地时是否真按字段上移？ | ADR 注释格式钉死 `// policy fork: PR-E lift to BackendPolicy::<field>`，grep 易追；PR-E SHALL grep 这些注释作 implementation checklist；本 PR tasks 加 grep + count 断言 |
| `extract_session_metadata_with_ongoing` 签名加 fs 参数破坏 cdt-api 内 7 个直接 caller + 集成测试 | 一轮 Edit 全过 + clippy；公开 wrapper `extract_session_metadata(path)` 保留为 helper for tests，内部用 `cdt_fs::local_handle()` |
| `find_subagent_jsonl` 等 4 helper 签名加 fs 参数破坏 caller | crate-private（pub(crate)），无外部 caller；caller 6 处 grep 一轮改齐 + 双结构 fallback 保留 |
| `xtask check-fs-direct-calls` CI 切 fail-on-match 后立挂（如本 PR 漏改一处或 ALLOWLIST 拼错） | 本 PR 提交前先在本地跑 `cargo run -p xtask -- check-fs-direct-calls`（不带 `--warn-only`）零违规再 push；xtask 新加 allowlist 校验（pattern 至少匹配 ≥1 文件 + reason 非空）防 stale rule |
| ALLOWLIST 扩展若未来不慎再误添业务路径 | ALLOWLIST.md 顶部加豁免准则段落 + xtask reason 列校验；review checklist 加"新加 ALLOWLIST 行 SHALL 在 PR description 引用 design D7 / D4" |
| SSH cache hit trust 后用户拿到陈旧数据 | 后台 batch 校验 spawn 立刻刷新；外部进程改文件 → mtime 变 → batch 拿 metadata 比对 mismatch → cache miss → SSE 推差量更新 UI；用户感知"列表先出但 1-2 秒后内容自动 refresh"（与 Local 现有 SkeletonThenStream 体验一致） |
| BufReader 32 KiB 在 Local 上反而比默认 8K 慢 | D1 micro-bench 已覆盖（500KB jsonl + 5MB jsonl 两个 size），dyn × 1.3 阈值兜底；32K 是 SFTP packet 上限同 size，与 Local NVMe 块大小（典型 256K-1M）相比也无退化 |
| baseline 退化（perf_cold_scan / perf_get_session_detail）| apply 前后跑 `bash scripts/run-perf-bench.sh --runs 5` 四维齐看；超阈值拒 |
| SSH 后台 batch 校验 task 在用户切 host 时仍跑完旧快照浪费 RTT | per-key cancel（abort handle 注册到 active_scans map）+ ssh_disconnect 时 abort（D3-bis） |
| codex 二审报新问题 | propose 阶段 codex 已审 design.md（本节修订采纳 4 Blocking + 5 High）；apply push 后再调 codex 多轮验证细节 |
| `image_disk_cache.rs` 抽 module = 顺手重构，是否 scope 扩散？ | 仅是 file split + 接口不变（Local + SSH 都写本地 cache，无 fs_kind 分支）；ALLOWLIST 单行 glob 干净；不涉及行为变化 |
| justfile bg-pr quoting 修法可能跨 shell 行为不一致 | 用 just `quote()` 函数（生成 shell-safe single-quoted literal）+ 显式 `--` 分隔 + bash shebang，多 shell 兼容 |
| mention.rs SSH graceful skip 引入新 IPC error code | 复用现有 ApiError 体系，只加一个 variant（或字符串 reason "not_supported_under_ssh"）；前端 UI 加 i18n 提示是 follow-up（codex Open Question 4） |

## Migration Plan

本 change 是基建改动 + SSH 用户感知改进，对前端 IPC 无 BREAKING（响应字段不变；新增 SSE event 类型继承现有 list_sessions 增量推机制），对外部测试无 BREAKING（公开签名保留 / private 改动 crate 内）。

**部署顺序**（apply 阶段建议）：

1. 加 `crates/cdt-api/src/ipc/image_disk_cache.rs` 新 module + `local.rs::get_image_asset` 调它（D4 file split，Local + SSH 都写本地 cache）
2. 改 `extract_session_metadata_with_ongoing` 签名 → fs.open_read scanner（D1）+ `is_file_stale` 切 fs.stat
3. 加 `parse_file_via_fs`（cdt-parse 新公开函数）+ `extract_parsed_messages_cached` cache miss 路径切之
4. 加 `MetadataCache::lookup_with_known_signature` + `lookup_trust_cached` + `ParsedMessageCache` 同形 helper
5. 改 `local.rs::list_sessions_skeleton` / `build_group_session_page` 实施 D3：SSH 路径与 Local 同走 page_jobs spawn；首屏 cache hit trust；spawn 后台 batch task 走 read_dir_with_metadata + SSE 推差量
6. 改 `local.rs::get_session_detail` line 2086+：用 `active_fs_and_context_strict()` 拿三元组同快照；line 2141 SSH 走 cache wrapper（与 Local 同入口）；line 2171 stale 仍 SSH skip + ADR
7. 改 `local.rs::get_image_asset` / `get_tool_output`：SSH 走 cache wrapper
8. 改 4 个 subagent helper 签名（D6）+ caller，保留 flat / nested 双结构
9. 改 `notifier.rs / http/routes.rs / cdt-config/**` —— 走 ALLOWLIST 不动代码
10. 改 `mention.rs` 加 `is_ssh` 参数 + SSH graceful skip 路径（D7）
11. `crates/cdt-fs/ALLOWLIST.md` 加 4 行豁免 + 顶部准则段
12. `xtask check-fs-direct-calls`：加 allowlist 校验逻辑（pattern ≥1 匹配 + reason ≥10 char）
13. `.github/workflows/ci.yml` 去掉 `--warn-only` flag
14. 加 D1 micro-bench + SSH cache hit counter bench（走 LocalDataApi public method）
15. 改 `.claude/rules/bg-task-dispatch.md` + `justfile` bg-pr recipe
16. perf 验证：apply 前后各跑 5 次 `bash scripts/run-perf-bench.sh --runs 5`
17. codex 二审 push 前 + push 后

**回滚**：本 change 改动隔离在 cdt-api / cdt-parse / cdt-fs ALLOWLIST / xtask 四处主体；revert PR 即可。无数据迁移、无前端联动 BREAKING（前端只是收到新 SSE event，旧前端兼容）。

## Open Questions

1. **D5 BufReader 32 KiB 是否会让 Local NVMe 略退化？** —— 实测 32K 在现代 NVMe 单次 read syscall 仍快（page size 4K，32K = 8 pages），预期 ≤ 5% wall 增；若 D1 micro-bench 显示 >10% 则降到 16K 重测。**留 open**让数据驱动。

2. **D2 line 2171 SSH stale check 是否需要远端 clock offset compensation？** —— 朴素 SSH skip 不完美（用户看不到 SSH session "5min 静默后切红"提示），但 SSH-aware clock offset 跑 `EPOCHSECONDS` 探测 / 时间同步是新行为。本 PR 保持 SSH skip + ADR；**留 PR-E** 评估 lift to BackendPolicy::stale_check_strategy 时再决定 compensation。

3. **D7 mention.rs SSH 错误码是 `ApiError::not_supported_under_ssh` 新 variant 还是字符串 reason `not_supported`?** —— 新 variant 是 spec change（IPC error 契约），字符串复用现有 not_found 但 reason 字段标 `"ssh-not-supported"`。本 PR 用字符串 reason 路径（最小侵入）；前端 UI i18n 提示 follow-up PR-G 决定。**留 open**。

4. **PR-F 用方案 C SFTP message-id pipeline 还是方案 B 远端 mass-stat 命令？** —— 用户决策（2026-05-22）："架构合理性优先"，本 PR Context 段已论证方案 C 符合 cdt-devtools-rs"无远端 shell 依赖"架构。本 PR follow-up 标 **方案 C**；PR-F 时若发现 russh-sftp 协议层并发限制不可解再回头评估方案 B。**已 closed by 用户决策**。

5. **冷启动重 user（30+ projects）首次连 SSH 仍 1.5s wall——是否需要 PR-D 内加更激进的 paginated cold start？** —— 当前 first page 默认 50 sessions 跨 10+ projects；可考虑 first page 缩小到 10-15 / 5 projects 让首屏快。但 UI 联动 + first page 大小决策属 ux 范畴。**留 follow-up** 让 SSH 用户实测后决定。
