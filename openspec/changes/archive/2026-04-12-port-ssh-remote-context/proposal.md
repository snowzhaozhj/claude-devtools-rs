## Why

`ssh-remote-context` 是第 11 个 capability（剩余 3 个之一）。`cdt-discover` 已定义 `FileSystemProvider` trait + `FsKind::Ssh` 枚举，`cdt-ssh` 是 stub。本 port 实现 SSH config 解析、连接管理、`SshFileSystemProvider`（实现 `FileSystemProvider` trait）、连接状态报告。

TS followups 标记为"完全匹配"——无 impl-bug 需修。

## What Changes

- 在 `cdt-ssh` 实现：
  - `SshConfigParser`：解析 `~/.ssh/config` Host alias → hostname/user/port/identity files
  - `ConnectionState` 状态枚举 + `ConnectionStatus` 报告
  - `SshConnectionManager`：connect/disconnect/test/get_status + 状态机
  - `SshFileSystemProvider`：实现 `cdt-discover::FileSystemProvider` trait over SFTP
- 新增依赖：`russh`（SSH 协议）+ `russh-sftp`（SFTP）+ `cdt-discover`（`FileSystemProvider` trait）

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
（无——按原 spec 实现）

## Impact

- **代码**：`crates/cdt-ssh/src/` 从 stub 扩展为完整实现
- **依赖**：新增 `russh`、`russh-sftp`、`async-trait`、`cdt-discover`
- **下游**：`cdt-api` 层调用 `SshConnectionManager` 切换 context，传递 `SshFileSystemProvider` 给 `ProjectScanner`
