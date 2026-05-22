## Context

`openspec/followups.md` line 265-267 的 coverage-gap：SSH context 下 project memory 不支持远端读写。当前现状（探查于 worktree HEAD）：

- `crates/cdt-api/src/ipc/local.rs::get_project_memory`（line 2625-2655）和 `read_memory_file`（line 2657-2682）在函数顶端调 `active_fs_and_policy()`，根据 `policy.supports_memory` early return graceful skip
- `BackendPolicy::for_ssh().supports_memory = false`（`crates/cdt-fs/src/backend_policy.rs:79`）—— SSH 永远走 graceful skip 分支
- `discover_memory_layers`（local.rs:3868）已经接 `&dyn FileSystemProvider`，**抽象层完备**；`validate_memory_file_name`（line 3979）是纯字符串校验不需 fs
- `cdt-fs::FileSystemProvider` trait 当前只暴露 9 个**只读**方法（详 fs-abstraction spec `Requirement: FileSystemProvider trait 暴露 7 个核心方法`，标题历史名留作 archive 兼容；本 change SHALL 同步 MODIFY 标题成"12 个核心方法"）：`kind / exists / read_dir / read_dir_with_metadata / read_to_string / stat / read_lines_head / open_read / stat_many`
- `cdt-ssh::SftpClient` trait（**位于 `crates/cdt-ssh/src/provider.rs`**，**不**是独立 `sftp_client.rs` 文件）同样只暴露读方法（`metadata / try_exists / read / read_dir / read_lines_head`）；`SshFileSystemProvider` 当前持 `Arc<dyn SftpClient>` + `Arc<SftpSession>`（**不**再用 `Arc<Mutex<SftpSession>>`，老 Mutex 已在前序 PR 移除——`russh-sftp` 的 `SftpSession` 公共 API 是 `&self` 方法，message-id 由库内部 channel 维护）
- 当前 `impl FileSystemProvider` 的实现总计 5 个：`LocalFileSystemProvider`（`crates/cdt-fs/src/local.rs:139`）、`SshFileSystemProvider`（`crates/cdt-ssh/src/provider.rs:188`）、`InstrumentedFs<P>`（`crates/cdt-fs/src/instrumentation.rs:149`）、测试侧 `SpyFs`（`crates/cdt-discover/tests/project_scanner.rs:179`）、`FakeSshFs`（`crates/cdt-api/src/ipc/session_metadata.rs:2067`）——加 trait 方法 SHALL 同 PR 实现全部 5 处
- `russh-sftp` 2.x 的 `SftpSession` API 已提供 `write` / `create_dir` / `remove_file` / `rename` 等异步方法
- TS 原版 `claude-devtools/src/main/ipc/memory.ts` 只暴露 read，没有 add/delete IPC
- 当前 Rust port **没有** `add_memory` / `delete_memory` IPC——本 change 是扩 spec（`memory-viewer` 当前 spec 含 "Operate on selected memory file" Requirement 但只覆盖 Open / Copy 现有 selected file 的操作，**不**含 add/delete）
- xtask `check-fs-direct-calls` 13 个 forbidden patterns 已含 `tokio::fs::write` / `tokio::fs::create_dir(_all)?` / `tokio::fs::remove(_file|_dir(_all)?)?`——任何业务代码加写 fs ops 都强制走 trait

**主要约束**：
1. fs-abstraction spec 把 trait 9 方法清单钉死，新增 trait 方法必须 MODIFY `Requirement: FileSystemProvider trait 暴露 7 个核心方法`
2. fs-abstraction spec 要求 trait 保持 dyn-safe，禁引入关联类型
3. fs-abstraction spec H1（业务路径不直调 `tokio::fs::*`）由 xtask CI gate 守护——本 change 加 IPC 写路径只能通过 `fs.write_atomic / create_dir_all / remove_file` 调用
4. ssh-remote-context spec 要求 SHALL NOT 在远端 spawn 工作进程，唯一允许的远端命令是 `printf %s "$HOME"`——所有 write 操作必须通过 SFTP 协议完成

## Goals / Non-Goals

**Goals:**

1. 修复 followups.md line 265-267 的 coverage-gap：SSH context 下 `get_project_memory` / `read_memory_file` 走真实远端 fs ops，不再 graceful skip
2. `FileSystemProvider` trait 暴露 atomic write / mkdir / remove 三个写方法，所有 backend（Local / SSH / 未来 HTTP）实现一致 atomic 语义
3. 新增 `add_memory(project_id, file, content)` / `delete_memory(project_id, file)` 两个 IPC method，行为契约完整定义；前端 `api.ts` 暴露 binding（不接 UI 按钮）
4. SSH 路径下所有 4 个 memory IPC method 走当前 active SSH context 的 fs provider，SSH dispatch contract test 覆盖
5. 写路径 atomic 保证：写失败不留半成品文件；同名文件覆盖原子（reader 永远看到旧或新整版）

**Non-Goals:**

1. 不接 UI 的 add/delete memory 按钮——`memory-viewer` UI Requirement 只规约行为契约（"add IPC SHALL 写入文件并返新 ProjectMemory"），UI 加按钮留 followup change
2. 不实现 memory layer cache（Rust 后端）——D3 决策详后；前端 `Sidebar` 现有 `memoryCache: Map<projectId, ProjectMemory>` 保持，写 IPC 返新 `ProjectMemory` 让前端直接 swap 避开 invalidation
3. 不解锁 SSH `supports_subagent_scan = false`——subagent JSONL 远端扫描有独立 spec 路径（fs-abstraction `for_ssh()` Scenario），不属本 change
4. 不实现 SFTP 并发 pipeline（多 message-id 并发 RTT）—— 留 PR-F；本 change 写 ops 走单串行 RTT 与现有 read 路径一致
5. 不引入 memory 写权限 / 配额 / 同步冲突检测——单用户单 session 编辑场景，简单 atomic write 已足够

## Decisions

### D1: 在 `FileSystemProvider` trait 上直接加 3 个写方法（不分裂 trait）

**候选方案：**

- **A（采纳）**：在 `FileSystemProvider` trait 上直接加 `write_atomic` / `create_dir_all` / `remove_file` 三个 async fn，所有现有 backend（Local / SSH）SHALL 实现；HTTP backend 当前用 `LocalFileSystemProvider` 包装也直接走 Local 实现
- **B**：新建独立 `WritableFileSystemProvider: FileSystemProvider` trait，写方法在子 trait 上；caller 需 downcast / 用 `Arc<dyn WritableFileSystemProvider>` 类型传递
- **C**：保持 `FileSystemProvider`，新方法 default impl 返 `FsError::Unsupported`，每个 backend 选择性 override

**为什么选 A：**

- Local / SSH 都需写能力，没 backend 能"只读不写"——分子 trait 的好处只有"未来某个 read-only backend 可以不实现"，但目前看不到这种 backend 落地路径（HTTP server 也是 Local fs 包装）
- 方案 B 要求 caller 决定用哪个 trait，所有写路径调用方都要写 `Arc<dyn WritableFileSystemProvider>`——但 `LocalDataApi` 内部 `active_fs_and_policy()` 返 `Arc<dyn FileSystemProvider>`，要么改返双 trait（侵入广），要么 caller 局部 downcast（运行时类型检查 + unwrap，丑且脆弱）
- 方案 C 默认 `Unsupported` 看似优雅但是**反 spec**——fs-abstraction spec 明确"trait SHALL 保持 dyn-safe，编译时强制实现"。default impl 返 Unsupported = 编译期允许跳过 = backend 漏实现 runtime 才发现。trait 契约层面让"必须实现"成为编译 gate 更安全
- A 的 trade-off 是 trait 名字"FileSystemProvider"既读又写——但这与 `std::fs` / `tokio::fs` 模块同时含读写的语义一致，命名层面无问题

**实施细节：**

```rust
// crates/cdt-fs/src/provider.rs
#[async_trait]
pub trait FileSystemProvider: Send + Sync + 'static {
    // 既有 9 方法保持不变 ...

    /// Atomic 写文件——写到 `<path>.tmp.<rand>` 后 rename 覆盖。
    /// 失败 SHALL 清理 tmp 文件（best-effort）。同 path 多 caller 并发写有 last-write-wins 语义。
    async fn write_atomic(&self, path: &Path, content: &[u8]) -> Result<(), FsError>;

    /// 递归创建目录（已存在不报错），等价 `tokio::fs::create_dir_all`。
    async fn create_dir_all(&self, path: &Path) -> Result<(), FsError>;

    /// 删文件（不存在 SHALL 返 `FsError::NotFound`），不递归删目录。
    async fn remove_file(&self, path: &Path) -> Result<(), FsError>;
}
```

trait 保持 dyn-safe（`async_trait` 宏脱糖到 `Box<dyn Future>` 不影响 dyn-safe）。

### D2: write_atomic 实现走 tmp+rename 跨 backend 一致

**候选方案：**

- **A（采纳）**：所有 backend 一律 write to `<path>.tmp.<rand_suffix>` + rename to `<path>`，rename 失败清理 tmp（best-effort）
- **B**：Local 走 tmp+rename，SSH 走 `SftpSession::write` 直写（SFTP write 是 truncate+write 不原子）
- **C**：每个 backend 自定 atomic 实现（Local 用 `tokio::fs::write` + rename，SSH 用 SFTP O_TRUNC+write）

**为什么选 A：**

- 一致语义最重要——caller 不需要知道 backend 类型也能信赖 atomic 保证
- B 的"SSH 直写"语义是 truncate+write 多 RTT，写到一半网络断开 → 文件半成品，下次读会拿到截断内容；这对 memory 文件（用户可能编辑 1KB-100KB markdown）尤其差
- C 看似更优但要求 reviewer 跨 backend 验证 atomic 性质——一致实现降低认知负担
- SFTP 协议层面的 atomic 性需要分情况看（codex design 二审 #5 修正）：
  - **OpenSSH SFTP server**（绝大多数 Linux/macOS 远端）：标准 `SSH_FXP_RENAME` 默认**不**支持目标已存在时原子覆盖（`rename(2)` 语义但拒绝 EEXIST）；OpenSSH 提供扩展 `posix-rename@openssh.com` 走 POSIX rename(2) 原子覆盖
  - **Windows OpenSSH server / 其他非 POSIX SFTP server**：rename 行为 server 实现相关，不保证原子覆盖
- 实施 SHALL 优先用 `posix-rename@openssh.com` 扩展（`russh-sftp` 通过 `SftpSession::rename` 自动协商支持）；首次 connect 时 SHALL 探测 server extensions（`SftpSession::extensions()`），含 `posix-rename@openssh.com` 则启用真原子路径，否则降级为"先 `remove_file(<target>)` 再 `rename(<tmp>, <target>)`"两步——降级路径 SHALL 在 `FsMetadata` capability 探测中标记 `atomic_rename: false`，本 change 不要求 caller 据此分支处理（行为契约层仍承诺 atomic，降级路径只是有极短窗口期可能让 reader 见到 `target missing`，单次写场景 acceptable）

**tmp suffix 来源**（codex design 二审 #4 修正）：使用进程内 `static AtomicU64` 单调递增计数器 `+ std::process::id()` 拼成 16-char hex。每次 `write_atomic` 调用 `WRITE_SEQ.fetch_add(1, Ordering::Relaxed)` 拿独占值，**绝对**不会与同进程内其他并发 `write_atomic` 调用冲突——避免依赖 `SystemTime::now()` 在 Windows 100ns 精度时钟下并发碰撞同纳秒的 race。pid 部分保险跨进程（罕见但理论可能：用户在同一目录跑 cdt-cli + Tauri app 并发改 memory）。冲突上限：单进程 2^64 次写后回卷（永不达成），跨进程同 pid 同 seq 概率 ≪ 1/2^64。

**清理失败 tmp 的 best-effort：** 写到 tmp 后 rename 失败 → 调 `remove_file(tmp_path)` 尝试清理，失败 SHALL 不传播错误（rename 失败已经向上抛错；tmp 残留是次要问题，下次写会用新 rand suffix）。

**实施细节：**

```rust
// crates/cdt-fs/src/local.rs
async fn write_atomic(&self, path: &Path, content: &[u8]) -> Result<(), FsError> {
    let tmp_path = path.with_extension(format!("tmp.{:016x}", gen_rand_suffix()));
    tokio::fs::write(&tmp_path, content).await.map_err(...)?;
    if let Err(e) = tokio::fs::rename(&tmp_path, path).await {
        let _ = tokio::fs::remove_file(&tmp_path).await; // best-effort
        return Err(...);
    }
    Ok(())
}

// crates/cdt-ssh/src/provider.rs (类似走 SftpClient::write + rename)
```

### D3: 不加 Rust 后端 memory layer cache，写 IPC 返新 `ProjectMemory`

**候选方案：**

- **A（采纳）**：不加 Rust 后端 cache；`add_memory` / `delete_memory` 写完后内部再调 `discover_memory_layers` 拿新结果直接返；前端 `Sidebar.memoryCache: Map<projectId, ProjectMemory>` swap 而不调用第二次 IPC
- **B**：新增 `MemoryLayerCache`（key = `(ContextId, project_id)`）按 fs.read_dir signature 失效，与 `MetadataCache` 同模式
- **C**：写完返 `()`，前端调 `get_project_memory` 重读

**为什么选 A：**

- memory 是低频路径——用户偶尔加个 note / 读个 reminder，不是 list_sessions 这种每次切 sidebar 都触发的热路径。加 cache ROI 极低（典型 < 10 个 .md 文件，read_dir + 至多 N 次 read_to_string，在 SSH 上也只 ~150ms wall）
- 加 cache = 写后必须 invalidate；context switch / SSH 断连 / 远端文件被外部进程改 都需考虑——管理成本远超读时省的 RTT
- 候选 C 多一次 IPC RTT；A 用一次 IPC 完成"写+查"，前端 UX 更好
- 候选 B 是合理 future work，但要先看是否真有性能问题；本 change 不引入

**写后查的实现：** `add_memory` / `delete_memory` 内部最后一步调 `discover_memory_layers(&*fs, &memory_dir)` 同步收集 layers 返回。这意味着每次写 = 1 read_dir + N read_to_string（N 个 .md 文件）；典型 N < 10 在 SSH 上 wall ~100ms，对单次写操作 acceptable。

### D4: `BackendPolicy::for_ssh().supports_memory = false → true`

**候选方案：**

- **A（采纳）**：直接改字段值 `false → true`，删 `LocalDataApi::get_project_memory` / `read_memory_file` 中 `if !policy.supports_memory { return ... }` 短路分支
- **B**：保留 `supports_memory` 字段语义但当前值改 `true`；保留短路分支不删（防御性 dead code）
- **C**：删 `supports_memory` 字段（SSH 已支持 memory，此 flag 不再有意义）

**为什么选 A：**

- 字段语义本来就是"backend 支持 memory CRUD"——SSH 改 true 后字段仍有意义（未来可能加 HTTP backend 不支持 memory 的场景，flag 留作架构扩展点）
- 短路分支是 dead code：`policy.supports_memory` 全局唯三处赋值（local true / ssh false-改-true / http true）都是 true，`if !true` 永远 false。dead code 只让 reviewer 困惑
- 候选 C 把 flag 删掉破坏 fs-abstraction spec 的 `BackendPolicy` 字段清单（`for_ssh` Scenario 钉死 `supports_memory = false`），需要 BREAKING change；本 change 不动字段语义只动值

### D5: `add_memory` / `delete_memory` 文件名校验复用 `validate_memory_file_name`

**候选方案：**

- **A（采纳）**：write 路径用同一 `validate_memory_file_name(file)` 校验函数，与 read 路径同语义（拒绝路径穿越 / 绝对路径 / 非 `.md` / 含 `/` `\` `:`）
- **B**：write 路径用更严格规则（如禁 `MEMORY.md` 覆盖、强制小写 等）
- **C**：write 路径用更宽松规则（允许 `.txt` / `.json` 等）

**为什么选 A：**

- 一致性：read 能读什么文件，write 就能写什么文件。用户在 UI 看到的 `extra_note.md` 既能读也能加（如果文件不存在）
- 候选 B 的"禁覆盖 MEMORY.md"看似安全但反直觉——用户编辑 MEMORY.md 是合理路径，禁覆盖等于禁编辑
- 候选 C 扩 `.md` 之外文件类型超出 `memory-viewer` spec 范围（spec 明确"系统 MUST 只列出 `.md` 文件"），用户写 `.json` 也读不到没意义

### D6: SSH 写路径走现有 retry 机制（与 read 路径一致）

**候选方案：**

- **A（采纳）**：复用 ssh-remote-context spec `Scenario: SFTP transient errors are retried`——`code=4 / EAGAIN / ECONNRESET / ETIMEDOUT / EPIPE` 重试 ≤ 3 次，指数退避 75ms × attempt；写失败封装 `FsError::TransientExhausted { attempts: 3, last_reason }`
- **B**：写路径不重试（避免重复写副作用）

**为什么选 A：**

- atomic write 走 tmp+rename，写 tmp 步骤可重试（同 tmp path 后写覆盖前次失败 tmp，无副作用——后写的 tmp seq 不同，原 tmp 留下当孤儿，best-effort cleanup 收尾）
- rename 步骤的重试有副作用风险：若 rename 已成功但 reply 丢失，retry 会拿到"src 不存在"错误。处理路径：rename 失败 SHALL 先调 `try_exists(<target>) && try_exists(<tmp>)` 探测——target 存在 + tmp 不存在 = rename 已成功（视为成功，避免误判失败再写一次）；target 不存在 + tmp 存在 = rename 真失败（按 transient retry）；双方都不存在 = 异常（向上抛 `FsError::Io`，让上层报错）。**不**做 metadata size/mtime content 验证（codex 二审 #4：size+mtime 无法证明内容同源，相同 size + 相近 mtime 在快速并发写下会误判）
- B 的"不重试"在网络抖动下会让用户觉得写失败但实际可能成功一半；A 的重试 + try_exists 验证更稳

### D9: `add_memory` 写后 RTT 成本 + 不引入 cache 复杂度

**Trade-off：** `add_memory` / `delete_memory` 内部最后一步调 `discover_memory_layers` 同步收集 layers 返回。这意味着每次写：
- Local 上：1 read_dir + N read_to_string，wall ~5-20ms（典型 N < 10）
- SSH 上：1 read_dir RTT + N 串行 read_to_string RTT × ~50ms = 100-550ms wall（典型 N < 10）

代替方案候选：
- **A（采纳）**：当前 design——写后实时调 `discover_memory_layers`
- **B**：写后只返 minimal `ProjectMemory`（只更新 `count` + 在原 `layers` 末尾插入新 entry，不重新读 `MEMORY.md` 索引），前端如需准确 layer 顺序再调一次 `get_project_memory`
- **C**：引入 `MemoryLayerCache` 持久化 layers，写时 invalidate

**为什么选 A：**

- 简单优先：B 看似省 N 次 read，但前端要"决定何时再调 get_project_memory" 增加 UX 决策点；C 的 cache invalidation 成本远超所省的 RTT
- SSH 上 ~500ms wall 是 acceptable——用户保存 markdown 编辑的反应窗口本来就 1-2s（编辑器 debounce + IPC dispatch）；多 0.5s 不影响感知
- 真痛点是"add 100 个文件批量"场景，但 memory 用例上 batch add 不存在（用户手工编辑笔记 1-2 个/次）
- 候选 C 在未来如有人 demand "memory list 实时更新" 才考虑——本 change 留 spec 字段（`policy.supports_memory: true`）即可，不引入 cache 基础设施

### D7: `delete_memory` 删 `MEMORY.md` 行为契约

**候选方案：**

- **A（采纳）**：允许删 `MEMORY.md`；删后 `discover_memory_layers` 自然返 `has_memory: false`（既有 spec 行为）
- **B**：禁删 `MEMORY.md`，只允许删 entry / orphan layers

**为什么选 A：**

- 用户清空 memory 是合理操作；UI 可加确认 dialog 但 IPC 层不应硬限
- 候选 B 制造 read-only 路径与 write 路径不对称——能读 MEMORY.md 就该能删
- `discover_memory_layers` 在 `MEMORY.md` 不存在时已正确处理（返 `has_memory: false`），无需额外 special case

### D8: `add_memory` 文件已存在时覆盖还是报错

**候选方案：**

- **A（采纳）**：覆盖（atomic write 语义保证 reader 看到旧或新整版）
- **B**：返 `AlreadyExists` 错误，强制 caller 先 delete 再 add
- **C**：单独 `update_memory` IPC 走覆盖路径，`add_memory` 走 create-only 路径

**为什么选 A：**

- 简单：一个 IPC 同时支持 create + update，前端无需根据是否存在选不同 IPC
- atomic write 的 tmp+rename 天然支持覆盖，rename 是 atomic replace
- 候选 C 要 2 个 IPC 加复杂度，但用户感知是一回事（"保存当前 markdown"）

## Risks / Trade-offs

| Risk | Mitigation |
|---|---|
| **fs trait 加 3 方法是 breaking change**（既有 backend 全部要实现） | Mitigated by 本仓库唯一 backend 实现是 Local + SSH（`cdt-fs/src/local.rs` + `cdt-ssh/src/provider.rs`），同 PR 一起加完；HTTP backend 当前用 Local 包装不需要单独实现 |
| **SFTP atomic rename 在某些 SFTP server 上不原子**（旧 server 实现可能拒 cross-fs rename，但本场景 tmp+target 同目录，不跨 fs） | Mitigated by tmp 文件与 target 永远同目录（`path.with_extension(...)`）；同 fs rename 是 POSIX 原子。risk acceptable |
| **写 IPC 失败 leave tmp residue**（rename 失败但 tmp 写成功；清理 tmp 也失败） | Mitigated by tmp 命名含 `AtomicU64` 单调序号 + pid 后缀，下次写不冲突；用户可手工删（残留 tmp 文件不影响 read，因为 `discover_memory_layers` 只列 `.md` 文件，tmp 文件是 `.tmp.<hex>` 后缀） |
| **SSH 远端 rename 不原子**（russh-sftp 2.1.2 不暴露 `posix-rename@openssh.com` 扩展 API） | Documented as known limitation：`Features` struct 仅含 hardlink/fsync/statvfs/limits 四 flag，所有 SSH `write_atomic` 一律走"先 `remove(target)` ignore NoSuchFile + 后 `rename(tmp, target)`"两步降级。降级路径有极短窗口（remove 完成到 rename 开始之间的 RTT，~50-100ms）reader 调 `read_memory_file` 可能拿到 `NotFound`——本 change **不**在 read 路径加 retry（codex PR 二审 ITEM 1：单用户单 session 编辑场景，UI 调 `add_memory` 后用返新 `ProjectMemory` swap 而非二次调 read，自然不重叠；跨 client 并发 add+read 罕见，加 retry 反而引入新的语义不确定性）。后续 russh-sftp 暴露 extensions API 时升级真原子路径 + 加 `[follow-up]` 跟踪 |
| **`add_memory` / `delete_memory` SSH 上 wall ~100-550ms**（写后调 `discover_memory_layers` 1+N 次 RTT） | Documented in D9 trade-off：用户感知 0.5s 在编辑保存场景内可接受；不引入 `MemoryLayerCache` 基础设施 |
| **`add_memory` 写完调 `discover_memory_layers` 多一次 read_dir RTT** | Mitigated by 这是 acceptable trade-off——单次写后查总 wall ~150ms 在 SSH 上低于"用户编辑 markdown 输入下一字符"的反应窗口；前端不需要再调 `get_project_memory`，省一次 IPC + invoke 开销 |
| **写路径 atomic 测试在 SSH fake 上无法真验** | Mitigated by `CountedFakeRemoteSftp` 实现 write_count + rename_count 计数器；测试断言"写=1次 write + 1次 rename"形态；真 atomic 性靠 SFTP server side OS 保证（不在 Rust port 责任域） |
| **`active_fs_and_policy` 在 SSH context 下首次调用要等 SSH 连接（200-500ms）**；现有 read 路径已走此路径 | 既有 risk 不变；本 change 不改 SSH 连接逻辑 |
| **xtask check-fs-direct-calls 不覆盖 trait 方法直调** | Out of scope：xtask 守护 `tokio::fs::*` 直调反模式，本 change 加 trait 方法不触发；trait 方法在业务路径调是合规的 |
| **memory 文件 size 上限**（超大 markdown 一次性 atomic write 占内存） | Out of scope：memory 文件设计上是 KB 级笔记，spec 不规约 size 上限；超 1MB 文件 atomic write 一次性 alloc 内容到内存，acceptable |

## Migration Plan

本 change 是**纯增量演进**——前向兼容：

1. fs trait 加方法是 source-breaking 但本仓所有 backend 同 PR 实现，CI 编译期 gate
2. `BackendPolicy::for_ssh().supports_memory = false → true` —— 旧前端在 SSH context 下原本拿 `has_memory: false`，新版拿真实数据；前端 `Sidebar` `if (memory.hasMemory)` 路径自动展开，无需前端改动（除新增 add/delete UI 留 followup 外）
3. 新增 `add_memory` / `delete_memory` IPC——纯增量，老前端不调不影响

回滚策略：单 PR revert 完整恢复——fs trait 方法移除 + supports_memory 改回 false + IPC method 删除。无外部状态需要清理（atomic write 失败时已自清 tmp，成功时 .md 文件本就是用户内容）。

## Open Questions

无。所有设计点已通过 D1-D8 决策；codex 二审若发现遗漏点会标 D9+。
