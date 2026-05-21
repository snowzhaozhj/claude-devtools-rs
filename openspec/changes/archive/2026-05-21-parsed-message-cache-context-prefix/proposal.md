## Why

刚 archive 的 change `metadata-cache-context-prefix`（PR-B）已把 `MetadataCache` 切到 `(ContextId, PathBuf)` key + `FileSystemProvider::stat`，但 `ParsedMessageCache`（`crates/cdt-api/src/ipc/parsed_message_cache.rs`）仍是裸 `HashMap<PathBuf, _>` + `tokio::fs::metadata`（deprecated `FileSignature::from_metadata`）。`openspec/specs/fs-abstraction/spec.md` Requirement §"ContextId 三元组作为 cache key 前缀" 与 §"fs-related cache 必须采用'单实例 + ContextId key 前缀'拓扑" 已显式把 `ParsedMessageCache` 列为 SHALL 句的约束对象（行 145、190、199）——本 change 是该 SHALL 句的落地实施。同时让 PR-C 与 PR-B 完全同形，便于 reviewer 类比；PR-D 后续可在统一形态上 wire SSH cache wrapper。

## What Changes

- `ParsedMessageCache.map` / `order` 的内部 key 升级为 `(cdt_fs::ContextId, PathBuf)` tuple；公开 API `lookup` / `insert` / `remove` / `remove_if_signature_mismatch` 签名扩 `ctx: &ContextId` 参数
- `extract_parsed_messages_cached` 签名扩 `fs: &dyn cdt_fs::FileSystemProvider` + `context_id: &cdt_fs::ContextId` 参数；内部用 `fs.stat()` 替换 `tokio::fs::metadata()`（移除 `#[allow(deprecated)]` + `FileSignature::from_metadata` 调用，改 `FileSignature::from_fs_metadata`）
- 2 处 callsite（`get_image_asset` Local 分支、`get_tool_output` Local 分支）+ test helper `prime_parsed_msg_cache_for_test` 改造，通过 PR-B 已加好的 `self.active_fs_and_context().await` 拿 `(fs, projects_dir, ctx)` 三元组传入
- `spawn_parsed_msg_cache_invalidator` 改造：watcher 始终是 Local 视角，内部 `ContextId::local(projects_dir.clone())` 合成一次后传给 `remove_if_signature_mismatch(&ctx, &path, &current_sig)`；stat 走 `cdt_fs::local_handle().stat()` 替换 `tokio::fs::metadata`
- `PARSED_MESSAGE_CACHE_CAPACITY` 保持 `50` 不变（详 design D3）
- spec MODIFY：`ipc-data-api` 三条 Requirement（`get_tool_output` 与 `get_image_asset` 走 parsed-message LRU 缓存 / parsed-message 缓存按 file-change 广播主动失效 / parsed-message 缓存 ownership 由 `LocalDataApi` 持有）把 cache key 描述由 `PathBuf` 改为 `(ContextId, PathBuf)`，加 4 个 Scenario 覆盖 Local/SSH 同字面 path 不串扰、switch_context 不清 cache、invalidator 用 Local ctx 失效、SSH callsite 仍走 inline 不查 cache 的 scope 边界
- 顺修：确认 `src-tauri/Cargo.lock` 与 workspace 同步（PR-B 已修一次，本 PR 复查）

**Non-Goals**：MetadataCache 已 PR-B 完成、30+ 处 `tokio::fs::*` 直调清理留 PR-D、scanner 切 `fs.open_read` 留 PR-D、SSH callsite 真正接入 cache wrapper 留 PR-D、`BackendPolicy` wire 留 PR-E、`build_chunks` 结果缓存（设计上明确"先缓存 parse 一层"，本 change 不动）。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ipc-data-api`：parsed-message 缓存的 3 条 Requirement 修订（key 形态 + invalidator ContextId 推算 + SSH scope 边界）
- `fs-abstraction`：无 MODIFY（PR-A 钉死的 SHALL 句已覆盖 ParsedMessageCache；本 change 不新增 fs-abstraction Requirement，只是 PR-A SHALL 的 implementation）

## Impact

- 代码：`crates/cdt-api/src/ipc/parsed_message_cache.rs`（key 类型 + 公开 API 签名）、`crates/cdt-api/src/ipc/local.rs`（2 处 callsite + invalidator + test helper）
- 测试：`crates/cdt-api/src/ipc/parsed_message_cache.rs` 内 7 个现有单测改造 + 新增 4 个；`crates/cdt-api/tests/perf_parsed_message_cache_ssh_hit.rs` 新增 fake-SSH counter bench
- 依赖：无新 crate 依赖；复用 PR-A 引入的 `cdt_fs::{ContextId, FileSystemProvider, FsKind, InstrumentedFs, local_handle}` + PR-B 引入的 `cdt_fs::FileSignature::from_fs_metadata`
- 性能：cache hit 路径不动；cache miss 路径 `tokio::fs::metadata` → `fs.stat` 仅多 vtable dispatch（几 ns），相对 stat syscall 本体几十 µs 可忽略
- IPC 字段：无变化（前端无破坏）
- 公开签名：`LocalDataApi::new` / `new_with_watcher` 不变（cache 字段构造器内部初始化）；`ParsedMessageCache` lookup/insert/remove 签名扩 ContextId（crate-private API，无外部调用方）
