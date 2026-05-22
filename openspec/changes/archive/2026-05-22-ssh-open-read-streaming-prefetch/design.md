## Context

PR-F SFTP message-id pipeline（已在 main）把 `SshFileSystemProvider::open_read` 对大文件改成 K=16 worker 并发飞 `SSH_FXP_READ`，wall ≈ ceil(N/K) × RTT。实现见 `crates/cdt-ssh/src/provider.rs::read_pipelined`：K 个 worker 各开独立 file handle、各读一段连续 byte range（`worker_id * chunks_per_worker .. (worker_id + 1) * chunks_per_worker`），`try_join_all` 收齐后按 `worker_id` 排序拼成完整 `Vec<u8>`，最后 `Cursor::new` 包装返 `Box<dyn AsyncRead>`。

设计副产物：peak RSS = `file_size`（5MB jsonl 进 5MB 内存）。`provider.rs:300-301` 自承 trade-off"放弃 BufReader 流式（peak RSS = file_size）换 N×RTT 消除"。

参考实现：TS 原版用 `ssh2.sftp.createReadStream(path)` 拿 readable stream，ssh2 库内部用 readahead 机制（默认窗口 ~64K，2 个 in-flight READ）；wall 在远端 RTT bound 时近似单 worker pipelined。peak RSS ~64K。Rust 端 russh-sftp 没有内置 readahead——`SftpSession::open` 返回的 `russh_sftp::client::fs::File` 每次 `AsyncRead::poll_read` 顶多触发 1 个 `SSH_FXP_READ`（32K），串行 N RTT。

约束：
- 不引入新 dep（已有 tokio mpsc / JoinSet）
- 不动 `SftpClient` trait API（不破 fake 路径与 `CountedFakeRemoteSftp` 计数语义）
- 不动 inherent `open_read_stream`（caller 显式调拿原生 `russh_sftp::client::fs::File` 仍需保留）
- 真实 SSH e2e 验证仍 punt（fork russh-sftp / mock `RawSftpSession` 工作量与价值倒挂）

## Goals / Non-Goals

**Goals:**

- `SshFileSystemProvider::open_read` 对大文件 peak RSS 与 K 而非 file_size 成正比（5MB jsonl 从 5MB → ~512K 量级）
- wall time 不退化（K=16 + chunks_per_worker 不变 → wall ≈ ceil(N/K) × RTT 与 PR-F 持平）
- 保留 `SftpError::Limited` 降级路径（server `open_handles` 上限时单 handle 流式）
- 错误传播完整：worker 内任一 `seek` / `read_exact` 失败 → `poll_read` 返 `Err`，不 silent drop
- Cancellation 联级 abort：reader 被 drop / 上游 `parse_file_via_fs` 被 abort → 所有 worker 自动停
- 测试覆盖：单元测试覆盖 PipelinedSftpReader 的 round-robin 调度 / EOF / 错误 / cancellation 行为
- 现有 `perf_ssh_cache_hit.rs` 的 op counter 断言**无需修改**（fake 路径走 `client.read` 计数语义守恒）
- 现有 `perf_ssh_scanner_chunked_read.rs` 的 wall < 2s 断言**无需修改**（fake 路径走 `client.read`，`ThrottledFakeSftpClient` 内已镜像 K-worker 模型）

**Non-Goals:**

- 真实 SSH e2e 验证（[punted] 备案在 followups.md）
- TS 原版 ~64K peak parity（K=16 × 32K = 512K 已经是数量级改善，更低需牺牲 wall）
- 改变 `read_to_string` 的 Vec<u8> 收集语义（`RusshSftpClient::read` trait 实现保留 K-worker pipelined → Vec<u8> 路径不变；只有 `open_read` 走流式）
- 改变 `SftpClient` trait API
- 改变小文件路径（< `SFTP_PIPELINE_MIN_BYTES`=256K 仍走单 RTT 全量，避免 K 个 open 的 spawn overhead）

## Decisions

### D1: 流式 buffer 拓扑——K 个 mpsc cap=1 receiver 走 round-robin 而非全局 BTreeMap reorder

候选方案：
- (a) **K 个 `mpsc::channel(1)`，round-robin chunk 分派（worker_id 处理 chunks [worker_id, worker_id + K, ...]）**（选）
- (b) 单个全局 `mpsc(K)` + worker 写 `(chunk_idx, bytes)` + 消费侧 `BTreeMap<usize, Vec<u8>>` reorder
- (c) K 个 `mpsc::channel(N)`（N = chunks_per_worker），消费按 worker_id 顺序排空（= 全量预取，无收益）
- (d) K 个 `mpsc::channel(M)`（M = 2 或更大），消费 round-robin，调大 buffer 容许 worker 跑得更远

**选 (a) 理由**：
- 拓扑最简：受体侧只需 `next_worker_id` 一个状态 + drain current chunk 的 cursor，无 reorder 数据结构
- 内存最低：peak ≈ K × (1 buffered + 1 in-flight) × `SFTP_PIPELINE_CHUNK_BYTES` = 16 × 2 × 32K ≈ 1MB worst-case；典型 consumer 不阻塞时 ≈ K × 1 × 32K = 512K
- 不退化为串行：round-robin 分派让 K 个 worker 都能在 t=0 同时飞第一次 read，t=50ms 全部就绪后随消费 pace 进入"消费一个就让对应 worker 继续读下一个"稳态——wall ≈ ceil(N/K) × RTT 与 PR-F 持平
- **(b) 缺点**：reorder buffer 上限不易给——worker 0 慢时 worker 1..K-1 跑远，BTreeMap 内堆积 N - 1 个 chunk = (N - 1) × 32K，5MB 文件 ≈ 5MB 退化到 PR-F 同等 peak；要给 reorder buffer 上限就要给 worker 加 sync 信号，复杂度反超 (a)
- **(c) 缺点**：等价 PR-F，peak = file_size
- **(d) 缺点**：buffer 越大 peak 越高且 wall 收益微（worker 已经能在 round-robin 稳态下保持 100% 利用率）

**round-robin 必须性证明**：若用**连续段分派**（worker_id 读 chunks [worker_id × M .. (worker_id + 1) × M]，M = chunks_per_worker），cap=1 的 channel 会让 worker 1..K-1 在第 1 个 chunk 就绪后立刻阻塞等待消费——消费 round-robin 先消 worker 0 的 M 个 chunk（M × 50ms = 500ms），其间 worker 1..K-1 都 idle 在 send 阻塞态。等消费切到 worker 1 时，worker 1 才能开始读第 2 个 chunk。串行化总 wall = K × M × RTT 退化到 PR-F 前的水平。round-robin 把"消费第 i 个 chunk = 释放 worker (i % K) 的 backpressure"，所有 worker 都能持续受消费节奏推进。

具体 chunk 分派：`worker_id = global_chunk_idx % n_workers`。EOF 通过 channel close 表达——worker 处理完自己最后一个 chunk 后 drop sender → 受体侧 `next_worker` 转到这个 worker 时收到 `Poll::Ready(None)` 视为 EOF。注：N 个 chunk 不整除 K 时尾部 worker（chunk_count <= worker_id）一开始就 close，但 round-robin 不会 next_worker 到这些 idx（消费严格按 0..global_chunk_count - 1 顺序取，next_worker 自然封顶在 max_chunk_idx % K，不会越界）。

### D2: 错误传播——经 channel 推 `io::Error`，consumer 端 fast-fail + 字节计数防 silent EOF

worker 内部任一 SFTP 操作失败：
- `sftp.open` 失败（首次开 handle）→ 在 `PipelinedSftpReader::open` 入口（`join_all` K opens）就返 `Err`，**进入降级或上抛**（详 D5）
- `seek` / `read_exact` 失败 → worker 转换 `russh_sftp::client::error::Error` 为 `io::Error::other(format!(...))` + `tx.send(Err(io_err)).await`，然后 `return`（自然 drop sender → channel close）
- consumer `poll_read` 收到 `Poll::Ready(Some(Err(io_err)))` → 返 `Poll::Ready(Err(io_err))`；同时把 reader 标记为 `error_seen = true`，后续 poll_read 视为终态返 `io::Error::other("PipelinedSftpReader already errored")`，避免 polling-after-error 行为未定义

**Silent EOF 防线（codex 二审 Blocker #1 + 第二轮 verify A）**：worker panic / channel 异常 close 不能让 consumer 把"early channel close"当 EOF 看待，否则 caller 收到截断的部分内容仍 parse 通过会产生静默截断。`PipelinedSftpReader` SHALL 在自身维护：
- `total_bytes_expected: u64` —— 构造时由 `sftp.metadata(path).len()` 给定
- `total_bytes_read: u64` —— 每次 poll_read 写入 `ReadBuf` 时累加 `n`
- EOF 判定：**当 round-robin 轮到的 next worker 的 receiver 在 `poll_recv` 返 `None`** 时（即该 worker 的 channel 已 close），**立即**按字节计数判：
  - `total_bytes_read == total_bytes_expected` → 真 EOF，返 `Poll::Ready(Ok(()))` 不写 buf；reader 标 `eof = true`
  - `total_bytes_read < total_bytes_expected` → silent truncation，返 `Poll::Ready(Err(io::Error::new(ErrorKind::UnexpectedEof, format!("PipelinedSftpReader closed early: expected {} bytes, got {}", expected, read))))`；标 `error_seen = true`
  - `>` 不可能（chunk 总长度 = file size 一致性由 worker 分派保证）

**为什么是"next 而非 all"判定**：round-robin 顺序保证 chunk `i` 必由 worker `i % K` 产生。当 consumer next_worker = N 拉到 None 时，下一个要消费的 chunk 全局 idx 是 j（满足 `j % K == N`），意味着该 worker 已无剩余 chunk 可发；其它 worker 即使还在跑也只会产生 `idx % K != N` 的 chunk，不会回填这个位置——所以这个时刻是判定流终结的精确点。继续等"all K close"会让 consumer 死等其它仍在跑的 worker（典型 worker 0 完 + 1..K-1 仍 read 第 2 chunk），实际是 hang。

这条防线让 worker spawn 后 panic（如 vec 分配 OOM）或 JoinSet 异常被 abort 时，consumer 端能感知到字节短缺并立即返 UnexpectedEof，不会静默截断。

不做：worker 之间 cross-cancel（一个 worker 失败不主动 abort 其它 worker）。理由：reader drop 时 JoinSet drop 联级 abort 已经覆盖；继续让其它 worker 跑到自然结束代价小（最坏多读 K-1 个 chunk 浪费），换实现简单。

### D3: Cancellation 联级 abort——JoinSet drop

`PipelinedSftpReader` 字段 `_workers: JoinSet<()>`：tokio ≥ 1.21 `JoinSet::drop` 会 abort 所有未完成 task。drop 路径：
- 消费侧主动 drop reader（如 `parse_file_via_fs` 内部 `BufReader<reader>` 提前 drop / for-loop break）→ reader drop → JoinSet drop → 全 K worker abort
- 上游 `tokio::spawn` 持有 reader 的 task 被 abort → reader drop → 同上

worker 内 `tx.send(...).await` 也会因 receiver drop 而 return Err（mpsc::Sender::send on closed channel），worker 自然 return。

不需要显式 `CancellationToken`——这套语义已经覆盖。

### D4: Limited fallback——降级到单 handle 流式（`sftp.open` 返回的 `File`）

PR-F 当前在 `RusshSftpClient::read` 内做 Limited fallback（match `SftpError::Limited` → `self.sftp.read(path)`）。streaming 路径不能复用这条——`sftp.read` 返 `Vec<u8>` 是全量预取（peak = file_size），降级回去 peak RSS 反而 worse than streaming。

新降级策略：
- `PipelinedSftpReader::open` 内部用 `futures::future::join_all`（**不是 `try_join_all`**）K 个 `sftp.open`，收齐 `Vec<Result<File, SftpError>>`
- 全部 `Ok` → 走 streaming
- 任一 `Err(SftpError::Limited(reason))` → 返 `Err(PipelinedOpenError::Limited { reason, partial_handles: Vec<File> })`，把已成功的 `File` 一并交还 caller
- 任一 `Err(其它)` → 返 `Err(PipelinedOpenError::Sftp(e))`

`SshFileSystemProvider::open_read` 接 Limited 时（codex 二审 Blocker #2）：
- **如果 `partial_handles` 非空** → 从 vec 取第 1 个 `File` 直接 `Box::new(file)` 返（**复用已开的 handle**，避免再开一次 sftp.open 撞同样 Limited + 避免依赖 russh-sftp `File::drop` 的同步 close 行为）；其余 `partial_handles` drop 让 russh-sftp 自身释放
- **如果 `partial_handles` 为空**（K=1 时 Limited / 或所有 K 都 Limited）→ 显式 `sftp.open(path).await?` 拿单 file handle；若仍 Limited 上抛 `FsError::Io { ErrorKind::Other }`

降级 SHALL 通过 `tracing::warn!(path, workers, partial_handle_count, reason, ...)` 让运维侧可见。

**关于 russh-sftp File::drop**（codex 二审 Blocker #2 余波）：russh-sftp 0.2.x 的 `File::drop` 是 best-effort（sync drop 不能跨 await 调 `SSH_FXP_CLOSE`，实际行为是把 close 请求加入背景队列由 `SftpSession` 异步处理）；server 端 handle 在 SSH 连接关闭时彻底释放。本设计接受 partial-handle drop 的潜在短暂 leak（典型时间 ~ms 量级），不阻断。

降级路径的 wall 退化是 acceptable trade-off——Limited 是 server 端罕见限制（典型 OpenSSH server `open_handles` 默认 ~100，K=16 远低于；触发场景是同一连接已并发开了 80+ 个其它 SFTP handle）。

### D5: 测试策略——纯算法层 + 合成 receiver 注入 + 生产路径分支选择 smoke

测试层级：
- (a) **chunk 分派纯函数测试**：抽 `chunk_distribution(total_size, chunk_bytes, max_workers) -> (n_workers, n_chunks)` + `assign_worker(chunk_idx, n_workers) -> usize`（worker_id 公式 `chunk_idx % n_workers`），单元覆盖典型 size（< CHUNK / 刚好 CHUNK / 5MB / K×CHUNK 整除 / 非整除）
- (b) **`PipelinedSftpReader` 行为测试**：加构造 helper `from_test_channels(receivers: Vec<mpsc::Receiver>, total_bytes_expected: u64) -> Self`（`#[cfg(test)]` only），允许测试直接给受体填模拟 chunks。覆盖：
  - round-robin 顺序读 N chunks，输出字节序正确
  - 某 worker 中途返 `Err(io)` → 下次 poll_read 返该 Err；再下次返 "already errored" 终态 Err（防 polling-after-error UB）
  - 所有 worker close 且 `total_bytes_read == total_bytes_expected` → 返 EOF（n=0）
  - 所有 worker close 但 `total_bytes_read < total_bytes_expected`（模拟 worker 静默退出）→ 返 `Err(UnexpectedEof)`（codex 二审 Blocker #1 防线）
  - 受体 drop → workers JoinSet abort（用 `tokio::sync::oneshot` 在 mock worker 末尾发 signal，测 timeout 内收到）
- (c) **生产路径分支选择 smoke**（codex 二审 Blocker #3）：抽 `pub(crate) fn pick_open_read_strategy(has_sftp: bool, size: u64) -> OpenReadStrategy`（返 `Streaming { n_workers } | SingleHandle | SmallFileBuffered | FakeBuffered` 枚举），单元覆盖 4 个组合：(生产, 大文件) → Streaming(K)、(生产, 小文件) → SmallFileBuffered、(fake, 大文件) → FakeBuffered、(fake, 小文件) → FakeBuffered。把"哪个 size 走哪个 branch"的 wiring 钉死在单测里——这条 smoke 不能保证真 SFTP 流式正确性，但能拦截"未来 PR 误把生产大文件 branch 接到 client.read 旧路径"这类 wiring 回归
- (d) **end-to-end fake 路径**：现有 `perf_ssh_scanner_chunked_read.rs` 用 `with_client + ThrottledFakeSftpClient` 走 `client.read` 全量路径——不接触新 PipelinedSftpReader 代码。这条 test 守护"fake 路径 + 30K jsonl scan wall < 2s"语义不变（fake 路径用 `client.read` + Cursor，与本 PR 改动无关）
- (e) **真实 SSH e2e**：[punted] 备案在 followups.md（理由：fork russh-sftp 或起 in-process SFTP test server 工作量与价值倒挂；与 TS 原版同样不测 ssh2 库本身）

**为什么不加 `SftpClient::open_read_stream` trait 方法**：会迫使 `CountedFakeRemoteSftp` 加新 counter + 改 `perf_ssh_cache_hit.rs` 5 处断言；新 counter 在 fake 路径下没法验证真实 streaming 行为（fake 不模拟真 K-worker 时序）。流式行为只在生产路径有意义——通过 `self.sftp.is_some()` 判生产/测试路径，让生产路径走新 `PipelinedSftpReader`，fake 路径走 `client.read` 全量。op counter 在 fake 路径下意义守恒。**Wiring 正确性由 (c) 分支选择 smoke 守护**——不靠 fake counter 测生产 streaming 路径。

### D7: Cancellation caveat——russh-sftp 内部 SFTP reply 处理是库契约假设

JoinSet drop abort 让 worker 内 `sftp.open / seek / read_exact` 的 future 在下次 poll 时返 Aborted。但底层 SFTP 协议是 request-reply 模型：worker 已发出 `SSH_FXP_READ` 请求且 server 已 executing；取消 local future 后 server 端 reply 仍会回到同一 `SftpSession` 的 `RawSftpSession` 内（按 `request_id` demux）。russh-sftp 0.2.x 的契约：未被 await 的 reply 进入 `RawSftpSession` 的 pending map，下次同 `request_id` 复用时会用 stale reply——实际中 `request_id` 单调递增不会复用，所以**不会**有 stale reply 误派给后续 request。本设计依赖 russh-sftp 该既有契约，不在本 change scope 内验证；如未来 russh-sftp 升级破坏此契约，需独立 fix。

### D6: 与 `RusshSftpClient::read` trait 实现的关系——并存不替换

`RusshSftpClient::read` trait 实现保留现有 K-worker pipelined 收集 `Vec<u8>` 路径不动。理由：
- `read_to_string` / `read_lines_head` 等 trait 方法天然消费 `Vec<u8>`，让它们继续走 K-worker 收集合理（拿到完整 String 才能 lossy 解 utf8）
- 这些场景的 caller 多是小文件（session metadata 探测 head 10 行，jsonl 行散数 KB）；少数大文件 caller（`read_to_string` 整文件）peak RSS 影响有限且使用频率低
- 改 trait 实现会牵连 `CountedFakeRemoteSftp::read` 与多个 op counter 测试

`SshFileSystemProvider::open_read` trait 实现 → 走新 `PipelinedSftpReader`（生产大文件）/ `sftp.read` Cursor（生产小文件）/ `client.read` Cursor（fake 测试路径）。这是 scanner 大文件流式的主入口，covered by 本 change。

## Risks / Trade-offs

- **[Risk] peak RSS 1MB worst-case vs TS ~64K**：本设计 K=16 × (1 buffered + 1 in-flight) × 32K = 1MB worst-case；TS 用 readahead window ~64K。差一个数量级但比 PR-F 5MB 仍是 5× 改善。**Mitigation**：把 K 调小（K=4 → 256K peak，wall 2s for 5MB；K=8 → 512K peak, wall 1s）需要 wall vs peak trade-off。当前选择保 PR-F wall parity。
- **[Risk] 中间流式错误无 retry**：PR-F 的 `with_retry` 包了 `client.read` 整次重试 SftpError::Transient；streaming 路径不能 mid-stream retry——首个 chunk 发出后受体已经 forward bytes，重试需要重置 cursor 状态。**Mitigation**：限定 retry 在 K opens 阶段（首字节流出前）；流式中失败直接 `Err` 传给 caller，caller（`parse_file_via_fs`）可重试 `parse_file_via_fs` 重新走 open_read。
- **[Risk] worker 之间不交叉取消失败 worker 浪费 K-1 次 read**：D2 决策保留所有 worker 跑完自然 drop；最坏多读 K-1 个 chunk = 32K × 15 ≈ 480KB 网络浪费 + ~50ms RTT 延迟受体 drop。可接受。
- **[Risk] tokio JoinSet drop abort 是 cooperative**：abort 标志在 task 下次 `.await` 点检查，正在 `read_exact.await` 的 worker 会被 cancel 触发 await 返 Err（CancellationErr / Aborted）；不会等到 next chunk 才取消。**Mitigation**：依赖 tokio 既有契约（`AbortHandle::abort` 立即 schedule cancellation，所有 await point 都是 cancellation point）。
- **[Trade-off] 小文件保留 `client.read` 全量**：< `SFTP_PIPELINE_MIN_BYTES`=256K 用 `sftp.read` 单 RTT，避免 K opens (16 × open RTT = 16 × 50ms = 800ms parallel ≈ 50ms but 浪费 K-1 个 handle)。Peak ≈ file_size 但小文件 max 256K 可忽略。
- **[Trade-off] Limited 降级单 handle 流式不再 K 并发**：罕见路径接受 wall 退化（N × RTT），换 peak RSS 1 个 chunk 受益。
- **[Trade-off] 不测真 SFTP e2e**：fork russh-sftp / mock `RawSftpSession` 工作量与价值倒挂（[punted] 备案 followups.md）。
