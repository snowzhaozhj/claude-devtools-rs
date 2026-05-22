## ADDED Requirements

### Requirement: SSH `open_read` 大文件走 K-worker prefetch streaming reader

`SshFileSystemProvider::open_read`（`cdt-fs::FileSystemProvider` trait 实现）对**生产路径**（即 `self.sftp` 字段为 `Some(Arc<SftpSession>)`，由 `SshFileSystemProvider::new` 构造）且**大文件**（`sftp.metadata(path).len() >= SFTP_PIPELINE_MIN_BYTES`，当前钉死 256 KiB）SHALL 返回一个**流式 K-worker prefetch reader**——内部由 K=`SFTP_PIPELINE_MAX_WORKERS`（当前 16）个 tokio task 并发飞独立 SFTP `SSH_FXP_READ`，把读到的 chunk 经有界 channel 推给消费侧，使得 reader 的 peak RSS 与 K 成正比而非与 `file_size` 成正比。

K-worker SHALL 用 **round-robin chunk 分派**：第 `i` 个 chunk（chunk 大小 `SFTP_PIPELINE_CHUNK_BYTES`=32 KiB）由 `worker_id = i % n_workers` 处理；消费侧按 `next_worker = (next_worker + 1) % n_workers` 顺序取——保证消费速度推进的同时所有 K 个 worker 都能持续被 backpressure 释放，wall ≈ `ceil(file_size / chunk_bytes / n_workers)` × RTT 与 PR-F 全量预取 baseline 持平。

**Limited 降级**：K 个 `sftp.open` 用 `futures::future::join_all` 预并发打开时若任一返回 `russh_sftp::client::error::Error::Limited(reason)`（server 端 SFTP `open_handles` 上限），`open_read` SHALL 降级到**单 handle 流式**——优先**复用** `join_all` 已成功返回的第 1 个 `russh_sftp::client::fs::File`（avoid 再开一次 `sftp.open` 撞同样 Limited + avoid 依赖 `File::drop` 同步 close 语义）；该类型实现 `tokio::io::AsyncRead`，直接 `Box::new(file)` 返；wall 退化到 `N × RTT` 但 peak RSS 仍受限单 chunk。partial_handles 仅在所有 K 个 open 都 Limited 时为空，此时 SHALL 显式 `sftp.open(path).await` 再开一次（接受可能继续 Limited 上抛）。降级 SHALL 通过 `tracing::warn!` 记录 path / workers / partial_handle_count / reason 让运维侧可见。

**小文件路径**：生产路径 + `file_size < SFTP_PIPELINE_MIN_BYTES` 仍 SHALL 走单 RTT `sftp.read(path)` 全量预取 + `Cursor::new` 包装——避免 K 个 `sftp.open` 的 spawn overhead 对小文件 wall 无收益反加 latency。

**Fake 测试路径**：`SshFileSystemProvider::with_client` 构造的 `self.sftp == None` 实例 SHALL 走 `SftpClient::read(path)` trait 方法 + `Cursor::new` 包装的原有路径——保留 `CountedFakeRemoteSftp::read_count` 等 op counter 语义，让现有 `crates/cdt-api/tests/perf_ssh_cache_hit.rs` 与 `crates/cdt-api/tests/perf_ssh_scanner_chunked_read.rs` 的 fake 路径断言**无需更新**。

**inherent `open_read_stream` 保留**：`SshFileSystemProvider::open_read_stream` (`pub async fn` 返 `russh_sftp::client::fs::File`) 行为不变——caller 显式调用拿原生 SFTP 句柄路径不受本 change 影响。

#### Scenario: 大文件生产路径返流式 K-worker prefetch reader

- **WHEN** caller 在生产构造（`SshFileSystemProvider::new(ctx, Arc<SftpSession>, remote_home)`）的 provider 上调 `open_read(path)` 且 `sftp.metadata(path).len() >= SFTP_PIPELINE_MIN_BYTES`
- **THEN** 返回的 `Box<dyn AsyncRead + Send + Unpin>` SHALL 是 `PipelinedSftpReader` 包装而非 `std::io::Cursor<Vec<u8>>`
- **AND** reader 实例 SHALL 持有 `n_workers = min(SFTP_PIPELINE_MAX_WORKERS, ceil(size / SFTP_PIPELINE_CHUNK_BYTES)).max(1)` 个 mpsc receiver（每个 capacity = 1）+ `JoinSet<()>` 拥有 K 个 worker task
- **AND** caller 持续 `poll_read` 直到 EOF 期间，进程 peak RSS 增量 SHALL ≤ `n_workers × 2 × SFTP_PIPELINE_CHUNK_BYTES`（最坏：每个 channel 1 个 buffered chunk + 每个 worker 1 个 in-flight chunk）

#### Scenario: 大文件 round-robin chunk 分派保 wall parity

- **WHEN** `PipelinedSftpReader::open` 启动 K worker
- **THEN** 第 `i` 个 chunk SHALL 由 worker `i % n_workers` 处理（worker 0 读 chunks [0, K, 2K, ...]，worker 1 读 chunks [1, K+1, 2K+1, ...]，依此类推）
- **AND** 消费侧 `poll_read` SHALL 按 `next_worker = (next_worker + 1) % n_workers` 严格轮询，确保消费推进直接释放每个 worker 的 backpressure
- **AND** total wall time SHALL ≈ `ceil(n_chunks / n_workers) × RTT`（与 PR-F 全量预取 baseline 持平，不退化为 `n_chunks × RTT` 串行）

#### Scenario: 小文件生产路径走单 RTT 全量预取

- **WHEN** caller 在生产构造的 provider 上调 `open_read(path)` 且 `sftp.metadata(path).len() < SFTP_PIPELINE_MIN_BYTES`
- **THEN** 实现 SHALL 调 `sftp.read(path).await` 拿全 `Vec<u8>`
- **AND** SHALL 返 `Box::new(std::io::Cursor::new(bytes))`
- **AND** SHALL NOT spawn K worker / 不创建 `PipelinedSftpReader`

#### Scenario: Fake 测试路径走 `SftpClient::read` 全量保 op counter 语义

- **WHEN** caller 在 fake 构造（`SshFileSystemProvider::with_client(ctx, Arc<dyn SftpClient>, remote_home)`）的 provider 上调 `open_read(path)`
- **THEN** 实现 SHALL 调 `self.client.read(path).await`（trait 方法）拿全 `Vec<u8>`
- **AND** SHALL 返 `Box::new(std::io::Cursor::new(bytes))`
- **AND** `CountedFakeRemoteSftp::read_count` 在此次调用后 SHALL 增 1（与 PR-F 前 PR-D 时期语义一致）

#### Scenario: `SftpError::Limited` 降级到单 handle 流式且优先复用已开 handle

- **WHEN** `PipelinedSftpReader::open` 内部用 `futures::future::join_all` 并发 K 个 `sftp.open`，收齐 `Vec<Result<File, SftpError>>` 后任一为 `Err(russh_sftp::client::error::Error::Limited(reason))`（server SFTP `open_handles` 限制）
- **THEN** `open_read` SHALL 降级到单 handle 流式：**优先复用 `Vec` 中已成功打开的第 1 个 `File`**（避免再次 `sftp.open` 撞同样 Limited，避免依赖 `russh_sftp::client::fs::File::drop` 的同步 close 语义）
- **AND** 若所有 K 个 `sftp.open` 都 Limited（如 K=1 时罕见场景），SHALL 显式 `sftp.open(path).await` 重试 1 次；若仍 Limited 上抛 `FsError::Io { ErrorKind::Other }`
- **AND** SHALL 把 `Vec` 中其余成功的 `File` 直接 drop 让 russh-sftp 自身释放（接受 best-effort close 的潜在短暂 server 端 handle leak，ms 量级，SSH 连接关闭时彻底释放）
- **AND** SHALL 通过 `tracing::warn!(path, workers, partial_handle_count, reason, ...)` 记录降级事件
- **AND** caller 仍能完整流式读到 EOF，peak RSS 不会超过单 chunk + tokio File 内部 buffer

#### Scenario: 任一 worker channel close 时立即按字节计数判定真 EOF 防 silent truncation

- **WHEN** `PipelinedSftpReader` 内 round-robin 轮到的 `next_worker` 对应 `mpsc::Receiver` 在 `poll_recv` 返 `None`（该 worker 正常退出或 panic 后 sender drop；round-robin 顺序保证此刻 stream 已无该位置的后续 chunk）
- **THEN** consumer SHALL **立即**（不等其它 worker 全 close）比较累计写入字节 `total_bytes_read` 与构造时记录的 `total_bytes_expected`（= `sftp.metadata(path).len()`）
- **AND** 若 `total_bytes_read == total_bytes_expected` → SHALL 返 `Poll::Ready(Ok(()))` 不写入 `ReadBuf`（标准 AsyncRead EOF 语义）；reader 标 `eof = true` 后续 poll_read 持续返 EOF
- **AND** 若 `total_bytes_read < total_bytes_expected` → SHALL 返 `Poll::Ready(Err(io::Error::new(ErrorKind::UnexpectedEof, format!("PipelinedSftpReader closed early: expected {} bytes, got {}", expected, read))))`（防 worker 静默退出 / JoinSet 异常 abort 让 caller 误把短读当 EOF）；reader 标 `error_seen = true` 后续 poll_read 返终态错误
- **AND** SHALL NOT 等所有 K 个 receiver 都 close 才判定（继续等会让 consumer hang 死等其它仍在飞 next-round chunk 的 worker）

#### Scenario: 生产路径分支选择钉死小文件 / 大文件 / fake 三 branch wiring

- **WHEN** 调用 `pub(crate) fn pick_open_read_strategy(has_sftp: bool, size: u64) -> OpenReadStrategy`
- **THEN** `(has_sftp=true, size >= SFTP_PIPELINE_MIN_BYTES)` SHALL 返 `OpenReadStrategy::Streaming { n_workers: usize }`
- **AND** `(has_sftp=true, size < SFTP_PIPELINE_MIN_BYTES)` SHALL 返 `OpenReadStrategy::SmallFileBuffered`
- **AND** `(has_sftp=false, _)` SHALL 返 `OpenReadStrategy::FakeBuffered`（fake 测试路径所有 size 都走 `client.read`）
- **AND** 此分支函数 SHALL 在 `crates/cdt-ssh/src/provider.rs::tests` 内有单元测试覆盖以上 4 个组合，拦截"未来 PR 误把生产大文件 branch 接到 client.read 旧路径"类 wiring 回归

#### Scenario: Worker 内部 SFTP 错误经 channel 传 `io::Error` 给消费侧

- **WHEN** 已构造的 `PipelinedSftpReader` 在某 worker 内 `file.seek(SeekFrom::Start(offset))` 或 `file.read_exact(&mut buf)` 调用返 `Err`
- **THEN** worker SHALL 把错误转换为 `tokio::io::Error::other(format!(...))` 并通过 `tx.send(Err(io_err)).await` 推给对应 receiver
- **AND** worker SHALL 然后 `return`（drop sender，channel close）
- **AND** consumer 的 `poll_read` 收到 `Poll::Ready(Some(Err(io_err)))` 时 SHALL 返 `Poll::Ready(Err(io_err))`
- **AND** SHALL NOT silent drop 错误（如 worker panic 后丢失错误信号让 consumer hang）

#### Scenario: Reader drop 联级 abort 所有 worker

- **WHEN** `PipelinedSftpReader` 持有者 drop reader（典型场景：上游 `parse_file_via_fs` 内部 `BufReader<reader>` 提前结束 / 上游 `tokio::spawn` task 被 abort）
- **THEN** `PipelinedSftpReader::_workers: JoinSet<()>` 字段 drop SHALL 触发所有未完成 worker task 的 `AbortHandle::abort`
- **AND** worker 内任一 `.await` 点（典型 `sftp.read_exact` / `tx.send`）SHALL 在下次 poll 时被 cancellation 返回 abort
- **AND** SHALL NOT 留 orphan task 在 tokio runtime 继续读 SFTP 浪费带宽

#### Scenario: EOF 通过 next round-robin worker channel close + 字节计数表达

- **WHEN** worker 处理完自己分到的最后一个 chunk 并通过 `tx.send(Ok(bytes)).await` 成功推给 consumer
- **THEN** worker SHALL `return`（自然 drop sender）
- **AND** consumer 下次 round-robin 轮询到该 worker 的 receiver 时 `mpsc::Receiver::poll_recv` SHALL 返 `None`
- **AND** consumer SHALL **立即**触发字节计数判定（如上 Scenario "任一 worker channel close 时立即按字节计数判定真 EOF 防 silent truncation"），不等其它 worker close；正常退出场景下此时 `total_bytes_read == total_bytes_expected` → 翻译为 `Poll::Ready(Ok(()))` 不再写入 `ReadBuf`（标准 AsyncRead EOF 语义）

#### Scenario: 大会话 scanner BufReader 接 `PipelinedSftpReader` 不破契约

- **WHEN** `cdt-parse::parse_file_via_fs` 在 SSH 生产路径下调 `fs.open_read(path)` 拿 reader，再 `BufReader::with_capacity(SCANNER_BUF_BYTES, reader)` 包装（容量 32 KiB 与 SFTP packet 上限对齐）
- **THEN** reader 实际是 `PipelinedSftpReader`，每次 `BufReader::fill_buf` SHALL 从 `PipelinedSftpReader::poll_read` 拿到下一个 32 KiB chunk（K-worker prefetch 提前飞 read 已让 chunk 通常在 channel 中就绪）
- **AND** scanner 全文 parse 完成 → `BufReader` drop → `PipelinedSftpReader` drop → JoinSet drop → worker cleanup
- **AND** 与 PR-F 全量预取 baseline 对比：scanner wall ≈ 持平（K-worker 并发数 + chunks_per_worker 不变）；进程 peak RSS 增量 SHALL 从 ≈ file_size 降到 ≈ `n_workers × 2 × 32 KiB`
