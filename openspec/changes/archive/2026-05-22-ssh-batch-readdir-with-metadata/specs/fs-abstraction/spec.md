## MODIFIED Requirements

### Requirement: `FileSystemProvider` trait 暴露 7 个核心方法

系统 SHALL 在 `cdt-fs::FileSystemProvider` trait 上暴露以下方法（编译时强制实现，default 实现可被 override）：

1. `fn kind(&self) -> FsKind`
2. `async fn exists(&self, path: &Path) -> bool`
3. `async fn read_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>`
4. `async fn read_dir_with_metadata(&self, path: &Path) -> Result<Vec<DirEntry>, FsError>`（default 实现：`read_dir` + 逐项 `stat`，N+1 RTT 兜底；后端 SHALL override 当 backend 协议原生支持 1 RTT 拿 dir + 全 entry attrs，避免性能退化）
5. `async fn read_to_string(&self, path: &Path) -> Result<String, FsError>`
6. `async fn stat(&self, path: &Path) -> Result<FsMetadata, FsError>`
7. `async fn read_lines_head(&self, path: &Path, max: usize) -> Result<Vec<String>, FsError>`
8. `async fn open_read(&self, path: &Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, FsError>`（新增，替代 SSH `open_read_stream` 破抽象）
9. `async fn stat_many(&self, paths: &[&Path]) -> Vec<Result<FsMetadata, FsError>>`（新增 batched API，default 实现走 `futures::future::join_all`）

trait SHALL 保持 dyn-safe（`&dyn FileSystemProvider` 可用），不引入关联类型。

**`read_dir_with_metadata` override 契约**（change `ssh-batch-readdir-with-metadata` 引入）：

- **SSH override 复用 read_dir 语义**：`SshFileSystemProvider::read_dir_with_metadata` SHALL override default impl 并直接调 `self.read_dir(path)`，复用 SFTP READDIR reply 自带的 entry attrs（详 ssh-remote-context spec `Read sessions and files over SSH with same contract` + change `ssh-batch-readdir-with-metadata` design D1）
- **missing mtime 语义**：override 后部分 entry 若 metadata = None（SFTP server 未返 mtime），caller SHALL 把此条视同 cache mismatch（走 cache wrapper miss 路径补齐），实现 SHALL NOT 在 trait 实现层做 per-entry stat fallback（否则退化为 N+1 RTT）

#### Scenario: open_read 在 Local 上返回 tokio::fs::File 包装

- **WHEN** caller 在 `LocalFileSystemProvider` 上调 `open_read(path)` 且 path 存在可读
- **THEN** SHALL 返回 `Ok(Box::new(tokio::fs::File))` 或等价包装
- **AND** 返回的 reader 实现 `AsyncRead + Send + Unpin`，caller SHALL 能用 `BufReader::new(reader).lines()` 流式按行读取

#### Scenario: open_read 在 SSH 上走 SFTP 流式句柄

- **WHEN** caller 在 `SshFileSystemProvider` 上调 `open_read(path)`
- **THEN** SHALL 返回 SFTP 句柄包装（`russh_sftp::client::fs::File` 或 wrapper），实现 `AsyncRead + Send + Unpin`
- **AND** caller SHALL NOT 需要 downcast 到 `SshFileSystemProvider` 才能流式读

#### Scenario: stat_many 默认实现走 join_all

- **WHEN** caller 调 `fs.stat_many(&[p1, p2, p3])` 且 provider 未 override
- **THEN** 实现 SHALL 通过 `futures::future::join_all` 并发 `fs.stat(p)` 所有 path
- **AND** 返回 `Vec<Result<FsMetadata, FsError>>` 顺序与 input paths 严格对应

#### Scenario: stat_many 在 SSH 上当前是假 batch

- **WHEN** caller 在 `SshFileSystemProvider` 上调 `stat_many(&[p1, p2, ...])`
- **THEN** 实现 SHALL 使用 trait default `join_all`
- **AND** 由于底层 `Arc<Mutex<SftpSession>>` 全锁，实际执行仍是串行 N 次 RTT —— 此限制属已知，留 PR-F（SSH session pipeline）解决；trait 契约层面 caller SHALL 一律调 `stat_many` 而非循环 `stat`

#### Scenario: read_dir_with_metadata default impl 是 N+1 RTT 兜底

- **WHEN** 某后端未 override `read_dir_with_metadata`
- **THEN** SHALL 走 trait default：先 `read_dir(path)` 拿 entries 列表，再对每条 `entry.kind.is_file()` 调 `stat(path.join(entry.name))` 补 metadata
- **AND** 总 op 数 SHALL 为 1 + N（N = file entry 数）
- **AND** 此 default impl 仅作为 trait 兜底；性能敏感 backend SHALL override 以避免 N+1 退化

#### Scenario: SSH override read_dir_with_metadata 复用 read_dir 不退化

- **WHEN** 在 SSH context 下调 `fs.read_dir_with_metadata(<remote_dir>)`
- **THEN** `SshFileSystemProvider` SHALL override default impl，直接 delegate 到 `self.read_dir(path).await`
- **AND** 底层 SFTP `SSH_FXP_READDIR` reply 1 个 RTT 返完整 dir 内容 + 每个 entry 的 attrs（size/mtime）
- **AND** 总 SFTP RTT 数 SHALL = 1（**SHALL NOT** 退化为 N+1，即使部分 entry mtime missing）
- **AND** 缺 mtime 的 entry（SFTP server 未返 modify time）SHALL 在 `DirEntry.metadata = None` 状态返给 caller，由 caller 上层语义（如 cache batch 校验）决定 fallback 路径——**SHALL NOT** 在 trait 实现层补 stat
