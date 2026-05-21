## Context

PR-A（change `unify-fs-abstraction`，2026-05-21 archive）落地了 `cdt-fs` crate：`FileSystemProvider` trait（含 `stat` / `open_read` / `read_to_string` 等抽象方法）、`ContextId` / `HostSignature` 类型、`InstrumentedFs` wrapper、`xtask check-fs-direct-calls` warn-only 守护，并写入了三条 SHALL 句作为 PR-B-F 的契约：

```
Requirement: ContextId 三元组作为 cache key 前缀
  → 任何 fs-related cache 把 ContextId 作为 key 的一部分（含 MetadataCache + ParsedMessageCache）
Requirement: fs-related cache 必须采用"单实例 + ContextId key 前缀"拓扑
  → 单实例 / key 含 ContextId 前缀 / LRU 全局 / switch_context 不清 cache
  → 本 change 不改 MetadataCache / ParsedMessageCache 现状（PR-B/C 才动）
```

PR-B（change `metadata-cache-context-prefix`，2026-05-21 archive）已把 `MetadataCache` 切到 `(ContextId, PathBuf)` key + `FileSystemProvider::stat`，并在 `LocalDataApi` 上加好了**就地合成** helper `pub(crate) async fn active_fs_and_context(&self) -> (Arc<dyn FileSystemProvider>, PathBuf, ContextId)`（`crates/cdt-api/src/ipc/local.rs:1030-1049`），单点提供"fs + projects_dir + ctx 来自同一快照"语义；`SshSessionManager::provider_and_context_id(&str) -> Option<(SshFileSystemProvider, ContextId)>` 原子 accessor 也已就位。

当前 `ParsedMessageCache`（`crates/cdt-api/src/ipc/parsed_message_cache.rs:32-124`）：
- `map: HashMap<PathBuf, ParsedMessageEntry>` —— **裸 PathBuf** key，跨 host 串扰
- `order: VecDeque<PathBuf>` —— LRU 队列
- `lookup(&mut self, path: &Path)` / `insert(&mut self, path: PathBuf, entry)` / `remove(&mut self, path: &Path)` / `remove_if_signature_mismatch(&mut self, path: &Path, current_sig: &FileSignature)`
- `extract_parsed_messages_cached(cache, path)` 内部 `tokio::fs::metadata(path).await` + `FileSignature::from_metadata(&meta)`（带 `#[allow(deprecated)]`，PR-A 在 `from_metadata` 挂了 `#[deprecated]`）
- `PARSED_MESSAGE_CACHE_CAPACITY = 50`

`LocalDataApi` 内 parsed-msg cache 调用方共 **2 处业务 + 1 处 test helper + 1 处 invalidator**：
- `local.rs:2483` `extract_parsed_messages_cached(&self.parsed_msg_cache, &jsonl_path)` in `get_image_asset` Local 分支
- `local.rs:2545` `extract_parsed_messages_cached(&self.parsed_msg_cache, &jsonl_path)` in `get_tool_output` Local 分支
- `local.rs:3217` `extract_parsed_messages_cached(&self.parsed_msg_cache, path)` in `prime_parsed_msg_cache_for_test`（`test-utils` feature）
- `local.rs:1673-1720` `spawn_parsed_msg_cache_invalidator` 内 watcher 订阅 → `tokio::fs::metadata` + `FileSignature::from_metadata` + `remove_if_signature_mismatch(&path, &current_sig)` / `remove(&path)`

SSH 分支（`get_tool_output` `local.rs:2511-2533` / `get_image_asset` `local.rs:2449-2471`）走 inline `fs.read_to_string + parse_jsonl_content`，**不**经过 cache wrapper——本 change 保留此 scope 边界，与 PR-B D8 完全同型。

性能基线（`.claude/rules/perf.md` + `tests/perf-baseline.json`）：本地 `perf_cold_scan` wall ≤ 500ms / user/real ≤ 0.6；`perf_get_session_detail` wall ≤ 500ms / user/real ≤ 0.7。本 change 仅改 cache key + stat 入口，cache 命中路径不动，**理论不影响 baseline**——仍 SHALL 在 apply 前后跑 5 runs 验证。

## Goals / Non-Goals

**Goals:**
- `ParsedMessageCache` 内部 key 升级为 `(ContextId, PathBuf)` tuple，与 PR-B `MetadataCache` 同形态
- `extract_parsed_messages_cached` 通过 `FileSystemProvider::stat` 而非 `tokio::fs::metadata` 走 stat；签名加 `fs: &dyn FileSystemProvider` + `context_id: &ContextId`
- `spawn_parsed_msg_cache_invalidator` 内部 stat 也走 `fs.stat()`（始终 Local：`cdt_fs::local_handle()`），并用 `ContextId::local(projects_dir)` 推算 cache key prefix
- `LocalDataApi` 2 处业务 callsite + 1 处 test helper 通过 `self.active_fs_and_context().await` 拿 `(fs, projects_dir, ctx)` 三元组传入
- 修订 `ipc-data-api` 3 条 Requirement 描述：cache key 形态 + invalidator ContextId 推算 + SSH callsite scope 边界
- 顺修 `src-tauri/Cargo.lock` 与 workspace 同步（PR-B 已修，本 PR 复查）

**Non-Goals:**
- **不**改 `MetadataCache`（PR-B 已完成）
- **不**清 18 处 `is_remote` 分叉 + 30+ 处 `tokio::fs::*` 直调（PR-D）
- **不**让 SSH callsite 真正接入 parsed-msg cache wrapper（PR-D；当前 SSH 分支仍走 inline `fs.read_to_string + parse_jsonl_content`）
- **不**改 LRU 数据结构（`VecDeque` LRU bump 仍 O(N)；详 D3）
- **不**引入 byte cap / TTL 等新机制
- **不**缓存 `build_chunks` 结果（spec `ipc-data-api/spec.md:1166` 显式说"先缓存 parse 一层"，本 change 不动）
- **不**新加 `LocalDataApi` 字段（复用 PR-B `active_fs_and_context()` helper）
- **不**新加 fs-abstraction Requirement（PR-A 已覆盖；本 change 是 SHALL 句的 implementation）

## Decisions

### D1: cache key 选 `(ContextId, PathBuf)` 而非 newtype struct

**问题**：`HashMap` key 类型选 `(ContextId, PathBuf)` tuple 还是 `ParsedMessageCacheKey { ctx: ContextId, path: PathBuf }` newtype struct？

**修法**：选 tuple `(ContextId, PathBuf)`。理由：

- PR-A `fs-abstraction/spec.md` §"fs-related cache 必须采用'单实例 + ContextId key 前缀'拓扑" Scenario "key 类型含 ContextId" 措辞 `"key 类型 SHALL 是 (ContextId, PathBuf) 或等价 newtype"`——tuple 是首选形态
- PR-B `MetadataCache` 已用 tuple `(ContextId, PathBuf)`；跨 cache 形态一致便于 reviewer 类比 + grep
- newtype 多一层间接 + `derive(Hash, Eq)` boilerplate，对 crate-private 单一 callsite 价值有限

**替代方案**：(a) newtype struct → 否决（与 PR-B 形态不一致 + 无收益的间接层）；(b) `String` 拼接 → 否决（`ContextId` 内含 `[u8; 32]` digest，序列化为 hex 浪费 + 损失类型安全）

### D2: 复用 PR-B `active_fs_and_context()` helper，**不**新加 LocalDataApi 方法

**问题**：parsed-msg cache callsite 需要 `(fs, ctx)`，是新加 helper 还是复用 PR-B 已有的？

**修法**：复用 `LocalDataApi::active_fs_and_context()`（PR-B `local.rs:1030-1049`）。返回 `(Arc<dyn FileSystemProvider>, PathBuf, ContextId)`，与本 change 需求完全一致。

理由：
- PR-B 在该 helper 的 doc comment 显式标 "fs + ctx 来自同一快照（详 change `metadata-cache-context-prefix` design D3 / D3-bis）"，已是 cache 路径的官方入口
- 不新加字段、不新加方法 = diff 最小化、跨 PR 一致性最高
- 即便后续 PR-D 把 `InstrumentedFs` wire 进来，包装点也是 `active_fs_and_context()` 内部，本 change 无需关心

**替代方案**：(a) 新加 `active_fs_and_context_for_parsed_msg()` 方法 → 否决（无差异化逻辑，纯重复）；(b) 直接展开 helper 体到 callsite → 否决（race window 处理逻辑复杂，复制易出错）

### D3: LRU 容量保持 `PARSED_MESSAGE_CACHE_CAPACITY = 50` 不变

**问题**：PR-B 把 `MetadataCache` 容量从 200 提到 2000（多 ContextId 共享），那 `ParsedMessageCache` 50 是否也要提？

**修法**：保持 `50` 不变。理由：

- **单 entry 内存量量级千倍**：metadata entry ≈ 400 字节（title + signature）；parsed-msg entry = `Arc<Vec<ParsedMessage>>`，大 session 1k-10k 消息 × 每条 ~1KB（uuid+text+tool_use_input blocks）= 1-10MB；50 entry × 平均 ~3MB ≈ 150MB 内存量级
- 用户在跨 Local + 1-2 SSH host 的"同时持有"窗口期，热点 session 数远小于 50（典型工作流：当前 active session + 最近几个 + 偶尔翻看历史的 5-10 个）；50 是充足的"日常热点 cap"
- **提到 200/2000 会让最坏内存膨胀到 600MB+，远超 perf budget 200MB**
- LRU bump O(N) 在 N=50 时平均 25 次扫描 ≈ ns 级，远比 metadata 2000 容量的 50µs 便宜，不必为此切 `LinkedHashMap`
- spec 现状（`ipc-data-api/spec.md:1168` 与 `:1207`）明确写 "缓存容量 SHALL 上限 50 entries"——保持 50 不动也避免触碰 spec MODIFY 容量数字（spec MODIFY 仅改 key 形态 + Scenario）

**替代方案**：(a) 提到 200 → 否决（内存膨胀风险，无收益）；(b) 引入 byte cap → 否决（超 scope，本 change 不引入新机制；留 follow-up）；(c) per-ContextId 子 LRU → 否决（违反 PR-A spec "LRU 容量按全局计算"）

**为何不立刻引入 byte cap**：单 entry 大小估算需要 instrumentation（运行时记录 `Vec<ParsedMessage>` 序列化大小或自实现 size_hint），属新机制；本 change scope 收窄到 "key + stat 路径"，与 PR-B 同形态。Follow-up 留在 design Open Questions。

### D4: invalidator 始终用 `ContextId::local(projects_dir)`，stat 走 `cdt_fs::local_handle().stat()`

**问题**：`spawn_parsed_msg_cache_invalidator` 订阅的 `FileWatcher::subscribe_files()` 永远来自 Tauri 本地 fs（`cdt-watch` 是 notify-based 的本地 inotify/FSEvents/ReadDirectoryChangesW 包装，**不**触发远端 SSH 文件变化）。Callsite 写入 cache 时也是 Local ctx（SSH callsite 走 inline 不查 cache，本 change D6）。所以 invalidator 用什么 ctx 推算？

**修法**：

1. invalidator 函数体内**一次性**合成 `let ctx = cdt_fs::ContextId::local(projects_dir.clone());`，循环内每次事件复用同一个 ctx clone（`ContextId::Clone` 复制 32-byte digest + PathBuf，廉价）
2. `tokio::fs::metadata(&path).await` 替换为 `cdt_fs::local_handle().stat(&path).await`——结果走 `FileSignature::from_fs_metadata(&meta)`
3. 调用 `cache.remove_if_signature_mismatch(&ctx, &path, &current_sig)` / `cache.remove(&ctx, &path)`（新签名）

理由：
- watcher 是 Local 视角的硬不变量（FileWatcher 不能跨 SSH 触发远端事件）—— 用 Local ctx 命中是**正确的**：与 Local callsite 写入的 entry 同 key
- 若用户在切 SSH context 时仍有 Local cache entry，watcher 收到 Local 文件改动事件仍能正确 invalidate（cache 中 Local ctx prefix 的 entry 会被 stat 比对 + remove）；这正是"全局共享 pool + 自然 LRU 淘汰"的期望行为
- stat 走 `local_handle().stat()` 让代码风格与 cache wrapper 一致（统一走 fs trait，便于 PR-D `InstrumentedFs` wire）

**与 PR-B `MetadataCache` 对比**：PR-B 的 `MetadataCache` 没有这条 watcher invalidate 路径——MetadataCache 完全靠被动 signature 校验。`ParsedMessageCache` 因 entry 大、cache miss 重 parse 成本高（大 session 200-400ms），所以加了主动 invalidate 兜底；本 change 保留这条路径并完成 ContextId 改造。

**为何不让 watcher 路径走 fs.stat 失败时 fall-through 到 raw fs**：`local_handle()` 永远返回 Local provider，`stat` 失败语义与 `tokio::fs::metadata` 失败完全一致（都是 `std::io::Error` 等价的 wrapping）；invalidator 的"保守 remove"路径与原行为完全一致。

**替代方案**：(a) invalidator 内部按 `active_fs_and_context()` 动态取 ctx → 否决（watcher 是 Local 硬绑定，动态取 ctx 反而引入"切到 SSH 时 watcher 推算用 SSH ctx 但 cache 写入是 Local ctx" 的不自洽风险）；(b) 保留 `tokio::fs::metadata` 不切 fs.stat → 否决（与 cache wrapper 风格分叉，PR-D Instrumented wire 时还要再改一次）

### D5: `switch_context` / `ssh_connect` / `ssh_disconnect` **不**清 parsed-msg cache

**问题**：用户切 SSH host A → B 时，host A 的 parsed-msg cache entry 该怎么办？

**修法**：选保留等 LRU 自然淘汰（与 PR-B D5 完全同型）。PR-A spec §"switch_context 时不必清 cache：不同 `ContextId` 的 entry 自然不命中（依赖 Hash/Eq 隔离），TTL + signature 校验照常工作"已钉死。

**关键点**：本 change scope 内 SSH callsite 仍走 inline 不查 cache（D6），所以 cache 实际只有 Local ctx 的 entry——`ssh_disconnect` 不清缓存的好处主要是"reconnect 后 host_signature 相同 → 同 ContextId → 复用旧 cache entry"将在 PR-D 让 SSH 接入 cache 后才生效。本 change 保持 D5 决策，让 PR-D 自然受益。

**为何不担心 cache entry 占内存**：Local ctx entry 在 LocalDataApi 生命周期内一直可能复用；50 容量上限自然兜底，无需主动清理。

**替代方案**：(a) switch / disconnect 时清整个 cache → 否决（loss principle，违反 PR-A spec）；(b) 清该 ContextId 的所有 entry → 否决（reconnect UX 差 + 实际本 change 无 SSH entry，纯实现复杂度）

### D6: SSH callsite **本 change** 仍走 inline 不查 cache wrapper

**问题**（与 PR-B D8 完全同型）：spec MODIFIED Requirement 要求 cache stat 走 `FileSystemProvider`，但 `get_tool_output` / `get_image_asset` 的 SSH 分支当前是 inline `fs.read_to_string + parse_jsonl_content`，**未**经过 `extract_parsed_messages_cached`。若强行让 SSH callsite 接入 cache wrapper，本 change 需要把 `parse_file(path).await`（内部 `tokio::fs::File::open`）也切到 `fs.open_read`——这是 PR-D 范围内的"30+ tokio::fs::* 直调清理" 一部分。

**修法**：本 change scope 收窄：

1. **spec 层面**只要求 stat 路径走 fs（已生效），**不要求**本 change 把 cache miss 后的 `parse_file` 路径也切 fs.open_read
2. **运行时层面**现有 SSH callsite（`get_tool_output` SSH 分支、`get_image_asset` SSH 分支）**不**经过 `extract_parsed_messages_cached`，继续走 inline `fs.read_to_string + parse_jsonl_content` 路径
3. cache wrapper 当前**有效**调用面 = Local context only；Local 下 `parse_file(path)` 内部 `tokio::fs::File::open` 正确（Local provider 就是 tokio::fs 包装），扫描路径不破
4. **后果**：PR-C 完成后 `ParsedMessageCache` key 拓扑对 SSH 已就位（spec 已合规），但 SSH cache hit 是"理论上能命中、实际还没人写入"状态——等 PR-D 真正把 SSH callsite 也走过 cache wrapper 时，才能享受 cache 收益；PR-D 同时需要把 `parse_file` 内部 `File::open` 切到 `fs.open_read`

**design 注解**：本 change 是"cache key 拓扑就位"+"Local stat 路径走 fs"两件事；不是"SSH cache 命中省 RTT"。SSH 命中省 RTT 是 PR-D 的工作。**这与 PR-B D8 是完全同形的 scope 边界**——reviewer 可类比理解。

**额外后果**（codex 二审 Q5 确认）：因 `parse_file` 内部走 `tokio::fs::File::open` 不经 fs trait，原计划的 "fake-SSH FileSystemProvider + miss→hit 计数 bench"（task 6 早期版本）**设计上行不通**——fake_ssh_fs 的 `open_read` 永不会被 cache wrapper miss 路径调用。本 PR task 6 改为"真磁盘 jsonl + InstrumentedFs(LocalFs) + 验证 stat 入口已切 fs trait + hit 不重 parse"；SSH 命中省 RTT 的 fake bench 留 FU-1 PR-D（与 parse_file 切 fs trait 一起做）。

**为何不在本 change 把 parse_file 也切 fs.open_read**：
- 改动量大：`cdt_parse::parse_file` 内部有 line-by-line `BufReader` + 流式状态机，切 `Box<dyn AsyncRead>` 需要重构 read loop（属 PR-D D1 micro benchmark scope）
- PR-D 才统一清 30+ 处 `tokio::fs::*` 直调；`parse_file` 是其中一处，与 `local.rs` 内多处一起处理更连贯
- 单独本 PR 改 `parse_file` = 把 PR-D 一部分拆出来，diff 散乱反而 reviewer 难追溯

**spec 显式 Scenario**：在 ipc-data-api delta 加 Scenario "本 change scope: stat 走 fs，parse_file 仍 tokio::fs，SSH callsite 未接入 cache"，让 reviewer 一目了然 PR-C 边界。

**替代方案**：(a) 本 PR 同时切 parse_file → 否决（PR-D 范围扩散）；(b) cache wrapper 对 SSH ctx panic / 返 Err → 否决（行为变化破现有 callsite 隐含约定）；(c) 当前方案 → 选中

### D7: `from_fs_metadata` 替换 `from_metadata`（deprecated）

**问题**：`extract_parsed_messages_cached` 当前用 `FileSignature::from_metadata(&std::fs::Metadata)`（PR-A 已挂 `#[deprecated]`，函数体 `#[allow(deprecated)]`）。改 `fs.stat()` 后返回的是 `cdt_fs::FsMetadata`，要走 `FileSignature::from_fs_metadata(&meta)`（PR-A 在 `crates/cdt-api/src/cache_signature.rs` 加了这个新构造器）。

**修法**：

1. `extract_parsed_messages_cached` 内部：`let meta = fs.stat(path).await.ok()?;` → `let sig = FileSignature::from_fs_metadata(&meta);`
2. 移除函数级 `#[allow(deprecated)]`
3. `spawn_parsed_msg_cache_invalidator` 同样：`fs.stat(&path).await` → `FileSignature::from_fs_metadata(&meta)`；移除内部 `#[allow(deprecated)]`

**风险**：`from_fs_metadata` 与 `from_metadata` 输出是否字节相等？答：是。PR-A `cache_signature.rs::FileSignature::from_fs_metadata` 在 Unix 上读 `FsMetadata.identity = FileIdentity::Unix { dev, ino }`、`mtime` / `size` 字段；与 `from_metadata` 读 `std::fs::Metadata` 的 `dev/ino/mtime/size` 完全等价（`LocalFileSystemProvider::stat` 内部就是 `tokio::fs::metadata().await.map(FsMetadata::from)`）。PR-B `MetadataCache` 已验证此等价性（cache hit 路径不破）。

**替代方案**：保留 `from_metadata` 不动 → 否决（deprecated 标记会让 clippy 报 warning，且最终 PR-D 全清时还得改一次）

### D8-bis: callsite 起始处**一次性**快照 `active_fs_and_context()` 取代 `active_fs_and_projects_dir()` + 二次快照

**问题**（codex 二审 Q6 Blocking）：现 `get_tool_output` `local.rs:2511` / `get_image_asset` `local.rs:2449` 起始处调 `let (fs, projects_dir) = self.active_fs_and_projects_dir().await?;` 拿快照 1；Local 分支再调一次 `let (fs2, _, ctx) = self.active_fs_and_context().await;` 拿快照 2。两次快照之间存在并发 `ssh_connect` 把 `active` 从 None 变 Some 的窗口 → 第一次返回 Local provider 但第二次返回 SSH ctx，造成 fs=Local + ctx=SSH 不自洽 → cache 写错 namespace。

**修法**：callsite 起始处**只调一次** `active_fs_and_context()` 拿三元组 `(fs, projects_dir, ctx)`，后续 `is_remote = fs.kind() == cdt_discover::FsKind::Ssh` 判断走 SSH 分支或 Local 分支，全用同一快照。Local 分支 cache wrapper 调用直接复用快照里的 `fs` + `ctx`，**不**再二次调 helper：

```rust
// 改造后
async fn get_tool_output(&self, root_session_id: &str, session_id: &str, tool_use_id: &str)
    -> Result<cdt_core::ToolOutput, ApiError>
{
    let (fs, projects_dir, ctx) = self.active_fs_and_context().await;
    let is_remote = fs.kind() == cdt_discover::FsKind::Ssh;
    let messages = if is_remote {
        // SSH 分支：用同一 fs 走 inline read_to_string + parse_jsonl_content（不查 cache）
        // ctx 在 SSH 分支不用，但保持快照一致性
        ...
    } else {
        // Local 分支：cache wrapper 用 &*fs + &ctx
        extract_parsed_messages_cached(&self.parsed_msg_cache, &*fs, &ctx, &jsonl_path).await
    };
    ...
}
```

`active_fs_and_context()` 是 PR-B 提供的"fs+projects_dir+ctx 同快照"helper，本就是为了消除 race 而设计；本 change 把现有 `active_fs_and_projects_dir()` 替换成它的范围**仅限**这两个 callsite（其它走 `active_fs_and_projects_dir()` 的方法不动，保持现状减小 PR 范围）。

### D8-bis-fix: 用户可见 IPC handler 走 strict 变体——SSH disconnect 不静默降级

**问题**（codex 二审 commit-stage Q1 = BUG）：D8-bis 让 `get_tool_output` / `get_image_asset` 改用 `active_fs_and_context()` 拿三元组，但该 helper 在 SSH active 但 provider 丢失（concurrent disconnect 中间态）时**静默降级**到 Local。原 `active_fs_and_projects_dir()` 在同场景返 `ApiError::not_found`。用户在 SSH context 下请求 `get_tool_output(sid)` 时，本地恰好有同 ID 的 jsonl 文件→会返 Local 数据。破"用户在 SSH context 下请求"语义契约。

**修法**：在 `LocalDataApi` 上加 `active_fs_and_context_strict() -> Result<(Arc<dyn FileSystemProvider>, PathBuf, ContextId), ApiError>` 严格变体。语义与旧 `active_fs_and_projects_dir` 一致：SSH active + provider lookup miss 时返 `not_found` 错；正常路径与 `active_fs_and_context` 完全一致（同样走 `provider_and_context_id` 原子 accessor，避免 SSH/Local 混合）。

**两个变体的使用边界**：

- **`active_fs_and_context()` (relaxed)**：仅 `prime_parsed_msg_cache_for_test`（`test-utils` feature 路径，构造时不接 SSH，行为等价）；`#[cfg(any(test, feature = "test-utils"))]` cfg-gated 避免 release 构建 dead_code。其它内部 cache 写入路径若未来需 relaxed 行为，再放开 cfg。
- **`active_fs_and_context_strict()` (strict)**：本 PR 切换的 4 处 callsite——这些 handler 需要 `(fs, projects_dir, ctx)` 三元组同快照，且会**读用户可见 session 内容**或返回**包含 SSH/Local 数据混合后果的列表**，所以 SHALL 在 SSH disconnect 中间态返错而非降级：
  - `get_tool_output`（line 2540 附近）
  - `get_image_asset`（line 2470 附近）
  - `build_group_session_page`（line 569，原 PR-B 引入的 relaxed callsite，本 PR 引入 strict 时一并修齐）
  - `list_sessions_skeleton`（line 1433，原 PR-B 引入的 relaxed callsite，本 PR 引入 strict 时一并修齐）

**仍走旧 `active_fs_and_projects_dir()` 的 user-facing handler**（如 `get_session_detail` / `search_sessions` / `list_repository_groups` / `read_agent_configs` 等约 9 处，详 `local.rs:1986/2018/2053/2276/2346/2449/2511/2631/3005`）保持旧行为不动——它们旧 helper 本就返 `Err(not_found)`，与本 PR 引入的 strict 语义等价，**无新降级风险**。统一迁移到 strict 是 PR-D `unify-fs-direct-calls` 范围（同时切 30+ 处 tokio::fs::* 直调与 18 处 is_remote 分叉清理）。

**为何不直接让 `active_fs_and_context()` 默认 strict**：cache wrapper 内部走的就是 relaxed 语义（cache 写入降级到 Local 是 design D3 的"safe miss"），改成 strict 会让 cache 写入抛错——破 cache wrapper 现有 fallback 路径。两个变体并存最清晰。

**替代方案**：(a) 用户可见 IPC handler 内联探 `ssh_mgr.active_context_id()` + 二次校验 `fs.kind()` → 否决（探+helper 之间还是 race，且代码分散）；(b) 改 `active_fs_and_context()` 默认 strict + cache 内部 catch 错降级 → 否决（破 cache wrapper 默认行为，cache callsite 重抓更乱）；(c) 当前方案（两变体并存） → 选中

**与 PR-B 对比**：PR-B 的 metadata cache callsite 也是相同模式（line 822 / 890 / 1397 / 1681 / 1756 / 1820 起始处只调一次 `active_fs_and_context()`），本 change 沿用同一模式。

**替代方案**：(a) 不变（两次快照） → 否决（已确认 race）；(b) 加锁让 active+ctx 全原子 → 否决（破封装，与 PR-B D3-bis 路线背离）；(c) 当前方案 → 选中

### D8: ipc-data-api spec MODIFY 范围 vs ADDED 范围

**问题**：spec 改动是把 3 条 Requirement 整段 MODIFIED 重写，还是只 ADDED 新 Scenario 在已有 Requirement 下？

**修法**：3 条都 MODIFIED，包含完整 Requirement 体 + 现有 Scenarios + 新 Scenarios（含上下文一致性修订）：

- Requirement "`get_tool_output` 与 `get_image_asset` 走 parsed-message LRU 缓存"：
  - 描述句把 `以 JSONL 文件 PathBuf 为 key` 改为 `以 (ContextId, PathBuf) 为 key`
  - 加 Scenario "Local 与 SSH 同字面 path 不串扰（理论场景：PR-D 后 SSH 接入 cache 时生效）"
  - 加 Scenario "本 change scope: SSH callsite 仍走 inline 不查 cache"

- Requirement "parsed-message 缓存按 file-change 广播主动失效"：
  - 描述句把 invalidator 推算逻辑加 `ContextId::local(projects_dir)` 推算
  - 加 Scenario "invalidator 用 Local ContextId 推算 cache key，与 Local callsite 写入 entry 一致命中"

- Requirement "parsed-message 缓存 ownership 由 `LocalDataApi` 持有"：
  - 不动 Requirement 主体（ownership 模式不变）
  - 加 Scenario "switch_context / ssh_connect / ssh_disconnect 不清 parsed-msg cache"

**为何整段 MODIFIED**：CLAUDE.md openspec §archive 顺序坑钉死了"MODIFIED 用完整 body 替换主 spec"——如果只写部分内容，archive 时会丢失原 scenarios（如 "缓存超过容量按 LRU 淘汰" "命中时把 key bump 到队首" 等）。3 条 Requirement 整段 copy 后修订是正确做法。

**替代方案**：(a) 只 ADDED 新 Scenarios → 否决（key 形态描述需 MODIFY，无法纯加）；(b) 部分 MODIFIED + 部分 ADDED → 否决（archive 时合并难追踪）

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| 2 处业务 callsite + 1 test helper + invalidator 签名漏改 → 编译错误 | 一轮 Edit 全过；compiler errors 直接揪出漏改；本地 `cargo check --workspace` + `cargo test --workspace` 双过 |
| `cache.lookup(&ctx, path)` / `cache.insert((ctx, path), entry)` 签名扩破坏 7 个现有单测 | 全部测试同步改，新增 4 个隔离 / LRU 混合 / switch 不清 cache / per-ctx 失效单测 |
| 50 容量 + 多 ContextId 共享让 Local 工作流热点被 SSH 挤掉 | 本 change scope 内 SSH callsite 不查 cache（D6）；Local-only 写入，50 容量充足。PR-D 让 SSH 接入后，若实测 50 不够再开 follow-up |
| `from_fs_metadata` 与 `from_metadata` 字节不等价 → cache hit miss-classify | 已在 D7 风险段分析；PR-B `MetadataCache` 已验证等价；本 change 现有 7 个 single-context 单测（`cached_hit_returns_arc_without_rereading` / `cached_miss_when_file_size_changes` / `cached_miss_when_inode_changes_via_rename` / `cached_stat_failure_returns_none_no_write` / `empty_file_is_cached_as_valid_empty_result` 等）改造后全过即保证等价 |
| invalidator 推 `ContextId::local(projects_dir)` 与 callsite 写入 `ContextId::local(active.projects_dir)` 在 root 切换瞬间不一致 | **此风险不存在于稳态**：`LocalDataApi::reconfigure_claude_root`（`local.rs:1305-1348`）在 `general.claudeRootPath` 切换时 abort 旧 watcher tasks（含旧 invalidator）+ 用新 `projects_dir` 重新 `spawn_watcher_runtime`（含新 invalidator）+ `*self.projects_dir.lock().await = projects_dir.clone()` —— invalidator 与 callsite 在 root 切换完成后都使用新 `projects_dir`。短暂的 abort/spawn 切换窗口内 invalidator 不工作（最坏后果：windows 内的 file-change 事件未触发主动 remove），但 cache 的被动 signature 校验在下次 lookup 仍能兜底，不引入数据正确性问题 |
| `local_handle().stat()` vs `tokio::fs::metadata` 性能差异 | vtable dispatch ≈ ns 级；stat syscall 本体几十 µs；overhead < 0.1% |
| baseline 退化 | apply 前后跑 `bash scripts/run-perf-bench.sh --runs 5`，wall / user / RSS / user-real-ratio 四维齐看；超阈值（PR-B baseline 已记录）拒合并 |
| `cargo clippy --workspace --all-targets -- -D warnings` 在签名扩参后报 `unused_variables` | callsite 全部 `let (fs, _projects_dir, ctx) = ...` 拿三元组传入即可；invalidator 内 ctx 参数实际被用到（传给 remove），无 unused 风险 |
| codex 二审报新问题 | propose 阶段先调 codex 拦下大方向（强制：cache 拓扑 + 跨 capability + 性能关键三项均命中）；apply push 后再调一轮验证细节 |

## Migration Plan

本 change 是行为契约级改动（cache key 拓扑），但对前端 IPC 无 BREAKING（响应字段不变），对外部测试无 BREAKING（公开签名保留）。**部署顺序**：

1. apply 阶段先动 `crates/cdt-api/src/ipc/parsed_message_cache.rs`：key 类型 + 公开 API 签名 + 7 个现有单测 + 4 个新单测
2. 再动 `crates/cdt-api/src/ipc/local.rs`：`get_image_asset` Local 分支、`get_tool_output` Local 分支、`prime_parsed_msg_cache_for_test`、`spawn_parsed_msg_cache_invalidator`
3. 再加 fake-SSH perf bench `crates/cdt-api/tests/perf_parsed_message_cache_ssh_hit.rs`
4. 最后跑 `cargo check --manifest-path src-tauri/Cargo.toml` 让 `src-tauri/Cargo.lock` 自然同步并 commit（PR-B 已修，本 PR 复查）
5. perf 验证：apply 前后各跑 5 次 `bash scripts/run-perf-bench.sh --runs 5`，对比四维

**回滚**：本 change 改动隔离在 `parsed_message_cache.rs` + `local.rs` 两个文件 + 一个新 perf bench fixture；revert PR 即可回滚。无数据迁移、无前端联动。

## Open Questions

1. **byte cap 是否必要？** —— 单 entry 估算 1-10MB，50 容量上限最坏 500MB。**保持 open 让 perf 数据驱动决策**：本 change 不引入，PR-D 让 SSH 接入 cache 后若实测内存峰值超 perf budget 200MB，再开 follow-up PR 引入 `current_bytes: AtomicUsize` + `max_bytes` 双闸门。
2. ~~**root 切换时 invalidator 是否需要重启？**~~ —— **已 closed by codex 二审 Q2**：实测确认 `LocalDataApi::reconfigure_claude_root`（`local.rs:1305-1348`）已经在 `general.claudeRootPath` 切换时 abort 旧 watcher tasks + 用新 `projects_dir` 重启 `spawn_watcher_runtime`（含 invalidator）+ 同步更新 `self.projects_dir`，整个失效路径自动用新 root 推算 ctx，不存在 stale projects_dir 问题。无需 follow-up。
3. **PR-D 接入 SSH cache 后是否需要 `extract_parsed_messages_cached` 提供 `fs: Arc<dyn FileSystemProvider>` 而非 `&dyn`？** —— 当前 PR-B `MetadataCache` 用 `&dyn`，是因 callsite 持 `Arc<dyn>` 后 `&*fs` 即可。本 change 保持 `&dyn` 一致；PR-D 若需在 cache miss 后 spawn 异步任务持 fs，再开 follow-up 评估转 `Arc<dyn>`。

4. **SSH disconnect 中间态返 `ApiError::not_found` vs 专用 `ApiError::ssh(...)` error code？** —— codex 二审 round-3 Q2 指出：strict 现在返 `not_found` 让 HTTP 映射 404，前端无法区分"SSH 临时 disconnect"与"资源真不存在"。但本 PR 的 strict 实际是**复刻**原 `active_fs_and_projects_dir()` 的 not_found 语义，**未引入新行为**——PR-B 之前已经是这个 error code。改 `ApiError::ssh(...)` 会扩 scope 到 IPC error 契约 + 前端 UX 改进（详 FU-4），属于跨 PR 的 error code 统一工作，本 PR 不做。**保持 open**。

5. **前端 Sidebar / sessionListStore 缺 SSH disconnect 错误态展示** —— codex 二审 round-3 Q3/Q4：当前 IPC list 接口失败时前端仅 `console.error` 后清空列表，无 disconnect 重试 UI。本 PR 不动 UI，留 FU-5。这与 Open Question 4 互相依赖——先定 error code 拓扑再做 UX。
