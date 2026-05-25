## ADDED Requirements

### Requirement: 跟踪 session 首见性以填写 revalidation hint

系统 SHALL 跟踪每个 `(project_id, session_id)` 组合是否曾被监视器观察过，用于在 watcher 层填写 `FileChangeEvent.session_list_changed` 字段——下游消费者（cache 失效、Tauri push 桥、HTTP SSE 桥）SHALL NOT 再依赖任何外部状态判定该字段。

跟踪集合初始为空（启动时不预填，避免启动期对全量 jsonl 文件做 stat）。系统 SHALL 接受"启动后 / Claude root 重配后 / SSH context 切换后旧 session 的下一次写事件被填为 `session_list_changed=true`"作为 false-positive trade-off——此行为让跟踪集合状态自愈，**不漏**首见信号。

字段填写规则：

- 主 session jsonl 写事件（路径形态 `<projects_dir>/<project_id>/<session_id>.jsonl`，`deleted=false`）：若 `(project_id, session_id)` 此前不在跟踪集合内，SHALL 填 `session_list_changed=true` 并将其加入集合；若已在集合内，SHALL 填 `session_list_changed=false`
- 主 session jsonl 删除事件（`deleted=true`）：SHALL **无条件**填 `session_list_changed=true` 并把对应组合从跟踪集合移除（无论组合此前是否在集合内）
- subagent jsonl 事件（路径形态 `<projects_dir>/<project_id>/<session_id>/subagents/agent-<sub_id>.jsonl`，详 `Route nested subagent JSONL changes to parent session` Requirement）：SHALL NOT 进入跟踪集合，对应事件 SHALL 填 `session_list_changed=false`——subagent 写入是父 session 内部增量，不应触发项目列表 / 总数视图刷新
- 顶层目录创建事件（`session_id=""`）：SHALL 填 `session_list_changed=false`——顶层 dir 创建由 `project_list_changed=true` 单独承载结构信号

跟踪集合的 key 规范化策略 SHALL 与 `known_projects`（详 `Route watch events case-insensitively on Windows` Requirement）一致——Windows 平台下跨大小写漂移 SHALL 视为同一组合，HashSet 仅保留单一条目。

#### Scenario: 已知 project 下首次见 session 触发 first-seen hint

- **WHEN** 已知 project `pa` 下首次出现 `<projects_dir>/pa/sa_new.jsonl` 写入事件，且 `(pa, sa_new)` 此前不在跟踪集合内
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "pa", session_id: "sa_new", deleted: false, project_list_changed: false, session_list_changed: true }`
- **AND** 跟踪集合 SHALL 在事件发出后包含 `(pa, sa_new)`

#### Scenario: 已知 session 后续追加 SHALL NOT 填 first-seen hint

- **WHEN** `(pa, sa)` 已在跟踪集合内，`<projects_dir>/pa/sa.jsonl` 被追加内容
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: false, project_list_changed: false, session_list_changed: false }`

#### Scenario: 删除已知 session 触发 hint 并清理跟踪集合

- **WHEN** `(pa, sa)` 已在跟踪集合内，`<projects_dir>/pa/sa.jsonl` 被删除
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "pa", session_id: "sa", deleted: true, session_list_changed: true }`
- **AND** 跟踪集合 SHALL 在事件发出后**不**含 `(pa, sa)`

#### Scenario: 删除从未见过的 session 仍触发 hint

- **WHEN** `(pa, sa_old)` 从未在跟踪集合内（启动后该 session 从未发生过写事件即被删除），`<projects_dir>/pa/sa_old.jsonl` 被删除
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "pa", session_id: "sa_old", deleted: true, session_list_changed: true }`
- **AND** 跟踪集合操作 SHALL 幂等（移除一个不存在的 key 不报错、不影响其他 key）

#### Scenario: subagent jsonl 事件不进入跟踪集合

- **WHEN** `(pa, sa, agent-sub-1)` 形如 `<projects_dir>/pa/sa/subagents/agent-sub-1.jsonl` 被写入
- **THEN** 订阅者 SHALL 收到对应 `FileChangeEvent`（路由到父 `(pa, sa)`，详 `Route nested subagent JSONL changes to parent session` Requirement）
- **AND** 该事件 SHALL 填 `session_list_changed=false`
- **AND** 跟踪集合 SHALL NOT 因该事件新增任何条目

#### Scenario: 启动后旧 session 第一次写触发 false-positive hint

- **WHEN** 系统启动，跟踪集合为空
- **AND** 用户在已经存在的 `<projects_dir>/pa/sa_existing.jsonl` 上追加内容（首次写事件）
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "pa", session_id: "sa_existing", deleted: false, session_list_changed: true }`（接受 false-positive trade-off，让跟踪集合状态自愈）
- **AND** 同一 `(pa, sa_existing)` 后续 append 事件 SHALL 填 `session_list_changed=false`

## MODIFIED Requirements

### Requirement: Watch SSH remote project directory via SFTP polling

系统 SHALL 在 SSH context 处于 `connected` 状态时启动远端 polling watcher，作为本地 OS 通知 watcher 的远端等价物。Watcher SHALL 每 3 秒调用一次 SFTP `read_dir(<remote_home>/.claude/projects/<project_id>/)` 列举所有 `.jsonl` 文件，并对每个文件取 `size` 与 `mtime`；维护 `BTreeMap<PathBuf, FileFingerprint { size: u64, mtime: Option<SystemTime> }>` baseline 与上一轮比较。差异判定 SHALL 同时考量 `size` 与 `mtime` 两个维度：(a) 新增文件 → emit；(b) `size` 变化 → emit；(c) `size` 不变但 `mtime` 变化 → emit（覆盖"截断后写到原长度"场景，单纯比 size 维度漏检）；(d) 文件不再出现 → emit deletion。SHALL 通过与本地 watcher **完全相同** 的 `FileChangeEvent` schema（字段 `project_id` / `session_id` / `deleted` / `project_list_changed` / `session_list_changed`）广播到所有订阅者。

`session_list_changed` 字段填写规则（与本地 watcher 行为对称，详 `跟踪 session 首见性以填写 revalidation hint` Requirement）：baseline 不含 path 的新增 emit SHALL 填 `session_list_changed=true`；baseline 含 path 但当前 readdir 不返的删除 emit SHALL 填 `session_list_changed=true`；baseline 含 path 且仍存在但 size/mtime 变化的追加 emit SHALL 填 `session_list_changed=false`。

**首次启动建 baseline 静默**：watcher 首次 spawn（系统启动后第一次为该 SSH context 起 polling task）时，第一次 poll SHALL NOT 触发任何事件——结果直接成为 baseline，baseline 内的 session 自然算"已见"，避免与本地 lazy 路径同样的 false-positive 问题。

**断连重连 baseline diff**：watcher 因 `ssh_disconnect` 或 transport 失败停止后，再次 spawn 同 context（用户手动重连 / dead-signal monitor 自动重连）时，调用方 SHALL 把上次停止时持有的 baseline 快照传入新 watcher。新 watcher 第一轮 poll 完成后 SHALL 把"新 readdir + stat 结果"与"上次 baseline 快照"做完整 diff，对断连期间出现的新 path emit `FileChangeEvent { ..., session_list_changed: true, deleted: false }`、对断连期间消失的 path emit `FileChangeEvent { ..., session_list_changed: true, deleted: true }`、对 size/mtime 变化的 path emit `FileChangeEvent { ..., session_list_changed: false, deleted: false }`。diff 完成后新 baseline 替换旧 baseline，进入正常 3s polling 循环。该机制保证 SSH 断连重连不漏首见信号——与本地 watcher lazy false-positive 行为达到同等鲁棒性。

调用方未提供旧 baseline 快照时（典型场景：进程重启 / 首次连接），新 watcher SHALL 退化到"首次启动建 baseline 静默"路径，断连期间新增 session 在该场景下漏 emit 一次（接受 trade-off）。

mtime 缺失策略：极少数 SFTP server 的 `stat` 不返回 mtime（`mtime = None`），此时 fingerprint 仅依赖 `size`；系统 SHALL 接受"截断后同长度重写"在该场景下漏检的 trade-off。Claude 写 JSONL 是 append-only，实际不存在该场景；watcher SHALL 在 mtime 缺失时把"fingerprint 退化为 size-only"标注到日志一次（避免 spam）。

第一次 poll SHALL NOT 触发任何事件（建 baseline 用）；系统 SHALL 额外每 30 秒运行一次 catch-up 比较作为兜底（防 SFTP 偶发丢失差异），catch-up 同样按 size + mtime 双维度比对。Watcher SHALL 在 `ssh_disconnect` 或 SSH transport 断开时 1 秒内停止并释放 SFTP 资源。

订阅者 SHALL 不感知事件来自本地还是远端 polling—— `FileChangeEvent` schema 完全统一，下游 `project-discovery` / `session-parsing` / Tauri push 桥 等消费者无须分支处理。

#### Scenario: First poll establishes baseline silently

- **WHEN** SSH context 首次切到 `connected`（调用方未提供上次 baseline 快照），watcher 启动
- **AND** 远端 `<remote_home>/.claude/projects/p1/` 已有 5 个 `.jsonl` 文件
- **THEN** 第一次 poll 完成后 watcher 内部 baseline SHALL 含 5 个条目
- **AND** SHALL NOT emit 任何 `FileChangeEvent`

#### Scenario: 断连重连首轮对断连期间增删做 diff

- **WHEN** 同 SSH context 之前已建过 baseline（含 `{sess-A, sess-B}`），后因断网停止；调用方在重新 spawn watcher 时传入该 baseline 快照
- **AND** 断连期间远端发生：新增 `sess-C.jsonl`、删除 `sess-A.jsonl`、`sess-B.jsonl` size 增长
- **AND** 重连后第一轮 poll 完成
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-C", deleted: false, session_list_changed: true }`
- **AND** SHALL emit `FileChangeEvent { project_id, session_id: "sess-A", deleted: true, session_list_changed: true }`
- **AND** SHALL emit `FileChangeEvent { project_id, session_id: "sess-B", deleted: false, session_list_changed: false }`
- **AND** 新 baseline SHALL 替换为当前 readdir 结果（`{sess-B, sess-C}`），后续按 3s 正常 polling 推进

#### Scenario: 调用方未提供旧 baseline 时退化为静默建 baseline

- **WHEN** SSH context 重连但调用方未传入上次 baseline 快照（如进程已重启 / 首次会话）
- **AND** 远端在重连前用户曾新增 `sess-X.jsonl`
- **THEN** watcher 第一轮 poll SHALL 静默建 baseline，含 `sess-X.jsonl` 在内的全部 readdir 结果
- **AND** SHALL NOT emit 任何 `FileChangeEvent`（接受漏 emit 一次的 trade-off）
- **AND** 后续 `sess-X.jsonl` 上的 size/mtime 变化 SHALL 触发对应 `session_list_changed=false` 事件

#### Scenario: Subsequent poll detects new session jsonl

- **WHEN** 远端在两次 poll 之间新增 `<remote_home>/.claude/projects/p1/sess-new.jsonl`
- **THEN** 下一轮 poll watcher SHALL emit `FileChangeEvent { project_id: "p1", session_id: "sess-new", deleted: false, project_list_changed: false, session_list_changed: true }`
- **AND** baseline SHALL 加入该文件 fingerprint

#### Scenario: Subsequent poll detects size change

- **WHEN** 已有文件 `sess-A.jsonl` size 从 1024 增长到 2048
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-A", deleted: false, session_list_changed: false }`

#### Scenario: Subsequent poll detects mtime change without size change

- **WHEN** 已有文件 `sess-B.jsonl` size 不变（仍是 1024）但 mtime 从 `T0` 变成 `T0 + 1s`
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-B", deleted: false, session_list_changed: false }`
- **AND** 该路径覆盖"截断后写回原长度"等单看 size 漏检的场景

#### Scenario: mtime missing degrades to size-only fingerprint

- **WHEN** SFTP server 的 `stat` 返回结构 `mtime = None`
- **THEN** watcher SHALL 把 fingerprint 退化为 `(size, None)`，仅按 size 维度判差异
- **AND** SHALL 在日志中标注一次"fingerprint 退化为 size-only"（避免 spam）
- **AND** 后续 poll 仍能检测 size 变化与新增 / 删除事件，仅"截断后同长度重写"会漏（接受 trade-off）

#### Scenario: Subsequent poll detects deletion

- **WHEN** 远端 `sess-A.jsonl` 被删除
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-A", deleted: true, session_list_changed: true }`

#### Scenario: 30 second catch-up timer

- **WHEN** SSH 连接持续 30 秒未发出任何 `FileChangeEvent`（即 3s polling 检测不到差异）
- **THEN** 系统 SHALL 在 30s 边界强制重跑一轮"全量 readdir + stat 与 baseline 比对"
- **AND** 任何之前漏检的差异 SHALL 在此轮被发出

#### Scenario: Watcher stops on disconnect

- **WHEN** 用户调 `ssh_disconnect` 或 SSH transport 因网络问题断开
- **THEN** 该 context 的 polling task SHALL 在 1s 内退出
- **AND** SFTP channel SHALL 被关闭，无资源泄漏

#### Scenario: Watcher tolerates transient SFTP errors

- **WHEN** 某轮 poll 中 `read_dir` 返回瞬时错误（`ETIMEDOUT` / `ECONNRESET`）
- **THEN** watcher SHALL 跳过本轮，下一轮（3s 后）再尝试
- **AND** SHALL NOT 因单次失败而停止 watcher 或要求 SSH 断开

#### Scenario: Subscribers consume remote events with same contract as local

- **WHEN** Tauri push 桥同时订阅本地 OS 通知事件流与远端 polling 事件流
- **THEN** 桥 SHALL NOT 区分事件来源；两类事件的 `FileChangeEvent` 字段 schema 完全一致
- **AND** 前端 webview `listen("file-change")` 收到的 payload 形态完全相同
