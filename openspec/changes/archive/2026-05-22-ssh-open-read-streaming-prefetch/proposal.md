## Why

PR-F SFTP message-id pipeline（change `ssh-batch-readdir-with-metadata` 之后追加的优化）把 `SshFileSystemProvider::open_read` 对大文件（≥ `SFTP_PIPELINE_MIN_BYTES`=256K）从串行多 RTT 改成 K=16 worker 并发飞 `SSH_FXP_READ`，wall 从 8.36s 压到 ~600ms。但实现里 `read_pipelined` 是**全量预取**——K 个 worker 各读完自己分到的连续 byte range 后 `try_join_all` 合并成 `Vec<u8>`，再用 `Cursor::new` 包装返回。后果：5MB jsonl 进 RSS 5MB，文件越大 peak 越高（`provider.rs:300-301` 注释自承 trade-off）。

TS 原版（`src/main/utils/jsonl.ts:62`、`metadataExtraction.ts:49`、`SessionContentFilter.ts:69` 全部用 `ssh2.sftp.createReadStream` + ssh2 库内置的 readahead）peak RSS 只有 ~64K。Rust 端 wall 已经追上来了，但 peak RSS 留作 PR-F 的自留 limitation（`openspec/followups.md::286-290` 第二条），现在补齐：让 `open_read` 内部 K-worker 并发飞 `SSH_FXP_READ` 的同时**走流式**——bounded ring buffer + 背压消费——peak RSS 与 K 而非 file_size 成正比。

## What Changes

- 新增内部 `PipelinedSftpReader`：实现 `tokio::io::AsyncRead`，持有 K 个 `mpsc::Receiver<Result<Vec<u8>, io::Error>>`（每个 receiver capacity = 1）+ K 个独立 `tokio::spawn` worker（owns JoinSet → drop reader 联级 abort 所有 worker）；worker 用**round-robin chunk 分派**（worker_id 处理 chunks [worker_id, worker_id + K, worker_id + 2K, ...]）保证全 K 并发不退化为串行
- `SshFileSystemProvider::open_read` (trait impl) 改造：
  - 生产路径（`self.sftp.is_some()`）+ 大文件（≥ `SFTP_PIPELINE_MIN_BYTES`）→ `PipelinedSftpReader::open` 返回流式 reader（peak RSS ≈ K × `SFTP_PIPELINE_CHUNK_BYTES`）
  - 生产路径 + 小文件（< 256K）→ `sftp.read()` 单 RTT 全量 + `Cursor`（与 PR-F 前一致，避免 K 个 open 的 spawn overhead）
  - Fake 测试路径（`with_client`，`self.sftp == None`）→ 走原 `client.read()` + Cursor 全量（保留 `CountedFakeRemoteSftp` 的 `read_count` 语义，`perf_ssh_cache_hit.rs` 既有断言**不需要改**）
- `SftpError::Limited` fallback：K 个 `sftp.open` 预先并行打开时若任一返回 Limited（server 端 `open_handles` 上限）→ 降级到单 handle `sftp.open(path).await?` 流式（File 实现 `AsyncRead`），保留 PR-F 已有的降级语义但适配流式上下文
- 错误传播：worker 内部任一 `seek` / `read_exact` 失败 → 把 `io::Error` 经 channel 推给 reader → `poll_read` 返 `Poll::Ready(Err(...))`，不 silent drop；reader 进入 error 终态，后续 poll_read 仍 Err
- Cancellation：`PipelinedSftpReader` 持有 `JoinSet`，drop reader 时 JoinSet 自动 abort 所有 worker（tokio ≥ 1.21 行为）；上游 `parse_file_via_fs` 被 abort 时联级停掉 worker 不留 orphan task
- 保留现有 inherent `SshFileSystemProvider::open_read_stream`（裸 `SftpSession.open` 返回 `russh_sftp::client::fs::File`）不动——caller 显式调用拿原生句柄路径不变
- 新增单元测试：`PipelinedSftpReader` 的 round-robin chunk 分派 / EOF / 错误传播 / cancellation 用合成 receiver 直接构造（不依赖真 SFTP）
- 更新 `crates/cdt-api/tests/perf_ssh_scanner_chunked_read.rs` —— `ThrottledFakeSftpClient` 走 `client.read` fake 路径，本 change 不改 fake 行为，bench wall < 2s 断言保持
- 更新 `openspec/followups.md::286-290` 自留 limitation 第二条为 `[done]`，留 [punted] 备案"真实 SSH e2e 验证仍 punt（fork russh-sftp / mock RawSftpSession 工作量与价值倒挂）"
- **不**改动：`RusshSftpClient::read` trait 实现（`read_to_string` 等 Vec<u8> 消费者仍走 K-worker pipelined 收集后返 Vec<u8>）；`SftpClient` trait API；fake `CountedFakeRemoteSftp` / `ThrottledFakeSftpClient`；`parse_file_via_fs` 签名

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `ssh-remote-context`: `open_read` 对大文件 SHALL 返回 K-worker prefetch streaming reader 而非全量预取 `Cursor<Vec<u8>>`；peak RSS 与 K 而非 file_size 成正比；保留 Limited 降级到单 handle 流式
- `fs-abstraction`: `Scenario "open_read 在 SSH 上走 SFTP 流式句柄"` 的语义补齐——SSH 实现 SHALL 不在 trait 层全量读入内存

## Impact

- **affected crate**: `cdt-ssh`（`provider.rs` 主改动 + 新增 `PipelinedSftpReader`）
- **affected tests**:
  - 新增 `cdt-ssh` 单元测试（PipelinedSftpReader 行为）
  - 现有 `cdt-api/tests/perf_ssh_cache_hit.rs` op counter 断言**不变**（fake 路径走 client.read 计数语义守恒）
  - 现有 `cdt-api/tests/perf_ssh_scanner_chunked_read.rs` wall < 2s 断言**不变**（fake 路径走 client.read，ThrottledFakeSftpClient 内部已镜像 K-worker 模型）
  - 现有 `cdt-ssh/src/provider.rs::tests::open_read_stream_unsupported_in_fake_path` 行为不变（inherent 方法保留）
- **performance impact**（PR 描述强制四维齐全）：
  - peak RSS：5MB jsonl 从 ~5MB → ~512K（K=16 × CHUNK_BYTES=32K，~10× 改善；TS 原版 ~64K，本 PR 比 TS 略高但低于 PR-F 一个数量级）
  - wall time：与 PR-F baseline 持平（K-worker 并发数 + chunks_per_worker 不变；round-robin 调度不增加 RTT）
  - user/sys time：与 PR-F 持平（spawn K worker 微增 sys，相对 SSH RTT bound 微不可察）
  - user/real ratio：仍 < 0.3（I/O bound）
- **依赖**：无新增 crate；用 tokio mpsc + JoinSet（已在 workspace dep）
- **降级路径**：`SftpError::Limited` 命中时 wall 退到 N×RTT 但流式 peak RSS 仍受限（单 handle File 内部按 read 调用 chunk by chunk）
- **followups.md L286-290**：第二条 streaming K-worker prefetch ✅ done
