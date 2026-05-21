## Why

PR-A（change `unify-fs-abstraction`，2026-05-21 已 archive）在 `fs-abstraction` capability 钉死了"任何 fs-related cache SHALL 把 `ContextId` 作为 key 前缀、SHALL 单实例 + key 含 ContextId 前缀拓扑、SHALL 通过 `FileSystemProvider` 访问 fs"三条 SHALL 句，并显式说明"本 change **不**改 `MetadataCache` 现状（PR-B 才动）"。当前 `crates/cdt-api/src/ipc/session_metadata.rs::MetadataCache` 仍以裸 `PathBuf` 为 key、用 `tokio::fs::metadata` 走 stat，LRU 容量 200——三处都违反 PR-A 钉的 SHALL。SSH host A 与 host B 远端同字面 jsonl 路径会跨 host 误命中、用户切回 Local 后 SSH cache entry 立刻被 LRU 200 容量挤出失效，这是 SSH/HTTP 卡顿核心成因。

本 change 把 `MetadataCache` 切到 `&dyn FileSystemProvider` + `(ContextId, PathBuf)` key + LRU 2000 容量，让 PR-A 钉死的拓扑落地；顺带修补 PR-A 的 `src-tauri/Cargo.lock` 同步漏洞（PR-A 加 4 个业务 crate 依赖 `cdt-fs` 但 `src-tauri/Cargo.lock` 未同步）。

## What Changes

### `MetadataCache` 拓扑

- `MetadataCache.map: HashMap<PathBuf, _>` SHALL 改为 `HashMap<(ContextId, PathBuf), _>`；LRU 容量上限 200 → 2000（跨 ContextId 共享 pool；详 design D4）
- `MetadataCache::lookup(&ContextId, &Path)` / `insert((ContextId, PathBuf), _)` 内部用 tuple key；`lookup` 命中后 SHALL 把 tuple key bump 到队首
- LRU 淘汰 SHALL 仍按全局最近最少使用，**不**按 ContextId 拆配额（依据 PR-A `fs-abstraction` spec §"fs-related cache 必须采用'单实例 + ContextId key 前缀'拓扑"第 3 条）

### `LocalDataApi` 新增就地合成 helper（codex 二审 D2 + D3 修正）

- `LocalDataApi` SHALL 提供新 inherent 方法 `async fn active_fs_and_context(&self) -> (Arc<dyn FileSystemProvider>, PathBuf, ContextId)`——内部单次读 `ssh_mgr.active_context_id().await` 决定走 SSH or Local 分支，**fs 与 ctx 来自同一快照**（避免 ssh_connect 强制 disconnect 旧 active 期间 fs/ctx 不一致的并发窗口）
- `LocalDataApi` SHALL NOT 持 `fs` 字段或 `current_context_id` 字段（design D2 / D3 钉死禁止——避免死字段污染 + 避免 fs/ctx 不一致 race）
- 现有 `active_fs_and_projects_dir` 暂保留 `pub(crate)` 兼容签名（内部转调 `active_fs_and_context` 丢弃 ctx）
- `switch_context` / `ssh_connect` / `ssh_disconnect` SHALL NOT 触 cache 相关状态（这三处行为完全保持现状）；**不**清空 cache（依据 PR-A spec §"switch_context 时不必清 cache：不同 `ContextId` 的 entry 自然不命中"）

### `SshSessionResources` / `SshSessionManager` 扩展

- `cdt_ssh::SshSessionResources` SHALL 新增 `host_signature: cdt_fs::HostSignature` 字段，`connect_inner` 在 `resolve_host_via_ssh_g` 后通过 `SshConfigDigestInput::from(&resolved)` + `HostSignature::from_ssh_config_fields(&input)` 计算并存入
- `SshSessionManager::context_id(&str) -> Option<cdt_fs::ContextId>` SHALL 新增（async，从 `SshSessionResources` 合成 `ContextId::ssh(host_signature, remote_home)`）；test helper `insert_test_context` 接受可选 `host_signature` 参数，缺省时按测试 fixture 字段 mock 一个

### `extract_session_metadata_cached` / `try_lookup_cached_metadata` 签名

- 两个函数 SHALL 新增 `fs: &dyn FileSystemProvider` + `context_id: &ContextId` 参数；内部用 `fs.stat(path).await` 替代 `tokio::fs::metadata(path).await`，结果走 `FileSignature::from_fs_metadata(&FsMetadata) -> Self` 构造（消除 PR-A 在 `cache_signature.rs::from_metadata` 上挂的 `#[deprecated]` 在本 change 内不强制全仓清——`local.rs` 内非 metadata cache 路径仍可能保留旧 `tokio::fs::metadata`，PR-D 才彻底清；本 change 仅切 metadata cache 链路）
- 现有签名 `pub async fn extract_session_metadata(path: &Path) -> SessionMetadata` SHALL 保留不变（spec `ipc-data-api/spec.md` §"extract_session_metadata 保持纯函数签名" Scenario 钉死）

### 性能验证 fixture（counter-based assertion，详 design EXTRA-4 修正）

- 新增 `crates/cdt-api/tests/perf_metadata_cache_ssh_hit.rs`（`#[ignore]`，CI 不跑，本地 dev 跑）：用 PR-A 已有 `cdt_fs::InstrumentedFs` 包装 `FakeSshFs`（每个 fs op 模拟 50ms RTT 用于 verbose 输出），通过 `with_fs_counter` 统计；500 session × 2 轮（miss + hit）：
  - 第一轮 miss：counters 显示 `stat ≈ 500` + `open_read or read_to_string ≥ 500`
  - 第二轮 hit：counters 显示 `stat ≈ 500` + **`open_read == 0` + `read_to_string == 0`**（验收硬约束：cache 命中后绝不再读全文件）

### `src-tauri/Cargo.lock` 同步

- PR-A 加 4 个业务 crate 依赖 `cdt-fs` 但 `src-tauri/Cargo.lock` 未同步（已确认 `src-tauri/Cargo.toml` 通过 `cdt-api`/`cdt-discover` 间接拉 `cdt-fs`）；本 change 跑 `cargo check --manifest-path src-tauri/Cargo.toml` 让 lockfile 自然更新并 commit

### BREAKING

**无运行时 BREAKING**。`extract_session_metadata` 公开签名保留；`MetadataCache::default()` / `MetadataCache::new(capacity)` 公开 API 保留（默认容量 2000）；`MetadataCache::lookup / insert` 是 crate-private，签名扩 `ContextId` 参数仅影响 crate 内调用方。`LocalDataApi::active_fs_and_projects_dir` 暂保留兼容，新增 `active_fs_and_context`。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ipc-data-api`: `MetadataCache` 内部 key 类型升级为 `(ContextId, PathBuf)`，LRU 容量 200 → 2000；`extract_session_metadata_cached` / `try_lookup_cached_metadata` 签名扩 `fs` + `context_id` 参数；cache stat 路径走 `FileSystemProvider::stat` 替代 `tokio::fs::metadata`。
- `ssh-remote-context`: `SshSessionResources` 新增 `host_signature` 字段；`SshSessionManager` 新增 `context_id(&str) -> Option<ContextId>` 查询方法；`connect_inner` 在 stage 0 resolve 之后即计算并保存 `HostSignature`。

## Impact

### 代码
- 修改 `crates/cdt-api/src/ipc/session_metadata.rs`：`MetadataCache.map` key 类型 + `lookup` / `insert` 签名 + LRU 容量；`extract_session_metadata_cached` / `try_lookup_cached_metadata` 走 `fs.stat` + `FileSignature::from_fs_metadata`
- 修改 `crates/cdt-api/src/ipc/local.rs`：新增 `active_fs_and_context` 就地合成方法（**不**改 `LocalDataApi` 字段、**不**改 `switch_context` / `ssh_connect` / `ssh_disconnect` 行为）；调用 cache 的 ~6 处（4 处 `extract_session_metadata_cached`、2 处 `try_lookup_cached_metadata`）传入 `fs` + `context_id`
- 修改 `crates/cdt-ssh/src/session.rs`：`SshSessionResources.host_signature` 字段；`connect_inner` 计算 + 存入；`SshSessionManager::context_id(&str)`；`insert_test_context` 兼容 helper
- 新增 `crates/cdt-api/tests/perf_metadata_cache_ssh_hit.rs`：`#[ignore]` fake-SSH cache 命中性能 bench
- 修改 `src-tauri/Cargo.lock`：自然同步 `cdt-fs` 依赖

### 规则与文档
- 不动 `.claude/rules/*`，不动 `CLAUDE.md`
- 不新增 spec capability，复用 PR-A 已有 `fs-abstraction` cache 拓扑 SHALL 句

### CI
- 现有 workspace 测试不破：所有 metadata cache 单测会按 `context_id` 重写参数；新加 `(ContextId, PathBuf)` key 单测覆盖 Local vs SSH 不串扰场景；`xtask check-fs-direct-calls` warn-only 通过（本 change 不动 H1 allowlist，仅替换 metadata cache 路径的 `tokio::fs::metadata` 一处）
- 新增 `perf_metadata_cache_ssh_hit` 标 `#[ignore]`，CI 不跑

### 性能
- 本地 baseline `perf_cold_scan` / `perf_get_session_detail` SHALL 不退化（cache 命中后跳过 fs.stat 等价 tokio::fs::metadata，热路径 wall 几乎不变）
- SSH cache hit/miss ratio 由新 bench 验收 ≥ 100×

### 依赖
- 无新依赖；`cdt-api` / `cdt-ssh` 已通过 PR-A 加 `cdt-fs = { workspace = true }` 依赖
- `src-tauri/Cargo.lock` 跟随 workspace 自然同步

### Out of scope（PR-A 已声明的 follow-up）
- 不改 `ParsedMessageCache`（PR-C）
- 不清 30+ 处 `tokio::fs::*` 直调 + 18 处 `is_remote` 分叉（PR-D）
- 不引入 `ProjectScanner` 结果 in-memory 复用 + `BackendPolicy` wire（PR-E）
- 不解开 SSH `Arc<Mutex<SftpSession>>` 全锁串行（PR-F）
- 不接入 `InstrumentedFs` instrumentation（PR-E 才接入业务）
