## ADDED Requirements

### Requirement: SSH 远端 memory CRUD 走真实 fs ops

系统 SHALL 在 SSH context 下完整支持 project memory CRUD：`get_project_memory` / `read_memory_file` / `add_memory` / `delete_memory` 四个 IPC method 在 active context 是 `Ssh<host>` 时 SHALL 通过当前 SSH `FileSystemProvider` 调用真实远端 fs ops，**不**得 graceful skip 返 `has_memory: false` / not_found。

`SshFileSystemProvider` SHALL 在 `cdt-fs::FileSystemProvider` trait 上实现 `write_atomic` / `create_dir_all` / `remove_file` 三个方法，行为契约：

- `write_atomic` SHALL 通过底层 SFTP 协议写到 `<path>.tmp.<atomic-seq-hex>.<pid-hex>`，写完调 SFTP rename 覆盖目标 path：
  - 优先走 `posix-rename@openssh.com` SFTP 扩展（`russh-sftp::SftpSession::extensions()` 在 connect 时探测，含此扩展则启用），由 OpenSSH server 提供 POSIX rename(2) 原子覆盖
  - 不支持时降级为两步：先 `client.remove(<target>)` 再 `client.rename(<tmp>, <target>)`——降级路径有极短窗口 reader 可能见 `target missing`，单次写场景 acceptable
  - rename 失败 SHALL best-effort 调 `remove_file(<tmp>)` 清理（清理失败不向上传播）
  - 服务端探测结果 SHALL 在 `SshFileSystemProvider` 内 cache 一次（per session），后续 `write_atomic` 直接读 cache 决策，不每次 connect 重探测
- `create_dir_all` SHALL 通过 SFTP 递归创建目录，对每段父目录先调 `try_exists` 探测，已存在跳过；缺失调 `mkdir` 创建。任何 SFTP rpc 失败 SHALL 走既有 retry 策略（`code=4 / EAGAIN / ECONNRESET / ETIMEDOUT / EPIPE` ≤ 3 次，指数退避 75ms × attempt）
- `remove_file` SHALL 通过 SFTP `SSH_FXP_REMOVE` 删文件；不存在 SHALL 返 `FsError::NotFound(path)`；路径是目录 SHALL 返 `FsError::Io { path, source: <ENOTEMPTY> }`，**不**递归删

`cdt-ssh::SftpClient` trait（位于 `crates/cdt-ssh/src/provider.rs`，**不**是独立 `sftp_client.rs` 文件）SHALL 新增 `write` / `mkdir` / `remove` / `rename` 四个方法，由 `RusshSftpClient` 实现 delegate 到 `russh_sftp::client::SftpSession::write` / `create_dir` / `remove_file` / `rename`。所有写操作 SHALL 与既有 read 操作复用同一 `Arc<dyn SftpClient>` + `Arc<SftpSession>`（**不**再用 `Arc<Mutex<SftpSession>>`，老 Mutex 已在前序 PR 移除——`SftpSession` 公共 API 是 `&self` 方法，message-id 由库内部 channel 维护）。SFTP message-id pipeline 并发支持留 PR-F；本 change 写路径与既有 read 路径同处一队列。

#### Scenario: SSH context 下 get_project_memory 走远端 read_dir + read_to_string

- **WHEN** active context 是 `Ssh<host>`，调用方调 `get_project_memory(project_id)`
- **THEN** 系统 SHALL 通过当前 SSH `FileSystemProvider` 调 `fs.read_dir(<remote_home>/.claude/projects/<base>/memory)` 列举 `.md` 文件
- **AND** 调 `fs.read_to_string(<memory_dir>/MEMORY.md)` 读 index 内容（如存在）
- **AND** 返回的 `ProjectMemory` SHALL 携带远端 layers 真实数据，`hasMemory` SHALL 为 `true`（当 memory 目录存在且含 `.md` 文件）
- **AND** 远端 fake provider 的 `read_dir_count` 与 `read_count` SHALL 各 ≥ 1

#### Scenario: SSH context 下 read_memory_file 走远端 read_to_string

- **WHEN** active context 是 `Ssh<host>`，调用方调 `read_memory_file(project_id, "MEMORY.md")`
- **THEN** 系统 SHALL 通过当前 SSH `FileSystemProvider` 调 `fs.read_to_string(<memory_dir>/MEMORY.md)`
- **AND** 返回的 `MemoryFileContent.content` SHALL 是远端文件内容，`filePath` SHALL 以远端 `<remote_home>` 为根
- **AND** SHALL NOT 返回 `ApiError::not_found` 含 "SSH context" 字样的占位错误（旧 graceful skip 文案）

#### Scenario: SSH context 下 add_memory 走远端 write_atomic + 自动创建 memory 目录

- **WHEN** active context 是 `Ssh<host>`，调用方调 `add_memory(project_id, "feedback_test.md", "content")` 且远端 `<memory_dir>` 当前不存在
- **THEN** 系统 SHALL 调 `fs.create_dir_all(<memory_dir>)` 确保目录就绪
- **AND** SHALL 调 `fs.write_atomic(<memory_dir>/feedback_test.md, content.as_bytes())` atomic 写入文件
- **AND** 写入完成后 SHALL 调 `discover_memory_layers(&*fs, &memory_dir)` 拿新 layers 列表
- **AND** 返回的 `ProjectMemory` SHALL 是写入后的最新状态（`hasMemory: true`，新文件出现在 `layers` 中）
- **AND** 远端 fake provider 的 `mkdir_count` SHALL ≥ 1（首次创建 memory 目录）；`write_count` 与 `rename_count` SHALL 各 ≥ 1（atomic write 对 tmp 文件 + rename）

#### Scenario: SSH context 下 delete_memory 走远端 remove_file

- **WHEN** active context 是 `Ssh<host>`，调用方调 `delete_memory(project_id, "feedback_test.md")` 且远端 memory 目录中存在该文件
- **THEN** 系统 SHALL 调 `fs.remove_file(<memory_dir>/feedback_test.md)`
- **AND** 删除完成后 SHALL 调 `discover_memory_layers(&*fs, &memory_dir)` 拿新 layers 列表
- **AND** 返回的 `ProjectMemory` SHALL 不再包含该文件
- **AND** 远端 fake provider 的 `remove_count` SHALL ≥ 1

#### Scenario: SSH context 下 add_memory 文件名校验拒绝路径穿越

- **WHEN** active context 是 `Ssh<host>`，调用方调 `add_memory(project_id, "../etc/passwd", "...")` 或 `add_memory(project_id, "secret.json", "...")`
- **THEN** 系统 SHALL 返 `ApiError::validation`，文案与 `read_memory_file` 路径穿越 / 非 `.md` 拒绝一致
- **AND** SHALL NOT 调任何远端 fs 写方法（`write_count` / `mkdir_count` / `rename_count` SHALL 全 0）

#### Scenario: SSH 写路径 transient 错误重试

- **WHEN** SFTP `write` / `mkdir` / `rename` / `remove` 任一 rpc 返回 `code=4 / EAGAIN / ECONNRESET / ETIMEDOUT / EPIPE`
- **THEN** 系统 SHALL 重试最多 3 次，每次间隔指数退避（75ms × attempt）
- **AND** 仍失败时 SHALL 把错误向上抛给调用方，封装为 `FsError::TransientExhausted { attempts: 3, last_reason }`

#### Scenario: SSH write_atomic rename 失败 best-effort 清理 tmp

- **WHEN** `SshFileSystemProvider::write_atomic(path, content)` 在写完 tmp 后调 SFTP rename 失败（非 transient，已重试 3 次）
- **THEN** 系统 SHALL 调 `fs.remove_file(<tmp_path>)` best-effort 清理 tmp 文件
- **AND** 清理失败 SHALL 不向上传播 error（rename 失败已是 primary error）
- **AND** 向调用方抛 `FsError::TransientExhausted { attempts: 3, last_reason }` 或对应 SFTP error
