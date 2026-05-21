## Context

`FileSystemProvider` trait（`crates/cdt-discover/src/fs_provider.rs`）2025 年随 `port-project-discovery` change 引入，是为了让 ssh-remote-context port 不改 `ProjectScanner` 业务代码就能复用。但抽象**只在 `ProjectScanner` / `ProjectPathResolver` 内**落地，cache 层（`MetadataCache` / `ParsedMessageCache` / `is_file_stale`）和 `cdt-api/src/ipc/local.rs` 的 30+ 处 IO 全绕过 trait，硬编码 `tokio::fs::*` 直调；调用方反复写 `if fs.kind() == Ssh { ... } else { ... }` 二选一分叉。

```
统计（2026-05-21 main = 9e193c9）：
- `is_remote` 分叉:                 18 处（PR #186 把数字从 9 翻倍）
- `tokio::fs::*` 直调（cdt-api 内）: 34 处
- 受影响 cache:                     2（MetadataCache / ParsedMessageCache）
```

用户报 SSH/HTTP 卡顿 5-10s（列表渲染 / 翻页 / 切项目），是本地 NVMe + 进程内 IPC 把"每次 IPC 重 scan / cache 永久 miss / 调用方暴力遍历"的浪费掩盖了。SSH 把 stat 50-100ms 放大 100-1000 倍后显形。

codex 异构二审（design 阶段前已跑一轮，agentId `afdf99285dd56713c`）指出 5 处漏洞：HTTP request 粒度不在 fs trait 解决范围、trait 缺分页语义未来仍会污染、`open_read` 不足以防 hot path 全量读、cache key 缺 `ContextId` 强语义、`FsError` 元方法不足。本 design.md 把 5 处漏洞内化为决策 D1-D8。

PR #186（`simplify-repository-as-project`）刚 merge，引入了**好基建**——`new_with_semaphore` 共享 read semaphore、build-time grep 拦 `ProjectScanner::new` 回归、k-way merge cursor 分页——但**没清理底层抽象债**，反而新增了 4 处 `is_remote` 分叉（`local.rs:1346-1462` 手工实现"SSH 路径骨架阶段直接 read_to_string + extract_metadata + inline emit"）。本 change 借鉴 PR #186 的 build-time grep 模式实现 `xtask check-fs-direct-calls`。

性能基线（`.claude/rules/perf.md`）：`list_repository_groups` 95ms / user-real=0.13 / RSS 59MB；`get_session_detail` 60-74ms。**本 change 是零业务变化的基建，不应影响这些 baseline**。

## Goals / Non-Goals

**Goals:**
- 把 `FileSystemProvider` trait 推到底——所有 fs 调用走 trait，cache 用 `FsMetadata` 而非 `std::fs::Metadata` 构造签名
- 消除 `cdt-api` / `cdt-config` 为拿 fs trait 而 import `cdt-discover` 的虚假依赖
- 建立"hot path 禁 N 次串行 stat + 业务代码禁直调 `tokio::fs`" 的契约 + CI gate
- 让 follow-up PR-B/C/D/E 有正确的基础设施可落，不再各自手工实现 cache adapter
- 全程不突破 `.claude/rules/perf.md` 的 wall/CPU/RSS 预算

**Non-Goals:**
- **不**改 `MetadataCache` 实现（PR-B 做）
- **不**改 `ParsedMessageCache` 实现（PR-C 做）
- **不**清理 18 处 `is_remote` 分叉 + 30+ 处 `tokio::fs` 直调（PR-D 做）
- **不**引入 `ProjectScanner` 结果 in-memory 复用（PR-E 做）
- **不**解决 SSH `Arc<Mutex<SftpSession>>` 全锁串行（PR-F 做）
- **不**实现 Tauri vs HTTP transport 抽象（更远期，但本 change 承认 HTTP backend `initial_load_policy: FullEager` 留锚点）
- **不**为 fs trait 加分页 / 排序语义（H5 显式声明 fs trait 是低层 FS API）

## Decisions

### D1: `FsMetadata.identity` 采 best-effort

**问题**：`FileSignature` 当前从 `std::fs::Metadata` 拿 Unix `(dev, ino)` 检测 rename-replace 同 size 同 mtime 的边界 case。cache 切到 fs trait 后，`FsMetadata` 现在只有 `{size, mtime}`，**Local 也会丢失 inode 检测能力**。

**修法**：`FsMetadata` 加 `identity: Option<FsIdentity>` 字段。`LocalFileSystemProvider` 在 Unix 上填 `Some(FsIdentity::Unix { dev, ino })`、Windows 填 `None`（stable Rust 拿不到 file index）；`SshFileSystemProvider` 永远填 `None`（SFTP 协议不暴露 inode）。

**Best-effort 风险**：SSH 上若用户 `mv` 一个 jsonl 替换另一个 sessionId 位置且 size 同 mtime 同 → cache 误命中。

**为何接受**：Claude Code 写 jsonl 是 append-only，rename-replace 不在威胁模型。codex 提出 stronger signature 方案（读头/尾 1KB hash）—— **否决**：每次 stat 多 1 个 RTT，SSH 上太贵；用户 2026-05-21 拍板选 best-effort。

**替代方案**：(a) 永远 None（Local 也退化）→ 否决：Local 损失能力；(b) hash head/tail → 否决：SSH 太贵；(c) 混合可配置 → 否决：复杂度不值。

### D2: trait 新建 `cdt-fs` crate，不留 cdt-discover 也不上移 cdt-core

**问题**：trait 当前住 `cdt-discover`。PR-B/C/D 把 cache + 业务路径切 trait 后，`cdt-api` / `cdt-config` 仅为拿 fs trait 而 import `cdt-discover` 是**虚假依赖**（这些 crate 不需要 discovery 业务逻辑）。

**修法**：新建 `crates/cdt-fs/`，搬迁 `FileSystemProvider` + `LocalFileSystemProvider` + `FsError` + `FsMetadata` + `FsKind` + `FsIdentity` + `DirEntry` + `EntryKind` + 新加的 `ContextId` + `BackendPolicy`。`cdt-discover` 用 `pub use cdt_fs::*` 一次性兼容老 import（不加 `#[deprecated]` 在本 change，避免业务路径未切完就被告警淹没；PR-D 完成后另开 cleanup PR 加 deprecated）。

**为何不上移 `cdt-core`**：codex 明确反对。`cdt-core` 当前承诺"sync 数据 + no runtime deps"——引入 async trait 即便 async_trait macro 不带 IO 也破契约（cdt-core 第一次有 async 接口）。新 crate 边界更清晰。

**为何不留 `cdt-discover`**：不消除虚假依赖等于没做"彻底"。用户 2026-05-21 拍板选新 crate。

**替代方案**：(a) 留 cdt-discover → 否决（虚假依赖）；(b) 上移 cdt-core → 否决（async 污染 sync crate）；(c) 新 crate `cdt-fs` → 选中。

**工程成本**：1-2 小时机械改动。cdt-fs Cargo.toml + 文件搬迁 + cdt-discover re-export + workspace.deps 一次完成。

### D3: `stat_many` SSH override 暂用 default `join_all`，真 SFTP pipeline 留 PR-F

**问题**：trait 加 `stat_many(&[&Path]) -> Vec<Result<FsMetadata, FsError>>` batched API（H2 hot path 规则的执行基础）。Local 走 `join_all` 真并发，但 SSH 端 `RusshSftpClient { sftp: Arc<Mutex<SftpSession>> }` 全锁串行 —— `join_all` 50 个 stat 全排队进同一 Mutex，实际仍是 50 次串行 RTT。

**修法**：trait `stat_many` default 实现走 `join_all`；`LocalFileSystemProvider` 不 override（default 即真并发）；`SshFileSystemProvider` **暂不 override**，用 default 实现，但 trait 文档 + design 明确："SSH `stat_many` 当前是假 batch，因为底层 `Arc<Mutex<SftpSession>>` 全锁串行。真 SFTP message-id 并发 pipeline 留 PR-F 处理。"

**为何接受暂时假 batch**：trait API 先就位让 PR-B/C/D 调用方一致写 `stat_many` 而非散落写 `for path in paths { fs.stat(path).await? }`。等 PR-F 解开 session 锁后，SSH override 自然受益。如果**没有这个 API**，调用方就只能继续写循环，PR-F 来时再批量重构调用方代价更大。

**替代方案**：(a) 强行本 PR 解 session 锁 → 否决（session 锁重构需深入 russh-sftp API 设计，独立 PR 更清晰）；(b) 不加 stat_many 等 PR-F → 否决（调用方写 for-loop 形成新债）。

### D4: `open_read` 返回 `Box<dyn AsyncRead + Send + Unpin>` 动态分发

**问题**：`SshFileSystemProvider::open_read_stream`（`crates/cdt-ssh/src/provider.rs:156-166`）是 inherent 方法，返 `russh_sftp::client::fs::File` 具体类型。注释明确写"不在 trait 中，因为返 SFTP 特定类型，跨 trait 抽象会引入类型耦合"。结果：调用方要流式读必须 downcast，破抽象。

**修法**：trait 加 `async fn open_read(&Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, FsError>`。`LocalFileSystemProvider` 包装 `tokio::fs::File` 用 `Box::new`；`SshFileSystemProvider` 包装 `russh_sftp::client::fs::File` 用 `Box::new`。动态分发 overhead 是 vtable lookup 几 ns，相对 SFTP 50ms RTT 完全可忽略；Local 上对 ~MB 级 jsonl streaming 也不是 hot path 瓶颈。

**为何不用关联类型（type Reader: AsyncRead）**：那会让 `&dyn FileSystemProvider` 失去 object safety，无法 `Arc<dyn FileSystemProvider>` 注入。本仓库 trait 设计就是要支持 dyn dispatch。

**替代方案**：(a) 关联类型 → 否决（破 dyn safety）；(b) 让 `SshFileSystemProvider::open_read_stream` 继续 inherent → 否决（破抽象，调用方被迫 downcast）；(c) `Pin<Box<dyn AsyncRead>>` → 选 `Box<dyn AsyncRead + Send + Unpin>` 即可（`Unpin` 让调用方可直接 `BufReader::new(reader).lines()` 不需 pin_mut）。

**性能量化要求**（codex R2 第 4 项）：本 change tasks SHALL 包含 Local 端 micro benchmark——同 jsonl 文件（典型 ~500KB 与 ~5MB 两个 size），对比 `tokio::fs::File::open + BufReader::lines` 直读路径 vs `FileSystemProvider::open_read` dyn 路径，跑 10 次取 min / median / stddev。dyn 路径 SHALL 在 median 上 ≤ 直读路径 × 1.3（vtable overhead 上限），超过则本 change 拒绝合并。具体 bench 入口在 tasks 11.7 之外另列 task 11.10。

### D5: `ContextId` 三元组 `(backend_kind, host_signature, root_or_home)`

**问题**（codex 第一轮高风险 #4 + 用户反复强调 + 第二轮 codex Blocking #1）：当前 `LocalDataApi` 持有 `metadata_cache: Arc<Mutex<MetadataCache>>` **单例**——跨 SSH host A → B 切换、HTTP server 切换、remote_home 变化都可能让同 path 字符串误命中旧 host 的 cache entry。

**修法**：在 `cdt-fs` 定义 `ContextId` 类型：

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContextId {
    pub backend_kind: FsKind,
    pub host_signature: Option<HostSignature>,   // None for Local
    pub root_or_home: PathBuf,                   // ~/.claude/projects/ for Local; <remote_home>/.claude/projects/ for Ssh
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostSignature {
    /// SHA-256 hash of the resolved ssh config 影响连接行为的字段集合（见 D5b 详情）
    pub config_digest: [u8; 32],
    /// 可读 label（仅展示 / 日志用，**不**参与 Hash/Eq）
    pub display_label: String,
}
```

`ContextId` 是所有 cache key 的**前缀**：`cache_key = (ContextId, path)`。`MetadataCache` / `ParsedMessageCache` / 任何 fs-related cache 切 trait 时（PR-B/C）SHALL 强制 key 加 `ContextId`，避免跨 host 串扰。

**前置到本 change 而非 follow-up**：codex 明确"如果不在 PR-A 钉死，PR-B 会把旧单例 bug 固化进新接口"。

**为何这个三元组够**：
- `backend_kind` 区分 Local / Ssh（防止 Local `~/.claude/projects/foo` 误命中 SSH 同路径）
- `host_signature` 区分多 SSH host 与同 host 不同 ssh config（A → B 切换、ProxyJump 配置变更、IdentityFile 切换等）
- `root_or_home` 区分 user 切 `claude_root` 配置 / 远端 multiple home fallback 命中不同目录

### D5b: `HostSignature.config_digest` 钉死为 resolved ssh config hash

**问题**（codex 第二轮 Blocking #1）：第一稿 design 写 "host_signature 倾向 `user@host:port`，PR-B 实施时若发现需要按 RemoteConfig 完整 hash 再调整"——但 spec 已经把 `user@host:port` 写成 SHALL，spec 与 design 自相矛盾。同一 `user@host:port` 但不同 `IdentityFile` / `ProxyJump` / Host alias 的 cache 串扰是真实风险（典型场景：用户用 ProxyJump 访问 IP 同 22 端口但跳板机不同 = 不同 host，但 `user@host:port` 视为同一）。

**钉死决策**：`host_signature.config_digest` SHALL 是 SHA-256 hash，hash 输入是按以下字段排序拼接的字节序列（来自 `ssh -G <alias>` 的 resolved config 输出）：

1. `hostname` — 解析后的实际 IP/hostname（非 alias）
2. `port`
3. `user`
4. `identityfile`（全部，按字典序排序后拼接）
5. `proxyjump`（如有）
6. `proxycommand`（如有）
7. `hostkeyalias`（如有）

**不**参与 hash 的字段（这些是连接行为参数，不影响"是否同一远端机器"判定）：`connecttimeout` / `serveraliveinterval` / `compression` / `loglevel` / `userknownhostsfile` 等。

`display_label` 取 `user@hostname:port` 作为人类可读 label，仅用于日志 / UI 显示，不参与 `PartialEq / Hash`。

**为何不用裸 `user@host:port` 字符串**：codex Blocking #1 反例——同 `user@host:port` 不同 ProxyJump 应该是不同 context，但裸字符串视为同一。

**为何不 hash 整个 ssh_config 文件**：连接无关字段（如 `loglevel`）变化时不应让全部 cache 失效，浪费。

**为何不用 `connection_id` 随机 UUID**：每次 reconnect 把 cache 全废，浪费；本 change 目标就是让 cache 跨 reconnect 复用同 host 数据。

**实施约束**：`HostSignature::from_ssh_config_fields(input: &SshConfigDigestInput) -> Self` 在 cdt-ssh 内的 `SshSessionManager::connect` 路径上构造，与 `SshFileSystemProvider` 注入 `LocalDataApi` 同一时机。Local backend 永远 `host_signature: None`。

### D5b-i: `SshConfigDigestInput` 边界与 cdt-ssh `ResolvedHost` 扩字段

**问题**（codex 第三轮 Blocking A）：第二稿写"`HostSignature::from_resolved_config(config: &ResolvedSshConfig)`"，但 cdt-ssh 现有 `ResolvedHost { host, port, user, identity_agent, identity_files, degraded }` 没有 `proxyjump / proxycommand / hostkeyalias` 字段；如果在 cdt-fs 自定义 `ResolvedSshConfig` 类型又与 cdt-ssh 冲突（同名 / 字段不齐）。

**钉死决策**：

1. **cdt-fs 内**定义最小 input 类型 `SshConfigDigestInput`（不引用任何 cdt-ssh 类型）：

```rust
// in cdt-fs/src/context_id.rs
pub struct SshConfigDigestInput {
    pub hostname: String,
    pub port: u16,
    pub user: String,
    pub identity_files: Vec<PathBuf>,        // 调用方传入前已字典序排序
    pub proxyjump: Option<String>,
    pub proxycommand: Option<String>,
    pub hostkeyalias: Option<String>,
}
```

`SshConfigDigestInput` 只是 `from_ssh_config_fields` 的入参形状定义，**不**是"resolved ssh config 的真相源"——真相源仍在 cdt-ssh `ResolvedHost`。

2. **cdt-ssh 扩展 `ResolvedHost` 字段**：在 `host_resolver.rs` 的 `ResolvedHost` 加 `proxyjump: Option<String>` / `proxycommand: Option<String>` / `hostkeyalias: Option<String>` 三个字段，从 `ssh -G` 输出解析（这三个字段 `ssh -G` 都会输出）。退化路径 `cdt-ssh::config_parser` 不解析这三个字段时 SHALL 设为 `None`（degraded 模式 host_signature 仍可计算但不含这些字段，避免阻塞连接）。

3. **从 `ResolvedHost` 到 `HostSignature` 的转换**：在 cdt-ssh 内（不在 cdt-fs 内，避免反向依赖）实现 `impl From<&ResolvedHost> for SshConfigDigestInput`，然后调 `HostSignature::from_ssh_config_fields(&input)` 算 digest。

**为何不在 cdt-fs 内复制 cdt-ssh 类型**：cdt-fs 不依赖 cdt-ssh（spec H1 要求 cdt-fs 不引用业务 crate）；类型来自上层（cdt-ssh）通过 input struct 注入是单向依赖，干净。

**ssh-remote-context spec 联动**（同 codex 第三轮 High E）：`ssh-remote-context` spec MODIFIED Requirement "Resolve SSH host alias via ssh -G"——加 SHALL "ssh -G 解析输出 SHALL 含 `proxyjump` / `proxycommand` / `hostkeyalias` 字段提取，`ResolvedHost` 含这三个字段"。退化路径下 SHALL 仍能产 `host_signature`（三字段为 `None`），不阻塞连接。

**ssh config 变化后 cache invalidation 时机**（codex 第三轮 High E 后半）：
- 用户改 `~/.ssh/config` → 下次 `ssh_resolve_host(alias)` 解析得到新 `ResolvedHost` → 新 `config_digest` → 新 `ContextId` → 自然不命中旧 cache entry（依赖 Hash/Eq 隔离）→ 旧 entry 走 LRU + TTL 自然淘汰
- **不**需要主动 invalidate / 不需要 watch ssh config 文件——因为 ContextId 是 cache key 一部分，digest 变即等价于"新 cache namespace"

**替代方案**：(a) cdt-fs 直接 ref cdt-ssh `ResolvedHost` → 否决（cdt-fs 引 cdt-ssh = 反向依赖 + 破 H1）；(b) 不在本 change 扩 `ResolvedHost`，PR-B 时再补 → 否决（PR-B 切 MetadataCache 时 ContextId 已 wire 到 cache key，`ResolvedHost` 缺字段会让 host_signature 永久退化模式，违反 D5b 设计）；(c) 把 `proxyjump` 等放可选 hash 输入 → 否决（破"resolved ssh config 的稳定 digest"语义）。

### D5c: cache 拓扑钉死为"单实例 + ContextId key prefix"

**问题**（codex 第二轮 Blocking #3）：design 第一稿只说 `cache_key = (ContextId, path)`，没规定 `MetadataCache` 是"单实例多 ContextId key prefix" 还是"每 ContextId 一个 cache 实例"。PR-B 若自由发挥拆成多实例，LRU 容量 / 清理语义 / `switch_context` 时序都会碎片化。

**钉死决策**：所有 fs-related cache（`MetadataCache` / `ParsedMessageCache` / 未来 `ProjectScanner` 结果 cache）SHALL 采用"**单实例 + key 含 ContextId 前缀**"拓扑：

- `LocalDataApi` 持有**一个** `Arc<Mutex<MetadataCache>>` 实例（保持 PR #186 起的现状）
- cache key 类型升级为 `(ContextId, PathBuf)`（PR-B 时改 `HashMap<PathBuf, _>` → `HashMap<(ContextId, PathBuf), _>`）
- LRU 容量按**全局**计算（200 → PR-B 时建议提到 2000；详 follow-up roadmap）
- `switch_context` 时**不必清 cache**——不同 context 的 key 自然不命中；TTL + signature 校验仍生效
- `ssh_disconnect` 时可选择"清该 ContextId 的所有 entry"或"保留等 LRU 自然淘汰"——本 change 不强制，由 PR-B 在 spec scenario 决定

**为何不每 ContextId 一个实例**：(a) 跨 context 的 LRU 全局策略碎片化（多实例总容量难算）；(b) ContextId 切换频繁时 cache 反复创建销毁，浪费；(c) HashMap key 加前缀的内存开销可忽略（PathBuf 已经几十字节，ContextId 加 32+ 字节）；(d) 调试时单一 cache 实例更易观测。

**替代方案**：(a) per-context 实例 → 否决（碎片化）；(b) 不加 ContextId key 仅用 path（现状）→ 否决（跨 host 串扰）。

**替代方案对 D5**：(a) cache key 只用 path（现状）→ 否决（跨 host 串扰）；(b) 加 `connection_id` 随机 UUID → 否决（每次 reconnect 把 cache 全废，浪费）；(c) 加 `last_reconfigure_timestamp` → 否决（复杂、需手动 invalidate）。

### D6: H3 业务策略层 `fs.kind() == Ssh` 收窄为"只选 policy 不复制算法"

**问题**（codex 第一轮高风险 #8 + 第二轮 Medium #6）：原 H3 措辞"业务策略层 LocalDataApi 内允许 `fs.kind() == Ssh` 分叉但 SHALL ADR"太宽——容易膨胀成"策略层"塞 SSH 特化的解析 / 排序 / 过滤 / cache key 计算；且 codex 第二轮指出"PR-D reviewer 在中间形态上会争议，design 应附 18 处分叉初步分类"。

**修法**：H3 收窄为以下硬约束：
- 业务**算法**代码（cache 实现、parser、grouper、sort 比较器、过滤谓词、cache key 构造）：`fs.kind()` 一律拒
- 业务**策略**层（`LocalDataApi`）：`fs.kind()` 允许但 SHALL ADR + inline 注释，且**只允许选 `BackendPolicy` 字段值**（如 `initial_load_policy: FullEager`），**不允许**复制业务算法
- 违反判据（PR review checklist + 未来 xtask 扩展）：算法分叉 = 同一算法在 `if Ssh / else` 两路径里写两遍

**现有 23 处 `fs.kind() == Ssh` / `is_remote` 分叉初步分类**（grep `crates/cdt-api/src/ipc/local.rs` @ 9e193c9）：

| line | 函数 / 上下文 | 初步分类 | 落地处理建议 |
|---|---|---|---|
| 806, 842 | `list_sessions_skeleton` cache lookup 旁路 | **算法分叉** | PR-D 切 trait + ContextId 后自然消除 |
| 1346, 1403, 1412, 1462 | `list_sessions_skeleton` SSE inline emit vs SSE-only | **策略分叉** | PR-E 上移到 `BackendPolicy::initial_load_policy` |
| 1897, 1929 | `get_session_detail` path 处理 | **算法分叉** | PR-D 走 `fs.open_read` 后消除 |
| 1964, 1971 | `get_session_detail` fallback chain | **算法分叉** | PR-D 走 trait 后消除 |
| 2003, 2019, 2033 | `get_session_detail` messages / candidates / is_ongoing | **算法分叉** | PR-D 走 trait 后消除 |
| 2187, 2189 | `get_session_summaries_by_ids` | **算法分叉** | PR-D 走 trait 后消除 |
| 2257, 2258 | session detail content 读 | **算法分叉** | PR-D 走 `fs.read_to_string` 消除 |
| 2360, 2361, 2422, 2423 | subagent scan | **算法分叉** | PR-D 走 `fs.read_dir` + `open_read` 消除 |
| 2916, 2926 | `list_repository_groups` 远端兼容 | **算法分叉** | PR-D 走 trait 后消除 |

**初步分类汇总**：~17 处算法分叉（H3 拒，PR-D 强制消除）+ ~4-6 处策略分叉（H3 允许，PR-E 上移到 `BackendPolicy`）。

**为何这是"初步分类"而非"最终分类"**：本 change 不动业务代码，无法在 apply 时验证；PR-D apply 时 reviewer SHALL 用此表作起点，逐行复核 + 在 PR-D 的 design.md 里固化最终分类（每行标 `algorithm` / `policy` / `temporary-workaround` + 对应 ADR 锚点）。

**为何不在本 change 直接消除**：PR-D 才做，本 change 只钉契约 + 提供初步清单减少 PR-D 时的 ambiguity。

**替代方案**：(a) 完全禁止业务代码 `fs.kind()` → 否决（部分策略真需要分叉，如 HTTP eager vs Tauri skeleton-then-SSE，否则必须开两套 IPC method 污染更大）；(b) 完全允许只口头规范 → 否决（已被 PR #186 证伪：开新功能时分叉数从 9 翻倍到 23）。

### D7: `xtask check-fs-direct-calls` 与 `build_time_invariants` 并存，allowlist 单源

**问题**（codex 第二轮 Medium #7）：第一稿写"复用 build_time_invariants 模式"含糊——`build_time_invariants` 是 `crates/cdt-api/tests/build_time_invariants.rs` **集成测试**（grep `ProjectScanner::new`），与 xtask **独立 binary**（grep `tokio::fs::*`）是不同机制；同时存在两套机制需明确 CI 入口 + allowlist 是否单源。

**钉死决策**：

**1. 两套机制并存，职责不同**：
- `build_time_invariants` 集成测试：保留 PR #186 现状，专注"特定 API 调用形态"（如 `ProjectScanner::new` 不准在生产代码出现）的回归拦截。本 change **不动**这套机制。
- 新增 `xtask check-fs-direct-calls`：独立 binary，专注"业务路径不准直调 `tokio::fs::*`"的硬约束 enforce。本 change 新建。

**2. allowlist 单源**：两套机制各自的"允许路径"清单 SHALL 统一住在 `crates/cdt-fs/ALLOWLIST.md` 的 `## Allowlist` 段（markdown table 格式）。

- `build_time_invariants` 测试代码内**引用** allowlist（如 `include_str!("../../cdt-fs/ALLOWLIST.md")` 然后解析 table，或在测试代码顶部加 doc-comment 链接 + 手工同步并加 build-time assert "本测试 allowlist 与规则文件一致"）
- `xtask check-fs-direct-calls` 同上读取 `crates/cdt-fs/ALLOWLIST.md` + parse table
- 任何 allowlist 增删 SHALL 改 ALLOWLIST.md，**不**改测试代码 / xtask 源码
- **为何不放 `.claude/rules/`**：`.claude/rules/*.md` 每会话自动加载（违反 30 行红线 + 本质上是 crate-local 数据），allowlist 是配置数据不是跨域操作纪律

**3. CI 入口**：两套都跑，`build_time_invariants` 走 `cargo test --workspace`，xtask 走单独 `cargo xtask check-fs-direct-calls` step。本 change CI 阶段 xtask 默认 `--warn-only`，PR-D 完成后另开 PR 切 fail-on-match（task 8.7 已记录）。

**为何不把 xtask 也做成集成测试**：(a) xtask 命令是手动诊断入口（`cargo xtask check-fs-direct-calls` 直接出报告），集成测试只在 `cargo test` 跑；(b) xtask 退出码语义更清晰（fail-on-match 是 binary 应有行为）；(c) 集成测试 + binary 各擅其职，并存不冲突。

**何时启用 enforce**：本 change 实现 xtask + 文档化，但**首次运行允许 warning 不 fail**——因为现有 30+ 处直调还在（PR-D 才清）。本 change tasks.md 标记"PR-D 完成后切换 xtask 为 fail-on-match"。

### D8: `BackendPolicy` 命名按 PR #186 line 1402-1411 实际语义，正交字段预留

**问题**（codex 第一轮中风险 #10 + 第二轮 Low #8）：早期方案叫 `BackendStrategy::{Eager, SkeletonWithSse}` 是比喻，读不懂；第二轮指出 `prefetch_next_page` 更像独立 prefetch 维度而非 `InitialLoadPolicy` 的第三 variant，扩展模型不完整。

**修法**：按 PR #186 `local.rs:1402-1411` 已有的"if is_remote { read_to_string + extract_metadata_from_parsed + inline emit }" 实际语义命名，且字段维度正交：

```rust
pub struct BackendPolicy {
    /// 首屏列表加载策略：FullEager 一次拿全 vs SkeletonThenStream 先骨架后增量
    pub initial_load_policy: InitialLoadPolicy,
    /// 初始页能接受的最大 round trips 数
    pub max_round_trips_for_initial_page: u8,
    /// 是否支持服务端推送（SSE / Tauri event）
    pub supports_incremental_updates: bool,
    /// 未来扩展锚点：是否启用下一页预取（PR-E 及之后扩展时填入实际值）
    pub prefetch_policy: PrefetchPolicy,
}

pub enum InitialLoadPolicy {
    /// 一次性同步等所有元数据 fetch 完才返回 (HTTP / SSH 默认)
    FullEager,
    /// 先返回骨架 + 后续 SSE 增量补全 (Local Tauri 默认)
    SkeletonThenStream,
}

pub enum PrefetchPolicy {
    /// 不主动预取下一页（本 change 默认值，所有 backend 均使用）
    None,
    /// 主动预取下一页（PR-E 及之后才可能引入）
    PrefetchNext,
}
```

**正交性**：`InitialLoadPolicy` 表达"首屏策略"（一次性 vs 增量补全），`PrefetchPolicy` 表达"翻页预取策略"（不预取 vs 预取下一页）。二者独立——HTTP 可以 `FullEager + None`，未来某个 high-bandwidth 桌面端可能 `SkeletonThenStream + PrefetchNext`，组合自由。

**反例（codex Low #8）**：禁止把 `PrefetchNext` 塞进 `InitialLoadPolicy` 当第三 variant——那是不同维度，违反正交。

**为何本 change 就引入 `PrefetchPolicy` 而非更晚**：D8 的核心约束是"扩展模型完整"——如果只引入 `InitialLoadPolicy`，PR-E reviewer 看到 `prefetch_next_page` 需求时会想"扩展 InitialLoadPolicy 第三 variant 还是新加字段"，争议成本高；本 change 预留 `prefetch_policy` 字段 + `PrefetchPolicy::None` 默认值，使将来扩展只是改字段值，不改类型。

**本 change 不 wire 到业务**：只定义 + 单测验证 enum 完整性。PR-E 才真正让 `LocalDataApi` 持有 `BackendPolicy` 字段并按值改 behavior。所有 backend `prefetch_policy` 本 change 均设 `None`（PR-E 引入实际预取逻辑时才有 backend 切到 `PrefetchNext`）。

**替代方案**：(a) 不在本 change 引入 → 否决（H4 锚点要落，否则 PR-E 时再设计来不及）；(b) 直接 wire 到业务 → 否决（本 change 守"零业务变化"边界）；(c) 只引入 `InitialLoadPolicy` 不加 `PrefetchPolicy` → 否决（codex Low #8 反对，扩展模型不完整）。

### D9: H1-H6 各自 enforce 方式钉死

**问题**（codex 第二轮 High #9）：spec 7 个 Requirement 与 H1-H6 契约的 enforce 方式混用——部分有 xtask（H1）、部分靠规则文件 + 人工 review（H2/H3）、部分有单测覆盖（H6 元方法）——但 spec 全部写成 SHALL，不区分"自动可验证 vs 人工 checklist"，reviewer 无法判断"违反 H2 时 CI 会拒还是仅靠 review 拦"。

**钉死决策**：每条 H1-H6 的 enforce 方式 SHALL 在 `openspec/specs/fs-abstraction/spec.md` 的 Requirement 内明示（不依赖独立散文档），对应 spec scenario 也 SHALL 标注 enforce 机制：

| 契约 | 描述 | Enforce 机制 | 触发时机 |
|---|---|---|---|
| **H1** | 业务路径禁直调 `tokio::fs::*` | `xtask check-fs-direct-calls`（本 change 实现，warn-only；PR-D 后切 fail-on-match） | CI step + 本地 `cargo xtask` |
| **H2** | hot path 禁 N 次串行 `fs.stat / read` | (a) `FsOpCounter` instrumentation 输出 tracing histogram（本 change 提供基础设施）；(b) 集成测试用 fake provider 断言 fs op 上限（PR-B/C/D 时按 IPC command 加测）；(c) PR review checklist | 本 change 阶段只有 (c) 可立即生效，(a)(b) 是 PR-B 起的能力 |
| **H3** | 业务算法禁 `fs.kind() == Ssh`；策略层允许但 SHALL ADR | (a) PR review checklist 按 D6 分类表逐行复核；(b) 未来可扩展 xtask 加规则 `algorithm-level fs.kind() detection`（高级模式匹配，本 change 不实现） | PR review |
| **H4** | HTTP backend 默认 FullEager + max_round_trips=1 | (a) `BackendPolicy::for_http()` 单测断言（本 change 实现）；(b) PR-E wire 时单测断言 `LocalDataApi(http_mode).policy == BackendPolicy::for_http()` | 本 change + PR-E |
| **H5** | fs trait 不承担分页 / 排序 | (a) 集成测试 `fs_trait_no_pagination_methods`（grep trait 方法签名禁含 `Cursor/Offset/SortBy/Order`，本 change 实现） | CI step（cargo test）|
| **H6** | `FsError` 必须可操作 | (a) 单测覆盖每个 variant 的 `is_retryable / should_invalidate_cache`（本 change 实现） | CI step（cargo test）|

H2 短期 enforce 弱（只有人工 review），但 instrumentation 基础设施 + spec scenario 落地后，PR-B/C/D 进 review 时 reviewer 可立即按 spec 加 `assert_fs_op_count(list_sessions) <= 50` 类断言。

**为何本 change 不为 H2 做完整自动 enforce**：H2 真完整 enforce 需要"每个 IPC command 定义 fs op 上限 budget"——属于业务路径决策，PR-B/C 时按 command 加；本 change 只提供 counter + 单测能跑的入口。

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| `cdt-fs` 新建 crate 引入 import path 大规模改动，PR diff 大 | 用 `pub use cdt_fs::*` 在 cdt-discover 兼容老 import，本 change 不强制现有 use 路径迁移；deprecated 提示留 PR-D 后另开 cleanup PR |
| `FsMetadata` 加字段破坏现有构造点 | `identity: Option<FsIdentity>` 是 `Option` 默认 `None`，所有现有构造点 `FsMetadata { size, mtime }` 编译破——但**这是好事**，强制审视所有构造点；预计 ~5-10 处（grep 拉清单一轮 Edit 补齐） |
| `FsError` 加 variant 影响 exhaustive match | 已 grep 确认全仓只有 `matches!(_, NotFound)` 和 `Err(FsError::NotFound(_)) if ssh` 单守卫，无 exhaustive match；加 variant 编译兼容 |
| `Box<dyn AsyncRead>` 动态分发 overhead | vtable lookup 几 ns，相对 SSH 50ms RTT / Local jsonl read 几 ms 完全可忽略；本 change 加测试断言"open_read 在 Local 上 ≤ 1.5x 原 tokio::fs::File 性能"防止重大退化 |
| stat_many SSH 假 batch 让 PR-F 拖延 | 在 trait 文档 + design.md D3 + spec scenario 显式记"SSH override 待 PR-F"；本 change tasks.md 末尾加 explicit follow-up checkbox 提醒 |
| xtask 启用时机：现有 30+ 处直调还在，立即 enforce 会 fail CI | 本 change xtask 实现后**只 warning 不 fail**，PR-D 完成后另开 PR 切换 enforce 级别 |
| `ContextId` 设计错引发 PR-B 重做 | 本 change 加 `ContextId` + `HostSignature` 单测覆盖：(a) Local vs SSH 不等价；(b) 同 host_signature 不同 home 不等价；(c) 不同 host_signature 同 home 不等价；(d) `display_label` 不参与 Hash/Eq；(e) `HostSignature::from_resolved_config` 输入字段顺序无关；(f) Hash + Eq + Clone 满足 cache key 需求 |
| 业务策略层"只选 policy 不复制算法" 判据模糊 | spec.md Requirement H3 列具体反例（"if Ssh { sort by mtime } else { sort by size }" 是复制算法 = 拒；"if Ssh { initial_load_policy: FullEager } else { SkeletonThenStream }" 是选 policy = 允许）；design.md D6 附 23 处分叉初步分类表减少 PR-D ambiguity |
| 性能基线（baseline 95ms）退化 | 本 change 零业务变化，理论上 baseline 不动；tasks.md 强制 `cargo test --release --test perf_cold_scan` apply 前后各跑 **5 次**取 min + median + stddev（codex 第二轮 Medium #11：3 次 min 不足以排除 5% 噪声），回归 median > 5% 或 stddev > 8ms 拒；另外加 Local micro benchmark（D4 量化要求）对比 `tokio::fs::File` 直读 vs `fs.open_read` dyn 路径，dyn ≤ 直读 × 1.3 |
| `HostSignature.config_digest` 计算错让 cache 失效或串扰 | 本 change 加单测：(a) 完整 ssh -G 配置 → 稳定 hash；(b) 同 `user@host:port` 但不同 IdentityFile → 不同 hash；(c) 改 `loglevel` / `compression` 不影响 hash（连接无关字段过滤）；(d) `display_label` 不参与 hash | 
| cache 拓扑选错（多实例 vs 单实例 + 前缀）| design D5c 钉死单实例 + ContextId key prefix；本 change 不动 cache 实现，但 spec Requirement 把"单实例多 key 前缀"作为 PR-B 必须遵循的 SHALL 句

## Migration Plan

本 change 是基建，**零业务变化**——无运行时迁移。

**部署顺序**（CI / 本地 dev）：
1. `cdt-fs` crate 建好 + 老 import path 通过 `pub use` 兼容 → 全 workspace 编译 OK
2. trait 4 缺口补齐 + SshFileSystemProvider impl trait 新方法 → 现有 cache / 调用方继续走旧路径
3. `xtask check-fs-direct-calls` warning-only 启用，`crates/cdt-fs/ALLOWLIST.md` 落地作为 allowlist SSOT
4. `openspec/specs/fs-abstraction/spec.md` 发布（archive 后 sync 到主 spec）

**回滚**：本 change 全部改动在新 crate + trait 默认实现内，回滚 = revert PR；现有业务代码完全不依赖新加的 trait 方法。

**Follow-up roadmap**（不在本 change，但 design.md 记录方向防漏）：
- PR-B：`MetadataCache` 切 fs trait + `ContextId` 强制 key（解 SSH 列表卡顿核心）
- PR-C：`ParsedMessageCache` 切 fs trait
- PR-D：清 18 处 `is_remote` 分叉 + 30+ 处 `tokio::fs` 直调；xtask 切 fail-on-match
- PR-E：`ProjectScanner` 结果 in-memory 复用 + `BackendPolicy` wire 到业务
- PR-F：SSH `Arc<Mutex<SftpSession>>` 锁解开，`stat_many` 真 pipeline

## Open Questions

1. **`cdt-fs` 是否独立 publish？** —— 当前仓库不 publish 任何 crate（all `publish = false`）；本 change 保持现状，不 publish。如未来需要让外部项目复用 fs trait（如 cdt-server 单独跑），再评估。

2. **`ContextId.host_signature` 的具体格式？** —— ~~倾向裸字符串~~ **第二轮 codex 评审已钉死**：D5b 决定 `HostSignature.config_digest` SHALL 是 resolved ssh config 的 SHA-256 hash（含 `hostname` / `port` / `user` / `identityfile` / `proxyjump` / `proxycommand` / `hostkeyalias` 字段，按字典序排序后拼接）；`display_label` 取 `user@hostname:port` 仅展示用。本 question closed。

3. **xtask 启用时机切换的具体触发条件？** —— 本 change tasks.md 写"PR-D 完成且 `xtask check-fs-direct-calls` 在 dev 机器零 warning 后另开 PR 切 fail-on-match"；具体哪个 commit 切换由 PR-D 完成时再评估。

4. **是否本 change 加 `read_range(offset, len)` API？** —— codex 提到 `open_read` 不足以防 hot path 全文读，建议补 `read_range`。**本 change 不加**——`open_read + BufReader::take(N)` 已能等价；真有需求 PR-F 时再加。
