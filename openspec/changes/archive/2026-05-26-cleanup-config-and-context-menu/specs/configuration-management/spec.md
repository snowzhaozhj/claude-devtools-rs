# configuration-management Spec Delta

## MODIFIED Requirements

### Requirement: Resolve and read mentioned files safely

系统 SHALL 把 `@path` mention 解析为相对于当前 session cwd 的路径并读取文件内容，拒绝逃逸到允许根之外的路径。

#### Scenario: Valid in-project mention

- **WHEN** mention `@docs/note.md` 解析后位于 session 的 project root 内
- **THEN** 文件 SHALL 被读取并返回，附带绝对路径、字符数、估算 token 数

#### Scenario: Path traversal attempt

- **WHEN** mention 解析到允许根之外（例如 `@../../etc/passwd`）
- **THEN** 读取 SHALL 被拒绝并返回 validation error

#### Scenario: Sensitive file blocked

- **WHEN** mention 解析后命中敏感文件模式（`.ssh/`、`.env`、`.aws/`、私钥等）
- **THEN** 读取 SHALL 被拒绝，即使路径在允许目录内

#### Scenario: Symlink escape

- **WHEN** mention 解析路径在 project root 内但符号链接目标在外部
- **THEN** 系统 SHALL canonicalize 路径，若真实路径在允许根外则拒绝

#### Scenario: Token limit exceeded

- **WHEN** 被引用文件估算 token 数超过调用方指定的最大值
- **THEN** 读取 SHALL 返回 `null` / `None`

### Requirement: Update notifications SHALL accept full triggers replacement

当调用方更新 notifications 段时，系统 SHALL 把 payload 的 `triggers` 字段解析为通知触发器数组、对每条做 trigger 校验拒绝非法条目、整体替换内存中的 triggers 列表、同步给运行时 trigger 调度器、最后持久化到磁盘。未识别的键 SHALL 仍被忽略但 SHALL 在日志中以 warn 级别附带键名记录，避免再次静默丢字段。

#### Scenario: triggers 字段被整体替换并落盘

- **WHEN** 调用方向 `update_config` IPC 发送 `section="notifications", data={ "triggers": [<新数组>] }`
- **THEN** 系统 SHALL 把 `notifications.triggers` 替换为该数组、同步给运行时 trigger 调度器、写入磁盘
- **AND** 下一次查询启用 triggers SHALL 返回新数组中 `enabled=true` 的子集

#### Scenario: 非法 trigger 拒绝整组写入

- **WHEN** 新 triggers 数组中任意一条 trigger 校验失败
- **THEN** 更新 SHALL 返回 validation error 携带该 trigger id 与失败原因
- **AND** 内存中 `notifications.triggers` 与运行时 trigger 调度器状态 SHALL 保持修改前状态（不部分写入）
- **AND** 磁盘文件 SHALL NOT 被更新

#### Scenario: 未知通知键发出 warn 但不报错

- **WHEN** payload 中含除 `enabled` / `soundEnabled` / `includeSubagentErrors` / `snoozeMinutes` / `triggers` 之外的其它键（例如 `fooBar`）
- **THEN** 该键 SHALL 被忽略，操作仍返回成功
- **AND** 系统 SHALL 在日志中以 warn 级别附带被忽略的键名记录该事件

### Requirement: 持久化「启动时自动检查更新」开关

应用配置 SHALL 包含 `autoUpdateCheckEnabled` 字段，类型为 bool，缺省值为 `true`，在配置文件缺失该字段时 SHALL 反序列化为默认值 `true`。该字段 SHALL 控制应用启动后台自动检查更新行为（详 [[app-auto-update]]），但 MUST NOT 影响手动「检查更新」按钮的可用性。

#### Scenario: 默认值为启用

- **WHEN** 首次启动或老配置文件中无该字段
- **THEN** 配置 `autoUpdateCheckEnabled` SHALL 反序列化为 `true`
- **AND** 后端启动后台检查 SHALL 正常执行

#### Scenario: 关闭开关并持久化

- **WHEN** 调用方通过 `update_config` IPC 把 `autoUpdateCheckEnabled` 设为 `false`
- **THEN** 系统 SHALL 把该字段持久化到磁盘
- **AND** 下次启动 SHALL 跳过后台自动检查

#### Scenario: 开启开关并持久化

- **WHEN** 调用方把 `autoUpdateCheckEnabled` 设为 `true`
- **THEN** 系统 SHALL 持久化为 `true`
- **AND** 下次启动 SHALL 恢复后台自动检查

#### Scenario: 与既有字段合并

- **WHEN** 老配置文件含其它字段但无 `autoUpdateCheckEnabled`
- **THEN** 加载逻辑 SHALL 与默认配置合并，该字段取默认值 `true`，其它已有字段 SHALL NOT 被覆盖

### Requirement: 持久化跳过的更新版本号

应用配置 SHALL 包含 `skippedUpdateVersion` 字段，类型为可空字符串（`null` 或版本号字符串）。缺省值为 `null`；配置文件缺该字段时 SHALL 反序列化为 `null`；持久化时若值为 `null` SHALL 在写盘 JSON 中省略该键以保持文件简洁。该字段用于记录用户主动「跳过此版本」的目标版本号。

#### Scenario: 默认值为空

- **WHEN** 首次启动或老配置文件中无该字段
- **THEN** 配置 `skippedUpdateVersion` SHALL 反序列化为 `null`
- **AND** 持久化时 `skippedUpdateVersion` 字段 SHALL NOT 出现在 JSON 中

#### Scenario: 写入跳过版本

- **WHEN** 调用方通过 `update_config` IPC 传 `{ section: "skippedUpdateVersion", data: "0.3.0" }` 或在 patch 中包含 `skippedUpdateVersion: "0.3.0"`
- **THEN** 系统 SHALL 把 `skippedUpdateVersion = "0.3.0"` 持久化到磁盘
- **AND** 下次读取 SHALL 返回该值

#### Scenario: 清空跳过版本

- **WHEN** 调用方传 `skippedUpdateVersion: null`
- **THEN** 系统 SHALL 把 `skippedUpdateVersion = null` 持久化
- **AND** 下次读取 SHALL 返回 `null`

#### Scenario: 与既有字段合并

- **WHEN** 老配置文件含 triggers / pinned 等字段但无 `skippedUpdateVersion`
- **THEN** 加载逻辑 SHALL 与默认配置合并，保留已有字段，`skippedUpdateVersion` 取默认 `null`，**不**覆盖其他字段

### Requirement: Migrate composite project IDs in pinned sessions on load

配置加载 SHALL 在反序列化配置后、暴露给消费方之前，扫描 `pinnedSessions` 字段（key 为 project_id），把含 `"::"` 分隔的 composite id（形如 `{baseDir}::{hash8}`）fold 为 base_dir（即 `"::"` 之前的部分）。fold 时若多个 composite key 共享同一 base_dir，SHALL 把它们的 pinned 数组合并并按 `(sessionId, pinnedAt)` 去重，**保留 `pinnedAt` 最早**的条目（即用户最早 pin 的时间戳）。

迁移触发（即检测到至少一个 composite key）SHALL 在写回配置文件前把当前文件备份到 `<config-path>.pre-merge-composite.bak`（覆盖已存在的同名备份），再原子写入新内容。备份命名与现有「损坏配置自动备份到 `.bak.<unix_timestamp_ms>`」机制独立，便于人工识别本次迁移的回滚点。

迁移 SHALL 是幂等的——纯粹基于 input 重写，不依赖任何「已迁移」标志位。写盘失败时 SHALL 在日志中以 warn 级别记录失败原因，**不**阻塞启动；下次启动加载时命中同样的 composite key 时再次尝试 fold + 写盘。

`hiddenSessions` 等其它以 project_id 为 key 的同形态配置字段 SHALL 同样应用本迁移规则。`NotificationTrigger.repositoryIds` 存的是 repository group 标识（git-common-dir 绝对路径，详见 [[project-discovery]] `Group projects by git worktree` Requirement），与 composite project id 形态完全不同，SHALL NOT 被本迁移触及。

#### Scenario: pinnedSessions 含 composite key 被 fold 为 base_dir

- **WHEN** 配置文件 `pinnedSessions` 含 `"-Users-foo-repo::abcd1234": [{ sessionId: "s1", pinnedAt: 1000 }]` 与 `"-Users-foo-repo::ef567890": [{ sessionId: "s2", pinnedAt: 2000 }]`
- **AND** 配置加载被触发
- **THEN** 加载完成后内存中的 `pinnedSessions` SHALL 含 `"-Users-foo-repo": [{ sessionId: "s1", pinnedAt: 1000 }, { sessionId: "s2", pinnedAt: 2000 }]`（顺序不要求，按 `sessionId` 字典序或 mtime 倒序均可）
- **AND** SHALL NOT 残留 `"-Users-foo-repo::abcd1234"` 或 `"-Users-foo-repo::ef567890"` key

#### Scenario: 同 sessionId 重复条目去重保留 pinnedAt 最早

- **WHEN** 配置文件含 `"D::h1": [{ sessionId: "s", pinnedAt: 200 }]` 与 `"D::h2": [{ sessionId: "s", pinnedAt: 100 }]`
- **AND** 配置加载被触发
- **THEN** 加载完成后 `pinnedSessions["D"]` SHALL 含**且仅含**一条 `{ sessionId: "s", pinnedAt: 100 }`

#### Scenario: 触发迁移时备份原文件

- **WHEN** 配置文件 `<path>` 含至少一条 composite key 且 fold 后内容与原内容不同
- **AND** 配置加载被触发
- **THEN** 系统 SHALL 在写回前把原文件内容写入 `<path>.pre-merge-composite.bak`
- **AND** 备份写盘 SHALL 在主文件原子写入之前完成

#### Scenario: 未含 composite key 不写盘

- **WHEN** 配置文件 `pinnedSessions` 所有 key 均不含 `"::"`
- **AND** 配置加载被触发
- **THEN** 系统 SHALL NOT 写回主配置文件、SHALL NOT 创建 `.pre-merge-composite.bak`

#### Scenario: 写盘失败不阻塞启动

- **WHEN** fold 检测到 composite key 需要写回
- **AND** 原子写入失败（磁盘满 / 权限拒绝）
- **THEN** 系统 SHALL 在日志中以 warn 级别记录失败原因
- **AND** 内存中的 fold 后状态仍 SHALL 暴露给消费方（避免运行时仍持有 composite key）
- **AND** 配置加载 SHALL 正常返回（不返回 Err）

#### Scenario: 迁移是幂等的

- **WHEN** 已 fold 的配置文件（不含 composite key）再次被加载
- **THEN** 系统 SHALL NOT 触发任何写盘
- **AND** 内存中的 `pinnedSessions` SHALL 与配置文件内容字节一致

#### Scenario: NotificationTrigger repositoryIds 不受迁移影响

- **WHEN** 配置文件含一条 trigger，其 `repositoryIds` 字段为 `["/Users/foo/repo/.git"]`
- **AND** `pinnedSessions` 同时含 composite key
- **AND** 配置加载被触发
- **THEN** 加载完成后该 trigger 的 `repositoryIds` SHALL 保持 `["/Users/foo/repo/.git"]` 字节不变（无论是否含 `"::"`）

### Requirement: HTTP server enabled / port SHALL be persisted in lockstep with lifecycle

应用配置 SHALL 持久化 `httpServer.enabled`（bool）与 `httpServer.port`（端口整数）字段到 `~/.claude/claude-devtools-config.json`，缺省值为 `enabled=false` / `port=3456`，配置文件缺该 section 时 SHALL 反序列化为该默认值。该字段 SHALL 与 `server-mode` capability 的 server lifecycle 协同：

- **`http_server_start(port)` IPC 成功**时 SHALL 把 `enabled=true` + `port=<入参>` 持久化（即使入参 port 与已存值相同）
- **`http_server_stop()` IPC**（成功或幂等）SHALL 把 `enabled=false` 持久化（`port` 字段保留，让用户下次开启时复用上次端口）
- **Tauri app 启动时**读取的 `enabled=true` SHALL 触发自动恢复（详 [[server-mode]]）
- **`http_server_start` 启动失败**（端口冲突 / 校验失败）SHALL **不**写持久化，避免把"想开但开不起来"的状态写盘

`port` 字段独立持久化让用户在 toggle 关闭后再开启时仍能记住上次配的端口；`enabled` 字段是用户意图（"我想要 server mode 开"），与运行时实际状态可能短暂不一致（启动时端口冲突的情况）。

#### Scenario: 启动 server 同时持久化 enabled=true 与 port

- **WHEN** 用户调 `http_server_start(port=3500)` 成功
- **THEN** `claude-devtools-config.json` SHALL 含 `httpServer.enabled = true` 与 `httpServer.port = 3500`
- **AND** 重启 Tauri app SHALL 自动启动 server 在 `127.0.0.1:3500`

#### Scenario: 关闭 server 仅写 enabled=false

- **WHEN** 用户调 `http_server_stop()`
- **THEN** `httpServer.enabled` SHALL 写为 `false`
- **AND** `httpServer.port` SHALL 保留上次成功值（不重置为默认 3456）

#### Scenario: 启动失败不写持久化

- **WHEN** 用户调 `http_server_start(port=3500)`，但 3500 已被占用
- **THEN** IPC SHALL 返回 `Err`
- **AND** `claude-devtools-config.json` 中 `httpServer.enabled` SHALL **不**被改为 `true`（保持 `false` 或上次成功值）

#### Scenario: 老配置文件无 httpServer 字段时使用默认

- **WHEN** 升级到含本 change 的版本，老配置文件无 `httpServer` 字段
- **THEN** 反序列化 SHALL 物化默认 `{ enabled: false, port: 3456 }`
- **AND** 行为 SHALL 与升级前一致（不自动启动 server）
