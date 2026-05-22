## Why

`openspec/followups.md` line 265-267 记录的 coverage-gap：change `fix-ssh-active-context-dispatch` 让 SSH context 下 `get_project_memory` 返 `has_memory=false / empty layers`、`read_memory_file` 返 `not_found`，UI 直接隐藏 memory 入口——SSH 用户根本看不到远端项目 memory。这是 graceful skip 而非真正实现。

`discover_memory_layers` 当前已经接 `&dyn FileSystemProvider`，缺的不是抽象层，而是：
1. `BackendPolicy::for_ssh().supports_memory = false` 把整个 IPC 在 dispatch 入口短路
2. `FileSystemProvider` trait 是**纯只读**的——没有 `write_atomic` / `create_dir_all` / `remove_file` 方法，无法实现 add/delete memory 写路径
3. `SshFileSystemProvider` / `SftpClient` trait 只暴露 read 方法，远端 SFTP write / mkdir / remove 能力未开放

完整 memory CRUD 远端支持是行为契约级 + 基础设施级改动，必须走 OpenSpec。

## What Changes

- 扩展 `cdt-fs::FileSystemProvider` trait，新增 3 个写方法（`write_atomic` / `create_dir_all` / `remove_file`），保持 dyn-safe；**所有现有 `impl FileSystemProvider`** 都 SHALL 实现：`LocalFileSystemProvider`（`crates/cdt-fs/src/local.rs`）、`SshFileSystemProvider`（`crates/cdt-ssh/src/provider.rs`）、`InstrumentedFs<P>`（`crates/cdt-fs/src/instrumentation.rs`）、测试侧 `SpyFs`（`crates/cdt-discover/tests/project_scanner.rs`）、`FakeSshFs`（`crates/cdt-api/src/ipc/session_metadata.rs`）；`InstrumentedFs` 的 `FsOpCounts` 同步加 `write_atomic / create_dir_all / remove_file` 三个 `AtomicU64` 计数字段
- 写路径 SHALL 是 atomic（tmp file + rename）跨 backend 一致——SFTP 也走 `<path>.tmp.<rand>` + `SSH_FXP_RENAME`
- 扩展 `cdt-ssh::SftpClient` trait（在 `crates/cdt-ssh/src/provider.rs` 内）+ `RusshSftpClient` 实现，新增 `write` / `mkdir` / `remove` / `rename` 方法
- 改 `BackendPolicy::for_ssh().supports_memory = false → true`：SSH 不再 graceful skip memory，走真实远端 fs 调用
- `LocalDataApi::get_project_memory` / `read_memory_file` 删除 supports_memory 短路分支，统一调 `discover_memory_layers(&*fs, ...)`（已经接 fs trait）
- **新增** `add_memory(project_id, file, content)` / `delete_memory(project_id, file)` 两个 IPC method（TS 原版没有这两个方法，本 change 扩 spec）：
  - `add_memory` 验证文件名 + atomic 写入 + 返回新 `ProjectMemory`（前端无需再调 `get_project_memory`）
  - `delete_memory` 验证文件名 + 删除文件 + 返回新 `ProjectMemory`
  - 两个 method SHALL 同步登记到 `EXPECTED_TAURI_COMMANDS` + Tauri `invoke_handler!` + 前端 `ui/src/lib/api.ts`
- 扩展测试 helper `CountedFakeRemoteSftp`：新增 `write_count` / `mkdir_count` / `remove_count` / `rename_count` AtomicUsize + 实现 write/mkdir/remove/rename mock + 内存 `written_files: Mutex<HashMap<String, Vec<u8>>>`
- IPC contract test 覆盖：SSH context 下 4 个 memory IPC 全走远端 fs；ssh-remote-context spec 新增 SSH memory CRUD Scenario；fs-abstraction spec MODIFY 把"trait 暴露 7-9 个核心方法"扩成 12 方法
- xtask `check-fs-direct-calls` allowlist 不变（写方法仍是 trait 内部调用，业务路径直接调 `fs.write_atomic` 不需要新例外）
- **本 change 不接 UI add/delete 按钮**——memory-viewer 当前只 read，UI 加 add/delete 按钮留 followup change（spec 行为已就位，UI 演进可独立推进）

## Capabilities

### New Capabilities

无（不引入新 capability）。

### Modified Capabilities

- `fs-abstraction`: `FileSystemProvider` trait 新增 3 个写方法；`BackendPolicy::for_ssh()` 的 `supports_memory` 字段值改 `true`；`xtask check-fs-direct-calls` 的 13 个 forbidden patterns 范围保持不变
- `ssh-remote-context`: 新增 SSH 远端 memory CRUD Requirement——`SshFileSystemProvider::write_atomic` / `create_dir_all` / `remove_file` 行为契约 + atomic 语义保证（rename via `SSH_FXP_RENAME`）+ retry 策略（与现有 `SFTP transient errors are retried` 对齐）
- `ipc-data-api`: 把 `Expose memory read operations` Requirement 扩成 `Expose memory CRUD operations`，新增 `add_memory` / `delete_memory` Scenario + `EXPECTED_TAURI_COMMANDS` 覆盖；`Dispatch project/session reads by active context` Requirement 把"`project_memory_dir`"加上 add/delete IPC 一同 SHALL 走 active SSH provider
- `memory-viewer`: 新增 `Operate memory CRUD over current backend` Requirement——SSH context 下 layers/read 行为与 Local 等价；add/delete IPC 行为契约（不规范 UI，UI 留 followup）

## Impact

- **Code**：
  - `crates/cdt-fs/src/provider.rs` 加 3 个 trait 方法（含 default impl 兜底）+ `local.rs` 实现 + xtask check 不变
  - `crates/cdt-ssh/src/provider.rs`（含 `SftpClient` trait 与 `RusshSftpClient` 实现）加 SFTP write/mkdir/remove/rename
  - `crates/cdt-api/src/ipc/local.rs` 删 supports_memory 短路；新增 add_memory / delete_memory IPC method + Tauri command；`EXPECTED_TAURI_COMMANDS` +2 项
  - `src-tauri/src/lib.rs` `invoke_handler!` +2 项
  - `ui/src/lib/api.ts` 加 add/delete IPC binding（仅 binding，不接 UI 按钮）
  - `crates/cdt-api/tests/common/fake_remote_sftp.rs` 加 4 类 op counter + write/mkdir/remove/rename mock
  - `crates/cdt-api/tests/ipc_contract.rs` SSH dispatch test 加 memory CRUD 覆盖；新增 `crates/cdt-api/tests/ssh_memory_crud.rs` 端到端
- **Spec**：MODIFY `fs-abstraction` / `ssh-remote-context` / `ipc-data-api` / `memory-viewer` 4 个 capability 的 spec
- **Followups**：本 change 关闭 followups.md line 265-267 的 coverage-gap，标 `→ done`；同时新增 followup "memory-viewer UI add/delete button 接入" 等待 UI change
- **BREAKING**：无——supports_memory=false → true 是 graceful 升级，前端 SSH context 下原本就调用 IPC（拿到 has_memory=false），现在拿到真实数据，UI 路径不变
