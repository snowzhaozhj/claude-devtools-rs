## MODIFIED Requirements

### Requirement: Watch Claude projects directory for session changes

系统 SHALL 递归监视当前 Claude root 下的 `projects` 目录，在 `.jsonl` 会话文件创建、修改、删除时发出变更事件。当前 Claude root SHALL 来自 `general.claudeRootPath`；当该字段为 `null` 时，系统 SHALL 监视默认 home 下 `.claude/projects/`。事件投递的 debounce / 节拍契约见 Requirement「事件投递时延、远端 polling 频率与停止时延」。

发出的 `FileChangeEvent` SHALL 在 watcher 能从既有 deleted 判定路径取到 mtime 时填入 `mtime_ms` 字段（毫秒 since UNIX epoch）；取不到时（典型：删除事件、运行环境无 mtime 元数据）SHALL 省略字段。watcher SHALL **不**为填这个字段新增任何额外 fs op——本地实现路径的 deleted 判定本身需要确认文件是否存在，与同一次系统调用合并取出 mtime 是无成本路径；先 deleted 判定再额外取一次 mtime 的形态 SHALL NOT 引入。

`mtime_ms` 字段填写规则的下游消费契约见 `[[ipc-data-api]]::ProjectScanCache 维护 per-project mtime overlay 让 cache 命中路径返回新鲜 mtime` Requirement——本 Requirement 仅承诺生产侧填字段的不变量。

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

#### Scenario: 已存在文件追加事件携带 mtime hint

- **WHEN** 已存在的 `.jsonl` 文件被追加内容，watcher 在 deleted 判定路径取到该文件的当前 mtime
- **THEN** 订阅者收到的 `file-change` 事件 payload SHALL 含 `mtime_ms` 字段，值等于追加完成时刻的 jsonl 文件 mtime（毫秒 since UNIX epoch）
- **AND** 字段值 SHALL 单调推进（同一 session 后续 append 的事件 mtime ≥ 上一条事件 mtime）

#### Scenario: 删除事件不携带 mtime

- **WHEN** `.jsonl` 文件被删除
- **THEN** 订阅者收到的 `file-change` 事件 payload SHALL 省略 `mtime_ms` 字段
- **AND** payload `deleted` 字段 SHALL 为 `true`

#### Scenario: 填 mtime 不增加 fs op

- **WHEN** 本地 watcher 处理一条普通追加事件
- **THEN** watcher SHALL 与既有 deleted 判定共用一次系统调用产出 mtime
- **AND** 若实现路径已存在 deleted 判定（典型：通过判文件存在性决定 deleted 字段），SHALL NOT 在该判定之后再独立取一次 mtime

### Requirement: Watch SSH remote project directory via SFTP polling

系统 SHALL 在 SSH context 处于 `connected` 状态时启动远端 polling watcher，作为本地 OS 通知 watcher 的远端等价物。Watcher SHALL 列举远端 `<remote_home>/.claude/projects/<project_id>/` 下所有 `.jsonl` 文件，并对每个文件取 `size` 与 `mtime`；维护一份 baseline 与上一轮比较。差异判定 SHALL 同时考量 `size` 与 `mtime` 两个维度：(a) 新增文件 → emit；(b) `size` 变化 → emit；(c) `size` 不变但 `mtime` 变化 → emit（覆盖"截断后写到原长度"场景，单纯比 size 维度漏检）；(d) 文件不再出现 → emit deletion。SHALL 通过与本地 watcher **完全相同** 的 `FileChangeEvent` schema（字段 `project_id` / `session_id` / `deleted` / `project_list_changed` / `session_list_changed` / `mtime_ms`）广播到所有订阅者。Polling 频率与 catch-up 兜底节拍详见 Requirement「事件投递时延、远端 polling 频率与停止时延」。

`session_list_changed` 字段填写规则（与本地 watcher 行为对称，详 `跟踪 session 首见性以填写 revalidation hint` Requirement）：baseline 不含 path 的新增 emit SHALL 填 `session_list_changed=true`；baseline 含 path 但当前 readdir 不返的删除 emit SHALL 填 `session_list_changed=true`；baseline 含 path 且仍存在但 size/mtime 变化的追加 emit SHALL 填 `session_list_changed=false`。

`mtime_ms` 字段填写规则（与本地 watcher 行为对称）：watcher 在 fingerprint diff 时已经持有远端文件 mtime，SHALL 透传到事件 payload 的 `mtime_ms` 字段（毫秒 since UNIX epoch）；远端 SFTP server 返回 mtime 缺失时 SHALL 省略字段。SSH 透传路径 SHALL NOT 引入任何额外 SFTP stat——透传源 mtime 已存在于既有 fingerprint。SSH 远端 mtime 与本机时钟跨 clock domain 的语义由下游 `[[ipc-data-api]]::ProjectScanCache 维护 per-project mtime overlay 让 cache 命中路径返回新鲜 mtime` Requirement 单独处理。

**首次启动建 baseline 静默**：watcher 首次 spawn（系统启动后第一次为该 SSH context 起 polling task）时，第一次 poll SHALL NOT 触发任何事件——结果直接成为 baseline，baseline 内的 session 自然算"已见"，避免与本地 lazy 路径同样的 false-positive 问题。

**断连重连 baseline diff**：watcher 因 `ssh_disconnect` 或 transport 失败停止后，再次 spawn 同 context（用户手动重连 / dead-signal monitor 自动重连）时，调用方 SHALL 把上次停止时持有的 baseline 快照传入新 watcher。新 watcher 第一轮 poll 完成后 SHALL 把"新 readdir + stat 结果"与"上次 baseline 快照"做完整 diff，对断连期间出现的新 path emit `FileChangeEvent { ..., session_list_changed: true, deleted: false, mtime_ms: <当前 mtime> }`、对断连期间消失的 path emit `FileChangeEvent { ..., session_list_changed: true, deleted: true }`（mtime_ms 省略）、对 size/mtime 变化的 path emit `FileChangeEvent { ..., session_list_changed: false, deleted: false, mtime_ms: <当前 mtime> }`。diff 完成后新 baseline 替换旧 baseline，进入正常 polling 循环（频率详 NFR Requirement）。该机制保证 SSH 断连重连不漏首见信号——与本地 watcher lazy false-positive 行为达到同等鲁棒性。

调用方未提供旧 baseline 快照时（典型场景：进程重启 / 首次连接），新 watcher SHALL 退化到"首次启动建 baseline 静默"路径，断连期间新增 session 在该场景下漏 emit 一次（接受 trade-off）。

mtime 缺失策略：极少数 SFTP server 的 `stat` 不返回 mtime（`mtime = None`），此时 fingerprint 仅依赖 `size`；系统 SHALL 接受"截断后同长度重写"在该场景下漏检的 trade-off。Claude 写 JSONL 是 append-only，实际不存在该场景；watcher SHALL 在 mtime 缺失时把"fingerprint 退化为 size-only"标注到日志一次（避免 spam）。mtime 缺失时事件 payload SHALL 省略 `mtime_ms` 字段。

第一次 poll SHALL NOT 触发任何事件（建 baseline 用）。Watcher SHALL 在 `ssh_disconnect` 或 SSH transport 断开时按 NFR 规约的停止时延停止并释放远端资源。

订阅者 SHALL 不感知事件来自本地还是远端 polling—— `FileChangeEvent` schema 完全统一，下游 `project-discovery` / `session-parsing` / 前端桥 等消费者无须分支处理。

#### Scenario: 首次 poll 静默建立 baseline

- **WHEN** SSH context 首次切到 `connected`（调用方未提供上次 baseline 快照），watcher 启动
- **AND** 远端 `<remote_home>/.claude/projects/p1/` 已有 5 个 `.jsonl` 文件
- **THEN** 第一次 poll 完成后 watcher 内部 baseline SHALL 含 5 个条目
- **AND** SHALL NOT emit 任何 `FileChangeEvent`

#### Scenario: 断连重连首轮对断连期间增删做 diff

- **WHEN** 同 SSH context 之前已建过 baseline（含 `{sess-A, sess-B}`），后因断网停止；调用方在重新 spawn watcher 时传入该 baseline 快照
- **AND** 断连期间远端发生：新增 `sess-C.jsonl`、删除 `sess-A.jsonl`、`sess-B.jsonl` size 增长
- **AND** 重连后第一轮 poll 完成
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-C", deleted: false, session_list_changed: true, mtime_ms: <C 当前 mtime> }`
- **AND** SHALL emit `FileChangeEvent { project_id, session_id: "sess-A", deleted: true, session_list_changed: true }`（mtime_ms 省略）
- **AND** SHALL emit `FileChangeEvent { project_id, session_id: "sess-B", deleted: false, session_list_changed: false, mtime_ms: <B 当前 mtime> }`
- **AND** 新 baseline SHALL 替换为当前 readdir 结果（`{sess-B, sess-C}`），后续按 NFR 规约的 polling 节拍推进

#### Scenario: 调用方未提供旧 baseline 时退化为静默建 baseline

- **WHEN** SSH context 重连但调用方未传入上次 baseline 快照（如进程已重启 / 首次会话）
- **AND** 远端在重连前用户曾新增 `sess-X.jsonl`
- **THEN** watcher 第一轮 poll SHALL 静默建 baseline，含 `sess-X.jsonl` 在内的全部 readdir 结果
- **AND** SHALL NOT emit 任何 `FileChangeEvent`（接受漏 emit 一次的 trade-off）
- **AND** 后续 `sess-X.jsonl` 上的 size/mtime 变化 SHALL 触发对应 `session_list_changed=false` 事件

#### Scenario: 后续 poll 检测新增 session jsonl

- **WHEN** 远端在两次 poll 之间新增 `<remote_home>/.claude/projects/p1/sess-new.jsonl`
- **THEN** 下一轮 poll watcher SHALL emit `FileChangeEvent { project_id: "p1", session_id: "sess-new", deleted: false, project_list_changed: false, session_list_changed: true, mtime_ms: <当前 mtime> }`
- **AND** baseline SHALL 加入该文件 fingerprint

#### Scenario: 后续 poll 检测 size 变化

- **WHEN** 已有文件 `sess-A.jsonl` size 从 1024 增长到 2048
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-A", deleted: false, session_list_changed: false, mtime_ms: <当前 mtime> }`

#### Scenario: 后续 poll 检测 mtime 变化但 size 不变

- **WHEN** 已有文件 `sess-B.jsonl` size 不变（仍是 1024）但 mtime 从 `T0` 变成 `T0 + Δ`
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-B", deleted: false, session_list_changed: false, mtime_ms: <T0 + Δ> }`
- **AND** 该路径覆盖"截断后写回原长度"等单看 size 漏检的场景

#### Scenario: SSH 透传 mtime 不增加 SFTP stat

- **WHEN** SSH polling watcher 完成一轮 fingerprint diff 并准备 emit 事件
- **THEN** 事件 payload 的 `mtime_ms` 字段 SHALL 来自既有 fingerprint 持有的远端 mtime
- **AND** SHALL NOT 为获取 `mtime_ms` 字段单独发起任何额外 SFTP `stat` 调用

#### Scenario: mtime 缺失退化为 size-only fingerprint

- **WHEN** SFTP server 的 `stat` 返回结构 `mtime = None`
- **THEN** watcher SHALL 把 fingerprint 退化为 `(size, None)`，仅按 size 维度判差异
- **AND** SHALL 在日志中标注一次"fingerprint 退化为 size-only"（避免 spam）
- **AND** 该路径下事件 payload SHALL 省略 `mtime_ms` 字段
- **AND** 后续 poll 仍能检测 size 变化与新增 / 删除事件，仅"截断后同长度重写"会漏（接受 trade-off）

#### Scenario: 后续 poll 检测删除

- **WHEN** 远端 `sess-A.jsonl` 被删除
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-A", deleted: true, session_list_changed: true }`（mtime_ms 省略）

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
