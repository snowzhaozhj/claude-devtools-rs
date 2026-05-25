## MODIFIED Requirements

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

系统 SHALL 在 SSH context 处于 `connected` 状态时启动远端 polling watcher，作为本地 OS 通知 watcher 的远端等价物。Watcher SHALL 每 3 秒调用一次 SFTP `read_dir(<remote_home>/.claude/projects/<project_id>/)` 列举所有 `.jsonl` 文件，并对每个文件取 `size` 与 `mtime`；维护 `BTreeMap<PathBuf, FileFingerprint { size: u64, mtime: Option<SystemTime> }>` baseline 与上一轮比较。差异判定 SHALL 同时考量 `size` 与 `mtime` 两个维度：(a) 新增文件 → emit；(b) `size` 变化 → emit；(c) `size` 不变但 `mtime` 变化 → emit（覆盖"截断后写到原长度"场景，单纯比 size 维度漏检）；(d) 文件不再出现 → emit deletion。SHALL 通过与本地 watcher **完全相同** 的 `FileChangeEvent` schema（字段 `project_id` / `session_id` / `deleted` / `project_list_changed`）广播到所有订阅者。

mtime 缺失策略：极少数 SFTP server 的 `stat` 不返回 mtime（`mtime = None`），此时 fingerprint 仅依赖 `size`；系统 SHALL 接受"截断后同长度重写"在该场景下漏检的 trade-off。Claude 写 JSONL 是 append-only，实际不存在该场景；watcher SHALL 在 mtime 缺失时把"fingerprint 退化为 size-only"标注到日志一次（避免 spam）。

第一次 poll SHALL NOT 触发任何事件（建 baseline 用）；系统 SHALL 额外每 30 秒运行一次 catch-up 比较作为兜底（防 SFTP 偶发丢失差异），catch-up 同样按 size + mtime 双维度比对。Watcher SHALL 在 `ssh_disconnect` 或 SSH transport 断开时 1 秒内停止并释放 SFTP 资源。

订阅者 SHALL 不感知事件来自本地还是远端 polling—— `FileChangeEvent` schema 完全统一，下游 `project-discovery` / `session-parsing` / Tauri push 桥 等消费者无须分支处理。

#### Scenario: First poll establishes baseline silently

- **WHEN** SSH context 切到 `connected`，watcher 启动
- **AND** 远端 `<remote_home>/.claude/projects/p1/` 已有 5 个 `.jsonl` 文件
- **THEN** 第一次 poll 完成后 watcher 内部 baseline SHALL 含 5 个条目
- **AND** SHALL NOT emit 任何 `FileChangeEvent`

#### Scenario: Subsequent poll detects new session jsonl

- **WHEN** 远端在两次 poll 之间新增 `<remote_home>/.claude/projects/p1/sess-new.jsonl`
- **THEN** 下一轮 poll watcher SHALL emit `FileChangeEvent { project_id: "p1", session_id: "sess-new", deleted: false, project_list_changed: false }`
- **AND** baseline SHALL 加入该文件 fingerprint

#### Scenario: Subsequent poll detects size change

- **WHEN** 已有文件 `sess-A.jsonl` size 从 1024 增长到 2048
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-A", deleted: false }`

#### Scenario: Subsequent poll detects mtime change without size change

- **WHEN** 已有文件 `sess-B.jsonl` size 不变（仍是 1024）但 mtime 从 `T0` 变成 `T0 + 1s`
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-B", deleted: false }`
- **AND** 该路径覆盖"截断后写回原长度"等单看 size 漏检的场景

#### Scenario: mtime missing degrades to size-only fingerprint

- **WHEN** SFTP server 的 `stat` 返回结构 `mtime = None`
- **THEN** watcher SHALL 把 fingerprint 退化为 `(size, None)`，仅按 size 维度判差异
- **AND** SHALL 在日志中标注一次"fingerprint 退化为 size-only"（避免 spam）
- **AND** 后续 poll 仍能检测 size 变化与新增 / 删除事件，仅"截断后同长度重写"会漏（接受 trade-off）

#### Scenario: Subsequent poll detects deletion

- **WHEN** 远端 `sess-A.jsonl` 被删除
- **THEN** watcher SHALL emit `FileChangeEvent { project_id, session_id: "sess-A", deleted: true }`

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
