## 1. `cdt-ssh` PipelinedSftpReader 核心实现

- [x] 1.1 在 `crates/cdt-ssh/src/provider.rs` 内（非 pub）新增 `enum PipelinedOpenError { Limited { reason: String, partial_handles: Vec<russh_sftp::client::fs::File> }, Sftp(russh_sftp::client::error::Error) }`
- [x] 1.2 新增 `struct PipelinedSftpReader { receivers: Vec<mpsc::Receiver<Result<Vec<u8>, io::Error>>>, _workers: JoinSet<()>, current: Vec<u8>, current_pos: usize, next_worker: usize, eof: bool, error_seen: bool, total_bytes_expected: u64, total_bytes_read: u64 }`
- [x] 1.3 实现 `PipelinedSftpReader::open(sftp: Arc<SftpSession>, path: String, size: u64) -> Result<Self, PipelinedOpenError>`：用 `futures::future::join_all`（**非 try_join_all**）并发 K=`SFTP_PIPELINE_MAX_WORKERS.min(n_chunks).max(1)` 个 `sftp.open(path.clone())`，收齐 `Vec<Result<File, SftpError>>`；任一为 `Err(SftpError::Limited(r))` → 返 `PipelinedOpenError::Limited { reason: r, partial_handles: <已成功的 File vec> }`；任一为其它 `Err(e)` → drop 已成功 handles 返 `PipelinedOpenError::Sftp(e)`；全部 Ok → 进入 spawn worker 阶段
- [x] 1.4 在 `open` 内为每个 worker spawn task：worker 内 loop `chunk_idx in [worker_id, worker_id + n_workers, worker_id + 2*n_workers, ...] < n_chunks`，每个 chunk 做 `file.seek(SeekFrom::Start)` + `file.read_exact(&mut buf[..chunk_len])`，成功 `tx.send(Ok(buf)).await`，失败 `tx.send(Err(io::Error::other(...))).await` + return；JoinSet handle 存入 `_workers`
- [x] 1.5 实现 `impl AsyncRead for PipelinedSftpReader::poll_read`：先 drain `self.current[self.current_pos..]` 并累加 `total_bytes_read`；耗尽且 `!eof && !error_seen` 时 `self.receivers[next_worker].poll_recv(cx)` → `Some(Ok(bytes))` 设 current + `next_worker = (next_worker + 1) % K`，`Some(Err(e))` 设 `error_seen=true` 返 Err，`None` 则**立即**按字节计数判（**不**等其它 receiver close，避免 hang）：`total_bytes_read == total_bytes_expected` → 设 `eof=true` 返 EOF；`total_bytes_read < total_bytes_expected` → 设 `error_seen=true` 返 `io::Error::new(UnexpectedEof, ...)`（codex 二审 Blocker #1 silent EOF 防线 + round 2 verify A：next-worker None 即终结判定）；`Pending` 返 Pending
- [x] 1.6 `poll_read` 在 `error_seen=true` 后续被 poll 时 SHALL 返 `io::Error::other("PipelinedSftpReader already errored")` 终态错误，避免 polling-after-error UB
- [x] 1.7 加 `#[cfg(test)] impl PipelinedSftpReader::from_test_channels(receivers: Vec<mpsc::Receiver<Result<Vec<u8>, io::Error>>>, total_bytes_expected: u64) -> Self`，让单测注入合成 receiver + 期望字节数

## 1b. 生产路径分支选择 helper（codex 二审 Blocker #3 wiring smoke）

- [x] 1b.1 新增 `pub(crate) enum OpenReadStrategy { Streaming { n_workers: usize }, SmallFileBuffered, FakeBuffered }` 与 `pub(crate) fn pick_open_read_strategy(has_sftp: bool, size: u64) -> OpenReadStrategy`：`(true, >= MIN_BYTES)` → Streaming, `(true, < MIN_BYTES)` → SmallFileBuffered, `(false, _)` → FakeBuffered
- [x] 1b.2 新增单元测试 `tests::pick_strategy_routes_production_large_to_streaming` 等 4 个组合 case，钉死 wiring

## 2. `SshFileSystemProvider::open_read` trait 实现改造

- [x] 2.1 修改 `crates/cdt-ssh/src/provider.rs::FileSystemProvider::open_read` 实现：用 `pick_open_read_strategy` 决定 branch
  - `OpenReadStrategy::Streaming` → `PipelinedSftpReader::open`，Limited 返 `partial_handles` 时优先复用第 1 个 `File`：`Box::new(partial_handles.into_iter().next().unwrap_or_else(|| <重 open>))`；其余 partial 直接 drop；`PipelinedOpenError::Sftp(e)` → `map_sftp_io`
  - `OpenReadStrategy::SmallFileBuffered` → `sftp.read(path).await? → Box::new(Cursor::new(bytes))`
  - `OpenReadStrategy::FakeBuffered` → 原 `with_retry(client.read) → Box::new(Cursor::new(bytes))` 路径不变
- [x] 2.2 生产路径上 `sftp.metadata` 调用做 1 次（不做 with_retry——streaming 上下文 mid-stream 不可重试；caller 在 `parse_file_via_fs` 失败后自行重试 open_read）
- [x] 2.3 Limited fallback 加 `tracing::warn!(path, workers = SFTP_PIPELINE_MAX_WORKERS, partial_handle_count, reason, "...")`
- [x] 2.4 修改 `provider.rs:290-304` doc comment：删去"放弃 BufReader 流式（peak RSS = file_size）换 N×RTT 消除"措辞，改写为"生产大文件走 PipelinedSftpReader K-worker prefetch streaming（peak ≈ K × CHUNK）；生产小文件走 sftp.read 单 RTT；fake 测试路径走 client.read + Cursor"

## 3. `cdt-ssh` 单元测试

- [x] 3.1 新增 `tests/round_robin_chunk_assignment`：验证 `chunk_idx % n_workers` 公式对 `(n_chunks=17, K=16)` / `(n_chunks=160, K=16)` / `(n_chunks=1, K=1)` 等典型组合的 worker_id 分布正确
- [x] 3.2 新增 `tests/pipelined_reader_round_robin_pull_order`：用 `from_test_channels` 注入 K=3 个 mpsc receiver，pre-fill 已知 chunk 序列，验证 `poll_read` 输出字节按 round-robin 重组与 input 等价
- [x] 3.3 新增 `tests/pipelined_reader_propagates_worker_error`：某 worker 的 receiver 推 `Err(io::Error)` 后 close，验证 `poll_read` 返该 Err 且 `error_seen` 防止后续 panic
- [x] 3.4 新增 `tests/pipelined_reader_eof_on_next_worker_close_with_full_bytes`：next_worker 的 receiver close + 累计字节 == `total_bytes_expected`（其它 receiver 仍 open 不影响），验证 `poll_read` 返 `Poll::Ready(Ok(()))` 不写入 buf
- [x] 3.5 新增 `tests/pipelined_reader_unexpected_eof_on_short_close`：next_worker 的 receiver close 但累计字节 < `total_bytes_expected`（模拟 worker silent panic），验证 `poll_read` 返 `Err(UnexpectedEof)`，**不**等其它 receiver 全 close（验证 hang gap 已堵 —— codex 二审 Blocker #1 + round 2 verify A）
- [x] 3.6 新增 `tests/pipelined_reader_drop_aborts_workers`：mock worker spawn 时持有 oneshot sender，drop reader 后用 `tokio::time::timeout(100ms)` 验 oneshot 收到 abort signal（worker 被 cancel 后 sender drop → recv 返 Err）
- [x] 3.7 新增 `tests/pipelined_reader_polling_after_error_returns_terminal_err`：worker 推 Err 后 consumer 二次 poll_read 验证返"already errored"终态错误，不 panic

## 4. `cdt-ssh::tests` 既有测试守护回归

- [x] 4.1 验证 `crates/cdt-ssh/src/provider.rs::tests::open_read_stream_unsupported_in_fake_path` 仍 pass（inherent 方法语义不变）
- [x] 4.2 验证 `read_to_string_decodes_utf8` / `read_to_string_retries_transient_then_succeeds` / `read_to_string_gives_up_after_max_transient` 仍 pass（`RusshSftpClient::read` trait 实现 + `client.read` 路径未改）

## 5. `cdt-api` 现有 SSH 测试守护回归

- [x] 5.1 验证 `crates/cdt-api/tests/perf_ssh_cache_hit.rs::ssh_list_sessions_first_call_populates_cache_via_batch` 仍 pass（fake 路径，op counter 语义守恒）
- [x] 5.2 验证 `crates/cdt-api/tests/perf_ssh_cache_hit.rs::ssh_list_sessions_second_call_hot_path_zero_fs_op` 仍 pass
- [x] 5.3 验证 `crates/cdt-api/tests/perf_ssh_cache_hit.rs::ssh_get_tool_output_second_call_one_stat_zero_read` 仍 pass（cache hit byte-equal `read_count` 不增；fake 路径）
- [x] 5.4 验证 `crates/cdt-api/tests/perf_ssh_cache_hit.rs::ssh_get_image_asset_second_call_one_stat_zero_read` 仍 pass
- [x] 5.5 验证 `crates/cdt-api/tests/perf_ssh_cache_hit.rs::ssh_disconnect_aborts_batch_task_no_orphan_broadcast` 仍 pass
- [x] 5.6 验证 `crates/cdt-api/tests/perf_ssh_scanner_chunked_read.rs::ssh_5mb_jsonl_scan_wall_under_2s`（`--ignored`）仍 pass（fake 路径 + 现有 `ThrottledFakeSftpClient` K-worker 模拟不变）
- [x] 5.7 验证 `crates/cdt-api/tests/ipc_contract.rs::active_ssh_context_reads_remote_projects_and_sessions` 仍 pass（fake 路径 fs op counter helper）

## 6. `openspec/followups.md` 更新

- [x] 6.1 把 `openspec/followups.md::286-290` 的 "PR-F 未做 的（已知 limitation，留 follow-up）" 第二条 "流式 `open_read_stream` K-worker prefetch" 标 ✅ done，引用 change `ssh-open-read-streaming-prefetch`；第一条 "真实 SSH server 上的 e2e 验证" 加 [punted] 备案不变

## 7. 工具链与本地验收

- [x] 7.1 `cargo fmt --all`
- [x] 7.2 `cargo clippy --workspace --all-targets -- -D warnings`
- [x] 7.3 `cargo test -p cdt-ssh`（含新单测）
- [x] 7.4 `cargo test -p cdt-api --test perf_ssh_cache_hit`（默认 CI 进，5 项断言全 pass）
- [x] 7.5 `cargo test -p cdt-api --release --test perf_ssh_scanner_chunked_read -- --ignored --nocapture`（手动跑，验 wall < 2s）
- [x] 7.6 `cargo test --workspace`（全 workspace 回归）
- [x] 7.7 `openspec validate ssh-open-read-streaming-prefetch --strict`
- [x] 7.8 `just preflight` 全绿（fmt + clippy + test + spec-validate 一把梭）

## 8. Perf 基线数据采集（PR 描述 Perf impact 段填）

- [x] 8.1 跑 `/usr/bin/time -lp cargo test --release -p cdt-api --test perf_ssh_scanner_chunked_read -- --ignored ssh_5mb_jsonl_scan_wall_under_2s --nocapture` 5 次取 min/median：记录 wall / user / sys / max RSS / user-real ratio；对比 PR-F baseline（main 上同 bench）
- [x] 8.2 在 PR 描述 `## Perf impact` 段填 5MB jsonl 四维数据 + 与 PR-F baseline 的 wall / RSS 差异

## N. 发布

- [x] N.1 push 分支 + 开 PR（PR 描述 Perf impact 四维齐全 + 引用 followups.md L286-290 第二条 closure）
- [x] N.2 wait-ci 全绿
- [x] N.3 codex 二审通过（重点查 6 个怀疑点：背压 / 错误传播 / Limited 单 handle / cache wrapper counter / cancellation 联级 / ParseError::Io 包装；如发现 bug：修 → push → 回到 N.2 重跑；可循环 M 次）
- [x] N.4 archive change（archive commit 作为 PR 最后一个 commit + 再次 wait-ci 全绿）
