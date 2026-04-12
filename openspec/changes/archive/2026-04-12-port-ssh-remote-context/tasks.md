## 1. 依赖 + 脚手架

- [x] 1.1 在 `cdt-ssh/Cargo.toml` 添加 `cdt-discover`（`FileSystemProvider` trait）、`async-trait`、`serde`、`serde_json`、`dirs` 依赖
- [x] 1.2 建立 `cdt-ssh/src/` module 结构：`lib.rs`、`config_parser.rs`、`connection.rs`、`provider.rs`、`error.rs`
- [x] 1.3 `cargo build -p cdt-ssh` 确认编译通过

## 2. Error 类型 + SSH config 解析

- [x] 2.1 在 `error.rs` 定义 `SshError` thiserror enum（`Io`、`Config`、`Connection`、`Sftp`、`Auth`）
- [x] 2.2 在 `config_parser.rs` 实现 `SshHostConfig { hostname, user, port, identity_files }` 和 `parse_ssh_config(path) -> Vec<SshHostConfig>`
- [x] 2.3 实现 `resolve_host(configs, alias) -> Option<SshHostConfig>`
- [x] 2.4 实现 `list_hosts(configs) -> Vec<String>`（排除通配符 `*`）
- [x] 2.5 单元测试：解析多 Host、通配符排除、alias 解析、缺失字段默认值

## 3. 连接状态 + 管理

- [x] 3.1 在 `connection.rs` 定义 `ConnectionState`（`Disconnected`/`Connecting`/`Connected`/`Error(String)`）和 `ConnectionStatus { context_id, state, host, user }`
- [x] 3.2 实现 `SshConnectionManager`：`new`、`connect`（状态机转换 + 记录连接信息）、`disconnect`、`test_connection`、`get_status`
- [x] 3.3 实现 context 切换：`get_active_context`、`set_active_context`
- [x] 3.4 单元测试：状态转换、connect→disconnect 循环、test_connection 不改变活跃 context

## 4. SshFileSystemProvider

- [x] 4.1 在 `provider.rs` 定义 `SshFileSystemProvider` struct，实现 `cdt_discover::FileSystemProvider` trait 的所有 6 个方法（`kind`/`exists`/`read_dir`/`read_to_string`/`stat`/`read_lines_head`）
- [x] 4.2 所有方法暂实现为返回 `FsError::Io` 的 placeholder（完整 SFTP 集成需要真实 SSH 连接，留给后续 integration test）
- [x] 4.3 `kind()` 返回 `FsKind::Ssh`
- [x] 4.4 单元测试：`kind()` 返回 `Ssh`、placeholder 方法返回错误

## 5. lib.rs 导出 + 集成

- [x] 5.1 在 `lib.rs` 通过 `pub use` 导出公开 API
- [x] 5.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 5.3 `cargo fmt --all`
- [x] 5.4 `cargo test -p cdt-ssh` 全测试通过

## 6. 文档 + 收尾

- [x] 6.1 更新根 `CLAUDE.md` 的 Capability→crate map：`ssh-remote-context` → `done ✓`
- [x] 6.2 `openspec validate port-ssh-remote-context --strict`
