## 1. cdt-api: 共享 `FileSignature` 抽象（capability: 跨 notifier + metadata）

- [x] 1.1 在 `crates/cdt-api/src/` 新增 `cache_signature.rs` 模块（pub(crate)），定义 `FileIdentity`（Unix `(dev, ino)` / Windows `(volume_serial, file_index)` / 兜底空），`FileSignature { mtime: SystemTime, size: u64, identity: FileIdentity }`，及 `FileSignature::from_metadata(meta: &std::fs::Metadata) -> Self` 构造函数（基于 `MetadataExt`）
- [x] 1.2 `FileSignature` 实现 `PartialEq` + `Eq` + `Clone` + `Debug`：所有字段一致才算相等
- [x] 1.3 单元测试：相同文件多次 stat 得到相同 `FileSignature`；不同 inode 文件 / 不同 size 文件 / 不同 mtime 文件得到不同 `FileSignature`；跨平台测试（用 `#[cfg(unix)]` + `#[cfg(windows)]` 分别覆盖 dev/ino 与 volume_serial/file_index 路径）
- [x] 1.4 `cargo clippy -p cdt-api --all-targets -- -D warnings` 通过；`cargo fmt --all` 通过
- [x] 1.5 `cargo test -p cdt-api --lib cache_signature` 通过

## 2. cdt-api: notifier `FileSignature` 缓存（capability: notification-triggers）

- [x] 2.1 在 `crates/cdt-api/src/notifier.rs` 内新增私有 `SignatureCache` struct（`HashMap<(String, String), FileSignature>` + `VecDeque<(String, String)>` 实现 LRU，cap 200），含 `lookup(&self, key) -> Option<&FileSignature>` 与 `insert(&mut self, key, sig)`（命中也 bump key 到队首避免冷热混淆）
- [x] 2.2 给 `NotificationPipeline` 加 `cache: std::sync::Mutex<SignatureCache>` 字段；构造器 `new` 初始化为空（保持 `make_pipeline` 测试 helper 兼容）
- [x] 2.3 在 `process_file_change` 入口、`triggers.is_empty()` 早返回之后、`parse_file` 之前加 stat：调 `tokio::fs::metadata(&jsonl_path)` → `FileSignature::from_metadata`；stat 失败走原路径不写缓存
- [x] 2.4 lookup cache：若命中且 `FileSignature` 与缓存完全一致，整段 return（跳过 parse + detect）
- [x] 2.5 cache miss 路径正常 parse + detect 后，将 `(project_id, session_id) → FileSignature` 写回缓存
- [x] 2.6 单元测试：相同 `FileSignature` 命中（用 tempdir + 计数 wrapper 或观测 `add_notification` 调用次数）；mtime 变化 miss；size 变小 miss；inode 变化 miss（`#[cfg(unix)]` 用 `std::fs::rename` 替换文件覆盖 inode）；stat 失败走 miss；超 200 entries LRU 淘汰；命中也 bump 到队首（连续访问同 key 不被淘汰）
- [x] 2.7 现有 `notifier_skips_deleted_events` / `notifier_missing_file_is_silent` 单测保持通过
- [x] 2.8 `cargo clippy -p cdt-api --all-targets -- -D warnings` 通过；`cargo fmt --all` 通过
- [x] 2.9 `cargo test -p cdt-api` 通过

## 3. cdt-api: `LocalDataApi` 持有 metadata cache（capability: ipc-data-api）

- [x] 3.1 在 `crates/cdt-api/src/ipc/session_metadata.rs` 内新增私有 `MetadataCache` struct（`HashMap<PathBuf, MetadataCacheEntry>` + `VecDeque<PathBuf>` LRU，cap 200）；`MetadataCacheEntry { signature: FileSignature, title: Option<String>, message_count: usize, messages_ongoing: bool, git_branch: Option<String> }`。命中也 bump 到队首
- [x] 3.2 `extract_session_metadata` 函数签名**不变**（保持 `pub async fn extract_session_metadata(path: &Path) -> SessionMetadata`，作为纯函数继续被现有单测使用）；新增 `pub(crate) async fn extract_session_metadata_cached(cache: &Mutex<MetadataCache>, path: &Path) -> SessionMetadata` 作为缓存 wrapper
- [x] 3.3 修改 `extract_session_metadata` 内部实现，把 `messages_ongoing` 中间值（`check_messages_ongoing` 的结果）独立暴露出来：拆出 `pub(crate) async fn extract_session_metadata_with_ongoing(path: &Path) -> (SessionMetadata, bool)`，原 `extract_session_metadata` 改为对它的薄 wrapper（取第一个返回值）。这样 cached 路径能拿到 messages_ongoing 写缓存
- [x] 3.4 `extract_session_metadata_cached` 实现：先 stat 拿 `FileSignature`；stat 失败 → 直接调 uncached `extract_session_metadata_with_ongoing` 返回（不写缓存）；stat 成功 → lookup cache，命中（`FileSignature` 完全一致）→ 返回 `SessionMetadata { title: cached.title.clone(), message_count: cached.message_count, is_ongoing: cached.messages_ongoing && !is_session_stale(signature.mtime, SystemTime::now()), git_branch: cached.git_branch.clone() }`，bump key 到队首；miss → 调 uncached、写缓存、返回
- [x] 3.5 给 `LocalDataApi` 加 `metadata_cache: Arc<std::sync::Mutex<MetadataCache>>` 字段；所有构造器（`new` / `new_with_xxx`）初始化为 `Arc::new(Mutex::new(MetadataCache::default()))`
- [x] 3.6 替换调用点：`crates/cdt-api/src/ipc/local.rs` 三处使用 `extract_session_metadata` 的地方：
  - `list_sessions_sync`（line ≈ 568）：改用 `extract_session_metadata_cached(&self.metadata_cache, path)`
  - `scan_metadata_for_page`（line ≈ 491）：函数签名加 `cache: Arc<Mutex<MetadataCache>>` 参数；spawn 时从 `&self` 传入；内部调 `extract_session_metadata_cached(&cache, &jsonl_path)`
  - 任何其它 `extract_session_metadata(` 调用（`grep -rn "extract_session_metadata(" crates/cdt-api/src/` 全覆盖）
- [x] 3.7 单元测试：相同 path+`FileSignature` 命中（用 mock 或新构造 `LocalDataApi` + tempdir + 观测）；mtime 变化 miss；size 变小 miss；inode 变化 miss（`#[cfg(unix)]`）；stat 失败 miss；wall clock 推进 5 min+ 让缓存命中条目的 `is_ongoing` 翻 false 但 cache 不被 invalidate（mtime 没变 cache key 仍匹配）；LRU 淘汰；命中也 bump 到队首
- [x] 3.8 现有 `extract_*` 单测全部 against `extract_session_metadata`（保持现有测试覆盖底层实现），不动；新增针对 `extract_session_metadata_cached` 的测试在 `mod tests` 内或新 `tests/metadata_cache.rs`
- [x] 3.9 `cargo clippy -p cdt-api --all-targets -- -D warnings` 通过；`cargo fmt --all` 通过
- [x] 3.10 `cargo test -p cdt-api` 通过

## 4. 验证与归档

- [x] 4.1 `cargo clippy --workspace --all-targets -- -D warnings` 通过（排除 src-tauri）
- [x] 4.2 `cargo test --workspace` 通过
- [x] 4.3 `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` 通过（src-tauri 独立 manifest——本 change 应不动 src-tauri）
- [x] 4.4 `npm run check --prefix ui` 通过（无 ui 改动，trivially 通过）
- [x] 4.5 `openspec validate multi-session-cpu-cache --strict` 通过
- [ ] 4.6 手动 smoke：`just dev` 启动，多 session 同时活跃 5 分钟，对比 `top -pid <cdt>` 的 CPU% 与改动前（用户实测）
- [ ] 4.7 codex 异构二审（实现完成后）：`Agent({ subagent_type: "codex:codex-rescue", ... })` 跑代码二审；发现 bug 修完再跑第二轮验证（按 CLAUDE.md `.claude/rules/codex-usage.md`）
- [ ] 4.8 PR push 后再跑一轮 codex 二审，通过后 `/opsx:archive multi-session-cpu-cache`（archive commit 作为 PR 最后一个 commit）
