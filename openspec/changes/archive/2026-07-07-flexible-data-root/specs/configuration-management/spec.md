## MODIFIED Requirements

### Requirement: Validate configuration fields before persistence

系统 SHALL 对传入的配置更新做校验（HTTP 端口范围、regex 模式、文件路径、枚举值合法性、URL 模板、scheme 白名单等），非法值 SHALL 被拒绝并附错误说明，不写入坏状态。校验失败 SHALL 整体拒绝该次 update（**不允许半写状态**），settings 文件保持原值。

HTTP 端口校验 SHALL 同时应用于：(a) `update_config` 直接更新 `httpServer.port` 字段；(b) `http_server_start(port)` 间接持久化 `httpServer.port` 字段（详 [[server-mode]]）。两条路径共用同一端口语义——任何能存入 `httpServer.port` 的值都已通过 1024–65535 范围校验。

`claudeRootPath` 校验 SHALL 接受两类合法形态：(a) 绝对路径（POSIX `/`、Windows 盘符、UNC）；(b) 以 `~/`（Windows 上等价 `~\`）开头的 home 相对路径。以 tilde 开头的值 SHALL 被规范化为**保留 tilde 原形**（不在持久化时展开为绝对路径），使配置跨机器 / 跨平台同步时仍可移植；实际 home 展开推迟到数据读取消费点（详 [[project-discovery]]）。其余非绝对、非 `~/` / `~\` 开头的值（相对路径、`~user/` 具名 home 形式）SHALL 被拒绝。

GeneralConfig 三字段校验：

- `externalEditor` / `terminalApp`：严格枚举校验，invalid 值返回 `ApiError::ValidationError`
- `searchEngine`：`Custom` variant 的 `urlTemplate` SHALL 含 `{query}` 占位符；URL scheme SHALL ∈ `{http, https}`（拒绝 `javascript:` / `file:` / `data:` / `chrome:` 等危险 scheme）
- `terminalApp` 跨平台不匹配（macOS 写 Windows / Linux 终端值或反之）SHALL **不**触发 ValidationError——保留写入并在运行时调用对应平台默认终端，附 warn 级日志（详 D-Impl-1）

#### Scenario: Invalid port number

- **WHEN** 调用方把 HTTP 端口设为 1024–65535 之外的值
- **THEN** 更新 SHALL 被拒绝并返回 validation error，已存储值 SHALL 保持不变

#### Scenario: Invalid regex pattern

- **WHEN** 调用方提交长度超过 100 字符的 regex 或含危险结构（嵌套量词等）
- **THEN** 该 regex SHALL 被拒绝并返回错误说明

#### Scenario: Invalid `claude_root_path`

- **WHEN** 调用方把 `claude_root_path` 设为既非绝对路径、也非 `~/` 开头的值（如相对路径 `foo/bar` 或具名 home `~alice/x`）
- **THEN** 更新 SHALL 被拒绝并返回 validation error
- **AND** 已存储值 SHALL 保持不变

#### Scenario: Tilde-prefixed `claude_root_path` accepted and stored verbatim

- **WHEN** 调用方把 `claude_root_path` 设为 `~/.qoder`
- **THEN** 更新 SHALL 被接受
- **AND** 持久化值 SHALL 为 `~/.qoder`（保留 `~/` 原形，不展开为绝对 home 路径）
- **AND** 下次读取配置 SHALL 返回同一 `~/.qoder`

#### Scenario: Windows backslash tilde accepted and stored verbatim

- **WHEN** Windows 上调用方把 `claude_root_path` 设为 `~\.qoder`
- **THEN** 更新 SHALL 被接受
- **AND** 持久化值 SHALL 保留 `~\` 原形（与 `~/` 等价处理，展开推迟到消费点）

#### Scenario: Empty `claude_root_path` clears override

- **WHEN** 调用方把 `claude_root_path` 设为 `null` 或仅空白字符串
- **THEN** 系统 SHALL 将该值规范化为 `None`

#### Scenario: http_server_start 入参端口超范围被拒绝

- **WHEN** 调用 `http_server_start(port=80)` 或 `http_server_start(port=70000)`
- **THEN** server SHALL **不**被启动
- **AND** `httpServer.port` SHALL 保持原值
- **AND** IPC SHALL 返回 validation error 文案

#### Scenario: 未知字段拒绝

- **WHEN** 前端调 `update_general` 含未注册键
- **THEN** 后端 SHALL 返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

#### Scenario: 枚举非法值拒绝

- **WHEN** 前端调 `update_general({ externalEditor: <非白名单值> })` 或 `update_general({ terminalApp: <非白名单值> })`
- **THEN** 后端 SHALL 返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

#### Scenario: SearchEngine.custom 缺 {query} 占位符或危险 scheme 拒绝

- **WHEN** 前端调 `update_general({ searchEngine: { type: "custom", urlTemplate: <缺 {query}> } })`，或 `urlTemplate` scheme ∈ `{javascript:, file:, data:, chrome:}` 等非 http/https
- **THEN** 后端 SHALL 返回 `ApiError::ValidationError`
- **AND** settings 文件 SHALL **不**被修改

#### Scenario: terminalApp 跨平台值不报错

- **WHEN** macOS 上前端调 `update_general({ terminalApp: <Linux 平台值> })`
- **THEN** 后端 SHALL 接受并持久化（统一 enum 跨平台合法）
- **AND** 后续运行时打开终端时 SHALL warn 级日志记录 mismatch + fallback 到 macOS 默认终端

## ADDED Requirements

### Requirement: Persist recent data root history

系统 SHALL 持久化用户切换过的数据根目录历史 `general.recentRoots`（字符串数组），供 UI 快速切换。写入 `claudeRootPath` 为非 `null` 值时，系统 SHALL 把该值加入 `recentRoots`：SHALL 去重、SHALL 按最近使用在前排序、SHALL 限制条目上限（超出上限时淘汰最久未用项）。

去重比较键 SHALL 为**规范化字符串**（trim 尾部路径分隔符；Windows 上大小写不敏感），不做文件系统 canonicalize。因持久化保留 tilde 原形而文件选择器返回展开后的绝对路径，同一目录经"手输 `~/x`"与"选择器选 `/home/u/x`"可能落成两条不同历史项——这是存原形策略的已知代价，两者均能正确切换，不做消费侧反向折叠。

`recentRoots` 中的路径 SHALL 与 `claudeRootPath` 采用同一存储形态与合法性口径（tilde 原形保留；相对路径 / `~user/` 具名 home 等非法项在加载与写入时 SHALL 被过滤，不进历史）。该字段 SHALL NOT 改变任何数据读取行为——仅作为 UI 快速切换的候选来源。`recentRoots` 的 append SHALL 走既有版本化 `update_config` 事务（继承乐观并发控制），SHALL NOT 经绕过版本检查的旁路写入。

#### Scenario: First launch with no config file

- **WHEN** 启动时配置文件不存在
- **THEN** `general.recentRoots` SHALL 为空数组

#### Scenario: Partial config missing recentRoots

- **WHEN** 配置文件解析成功但缺 `recentRoots` 字段（旧版本写入的配置）
- **THEN** 系统 SHALL 合并为空数组，其余字段保留原值

#### Scenario: Switching root appends to history

- **WHEN** 调用方把 `claudeRootPath` 更新为 `~/.qoder`
- **THEN** `~/.qoder` SHALL 出现在 `recentRoots` 中
- **AND** SHALL 位于最近使用位置（数组首位或等价 MRU 语义）

#### Scenario: Re-selecting existing root dedupes

- **WHEN** 调用方把 `claudeRootPath` 更新为一个已存在于 `recentRoots` 的路径
- **THEN** `recentRoots` SHALL NOT 出现该路径的重复项
- **AND** 该路径 SHALL 移动到最近使用位置

#### Scenario: Invalid recentRoots entries filtered on load

- **WHEN** 配置文件的 `recentRoots` 含非法项（相对路径 `foo/bar`、具名 home `~alice/x` 或非字符串）
- **THEN** 系统 SHALL 在加载时过滤掉这些非法项
- **AND** 保留的 `recentRoots` SHALL 只含合法数据根（绝对路径或 tilde 原形）
