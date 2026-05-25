## ADDED Requirements

### Requirement: 事件投递时延、远端 polling 频率与停止时延

系统 SHALL 在以下时延 / 频率契约内对外提供事件流，作为可观察的非功能契约（NFR）。本 Requirement 是 file-watching 全部数字契约的唯一归宿——FR 段 MUST NOT 内嵌相同数字。

- **本地 debounce**：系统 SHALL 把同一文件在 100ms 窗口内的连续变更事件合并为一条事件后发出。
- **SSH 远端 polling 频率**：系统 SHALL 在 SSH context 处于 `connected` 状态时每 3 秒发起一次远端目录列举，比对上一轮 baseline 后发出差异事件。
- **SSH 远端 catch-up 兜底**：系统 SHALL 额外每 30 秒强制重跑一轮"全量列举 + baseline 比对"作为兜底，捕获 polling 漏检的差异。
- **断开停止时延**：当 SSH transport 断开或用户主动断连时，系统 SHALL 在 1 秒内停止该 context 的远端 polling 任务并释放远端会话资源。

#### Scenario: 同一文件在 debounce 窗口内连发多次写入仅出一次事件

- **WHEN** 一个文件在 30ms 内发生 5 次写事件
- **THEN** 订阅者 SHALL 在 debounce 窗口结束后**恰好**收到一条 `file-change` 事件

#### Scenario: SSH 连接持续未发任何事件时 catch-up 兜底

- **WHEN** SSH 连接持续 30 秒未发出任何 `FileChangeEvent`（即 polling 检测不到差异）
- **THEN** 系统 SHALL 在 30s 边界强制重跑一轮"全量列举 + baseline 比对"
- **AND** 任何之前漏检的差异 SHALL 在此轮被发出

#### Scenario: SSH 断开后 polling 任务及时停止

- **WHEN** 用户主动断连或 SSH transport 因网络问题断开
- **THEN** 该 context 的 polling 任务 SHALL 在 1s 内退出
- **AND** 远端会话资源 SHALL 被关闭，无资源泄漏

## MODIFIED Requirements

### Requirement: Watch Claude projects directory for session changes

系统 SHALL 递归监视当前 Claude root 下的 `projects` 目录，在 `.jsonl` 会话文件创建、修改、删除时发出变更事件。当前 Claude root SHALL 来自 `general.claudeRootPath`；当该字段为 `null` 时，系统 SHALL 监视默认 home 下 `.claude/projects/`。事件投递的 debounce / 节拍契约见 Requirement「事件投递时延、远端 polling 频率与停止时延」。

#### Scenario: 新建会话文件

- **WHEN** 一个新的 `.jsonl` 文件出现在被监视的项目目录下
- **THEN** 订阅者 SHALL 在 debounce 窗口内收到一条 `file-change` 事件，携带 project id 与 session id

#### Scenario: 已存在会话文件被追加

- **WHEN** 已存在的 `.jsonl` 文件被追加内容
- **THEN** 订阅者 SHALL 收到对应 session 的 `file-change` 事件

#### Scenario: 会话文件被删除

- **WHEN** `.jsonl` 文件被删除
- **THEN** 订阅者 SHALL 收到带删除指示的 `file-change` 事件

#### Scenario: 自定义 Claude root 的 projects 监视

- **WHEN** 当前 Claude root 配置为 `/data/claude-alt`
- **THEN** watcher SHALL 递归监视 `/data/claude-alt/projects/`
- **AND** watcher SHALL NOT 监视默认 `~/.claude/projects/`

### Requirement: Watch Claude todos directory

系统 SHALL 监视当前 Claude root 下的 `todos` 目录中 `.json` 文件变化，并发出携带 session id 的 `todo-change` 事件。当前 Claude root SHALL 来自 `general.claudeRootPath`；当该字段为 `null` 时，系统 SHALL 监视默认 home 下 `.claude/todos/`。

#### Scenario: Todo 文件被更新

- **WHEN** 当前 Claude root 下 `todos/<sessionId>.json` 被更新
- **THEN** 订阅者 SHALL 收到携带该 session id 的 `todo-change` 事件

#### Scenario: 自定义 Claude root 的 todos 监视

- **WHEN** 当前 Claude root 配置为 `/data/claude-alt`
- **THEN** watcher SHALL 监视 `/data/claude-alt/todos/`
- **AND** watcher SHALL NOT 监视默认 `~/.claude/todos/`

### Requirement: Survive transient filesystem errors

系统 SHALL 记录并忽略瞬时错误（permission denied、临时锁占用），不终止 watcher。

#### Scenario: 单文件瞬时权限错误

- **WHEN** watcher 对单个文件 stat 时遇到权限错误
- **THEN** watcher SHALL 记录错误并继续监视其他文件

### Requirement: Broadcast events to multiple subscribers

系统 SHALL 把每条已发出的事件无差别地分发给所有当前活跃的订阅者（前端 webview、HTTP 客户端、in-process 后台服务如通知管线），不重复也不遗漏；任一订阅者的滞后 SHALL NOT 阻塞另一订阅者的投递。

#### Scenario: 两个订阅者并存

- **WHEN** 一次文件变更触发事件且当前有两个订阅者
- **THEN** 两个订阅者 SHALL 各自**恰好**收到一次该事件

#### Scenario: 通知管线与 IPC 消费者并存订阅

- **WHEN** 通知管线启动时订阅文件变更，同时前端 webview 桥也持有一个订阅
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

#### Scenario: 新建 project 目录

- **WHEN** watcher 已启动，且当前 Claude root 下 `projects/new-project/` 被创建
- **THEN** 订阅者 SHALL 在 debounce 窗口内收到一条表示项目列表可能变化的事件
- **AND** 该事件 SHALL 携带新 project id 或足够的信息让 UI 触发项目列表重扫

#### Scenario: 新 project 下首个会话文件

- **WHEN** watcher 已启动，且当前 Claude root 下 `projects/new-project/session-a.jsonl` 被创建
- **THEN** 订阅者 SHALL 收到可触发项目列表重扫的事件
- **AND** 订阅者 SHALL 仍能收到针对 `session-a` 的 `file-change` 事件

#### Scenario: dir-create 后紧跟首个 jsonl 都触发项目列表变更

- **WHEN** watcher 已启动，先收到当前 Claude root 下 `projects/new-project/` 顶层目录创建事件，紧随其后收到该 project 下首个 `projects/new-project/session-a.jsonl` 写入事件
- **THEN** 订阅者 SHALL 先收到一条 `FileChangeEvent { project_id: "new-project", session_id: "", project_list_changed: true }`
- **AND** 订阅者 SHALL 接着收到一条 `FileChangeEvent { project_id: "new-project", session_id: "session-a", project_list_changed: true }`
- **AND** dir-create 分支 MUST NOT 把 `new-project` 写入 `known_projects` —— 首次 mark 由 jsonl 事件独占消耗

#### Scenario: dir-create 不消耗首次见到该 project 的 mark

- **WHEN** watcher 已启动，收到当前 Claude root 下 `projects/never-written/` 顶层目录创建事件，但此后**未**写入任何 `.jsonl`
- **THEN** 订阅者 SHALL 收到 `FileChangeEvent { project_id: "never-written", session_id: "", project_list_changed: true }`
- **AND** `known_projects` 内部状态 MUST NOT 包含 `never-written`，使得未来该 project 下首次出现 `.jsonl` 时仍能 emit `project_list_changed=true`

#### Scenario: 项目刷新事件广播到所有订阅者

- **WHEN** 新 project 目录触发项目刷新事件且当前有两个订阅者
- **THEN** 两个订阅者 SHALL 各自收到该事件，任一订阅者滞后 SHALL NOT 阻塞另一订阅者

#### Scenario: 已知 project 下 session 变更不刷新项目列表

- **WHEN** watcher 已启动，且已知 project 下的 session `.jsonl` 被追加内容
- **THEN** 订阅者 SHALL 收到针对该 session 的 `file-change` 事件
- **AND** 该事件 MUST NOT 标记项目列表变化

#### Scenario: 嵌套 jsonl 不被 project watcher 误识别

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

系统 SHALL 在 SSH context 处于 `connected` 状态时启动远端 polling watcher，作为本地 OS 通知 watcher 的远端等价物。Watcher SHALL 列举远端 `<remote_home>/.claude/projects/<project_id>/` 下所有 `.jsonl` 文件，并对每个文件取 `size` 与 `mtime`；维护一份 baseline 与上一轮比较。差异判定 SHALL 同时考量 `size` 与 `mtime` 两个维度：(a) 新增文件 → emit；(b) `size` 变化 → emit；(c) `size` 不变但 `mtime` 变化 → emit（覆盖"截断后写到原长度"场景，单纯比 size 维度漏检）；(d) 文件不再出现 → emit deletion。SHALL 通过与本地 watcher **完全相同** 的 `FileChangeEvent` schema（字段 `project_id` / `session_id` / `deleted` / `project_list_changed`）广播到所有订阅者。Polling 频率与 catch-up 兜底节拍详见 Requirement「事件投递时延、远端 polling 频率与停止时延」。

mtime 缺失策略：极少数 SFTP server 的 `stat` 不返回 mtime（`mtime = None`），此时 fingerprint 仅依赖 `size`；系统 SHALL 接受"截断后同长度重写"在该场景下漏检的 trade-off。Claude 写 JSONL 是 append-only，实际不存在该场景；watcher SHALL 在 mtime 缺失时把"fingerprint 退化为 size-only"标注到日志一次（避免 spam）。

第一次 poll SHALL NOT 触发任何事件（建 baseline 用）。Watcher SHALL 在 `ssh_disconnect` 或 SSH transport 断开时按 NFR 规约的停止时延停止并释放远端资源。

订阅者 SHALL 不感知事件来自本地还是远端 polling—— `FileChangeEvent` schema 完全统一，下游 `project-discovery` / `session-parsing` / 前端桥 等消费者无须分支处理。

#### Scenario: 首次 poll 静默建立 baseline

- **WHEN** SSH context 切到 `connected`，watcher 启动
- **AND** 远端 `<remote_home>/.claude/projects/p1/` 已有 5 个 `.jsonl` 文件
- **THEN** 第一次 poll 完成后 watcher 内部 baseline SHALL 含 5 个条目
- **AND** SHALL NOT emit 任何 `FileChangeEvent`

#### Scenario: 后续 poll 检测新增 session jsonl

- **WHEN** 远端在两次 poll 之间新增 `<remote_home>/.claude/projects/p1/sess-new.jsonl`
- **THEN** 下一轮 poll watcher SHALL emit `FileChangeEvent { project_id: "p1", session_id: "sess-new", deleted: false, project_list_changed: false }`
- **AND** baseline SHALL 加入该文件 fingerprint

#### Scenario: 后续 poll 检测 size 变化

- **WHEN** 已有文件 `sess-A.jsonl` size 从 1024 增长到 2048
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-A", deleted: false }`

#### Scenario: 后续 poll 检测 mtime 变化但 size 不变

- **WHEN** 已有文件 `sess-B.jsonl` size 不变（仍是 1024）但 mtime 从 `T0` 变成 `T0 + 1s`
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-B", deleted: false }`
- **AND** 该路径覆盖"截断后写回原长度"等单看 size 漏检的场景

#### Scenario: mtime 缺失退化为 size-only fingerprint

- **WHEN** SFTP server 的 `stat` 返回结构 `mtime = None`
- **THEN** watcher SHALL 把 fingerprint 退化为 `(size, None)`，仅按 size 维度判差异
- **AND** SHALL 在日志中标注一次"fingerprint 退化为 size-only"（避免 spam）
- **AND** 后续 poll 仍能检测 size 变化与新增 / 删除事件，仅"截断后同长度重写"会漏（接受 trade-off）

#### Scenario: 后续 poll 检测删除

- **WHEN** 远端 `sess-A.jsonl` 被删除
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-A", deleted: true }`

#### Scenario: SSH 断开时 watcher 立即停止

- **WHEN** 用户调 `ssh_disconnect` 或 SSH transport 因网络问题断开
- **THEN** 该 context 的 polling 任务 SHALL 按 NFR 规约的停止时延退出
- **AND** 远端会话资源 SHALL 被关闭，无资源泄漏

#### Scenario: Watcher 容忍瞬时 SFTP 错误

- **WHEN** 某轮 poll 中远端列举返回瞬时错误（`ETIMEDOUT` / `ECONNRESET`）
- **THEN** watcher SHALL 跳过本轮，下一轮再尝试
- **AND** SHALL NOT 因单次失败而停止 watcher 或要求 SSH 断开

#### Scenario: 订阅者消费远端事件与本地契约一致

- **WHEN** 前端桥同时订阅本地 OS 通知事件流与远端 polling 事件流
- **THEN** 桥 SHALL NOT 区分事件来源；两类事件的 `FileChangeEvent` 字段 schema 完全一致
- **AND** 前端 webview 收到的 payload 形态完全相同

## REMOVED Requirements

### Requirement: Debounce rapid file events

**Reason**: 100ms debounce 是纯数字契约（NFR），按 `openspec/SPEC_GUIDE.md::4 层骨架::第 3 条`「FR 与 NFR 分开」原则迁出 FR 段，统一归入新增 NFR Requirement「事件投递时延、远端 polling 频率与停止时延」。行为契约（"同一文件连发的多次写入只投递一次事件"）保留在 NFR Requirement 的 Scenario「同一文件在 debounce 窗口内连发多次写入仅出一次事件」。

**Migration**: 行为不变；订阅方 SHALL 继续在 debounce 窗口内只收到一次合并后的事件。新 NFR Requirement 提供数字契约的唯一归宿；其它 Requirement Body 引用 debounce 时只描述"在 debounce 窗口内"而不重复写出 `100ms` 数字。
