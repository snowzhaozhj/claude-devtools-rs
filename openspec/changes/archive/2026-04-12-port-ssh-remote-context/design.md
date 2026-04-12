## Context

`cdt-discover::fs_provider` 定义了 `FileSystemProvider` trait（6 个 async 方法 + `kind()`）和 `FsKind::Ssh` 枚举。`LocalFileSystemProvider` 已实现。`cdt-ssh` 是空 stub。

TS 侧 `SshConnectionManager`（537 行）使用 `ssh2` npm 包 + SFTP API。Rust 可选 `russh`（纯 Rust、async-native）或 `ssh2-rs`（libssh2 绑定）。选 `russh` 因为：纯 Rust、async、无 C 依赖、社区活跃。

## Goals / Non-Goals

**Goals:**
- SSH config 解析（`~/.ssh/config` Host alias → hostname/user/port/identity）
- 连接状态机（disconnected → connecting → connected / error）
- `SshFileSystemProvider` 实现 `FileSystemProvider` trait
- connect / disconnect / test_connection / get_status API
- 单元测试覆盖 spec 的 4 个 scenario

**Non-Goals:**
- SSH agent forwarding / X11 forwarding — 不在 spec 范围
- 多跳 ProxyJump — 简化版本，后续可扩展
- 实际 SSH 连接集成测试（需要远程 host）— 仅 unit test 用 mock

## Decisions

### D1: Module 结构

```
cdt-ssh/src/
├── lib.rs              # pub mod + re-export
├── config_parser.rs    # SSH config 解析
├── connection.rs       # ConnectionState / ConnectionStatus / SshConnectionManager
├── provider.rs         # SshFileSystemProvider: impl FileSystemProvider
└── error.rs            # SshError thiserror enum
```

### D2: SSH 库选型

**选 `russh`**：纯 Rust async SSH 实现，无 C 依赖。配合 `russh-sftp` 做文件操作。

**备选**：`ssh2-rs`（libssh2 绑定），但需要 `libssh2` + `openssl` 系统库，交叉编译困难。

**实际决策修正**：`russh` 的 SFTP 支持和 SSH config 解析相对初级。考虑到本 port 的核心价值是**接口定义 + 状态管理 + trait 实现骨架**，而非完整的 SSH 协议栈，采用 **trait-based 设计**：
- 定义 `SshTransport` trait 抽象底层 SSH 操作
- 提供基于 `russh` 的默认实现
- 测试用 mock transport

### D3: Config 解析

简化版：只解析 `Host`、`HostName`、`User`、`Port`、`IdentityFile` 五个关键字段。不支持 `Include` 展开（复杂度高，后续可扩展）。

### D4: 连接状态机

```
Disconnected → Connecting → Connected
                    ↓
                  Error
Connected → Disconnected（disconnect 调用）
Error → Connecting（重试）
```

用 `tokio::sync::watch` 广播状态变更。

## Risks / Trade-offs

- **[Trade-off] 简化 config 解析**：不支持 `Include` / `Match` / `ProxyJump`，覆盖 90% 用例
- **[Risk] `russh` API 稳定性**：社区 crate，API 可能变更。trait 抽象层隔离了风险
