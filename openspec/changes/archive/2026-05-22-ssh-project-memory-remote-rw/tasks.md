## 1. cdt-fs trait 扩 3 个写方法（Local 实现 + 5 处 impl 同步）

- [x] 1.1 `crates/cdt-fs/src/provider.rs::FileSystemProvider` 加 `async fn write_atomic(&self, path: &Path, content: &[u8]) -> Result<(), FsError>` / `async fn create_dir_all(&self, path: &Path) -> Result<(), FsError>` / `async fn remove_file(&self, path: &Path) -> Result<(), FsError>`，保持 dyn-safe（`async_trait` 宏脱糖），文档串说明 atomic 契约 / mkdir 已存在不报错 / remove_file 不递归
- [x] 1.2 `crates/cdt-fs/src/local.rs::LocalFileSystemProvider` 实现 3 方法：
  - `write_atomic`：用 `tokio::fs::write(<path>.tmp.<seq>.<pid>) + tokio::fs::rename` 原子覆盖；失败 best-effort `tokio::fs::remove_file(<tmp>)`；suffix 来源新增 module-level `static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);` 单调递增（**不**用 `SystemTime::now()` 纳秒——Windows 100ns 时钟精度并发碰撞 race），格式 `format!("{:016x}.{:08x}", WRITE_SEQ.fetch_add(1, Relaxed), std::process::id())`
  - `create_dir_all`：直接 delegate 到 `tokio::fs::create_dir_all`；`AlreadyExists` 错误 SHALL 转 `Ok(())`（实际 tokio API 不报这个，但加防御）
  - `remove_file`：delegate 到 `tokio::fs::remove_file`；`NotFound` SHALL 映射为 `FsError::NotFound(path)`
- [x] 1.3 `crates/cdt-fs/src/local.rs` 加 unit test：write_atomic 覆盖现有内容、并发写串行化（用 `#[tokio::test(flavor = "multi_thread")]` + `JoinSet` 起 4 个 task 同 path 写不同内容、最终 read 是某一个完整内容）、tmp file 不留存（用 `read_dir` 验证目录里没有 `.tmp.*` 后缀残留）；create_dir_all 已存在不报；remove_file 不存在返 NotFound
- [x] 1.4 `crates/cdt-fs/src/instrumentation.rs::InstrumentedFs<P>` 同步实现 3 个新方法（每个方法入口 `FsOpCounter::current().record_<op>()` 后 delegate 到 `inner.<op>()`）；`FsOpCounts` struct 加 `pub write_atomic: AtomicU64` / `pub create_dir_all: AtomicU64` / `pub remove_file: AtomicU64` 三个字段，Drop emit `tracing::info!` 同步加这三个字段
- [x] 1.5 `crates/cdt-discover/tests/project_scanner.rs::SpyFs` 同步实现 3 个新方法（测试 spy fixture——可 `unimplemented!()` 兜底因为该测试不调写路径；如有 callsite 走写则按测试需求实现）
- [x] 1.6 `crates/cdt-api/src/ipc/session_metadata.rs::FakeSshFs` 同步实现 3 个新方法（同 SpyFs 处理：默认 `unimplemented!()` 兜底，本测试模块当前不走 memory CRUD 路径；如未来加测试需求再扩）
- [x] 1.7 `crates/cdt-fs/ALLOWLIST.md` 不需要新增条目（write 方法的 `tokio::fs::*` 调用都在 `crates/cdt-fs/src/local.rs` 内已 allowlist）；但 SHALL run `cargo xtask check-fs-direct-calls` 验证 0 violation

## 2. cdt-ssh SftpClient + provider 加写方法

- [x] 2.1 `crates/cdt-ssh/src/provider.rs::SftpClient` trait 加 4 方法 `write` / `mkdir` / `remove` / `rename`。**不**加 `server_supports_posix_rename`——russh-sftp 2.1.2 不暴露 `posix-rename@openssh.com` 扩展 API（`Features` struct 仅含 hardlink/fsync/statvfs/limits 四 flag），实施 SHALL 走"先 remove ignore-not-found 后 rename"两步降级（design D2 修订）
- [x] 2.2 `crates/cdt-ssh/src/provider.rs::RusshSftpClient` 实现 4 方法 delegate 到 `russh_sftp::client::SftpSession::{write,create_dir,remove_file,rename}`
- [x] 2.3 `crates/cdt-ssh/src/provider.rs::SshFileSystemProvider` 实现 trait write_atomic / create_dir_all / remove_file（含 `SSH_WRITE_SEQ: AtomicU64` 单调序号、两步降级 rename、with_retry 包装）
- [x] 2.4 写路径覆盖测试通过 `crates/cdt-api/tests/ipc_contract.rs::active_ssh_context_reads_remote_projects_and_sessions` 端到端驱动——真 IPC 调 `add_memory` / `delete_memory` 经 `SshFileSystemProvider` 触发 `CountedFakeRemoteSftp` 的 write/mkdir/remove/rename counter；纯 provider 单测留 follow-up

## 3. 测试 helper 扩展 CountedFakeRemoteSftp

- [x] 3.1 `crates/cdt-api/tests/common/fake_remote_sftp.rs::CountedFakeRemoteSftp` 加字段：`pub write_count: Arc<AtomicUsize>` / `pub mkdir_count: Arc<AtomicUsize>` / `pub remove_count: Arc<AtomicUsize>` / `pub rename_count: Arc<AtomicUsize>`；`written_files` 已有的 `files: HashMap` 改 `Arc<Mutex<HashMap<String, Vec<u8>>>>` 让 write 修改可见
- [x] 3.2 `CountedFakeRemoteSftp` 实现 `SftpClient::write` / `mkdir` / `remove` / `rename`：write 写 `files` 同时更新父 dir entry，mkdir 写 `dirs` 加空 `Vec<RemoteEntry>`，remove 删 file entry + 父 dir entry，rename 改 files key 同时更新 src / dst 两个父 dir entry。**不**含 `server_supports_posix_rename` mock（trait 已删，详 2.1）
- [x] 3.3 加 helper method `snapshot_write_counters(&self) -> FakeWriteCounters` 返回 (write/mkdir/remove/rename) 4 个 count，与既有 `snapshot_counters()` 对称；同时加 `add_dir(&self, parent, name)` / `add_file(&self, parent_dir, name, content)` / `read_file(&self, path) -> Option<Vec<u8>>` helper 让 memory CRUD test 可准备 fixture + 断言写入内容

## 4. cdt-api IPC method 实现

- [x] 4.1 `crates/cdt-api/src/ipc/local.rs::get_project_memory` 删 `if !policy.supports_memory { return Ok(empty) }` 短路分支（line 2632-2640）；保留 `let (fs, _, _, policy, _) = self.active_fs_and_policy().await?` 但 policy 字段不再用（保持 helper 调用为一致 dispatch 入口）
- [x] 4.2 `crates/cdt-api/src/ipc/local.rs::read_memory_file` 删 graceful skip 短路（line 2664-2668）
- [x] 4.3 `crates/cdt-api/src/ipc/local.rs` 加 IPC method `add_memory(&self, project_id: &str, file: &str, content: &str) -> Result<ProjectMemory, ApiError>`：
  - 调 `active_fs_and_policy()` 拿 fs
  - `validate_memory_file_name(file)?`
  - `let memory_dir = self.project_memory_dir(project_id).await?`
  - `fs.create_dir_all(&memory_dir).await?`
  - `fs.write_atomic(&memory_dir.join(safe_file), content.as_bytes()).await?`
  - 重新调 `discover_memory_layers(&*fs, &memory_dir).await?` + 拼 `ProjectMemory` 返
- [x] 4.4 `crates/cdt-api/src/ipc/local.rs` 加 IPC method `delete_memory(&self, project_id: &str, file: &str) -> Result<ProjectMemory, ApiError>`：
  - 调 `active_fs_and_policy()` 拿 fs
  - `validate_memory_file_name(file)?`
  - `let memory_dir = self.project_memory_dir(project_id).await?`
  - `fs.remove_file(&memory_dir.join(safe_file)).await?`（NotFound 错误向上传播为 `ApiError::not_found`）
  - 重新调 `discover_memory_layers` + 返 `ProjectMemory`
- [x] 4.5 `crates/cdt-api/src/ipc/local.rs::DataApi` trait 加 `async fn add_memory(...)` / `async fn delete_memory(...)` 签名
- [x] 4.6 `crates/cdt-api/src/ipc/local.rs::EXPECTED_TAURI_COMMANDS` 加 `add_memory` / `delete_memory` 两项

## 5. Tauri command + 前端 binding

- [x] 5.1 `src-tauri/src/lib.rs` 加 Tauri command wrapper `#[tauri::command] async fn add_memory(...)` / `delete_memory(...)`，登记到 `invoke_handler!` 宏
- [x] 5.2 `ui/src/lib/api.ts` 加 `addMemory(projectId, file, content): Promise<ProjectMemory>` / `deleteMemory(projectId, file): Promise<ProjectMemory>` 两个 export，`invoke("add_memory", { projectId, file, content })` / `invoke("delete_memory", { projectId, file })`
- [x] 5.3 `ui/src/lib/api.ts` 中 `mockApi` （如有）补 add/delete mock；`ui/src/test/setup.ts` 或 fixtures 同步 mockIPC clauses

## 6. 测试覆盖

- [x] 6.1 `crates/cdt-api/tests/ipc_contract.rs::active_ssh_context_reads_remote_projects_and_sessions` 把旧"`get_project_memory` 走 supports_memory=false graceful skip"段替换为：4 个 memory IPC（get_project_memory / read_memory_file / add_memory / delete_memory）顺序调用，前两者 + read 路径 counter 断言（read_dir/read 增量 ≥ 1），后两者 + write 路径 counter 断言（write/rename 各增量 ≥ 1，delete 触发 remove 增量 ≥ 1）；同 test 末尾加路径穿越 / 非 .md / 含 `/` 校验失败 SHALL 不触发任何 write op 的形态守护
- [x] 6.2 `add_memory` / `delete_memory` 返 `ProjectMemory` shape 在同一 6.1 test 内通过 `updated.layers.iter().any(|l| l.file == "...")` 直接断言；既有 `project_memory_serializes_camelcase` / `memory_file_content_serializes_camelcase` 序列化测试不变（仍守护 camelCase 契约）
- [x] 6.3 `crates/cdt-api/tests/ipc_contract.rs::EXPECTED_TAURI_COMMANDS` + `expected_tauri_commands_count_is_47`（原 45 → 47）同步更新
- [x] 6.4 SSH context 下 memory CRUD 端到端测试合并到 `ipc_contract.rs::active_ssh_context_reads_remote_projects_and_sessions`（**不**单开 `ssh_memory_crud.rs`）——同一 IPC dispatch test 已有 SSH fixture / context insert / counter 断言基础设施，复用更内聚；如未来需要更细粒度的 fault injection（rename 失败 / posix-rename 不支持降级 / write 中途断开）再单开 test 文件
- [x] 6.5 并发原子写测试合并到 `crates/cdt-fs/src/local.rs::tests::write_atomic_concurrent_writes_yield_intact_content`（**不**单开 `tests/write_atomic_concurrency.rs`）——`#[tokio::test(flavor = "multi_thread", worker_threads = 4)]` + spawn 8 个 task 同 path 写 16 KiB 不同内容，最终 read 是某一次写的整版（all bytes 同字节）+ tmp 残留检查
- [x] 6.6 校验失败覆盖通过 6.1 test 末尾 + 既有 `read_memory_file_rejects_path_traversal_and_non_markdown` inline test 共同守护——validate_memory_file_name 复用让两个写路径与 read 路径同语义
- [x] 6.7 既有 inline 单元测试 `project_memory_discovers_index_entries_and_orphans` / `project_memory_missing_dir_returns_empty` / `read_memory_file_rejects_path_traversal_and_non_markdown` 全绿——`cargo test --workspace` 跑通确认 Local 路径行为不退化

## 7. spec validate + lint

- [x] 7.1 `openspec validate ssh-project-memory-remote-rw --strict` 通过
- [x] 7.2 `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 7.3 `cargo test --workspace` 全绿（含 cdt-fs / cdt-ssh / cdt-api 三 crate 单测 + integration）
- [x] 7.4 `pnpm --dir ui run check` 通过（svelte-check 不报新错）
- [x] 7.5 `cargo xtask check-fs-direct-calls` 0 violation

## 8. 性能与 followups

- [x] 8.1 跑 `bash scripts/run-perf-bench.sh` 跑既有 baseline 确认 SSH list 路径未退化（add 写方法不在 hot path，read 路径删 supports_memory 短路只省一个 if，影响微）
- [x] 8.2 `openspec/followups.md` line 265-267 标 `[coverage-gap → done]`，注明本 change slug + 关键修法
- [x] 8.3 `openspec/followups.md` 新增 `[follow-up] memory-viewer UI 接入 add/delete 按钮` 条目，引用本 change 关闭的行为契约

## N. 发布

- [ ] N.1 push 分支 + 开 PR
- [ ] N.2 wait-ci 全绿
- [ ] N.3 codex 二审通过（如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [ ] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
