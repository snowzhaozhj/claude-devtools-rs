# file-watching Specification

## Purpose

监视 `~/.claude/projects/` 与 `~/.claude/todos/` 文件系统变化，以 debounce 后的 broadcast 通道把 `file-change` / `todo-change` 事件分发给多类订阅者（Tauri IPC 层、HTTP SSE、in-process 通知 pipeline），使前端 UI 与后台服务能够实时感知会话与待办变更。
## Requirements
### Requirement: Watch Claude projects directory for session changes

系统 SHALL 递归监视当前 Claude root 下的 `projects` 目录，在 `.jsonl` 会话文件创建、修改、删除时发出变更事件。当前 Claude root SHALL 来自 `general.claudeRootPath`；当该字段为 `null` 时，系统 SHALL 监视默认 home 下 `.claude/projects/`。

#### Scenario: New session file created
- **WHEN** 一个新的 `.jsonl` 文件出现在被监视的项目目录下
- **THEN** 订阅者 SHALL 在 debounce 窗口内收到一条 `file-change` 事件，携带 project id 与 session id

#### Scenario: Existing session file appended
- **WHEN** 已存在的 `.jsonl` 文件被追加内容
- **THEN** 订阅者 SHALL 收到对应 session 的 `file-change` 事件

#### Scenario: Session file deleted
- **WHEN** `.jsonl` 文件被删除
- **THEN** 订阅者 SHALL 收到带删除指示的 `file-change` 事件

#### Scenario: Custom Claude root projects watcher
- **WHEN** 当前 Claude root 配置为 `/data/claude-alt`
- **THEN** watcher SHALL 递归监视 `/data/claude-alt/projects/`
- **AND** watcher SHALL NOT 监视默认 `~/.claude/projects/`

### Requirement: Watch Claude todos directory

系统 SHALL 监视当前 Claude root 下的 `todos` 目录中 `.json` 文件变化，并发出携带 session id 的 `todo-change` 事件。当前 Claude root SHALL 来自 `general.claudeRootPath`；当该字段为 `null` 时，系统 SHALL 监视默认 home 下 `.claude/todos/`。

#### Scenario: Todo file updated
- **WHEN** 当前 Claude root 下 `todos/<sessionId>.json` 被更新
- **THEN** 订阅者 SHALL 收到携带该 session id 的 `todo-change` 事件

#### Scenario: Custom Claude root todos watcher
- **WHEN** 当前 Claude root 配置为 `/data/claude-alt`
- **THEN** watcher SHALL 监视 `/data/claude-alt/todos/`
- **AND** watcher SHALL NOT 监视默认 `~/.claude/todos/`

### Requirement: Debounce rapid file events

系统 SHALL 把同一文件在 100ms 窗口内的连续变更事件合并为一条事件后发出。

#### Scenario: Burst of writes
- **WHEN** 一个文件在 30ms 内发生 5 次写事件
- **THEN** 订阅者 SHALL 在 debounce 窗口结束后**恰好**收到一条 `file-change` 事件

### Requirement: Survive transient filesystem errors

系统 SHALL 记录并忽略瞬时错误（permission denied、临时锁占用），不终止 watcher。

#### Scenario: Temporary permission error on one file
- **WHEN** watcher 对单个文件 stat 时遇到权限错误
- **THEN** watcher SHALL 记录错误并继续监视其他文件

### Requirement: Broadcast events to multiple subscribers

系统 SHALL 把每条已发出的事件无差别地分发给所有当前活跃的订阅者（Electron renderer 经 IPC、HTTP 客户端经 SSE、in-process 后台服务如通知 pipeline），不重复也不遗漏。

#### Scenario: Two subscribers present
- **WHEN** 一次文件变更触发事件且当前有两个订阅者
- **THEN** 两个订阅者 SHALL 各自**恰好**收到一次该事件

#### Scenario: Notification pipeline subscribes alongside IPC consumers
- **WHEN** 通知 pipeline 启动时调用 `subscribe_files()`，同时 Tauri IPC 层也持有一个订阅
- **THEN** 两个订阅者 SHALL 独立收到每一次 debounce 后的 `FileChangeEvent`，且任一订阅者的滞后 SHALL NOT 影响另一订阅者的投递

### Requirement: Route nested subagent JSONL changes to parent session

系统 SHALL 把形如 `<projects_dir>/<project_id>/<session_id>/subagents/agent-<sub_session_id>.jsonl` 的嵌套 subagent JSONL 写入路由为父 `(project_id, session_id)` 的 `FileChangeEvent`，复用与父 session JSONL 相同的 broadcast channel 与 payload schema。`agent-acompact*.jsonl` 与非 `agent-*.jsonl` 命名的文件 SHALL NOT 触发 `FileChangeEvent`。旧结构 `<projects_dir>/<project_id>/agent-*.jsonl`（无父 session 目录嵌套）不在本 Requirement 范围。

嵌套分支 emit 的 `FileChangeEvent.project_list_changed` MUST 固定为 `false`，**不**走既有 2 层路径的 `!deleted && mark_project_seen(project_id)` 派生逻辑。理由：嵌套 subagent 写入只是"父 session 内部增量"信号，不应当让前端 `DashboardView` / `Sidebar` 误以为新项目出现而刷新整个项目列表（极端 race 下若父 session JSONL 尚未触发过事件而子 session 已写入，`mark_project_seen` 会返回 `true`，必须显式短路）。

#### Scenario: Subagent JSONL 文件追加触发父 session 刷新

- **WHEN** `<projects_dir>/p1/sess-A/subagents/agent-sub-1.jsonl` 被追加内容
- **THEN** 订阅者 SHALL 在 debounce 窗口结束后收到一条 `FileChangeEvent { project_id: "p1", session_id: "sess-A", deleted: false, project_list_changed: false }`

#### Scenario: 嵌套分支强制 project_list_changed=false

- **WHEN** `<projects_dir>/p2/sess-B/subagents/agent-sub-9.jsonl` 是 watcher 第一次看到 `p2` 项目（父 session JSONL 此前未触发过任何事件）
- **THEN** 即使内部 `mark_project_seen("p2")` 第一次会返回 `true`，emit 的 `FileChangeEvent.project_list_changed` SHALL 为 `false`（嵌套分支硬编码 `false`，不从 `mark_project_seen` 派生），避免前端误以为有新项目出现并刷新整个项目列表

#### Scenario: Subagent JSONL 文件首次创建触发父 session 刷新

- **WHEN** `<projects_dir>/p1/sess-A/subagents/agent-sub-2.jsonl` 首次出现
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "p1", session_id: "sess-A", deleted: false, .. }`，与父 session JSONL 写入的事件 schema 完全一致

#### Scenario: Subagent JSONL 文件删除触发父 session 刷新

- **WHEN** `<projects_dir>/p1/sess-A/subagents/agent-sub-1.jsonl` 被删除
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "p1", session_id: "sess-A", deleted: true, .. }`

#### Scenario: agent-acompact 文件被忽略

- **WHEN** `<projects_dir>/p1/sess-A/subagents/agent-acompact-xyz.jsonl` 被写入
- **THEN** 订阅者 SHALL NOT 收到任何 `FileChangeEvent`

#### Scenario: 非 agent- 前缀文件被忽略

- **WHEN** `<projects_dir>/p1/sess-A/subagents/notes.txt` 或 `<projects_dir>/p1/sess-A/subagents/random.jsonl` 被写入
- **THEN** 订阅者 SHALL NOT 收到 `FileChangeEvent`

#### Scenario: 旧结构 agent-*.jsonl 不进入本 Requirement 的嵌套判定分支

- **WHEN** `<projects_dir>/p1/agent-sub-3.jsonl`（旧结构 2 层路径，无 `<session_id>/subagents/` 嵌套）被写入
- **THEN** 本 Requirement 的"嵌套 subagent → 父 session"路由 SHALL NOT 触发；该路径 SHALL 由既有 `Watch Claude projects directory for session changes` Requirement 的 2 层判定处理（按 `agent-sub-3` 作为 `session_id` 发出 `FileChangeEvent`），其语义不属本 change 改动范围

### Requirement: Watch project directory additions

系统 SHALL 在当前 Claude root 下的 `projects` 目录运行中新建一级 project 目录或该 project 下首个 `.jsonl` 会话文件时发出可被订阅者识别的项目刷新事件。该事件用于触发项目列表重扫，不替代 `project-discovery` 的权威扫描结果。系统 MUST NOT 因已知 project 下的普通 `.jsonl` 修改反复标记项目列表变化。

顶层 dir-create 事件（`<projects_dir>/<project_id>/` 目录创建本身）SHALL emit `FileChangeEvent { project_id, session_id: "", project_list_changed: true }`。该分支 MUST NOT 调用 `mark_project_seen` 写入 `known_projects` —— "首次见到 `project_id`"的标记 SHALL 在紧随的第一条 `<projects_dir>/<project_id>/<session_id>.jsonl` 写入事件中独占消耗，使该 jsonl 事件 SHALL 仍 emit `project_list_changed=true`，触发前端项目列表重扫时 scanner 能看到已落盘的 jsonl。理由：dir-create 事件触发的 scan 在空目录下因 `project-discovery` 的 `scan_project_dir` 会跳过无 `.jsonl` 的目录而拿不到新 project，必须依赖 jsonl 事件**再次**触发刷新；若 dir-create 提前消耗 mark，后续 jsonl 事件会降级为 `project_list_changed=false`，前端永不重扫。

#### Scenario: New project directory created

- **WHEN** watcher 已启动，且当前 Claude root 下 `projects/new-project/` 被创建
- **THEN** 订阅者 SHALL 在 debounce 窗口内收到一条表示项目列表可能变化的事件
- **AND** 该事件 SHALL 携带新 project id 或足够的信息让 UI 触发项目列表重扫

#### Scenario: First session file in new project created

- **WHEN** watcher 已启动，且当前 Claude root 下 `projects/new-project/session-a.jsonl` 被创建
- **THEN** 订阅者 SHALL 收到可触发项目列表重扫的事件
- **AND** 订阅者 SHALL 仍能收到针对 `session-a` 的 `file-change` 事件

#### Scenario: dir-create followed by first jsonl both signal project list change

- **WHEN** watcher 已启动，先收到当前 Claude root 下 `projects/new-project/` 顶层目录创建事件，紧随其后收到该 project 下首个 `projects/new-project/session-a.jsonl` 写入事件
- **THEN** 订阅者 SHALL 先收到一条 `FileChangeEvent { project_id: "new-project", session_id: "", project_list_changed: true }`
- **AND** 订阅者 SHALL 接着收到一条 `FileChangeEvent { project_id: "new-project", session_id: "session-a", project_list_changed: true }`
- **AND** dir-create 分支 MUST NOT 把 `new-project` 写入 `known_projects` —— 首次 mark 由 jsonl 事件独占消耗

#### Scenario: dir-create does not consume mark_project_seen

- **WHEN** watcher 已启动，收到当前 Claude root 下 `projects/never-written/` 顶层目录创建事件，但此后**未**写入任何 `.jsonl`
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "never-written", session_id: "", project_list_changed: true }`
- **AND** `known_projects` 内部状态 MUST NOT 包含 `never-written`，使得未来该 project 下首次出现 `.jsonl` 时仍能 emit `project_list_changed=true`

#### Scenario: Project refresh event is broadcast to all subscribers

- **WHEN** 新 project 目录触发项目刷新事件且当前有两个订阅者
- **THEN** 两个订阅者 SHALL 各自收到该事件，任一订阅者滞后 SHALL NOT 阻塞另一订阅者

#### Scenario: Existing project session change does not refresh projects

- **WHEN** watcher 已启动，且已知 project 下的 session `.jsonl` 被追加内容
- **THEN** 订阅者 SHALL 收到针对该 session 的 `file-change` 事件
- **AND** 该事件 MUST NOT 标记项目列表变化

#### Scenario: Nested jsonl is ignored by project watcher

- **WHEN** watcher 收到当前 Claude root 下 `projects/project-a/subagents/agent-a.jsonl` 的变化
- **THEN** `file-watching` SHALL NOT 把 `project-a/subagents` 当成 project id 发出 session `file-change`

### Requirement: Route watch events case-insensitively on Windows

文件 watcher 在判定一个 OS 通知回调路径是否落入被监视目录时 SHALL 使用跨平台规范化的前缀匹配，使**Windows 平台**上回调返回的大小写与 canonicalize 后的 `projects_dir` / `todos_dir` 不一致时仍能正确路由事件，**非 Windows 平台**保持字节精确比较。

`known_projects` 去重容器 SHALL 使用同一规范化策略——同一 project 目录在 Windows 上无论以何种大小写出现都 SHALL 只占一个条目；首次见到该 project 的 mark 语义不被大小写漂移破坏。

跨平台规范化 SHALL 与 `project-discovery::Compare paths case-insensitively on Windows` Requirement 共享同一 helper（不允许 `cdt-watch` 自行实现 lowercase / startsWith 逻辑）。

#### Scenario: Windows 上 OS 通知大小写漂移仍正确路由

- **WHEN** 在 Windows 平台运行，`projects_dir = C:\Users\Alice\.claude\projects`，OS 通知回调返回路径 `c:\users\alice\.claude\projects\-Users-Alice-app\session-1.jsonl`
- **THEN** watcher SHALL 把该事件归入 `projects` 命名空间
- **AND** SHALL 正确剥离前缀提取出 `project_id = "-Users-Alice-app"` 与 `session_id = "session-1"`

#### Scenario: 非 Windows 平台保持精确前缀匹配

- **WHEN** 在 Linux 或 macOS 平台运行，`projects_dir = /home/alice/.claude/projects`，OS 通知回调返回路径 `/home/Alice/.claude/projects/-foo-bar/session.jsonl`（注意 `Alice` vs `alice`）
- **THEN** watcher SHALL 不把该事件视为 `projects_dir` 子项（前缀不匹配）
- **AND** 不发出 `FileChangeEvent`

#### Scenario: known_projects 在 Windows 上对大小写漂移去重

- **WHEN** 在 Windows 平台运行，`mark_project_seen` 先以 `C:\projects\foo` 的形式插入，后以 `c:\projects\FOO` 查询
- **THEN** 第二次查询 SHALL 报告 "已见过"，`mark_project_seen` SHALL 返回 `false`，`known_projects` 内部 SHALL 仅含一个条目

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

