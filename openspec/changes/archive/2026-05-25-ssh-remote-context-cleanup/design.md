## Context

`ssh-remote-context` 主 spec（808 行 / 14 Requirement / 92 Scenario）在 port 期 + 多次性能 / 健壮性迭代（read_dir_with_metadata batch / cache hit trust / SkeletonThenStream / russh keepalive / 大文件 K-worker prefetch / 写路径 SFTP API 扩展等）层层叠加，累计 78 处 spec-purity 反模式命中，是全仓密度最高 spec。命中分布：

| 类 | 数 | 主要形态 |
|---|---|---|
| p1 内部模块/类/函数名 | 34 | `russh::client::connect` / `Arc<Mutex<SftpSession>>` / `Box<dyn AsyncRead + Send + Unpin>` / `tracing::warn!(target: ...)` / `cdt_analyze::check_messages_ongoing` 等 Rust 类型签名与函数路径 |
| p2 源文件路径 | 4 | `crates/cdt-ssh/src/provider.rs` / `crates/cdt-api/tests/perf_ssh_*.rs` 等 |
| p3 PR/issue 引用 | 3 | `PR #171` / `PR #205` / `issue #231` / `PR-D` / `PR-F` |
| p4 数字诊断/baseline | 19 | `50 sessions × 50ms = 2.5s` / `< 500ms` / `25s` / `8s` / `75ms` / `5s` / `15s` / `~75s` / `60s` 等具体耗时 |
| p5 实现开关 const | 1 | `PERMANENT_FAILURE_THRESHOLD` |
| p6 库框架名 | 17 | `tokio JoinSet` / `russh` / `russh-keys` / `russh-sftp` / `tracing::` / `broadcast::` / `tauri-plugin-X` / `#[serde]` / `serde` |

当前 baseline `scripts/spec-purity-baseline.txt` 行 `spec/ssh-remote-context 78`。

frontend-test-pyramid 同批姊妹（PR #309）已 archive 在 `change spec-cleanup-frontend-test-pyramid`，工艺直接复用——本 change 是其姊妹工艺第二次实战。

## Goals / Non-Goals

**Goals:**

- 78 处 spec-purity 命中降至接近 0（目标 ≤ 5，接受少量协议常量名 / 标准抽象层概念不可避免命中）
- 行为契约语义 100% 保持不变——14 Requirement / 92 Scenario 的 SHALL / MUST 句语义对等，外部可观察行为不改
- 移除的实现细节作为"参考实现指引"记录在本 design.md，方便后续 reviewer / 维护者溯源到当前实现

**Non-Goals:**

- 不改代码 / 测试 / 配置
- 不改 Requirement / Scenario 数量级（允许等价合并/拆分但不允许丢语义）
- 不改其它 capability spec
- 不改 Purpose 段（已经简洁，无反模式）

## Decisions

### D-1：行为契约 100% 不变

**问题**：78 hits 中部分句子表面像反例（如 "wall time SHALL ≈ 18s"）实则承载用户可感知的性能契约。

**决策**：所有 SHALL / MUST 句的**语义**完全对等迁移：
- 性能数字（25s / 8s / 75ms 等）改为相对描述："超时上限"、"快速重试"、"指数退避" —— 数字保留在本 design.md 参考实现指引段
- 内部类型签名改为概念描述：`Box<dyn AsyncRead + Send + Unpin>` → "异步流式读取句柄"；`Arc<Mutex<SftpSession>>` → "受互斥保护的 SFTP session"
- Rust trait / 函数名（`stat_many` / `open_read` / `read_dir_with_metadata`）SHALL 保留——它们是 cross-capability 抽象层 API 名（`cdt-fs::FileSystemProvider` trait），下游 capability spec（`project-discovery` / `session-parsing`）也直接引用，属"协议常量"类不动
- RFC2119 关键词（SHALL / MUST / SHOULD / MAY）保留英文

**理由**：SPEC_GUIDE 明确 "spec = 用户感知 + 系统外部承诺"。性能数字若是用户感知阈值（< 500ms 首屏）保留为契约性表述（"显著低于首屏预算"），具体毫秒数留 design；若是实现层调优（75ms × attempt 重试退避）整段抽象为"指数退避"。

### D-1b：apply 阶段反转——用户感知数值阈值一律保留具体数值（codex 二审）

**触发**：codex 二审（PR #312）找到 4 blocking + 1 major：
- 连接握手 TCP 5s / SFTP 8s / 总外层 25s 硬超时被抽象为"独立硬超时"——丢失用户可感知"等连接最长多久"
- 退出断开 3s 阻塞上限被抽象为"受配置上限约束"——丢失用户可感知"关闭应用时最长卡多久"
- polling watcher `PERMANENT_FAILURE_THRESHOLD = 3` / `TIMEOUT_FAILURE_THRESHOLD = 6` 与 9s / 18s 自愈窗口被抽象为"两 counter + 远低于主观放弃阈值"——丢失"transport 死多久后自愈"的可测断言
- keepalive `SSH_KEEPALIVE_INTERVAL = 15s` / `SSH_KEEPALIVE_MAX = 3` / ~75s off-by-one 窗口被抽象为"约定常量"——丢失硬故障检测时长契约
- `create_dir_all` retry 关键字 + 3 次 + 75ms 被抽象为"既有 retry 策略"——丢失瞬时错误码白盒分类（与 polling watcher 三分类对称）

**反转**：D-1 原"性能数字一律相对描述"过粗，**反转为按数字性质三分**：

| 数字性质 | 处理 | 例子 |
|---|---|---|
| **用户可感知阈值** | spec **保留具体数值**（含 const 名）作为可测契约 | 连接 TCP 5s / SFTP 8s / 总 25s / 退出 3s / 自愈 9s 与 18s / keepalive 15s 与 75s / poll 1s 取消上限 |
| **协议层硬约束** | spec 用**定性描述**引用协议（"与底层 SFTP READ 单消息上限对齐"），**具体数值留 design** | SFTP packet 上限 32 KiB |
| **实现层调优**（无可观察用户后果，工程师调优旋钮） | 移到 design.md 参考实现指引段，spec 抽象为定性描述 | BufReader 容量数值 / 重试退避基数 75ms / channel capacity / 调度参数 |
| **SFTP 瞬时错误码白盒列表** | spec **保留**作为分类契约（与 polling watcher Permanent/Timeout/OtherTransient 对称） | `code=4` / `EAGAIN` / `ECONNRESET` / `ETIMEDOUT` / `EPIPE` |
| **实证 metric**（"95ms 实测" / bench JSON baseline 数字 / 性能报告快照） | `tests/perf-baseline.json` / design 历史段 | 不放 spec |

**结果**：spec delta 反模式从 0 涨到约 30（数字 + const 名）；同 PR 同 commit 把 baseline `spec/ssh-remote-context` + `change/ssh-remote-context-cleanup` 两条都更新到清理后真实数。可接受——这些数字是行为契约本身，不是反例。

**与原 D-1 关系**：D-1 大方向（"行为契约 100% 不变"）保留；D-1b 把"数字怎么处理"细化。删除 D-1 / 保留 D-1b 是覆盖式反转，但 design.md 历史段保留 D-1 完整文本（按 openspec/CLAUDE.md 第 7 条规约）。

### D-2：反例分类处理策略

按 78 hits 各类分别给出处理规则，apply 阶段照表批改：

| 类 | 处理方式 |
|---|---|
| **p1 内部模块/类/函数名**（34）| Rust 类型签名（`Arc<Mutex<...>>` / `Box<dyn ...>` / `Vec<...>`）→ 概念描述（"互斥保护的 X" / "异步流式 X 句柄" / "X 列表"）；具名函数路径（`russh::client::connect` / `tracing::warn!(...)`）→ 行为描述（"建立 SSH 传输层" / "写入运维侧可见日志"）；trait 抽象层方法名（`stat_many` / `open_read` 等 `FileSystemProvider` trait 方法）**保留**——属 cross-capability 协议名 |
| **p2 源文件路径**（4）| 全部移除，改为抽象描述："cdt-ssh provider 模块" → "SSH 文件系统 provider"；测试文件名 → "对应回归测试"。具体路径移至本 design.md |
| **p3 PR/issue 引用**（3）| 全部移除——`PR #171` / `PR-D` / `PR-F` / `issue #231` 改为相对时序描述："前序 change" / "独立后续 change" / "用户报告的连接超时场景"；具体 PR 编号留 design.md 历史段 |
| **p4 数字诊断**（19）| 用户感知阈值（< 500ms 首屏 / 用户等 30s 等）→ 抽象为"显著低于首屏预算" / "用户主观放弃阈值之内"；实现层调优数字（指数退避基数 / SSH 握手各阶段超时）→ 整段抽象为定性描述，具体毫秒数留 design.md；buffer 容量 32 KiB 与 SFTP packet 上限对齐这类**协议层硬约束**改为"与底层协议消息上限对齐"，具体数值留 design |
| **p5 实现开关 const**（1）| `PERMANENT_FAILURE_THRESHOLD` → "累计达到永久失败阈值" |
| **p6 库框架名**（17）| `russh` / `russh-keys` / `russh-sftp` / `tokio JoinSet` / `broadcast::` / `tracing::` / `tauri-plugin-X` / `serde` / `#[serde]` 全部移除，改为抽象层名：SSH 协议栈库 → "SSH transport"；JoinSet → "并发任务集合"；broadcast → "事件订阅 channel"；tracing 日志 → "结构化日志"；serde 注解 → "序列化注解"；tauri-plugin-X → "宿主桥接"。具体库名留 design.md |

### D-3：必要 Scenario 命名修复

逐 Scenario 看命名是否符合 SPEC_GUIDE.md::Scenario 命名视角（"用户/系统外部可观察事件视角"）。当前 92 Scenario 标题里部分是实现视角命名，apply 阶段一并修：

| 当前命名 | 改为 | 理由 |
|---|---|---|
| `Scenario: open_read 是 trait 方法不再是 inherent` | `Scenario: 调用方通过 trait 句柄即可流式读远端文件` | 原命名是 "trait 方法 vs inherent" 的实现切换视角 |
| `Scenario: stat_many 当前是 SSH 已知假 batch` | `Scenario: SSH 批量 stat 退化为顺序 RTT 是已知限制` | 保留"已知限制"语义但改用行为视角 |
| `Scenario: SSH list 路径 hot path cache hit trust（用户感知卡顿消失）` | `Scenario: 切回已访问 SSH host 列表立刻显示无可感知卡顿` | 去掉实现术语 "cache hit trust" |
| 其它 | 维持 | 已是行为视角（如 "Connect by host alias from ssh config"） |

预计修复 ≤ 8 个 Scenario 命名（命中 D-3 表的 + apply 阶段发现的少量）。

### D-4：trait 抽象层 API 名保留例外

**问题**：D-2 表说 "trait 抽象层方法名保留"，但 `cdt-fs::FileSystemProvider` 这个串本身是 p1 命中（`cdt_fs::Trait`）。

**决策**：trait 名 `FileSystemProvider` 与方法名（`exists` / `read_to_string` / `read_dir` / `read_dir_with_metadata` / `stat` / `stat_many` / `read_lines_head` / `open_read`）保留，**但**带 crate 前缀的 `cdt-fs::FileSystemProvider` 改为 "文件系统 provider 接口"——crate 名是实现组织决策，重构 / 改名不应破 spec。

实施：grep `cdt-fs::` / `cdt-ssh::` / `cdt_analyze::` / `cdt_api::` 等 crate 前缀全部移除，方法名保留裸写。

## Risks / Trade-offs

- **[语义漂移]** 78 hits 批量改写有可能漏掉某句的隐含行为约束（如 "8s SFTP open 超时" 的 8 秒是用户感知阈值还是实现调优？）→ Mitigation：每个 Requirement 独立 commit；apply 完成后跑 spec-guide-reviewer 自审；reviewer 二审重点查行为对等
- **[残留命中]** D-1 决策保留 trait 方法名 / 协议常量，可能剩 5-10 处灰色命中 → Mitigation：接受目标 ≤ 5，超出由 lead 在 baseline 上 approve
- **[阅读成本]** 移除具体数字 / 库名后，新 contributor 看 spec 难判断"当前实现长啥样" → Mitigation：本 design.md 作为参考实现指引留档；contributor 找具体值时去 design.md 而非 spec.md
- **[archive 顺序]** 本 PR 是 ssh-remote-context 单 cap，与同期 PR 4 (`configuration-management` / `frontend-context-menu`) / PR 5 (`sidebar-navigation`) 不撞 cap，可独立 archive 不撞顺序坑

## 参考实现指引（从 spec 移出的实现细节）

以下为当前实现对应关系，供维护者参考。**本节内容不属行为契约**，纯文档备忘——重构这些实现不需要改本 spec。

### Rust 库与抽象层映射

| spec 抽象 | 当前实现 |
|---|---|
| SSH 协议栈库 | `russh` + `russh-keys` + `russh-sftp` |
| 文件系统 provider 接口 | `cdt-fs::FileSystemProvider` trait |
| SSH 文件系统 provider | `cdt-ssh::SshFileSystemProvider` |
| SSH session manager | `cdt-ssh::SshSessionManager` |
| 受互斥保护的 SFTP session | `Arc<Mutex<SftpSession>>`（已知限制：阻碍 message-id pipeline 并发） |
| 异步流式读取句柄 | `Box<dyn AsyncRead + Send + Unpin>` |
| 并发任务集合 | `tokio::task::JoinSet` |
| 事件订阅 channel | `tokio::sync::broadcast::Sender<SshStatusChange>` |
| 结构化日志 | `tracing::warn!(target: "cdt_api::perf", ...)` |
| 宿主桥接（Tauri）| `tauri::AppHandle::emit_all` |
| 序列化注解 | `#[serde(tag = "...")]` / `#[serde(skip_serializing_if = "...")]` |
| Tauri 文件操作插件（如有引用）| `tauri-plugin-fs` |

### 源文件路径

| 模块 | 当前路径 |
|---|---|
| SSH provider | `crates/cdt-ssh/src/provider.rs` |
| SSH session manager | `crates/cdt-ssh/src/session.rs` |
| Cache hit perf 测试 | `crates/cdt-api/tests/perf_ssh_cache_hit.rs` |
| Scanner 分块读测试 | `crates/cdt-api/tests/perf_ssh_scanner_chunked_read.rs` |

### 性能与超时具体数值

| spec 抽象 | 当前数值 |
|---|---|
| SSH 连接外层硬超时 | 25 秒 |
| TCP probe 超时 | 5 秒 |
| SFTP subsystem open 超时 | 8 秒 |
| SFTP 瞬时错误重试次数 | 3 次 |
| SFTP 瞬时错误退避基数 | 75 ms × attempt 指数退避 |
| Polling watcher 默认 interval | 3 秒 |
| 用户主观放弃阈值（issue #231） | 用户走死 SFTP 等约 30 秒 |
| Sidebar 首屏预算 | < 500 ms |
| SSH list 串行 stat 反例数据 | 50 sessions × 50ms = 2.5s wall（超预算 5×） |
| Polling watcher 自愈检测窗 | 约 75 秒 |
| russh keepalive 间隔 | 15 秒 |
| Scanner BufReader 容量 | 32 KiB（与 SFTP `SSH_FXP_READ` reply 单消息上限对齐） |
| App 退出 graceful disconnect 上限 | 3 秒 |
| 永久失败阈值 (`PERMANENT_FAILURE_THRESHOLD`) | 累计 N 次后触发自愈 disconnect |

### 历史 PR 引用映射

| spec 抽象 | 历史 PR / change |
|---|---|
| 前序 change（per-session 朴素 cache 验证基线） | PR-A / PR-B / PR-C 相关 perf 系列 |
| Cache hit trust 落地 | PR-D（已 merge） |
| SkeletonThenStream SSH 入口 | PR-D（已 merge） |
| `read_dir_with_metadata` SFTP 单 RTT 优化 | change `ssh-batch-readdir-with-metadata` |
| russh keepalive 落地 | PR #205 |
| 用户连接 timeout 场景 | issue #231 |
| `MetadataCache` 公共 API 改造 | PR #171 |
| message-id pipeline 并发改造 | 独立后续 change（方案 C 路径，保持"无远端 shell"假设） |
| 写操作 SFTP API 扩展（write/mkdir/remove/rename）| change `ssh-memory-crud` |
