## Context

PR-A（change `unify-fs-abstraction`，2026-05-21 archive）落地了 `cdt-fs` crate：`FileSystemProvider` trait（含 `open_read` / `stat_many` 等抽象方法）、`ContextId` / `HostSignature` / `SshConfigDigestInput` 类型、`BackendPolicy` enum、`InstrumentedFs` wrapper、`xtask check-fs-direct-calls` warn-only 守护。`fs-abstraction` spec 钉死了三条 SHALL 句作为 PR-B-F 的契约：

```
Requirement: ContextId 三元组作为 cache key 前缀
  → 任何 fs-related cache SHALL 把 ContextId 作为 key 的一部分
Requirement: fs-related cache 必须采用"单实例 + ContextId key 前缀"拓扑
  → 单实例 / key 含 ContextId 前缀 / LRU 全局 / switch_context 不清 cache
  → 本 change 不改 MetadataCache 现状（PR-B 才动）
```

当前 `MetadataCache`（`crates/cdt-api/src/ipc/session_metadata.rs:379-433`）：
- `map: HashMap<PathBuf, MetadataCacheEntry>` —— **裸 PathBuf** key，跨 host 串扰
- `extract_session_metadata_cached` / `try_lookup_cached_metadata` 直接 `tokio::fs::metadata(path).await` + `FileSignature::from_metadata(&std::fs::Metadata)`（PR-A 已挂 `#[deprecated]`）
- `METADATA_CACHE_CAPACITY = 200` —— Local NVMe + SSH 远端 jsonl 共享同 cache 时 200 容量太挤，SSH 切回 Local 后 SSH entry 几次列表就被挤光

`LocalDataApi` 内 metadata cache 调用方共 6 处：
- `local.rs:822` `try_lookup_cached_metadata` (`get_session_detail` fast-path)
- `local.rs:890` `extract_session_metadata_cached` (`scan_metadata_for_page` 入参)
- `local.rs:1397` `try_lookup_cached_metadata` (`list_sessions_skeleton` fast-path)
- `local.rs:1681` `extract_session_metadata_cached` (`scan_metadata_for_page` 内部 spawn 任务)
- `local.rs:1756` `extract_session_metadata_cached` (`get_session_summaries_by_ids` 单 path 入口)
- `local.rs:1820` `extract_session_metadata_cached` (`get_session_summaries_by_ids` per-id 循环)

`SshSessionResources`（`crates/cdt-ssh/src/session.rs:108-122`）当前不保存 `HostSignature`——`ResolvedHost` 在 `connect_inner` 内只用一次后丢弃。PR-A 已加 `impl From<&ResolvedHost> for cdt_fs::SshConfigDigestInput`（`host_resolver.rs:68`），转换路径就绪。

性能基线（`.claude/rules/perf.md` + `tests/perf-baseline.json`）：本地 `perf_cold_scan` wall ≤ 500ms / user/real ≤ 0.6；`perf_get_session_detail` wall ≤ 500ms / user/real ≤ 0.7。本 change 仅改 cache key + stat 入口，cache 命中后 fs 访问被跳过，**理论不影响 baseline**——但仍 SHALL 在 apply 前后跑一次 5 runs 验证（详 Risks 段）。

## Goals / Non-Goals

**Goals:**
- `MetadataCache` 内部 key 升级为 `(ContextId, PathBuf)` tuple，LRU 容量 200 → 2000
- `extract_session_metadata_cached` / `try_lookup_cached_metadata` 通过 `FileSystemProvider::stat` 而非 `tokio::fs::metadata` 走 stat；签名加 `fs: &dyn FileSystemProvider` + `context_id: &ContextId` 参数
- `LocalDataApi` 持 `Arc<dyn FileSystemProvider>` + 运行时可变 `Mutex<ContextId>`；`switch_context` / `ssh_connect` / `ssh_disconnect` 同步更新 `current_context_id`
- `SshSessionResources` 持 `HostSignature`；`SshSessionManager::context_id(&str)` 暴露 `Option<ContextId>` 查询
- 新增 fake-SSH bench 验收 hit/miss ≥ 100×
- 顺修 `src-tauri/Cargo.lock` 与 workspace 不同步

**Non-Goals:**
- **不**改 `ParsedMessageCache`（PR-C）
- **不**清 18 处 `is_remote` 分叉 + 30+ 处 `tokio::fs::*` 直调（PR-D）
- **不**引入 `ProjectScanner` 结果 in-memory 复用 + `BackendPolicy` wire（PR-E）
- **不**解开 SSH `Arc<Mutex<SftpSession>>` 全锁串行（PR-F）
- **不**接入 `InstrumentedFs` / `with_fs_counter` 到业务路径（PR-E）
- **不**清理 `cache_signature.rs::from_metadata` 上的 `#[deprecated]` 在全仓的所有 callsite——本 change 仅在 metadata cache 路径切到 `from_fs_metadata`，其它路径 PR-D 才动
- **不**新增 spec capability，复用 PR-A `fs-abstraction` 已有 SHALL 句

## Decisions

### D1: cache key 选 `(ContextId, PathBuf)` 而非 newtype struct

**问题**：`HashMap` key 类型选 `(ContextId, PathBuf)` tuple 还是 `MetadataCacheKey { ctx: ContextId, path: PathBuf }` newtype struct？

**修法**：选 tuple `(ContextId, PathBuf)`。理由：

- PR-A `fs-abstraction/spec.md` §"fs-related cache 必须采用'单实例 + ContextId key 前缀'拓扑" Scenario "key 类型含 ContextId" 措辞 `"key 类型 SHALL 是 (ContextId, PathBuf) 或等价 newtype"`——tuple 是首选形态
- newtype 多一层间接 + `derive(Hash, Eq)` boilerplate，对单 cache 单 callsite 价值有限
- 未来 PR-C `ParsedMessageCache` 切 `(ContextId, PathBuf)` 时同样用 tuple，跨 cache 形态一致

**替代方案**：(a) newtype struct → 否决（无收益的间接层）；(b) `String` 拼接 → 否决（`ContextId` 内含 `[u8; 32]` digest，序列化为 hex 浪费 + 损失类型安全）

### D2: **不**为 PR-E 预留 `self.fs` 字段——`LocalDataApi` 不持 fs

**问题**：早期设计稿想在 `LocalDataApi` 加 `fs: Arc<dyn FileSystemProvider>` 字段作 PR-E `InstrumentedFs` 锚点；但 cache callsite 6 处都按"当前 active context"取 fs（`active_fs_and_context().await`），`self.fs` 字段在本 change 运行时不会被读——典型死字段 + 为假想未来预留。

**修法**：**不加** `fs` 字段。`LocalDataApi` 字段维持现状（不引入"作 PR-E 锚点而存在"的成员）。cache callsite 全部走 `active_fs_and_context().await` 拿当前 active provider（Local 走 `cdt_fs::local_handle()`，SSH 走 `Arc::new(provider)`），与现有 `active_fs_and_projects_dir` 一致。PR-E 引入 `InstrumentedFs` 时再决定如何 wire——可能也是在 `active_fs_and_context` 内部包装，与 `LocalDataApi` 字段无关。

codex 二审 D2 已指出：保留死字段违反"don't design for hypothetical future requirements"。本节修订采纳此意见。

**替代方案**：(a) 保留 `self.fs` 作锚点 → 否决（死字段污染 + 误导 reviewer）；(b) 加 `self.fs` 且在 Local 分支真用 → 否决（Local 分支当前直接 `local_handle()` 即 `Arc<LocalFileSystemProvider>`，与持 `Arc<dyn>` 字段等价但多层 indirection）

### D3: `current_context_id` 在 `active_fs_and_context` 内**就地合成**——**不**持 `Mutex<ContextId>` 字段

**问题**（codex 二审 D3 Blocking）：早期稿计划 `LocalDataApi` 持 `Mutex<ContextId>` 字段 + 在 `switch_context` / `ssh_connect` / `ssh_disconnect` 三处主动更新。但 `ssh_connect`「连接新 host 时强制 disconnect 旧 active」在 `cdt_ssh::SshSessionManager::connect` 内自动跑（`session.rs:323-326`）—— `disconnect(old)` 把 `active = None`，然后 `connect(new)` 才把 `active = Some(new)`。

中间存在并发窗口：`active_context_id() == None`（Local 状态）但 `LocalDataApi.current_context_id` 仍是旧 SSH `ContextId`。此时一个并发 IPC 调用 cache lookup → `active_fs_and_context()` 拿 fs（按 active = None 走 Local provider）但 ctx 字段返回的是旧 SSH `ContextId`——fs/ctx 不一致，cache hit 命中错误（如果路径字面相同），或永远 miss 但写入用旧 SSH ctx prefix，让 cache 污染。

**修法**：**不持 `current_context_id` 字段**；`active_fs_and_context()` 每次按当前 `ssh_mgr.active_context_id().await` 调用结果**就地合成** fs + ctx，保证 fs 与 ctx 来自同一快照：

```rust
async fn active_fs_and_context(&self) -> (Arc<dyn FileSystemProvider>, PathBuf, ContextId) {
    if let Some(ssh_id) = self.ssh_mgr.active_context_id().await {
        if let Some(provider) = self.ssh_mgr.provider(&ssh_id).await {
            let projects_dir = provider.remote_home().to_path_buf();
            let ctx = self.ssh_mgr.context_id(&ssh_id).await
                .unwrap_or_else(|| ContextId::local(projects_dir.clone()));
            return (Arc::new(provider), projects_dir, ctx);
        }
    }
    // Local 或 SSH provider 已被 drop（disconnect 中间态）→ 都走 Local
    let projects_dir = self.projects_dir.lock().await.clone();
    let ctx = ContextId::local(projects_dir.clone());
    (cdt_fs::local_handle(), projects_dir, ctx)
}
```

**关键不变量**：函数内**单一**入口 `self.ssh_mgr.active_context_id().await` 决定走 SSH 还是 Local 分支；同一 if 块内拿 provider + ctx，绝不会出现 "fs = Local + ctx = SSH"。Local 分支统一构造 `ContextId::local(projects_dir)`，无外部状态依赖。

**性能**：每次 cache callsite 多两次 `ssh_mgr.lock()` 调用（active + provider/context_id），单次锁争用 < 1µs；cache hit 路径整体 wall 仍由 fs.stat 主导（Local stat 几十 µs，SSH 50ms RTT）—— 锁争用占比 < 1%，可忽略。

**`switch_context` / `ssh_connect` / `ssh_disconnect` 不再需要触 cache 相关状态**——`ssh_mgr` 自身 atomic 切换 `active` 字段即可，本 change 这三处只需删掉旧设计中"同步更新 current_context_id"逻辑（即原 tasks 4.9-4.11），其它行为保持不变。

**替代方案**：(a) Mutex<ContextId> + 三处同步更新 → 否决（codex Blocking：disconnect/connect 中间态不一致）；(b) Mutex<ContextId> + 加锁 disconnect-connect 原子 → 否决（要把 ssh_mgr 内部 atomic 操作外推到 LocalDataApi，污染层次）；(c) 就地合成（本节版本）→ 选中

### D3-bis: `ssh_mgr.active_context_id` 与 `ssh_mgr.provider_and_context_id` 之间的 atomicity

**问题**：`active_fs_and_context()` 先调 `active_context_id().await` 拿 ID，再去拿 provider 与 ContextId。早期稿用两个独立 accessor（`provider(&id)` + `context_id(&id)`），三次独立 lock；codex 二审 commit-stage Blocking 指出：第二次与第三次 lock 之间若并发 disconnect/replace，provider 命中但 ctx 返 None，代码会返回 `(SSH provider, ContextId::local(remote_home))`——SSH/Local 混合不自洽，cache 写入会用 SSH provider stat 配 Local namespace。

**修法**：在 `SshSessionManager` 暴露原子 accessor `pub async fn provider_and_context_id(&self, context_id: &str) -> Option<(SshFileSystemProvider, ContextId)>`，单次 `sessions.lock()` 内同时返 provider + ContextId；二者要么同时 Some 要么同时 None。`active_fs_and_context` 调用此原子方法，None 时**整体** fall-through 到 Local 三元组——绝不返回 SSH/Local 混合。

**为何 active_context_id 与 provider_and_context_id 之间仍可能 race**：在 `active=Some` 与 `provider_and_context_id` 之间并发 disconnect 后，前者返 Some 但后者返 None——此场景由 `if let Some((provider, ctx)) = ...` 的 None 分支 fall-through 到 Local，与 `active=None` 同分支处理；安全降级，不产生混合。

**为何不把 active+provider+ContextId 全部塞进一把 sync lock**：会强制 `LocalDataApi` 持有 `ssh_mgr.active` 与 `sessions` 的私有引用，破封装；当前 `(active_context_id, provider_and_context_id)` 两次 lock 之间的窗口足够窄（μs 级），并发 disconnect 在此 race 的实际触发概率极低，安全降级语义已正确处理。

### D4: LRU 容量 200 → 2000，全局共享 pool

**问题**：PR-A spec §"LRU 容量按全局计算" 钉死全局 pool，但**没指定具体容量**。当前 200 是 PR `multi-session-cpu-cache`（2026 年）单 Local 场景下的取值。SSH 切换后多 ContextId 共享同 pool，200 容量很快被挤光。

**修法**：常量 `METADATA_CACHE_CAPACITY = 2000`。理由：

- typical user 总 session 数（含历史归档）500-2000；Local + 1-2 个 SSH host 同时持有 entry → 2000 够覆盖"日常切换不丢"
- 单 entry 内存粗算：`(ContextId, PathBuf)` ≈ 200 字节（PathBuf 平均 ~120 + ContextId 含 [u8;32] digest + PathBuf root_or_home + display_label）+ `MetadataCacheEntry` ≈ 200 字节（title 平均 ~80 + signature ~80 + 其它）≈ 400 字节/entry → 2000 entries ≈ 800KB，毫不敏感
- LRU bump 在 hit 时仍 O(N) on `VecDeque<PathBuf>::iter().position(==)` （当前实现）—— 2000 容量下平均查找 ~1000 步，~50µs，对 IPC wall < 500ms 预算可忽略
- 未来若仍不够，PR-C `ParsedMessageCache` 可考虑改 `LinkedHashMap` 让 LRU bump O(1)；本 change 不改 LRU 数据结构

**性能担忧验证**：cache **命中**路径不动 LRU bump 之外的 fs，wall 不会增；cache **未命中**路径调 fs.stat（替代 tokio::fs::metadata），Local 走 LocalFileSystemProvider 走 tokio::fs::metadata 经一层 trait dispatch ≈ 几 ns vtable lookup vs 几十 µs stat 本体——overhead < 0.1%。

**替代方案**：(a) 200 不动 → 否决（多 ContextId 共享时 SSH cache 易失效）；(b) 5000 / 10000 → 否决（占内存多，typical user 用不到）；(c) per-ContextId 子 LRU → 否决（违反 PR-A spec "LRU 容量按全局计算"）

### D5: `switch_context` / `ssh_connect` / `ssh_disconnect` **不**清 cache

**问题**：用户切 SSH host A → B 时，host A 的 cache entry 该怎么办？(a) 立刻清掉；(b) 保留等 LRU 自然淘汰；(c) ssh_disconnect 时清掉那个 host 的所有 entry

**修法**：选 (b) 保留等 LRU 自然淘汰。PR-A spec §"switch_context 时不必清 cache：不同 `ContextId` 的 entry 自然不命中（依赖 Hash/Eq 隔离），TTL + signature 校验照常工作"已钉死。

**`ssh_disconnect` 时同理保留**：用户可能很快 reconnect 同 host（典型场景：网络抖断了 reconnect、Wifi 切换），reconnect 后 `host_signature` 相同 → 同 `ContextId` → 复用旧 cache entry。如果 disconnect 时清掉，reconnect 后所有 session 列表 cold scan 一遍，UX 体验差。

**为何不担心断网期间远端文件变化**：cache 命中时仍走 `signature == sig`（`(mtime, size, identity)` byte-equal）校验；远端文件变 mtime/size → signature 不等 → miss → 重扫。signature 校验已经守住数据新鲜度。

**替代方案**：(a) switch / disconnect 时清整个 cache → 否决（loss principle，违反 PR-A spec）；(b) 清该 ContextId 的所有 entry → 否决（reconnect UX 差）

### D5-bis: 远端时钟回拨 + 远端重启的 false-positive 风险（codex 二审 D5 已澄清为 non-issue）

**问题**（codex 二审 D5 = non-issue 但需 design 明示）：远端 SSH host 重启后系统时钟回拨，远端 jsonl 文件 mtime/size 巧合不变 → cache stat 拿 signature 与缓存等价 → false-positive 命中。

**修法**：保留现有 `is_session_stale(signature.mtime, SystemTime::now())` 5min wall-clock 兜底。命中后 `is_ongoing` 字段按当前 wall clock 实时合成，远端旧 cache entry 若 mtime 落在 5min 前 → `is_ongoing = false`，UI 上会显示为 done session 而非 active；用户看到结果是"会话列表正常但状态字段保守"，不会触发 bug。底层数据陈旧的情况由用户后续任何写操作（追加消息 → mtime 变 → signature mismatch → cache miss → 重扫）自然恢复。这与 PR-A spec §"缓存命中后实时重算 stale 状态" Scenario 兼容，无需新加 SHALL。

### D6: `HostSignature` 存 `SshSessionResources` 而非每次 resolve

**问题**：拿 SSH 的 `ContextId` 需要 `HostSignature`。`HostSignature::from_ssh_config_fields` 输入是 `SshConfigDigestInput`，后者从 `ResolvedHost` 转。`ResolvedHost` 从 `resolve_host_via_ssh_g(alias)` 取。每次拿 ContextId 都要跑 `ssh -G alias` 子进程？

**修法**：在 `SshSessionResources` 新增 `host_signature: cdt_fs::HostSignature` 字段；`connect_inner` 在 `stage 0`（已 resolve 完）后立刻计算：

```rust
// session.rs::connect_inner 修改
let resolved = resolve_host_via_ssh_g(&request.host).await?;
let host_signature = {
    let input: SshConfigDigestInput = (&resolved).into();
    cdt_fs::HostSignature::from_ssh_config_fields(&input)
};
// 后续 stage 1-5 不变
// 末尾构造 SshSessionResources 时填入 host_signature
```

`SshSessionManager::context_id(&str)` 查 `sessions[ctx_id]` 拿 `host_signature` + `remote_home` 合成 `ContextId::ssh(host_signature.clone(), remote_home.clone())`。

**为何不每次 resolve**：`ssh -G` 子进程 ~50-200ms（fork + load ssh + parse config）；每次 metadata cache lookup 跑一次完全不可接受。

**为何不存 `ContextId` 直接而存 `HostSignature` + 让 manager 合成**：`ContextId::ssh(host_signature, remote_home)` 是简单 struct 构造，避免数据冗余；`remote_home` 已经在 `SshSessionResources` 字段里。

**替代方案**：(a) 每次 resolve → 否决（性能不可接受）；(b) `SshSessionResources` 存完整 `ContextId` → 否决（数据冗余）；(c) `SshSessionManager` 全局 `HashMap<context_id_str, HostSignature>` → 否决（与 `sessions` HashMap 重复管理，易脱钩）

### D6-bis: ssh -G success → degraded fallback 之间 `HostSignature` 不同是 by-design

**问题**（codex 二审 D6 High）：第一次 `connect_inner` 跑 `resolve_host_via_ssh_g(alias).await` 成功，拿到完整 `ResolvedHost { proxyjump, proxycommand, hostkeyalias }` → `HostSignature` digest A；第二次 reconnect 时 ssh binary 缺失或子进程失败 → `fallback_via_config_parser` 路径 → `ResolvedHost { proxyjump: None, proxycommand: None, hostkeyalias: None }` → `HostSignature` digest B（**不同**）→ 同 ContextId 字符串 / 同 host alias 的 cache entry 不复用。

**钉死决策**：此行为是 **by-design safe miss**，**不**是 bug：

- 走 fallback 时，`config_parser` 拿不到 `ProxyJump` / `ProxyCommand` / `HostKeyAlias`——意味着系统对该 host 的**连接拓扑认知降级**，不同于 ssh -G 路径
- 在两种认知模型下，安全地落到不同 cache namespace 比"假设两种 digest 等价"更稳——`config_parser` 路径若误命中 `ssh -G` 路径写入的 entry，可能基于错误的连接假设拿到陈旧远端数据
- 用户体感：reconnect 时若 ssh -G 失败，session 列表会冷扫一遍（多几秒），但绝不会拿到串扰数据
- 在 `ssh-remote-context` spec delta 显式加一个 Scenario "degraded fallback 与 ssh -G 路径 `ContextId` 安全不等"，固化此契约

**替代方案**：(a) 强制 ssh -G 失败时使用上次成功的 `host_signature`（持久化在内存）→ 否决（破"resolved ssh config 的纯函数 digest"语义，且若用户 `~/.ssh/config` 真改了 ProxyJump，会错误命中旧 cache）；(b) 把 `proxyjump/command/alias` 三字段从 hash 输入剔除 → 否决（违反 PR-A D5b 钉死的 digest 字段集合）

### D8: cache miss 后扫描路径**本 change** 仍走 `tokio::fs::File::open`——SSH cache 路径不依赖此 scanner

**问题**（codex 二审 EXTRA-2 Blocking）：spec MODIFIED Requirement 要求 cache stat 走 `FileSystemProvider`，但 `extract_session_metadata_with_ongoing` 内部 cache miss 后的扫描路径（`File::open(path)` + `BufReader::lines`）**未**走 fs trait。若调用方传 SSH ctx，cache miss 后会用 `tokio::fs::File::open` 读 SSH 远端 path 必然失败。

**修法**：本 change scope 收窄：

1. **spec 层面**只要求 stat 路径走 fs（已生效），**不要求**本 change 把扫描路径也切 fs.open_read
2. **运行时层面**现有 SSH callsite **不**经过 `extract_session_metadata_cached`：`list_sessions_skeleton` 对 SSH context 走 inline `read_to_string + extract_metadata_from_parsed` 路径（`local.rs:1346-1462`，PR-A design D6 分类表中标为"策略分叉"），不调 cache wrapper；`get_session_detail` SSH 路径同样直读 fs，不查 metadata cache
3. cache wrapper 当前**有效**调用面 = Local context only。Local context 下 `tokio::fs::File::open` 正确（Local provider 就是 tokio::fs 包装），扫描路径不破
4. **后果**：PR-B 完成后 `MetadataCache` key 拓扑对 SSH 已就位（spec 已合规），但 SSH cache hit 是"理论上能命中、实际还没人写入"状态——等 PR-D 真正把 SSH callsite 也走过 cache wrapper 时，才能享受 cache 收益；PR-D 同时需要把 scanner 内部 `File::open` 切到 `fs.open_read`

**design 注解**：本 change 是"cache key 拓扑就位"+"Local stat 路径走 fs"两件事；不是"SSH cache 命中省 RTT"。SSH 命中省 RTT 是 PR-D 的工作。这与 PR-A roadmap "PR-B：MetadataCache 切 fs trait + ContextId 强制 key（解 SSH 列表卡顿核心）" 描述微调——PR-B 落地"key + stat 路径"，PR-D 落地"scan 路径 + SSH callsite 接入"才能真正解 SSH 列表卡顿。本 change tasks.md 末尾加 follow-up 提醒。

**为何不在本 change 把 scanner 也切 fs.open_read**：
- 改动量大：scanner 内有 line-by-line BufReader + 流式状态机，切 `Box<dyn AsyncRead>` 需要重构 read loop（AsyncBufReadExt::lines 接受任意 AsyncBufRead，但 `BufReader::new(Box::new(file))` vs `BufReader::new(tokio::fs::File)` 性能差异需 micro-bench——属 D4 micro benchmark scope）
- PR-D 才统一清 30+ 处 `tokio::fs::*` 直调；scanner 是其中一处，与 `local.rs` 内多处一起处理更连贯
- 单独本 PR 改 scanner = 把 PR-D 一部分拆出来，diff 散乱反而 reviewer 难追溯

**spec 显式 Scenario**：在 ipc-data-api delta 加 Scenario "本 change scope: stat 走 fs，scan 仍 tokio::fs，SSH callsite 未接入"，让 reviewer 一目了然 PR-B 边界。

**替代方案**：(a) 本 PR 同时切 scanner → 否决（PR-D 范围扩散）；(b) cache wrapper 对 SSH ctx panic / 返 Err → 否决（行为变化破现有 callsite 隐含约定）；(c) 当前方案 → 选中

### D7: `src-tauri/Cargo.lock` 顺手同步

**问题**：PR-A 在 4 个业务 crate 加 `cdt-fs = { workspace = true }` 依赖，但 `src-tauri/Cargo.lock` 未同步（PR-A 没显式跑 `cargo check --manifest-path src-tauri/Cargo.toml`）。检查现状：

```bash
$ grep -c '"name": "cdt-fs"' src-tauri/Cargo.lock 2>/dev/null
0 if not present, 1 if present
```

如果 lockfile 缺少 `cdt-fs` 条目，Tauri build 时会 fail（manifest 引 cdt-api → cdt-api 引 cdt-fs 但 lockfile 无 cdt-fs hash）。

**修法**：本 change apply 阶段最后一步跑 `cargo check --manifest-path src-tauri/Cargo.toml`，让 lockfile 自然同步并 commit。如果 `src-tauri/Cargo.lock` 已含 cdt-fs（PR-A 实际有同步但用户描述里说漏），改动为 0 字节也无害。

**为何不开 separate PR**：本 PR diff 已包含 fs 抽象切换，加一个 lockfile 自动同步不增加 reviewer 认知负担；分开会让 PR-B 的 src-tauri build 仍可能挂（如果 PR-A 真没同步）。

**替代方案**：(a) 单独 PR fix → 否决（无意义额外 round trip）；(b) 不修等用户报错 → 否决（如果 PR-A 真漏了 build 会挂）

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| 6 处 callsite 改签名工作量 + 漏改 → 编译错误 | 一轮 Edit 全过；compiler errors 直接揪出漏改；本地 `cargo check --workspace` + `cargo test --workspace` 双过 |
| `SshSessionManager::context_id(&str)` 的 lock 与现有 `provider(&str)` / `context_state(&str)` 共享 `sessions: Arc<Mutex<HashMap>>` 锁可能加剧争用 | `context_id` 走同一 `sessions.lock().await` + clone `host_signature` + `remote_home` 立刻释放，与 `provider` 模式一致；无新锁顺序问题 |
| `SshSessionResources` 加字段破坏 `insert_test_context` 测试构造点 | 给 `insert_test_context` 加默认参数：`host_signature: Option<HostSignature>`，缺省时按 host+port 字符串 mock 一个 fake digest；现有调用方编译兼容 |
| LRU 2000 容量增加 ~600KB 内存 | 可忽略；典型 Tauri 进程 RSS 150-300MB，800KB 是 < 0.5% |
| LRU bump O(N) on 2000 entries → ~50µs/hit | 实测 IPC list_sessions cache hit 已是百微秒级别；50µs 增加是相同量级噪音，不影响 wall < 500ms 预算。若 perf bench 显示退化，开 follow-up PR 把 `VecDeque<PathBuf>` 换 `LinkedHashMap` 让 bump 变 O(1)；本 PR 不做以缩小 diff |
| `extract_session_metadata_cached` 签名加 `fs` + `context_id` 参数破坏外部测试 | crate-private（`pub(crate)`），无外部直接调用方；crate 内单测同步改即可 |
| `try_lookup_cached_metadata` 同上 | 同上 |
| fake-SSH bench 引入 tokio::time::pause 在 ipc test runtime 内 | bench 标 `#[ignore]` 不进 CI；本地 dev `cargo test -- --ignored --nocapture` 跑；tokio::time::pause 是显式 opt-in，不污染其他测试 |
| `tokio::fs::metadata` → `fs.stat` 替换可能让 stat 失败语义变化（`tokio::fs::metadata` 返 `std::io::Error`，`fs.stat` 返 `cdt_fs::FsError`） | 现有调用方都是 `match { Ok(meta) => Some(...), Err(_) => None }` 形态——FsError 与 io::Error 都吃 `Err(_)`，行为一致；新加单测覆盖 stat 失败时 cache 不被污染（已在 spec 既有 Scenario "stat 失败时走 cache miss"覆盖，本 change 不动该 Scenario） |
| baseline 退化 | apply 前 baseline + apply 后跑 `bash scripts/run-perf-bench.sh --runs 5`，wall / user / RSS / user-real-ratio 四维齐看；超阈值（PR-A baseline 已记录）拒合并；perf_get_session_detail 本地无 fixture 跳过——与 PR-A 同情况 |
| `MetadataCache` 单测信赖 `cache.lock().unwrap().lookup(path)` 当前签名 | 全部测试同步改 `cache.lock().unwrap().lookup(&ctx, path)`；新增 `local_vs_ssh_keys_do_not_collide` 等单测覆盖 (ContextId, PathBuf) key 隔离 |
| `LocalDataApi` 字段加 `fs` + `current_context_id` 破坏现有 `pub fn new(...)` 签名 | 不改 `new` / `new_with_watcher` 公开签名；构造器内部初始化新字段（用 `cdt_fs::local_handle()` + `ContextId::local(projects_dir.clone())`）；ContextId 在 `new` 里按 `scanner.projects_dir()` 算 |
| `host_signature` digest 在 fake test provider 上 mock 假值导致跨测试串扰 | test helper 接受 `Option<HostSignature>`，None 时按 `(host, port, user)` 字符串拼接做 SHA-256 mock；不同 host 自然产不同 digest |
| codex 二审报新问题 | propose 阶段先调 codex 拦下大方向；apply 阶段 push 后再调一轮验证细节 |

## Migration Plan

本 change 是行为契约级改动（cache key 拓扑 + fs trait 注入），但对前端 IPC 无 BREAKING（响应字段不变），对外部测试无 BREAKING（公开签名保留）。**部署顺序**：

1. apply 阶段先动 `cdt-ssh::SshSessionResources` + `SshSessionManager::context_id` 暴露
2. 再动 `cdt-api::session_metadata::MetadataCache` 内部 key + cached/lookup 函数签名
3. 再动 `cdt-api::ipc::local::LocalDataApi` 字段 + 构造器 + 6 处 callsite + switch/connect/disconnect 同步
4. 最后跑 `cargo check --manifest-path src-tauri/Cargo.toml` 让 `src-tauri/Cargo.lock` 自然同步并 commit
5. perf 验证：apply 前后各跑 5 次 `bash scripts/run-perf-bench.sh --runs 5`，对比四维

**回滚**：本 change 改动隔离在 `cdt-ssh::session.rs` / `cdt-api::ipc::session_metadata.rs` / `cdt-api::ipc::local.rs` 三个文件 + 一个新 bench fixture；revert PR 即可回滚。无数据迁移、无前端联动。

## Open Questions

1. ~~**`LocalDataApi.fs` 字段在本 change 是否纯锚点？**~~ —— **已 closed by D2 修订**：不加 `fs` 字段（codex 二审 D2 High 拒绝死字段）；PR-E 引入 `InstrumentedFs` 时再决定 wire 方式。
2. **LRU bump O(N) 改 O(1) 何时做？** —— 本 change 标 follow-up：若 perf bench 显示 LRU bump 成 bottleneck（hit 路径 wall > 100µs），开新 PR 切 `LinkedHashMap`；当前预测不会，**保持 open** 让 perf 数据驱动决策。
3. ~~**`SshSessionResources.host_signature` 是否要 `Option` 兜底 degraded 模式？**~~ —— **已 closed by D6-bis 修订**：不需要 `Option`。`HostSignature::from_ssh_config_fields` 在 `proxyjump = proxycommand = hostkeyalias = None`（fallback 路径）下仍能产 32-byte digest（zero-padded length-prefix 编码），degraded 与 ssh -G 路径产**不同** digest 是 by-design safe miss；PR-A D5b-i 已测 Scenario `degraded_mode_none_proxyjump_still_yields_digest`，本 change ssh-remote-context delta 新加 Scenario 显式固化"degraded 与 ssh -G 路径 ContextId 不等是 by-design"契约。
