# settings-ui Specification

## Purpose

定义 Settings 页面的行为契约：打开方式（TabBar 齿轮单例 tab）、section 导航（General / Notifications）、配置展示与修改、trigger 列表的启用 / 禁用与持久化。所有布尔开关 SHALL 走统一的 `SettingsToggle` 滑块组件以保证视觉一致与无障碍可达。
## Requirements
### Requirement: 打开 Settings 页面

用户 SHALL 能通过 TabBar 齿轮图标打开 Settings 页面。Settings tab SHALL 为单例——若已打开则切换焦点。

#### Scenario: 点击齿轮图标打开 Settings
- **WHEN** 用户点击 TabBar 的齿轮图标且无 Settings tab
- **THEN** 系统 SHALL 创建 type 为 "settings" 的 tab 并设为 active

#### Scenario: 重复点击齿轮图标
- **WHEN** 已有 Settings tab 时用户再次点击齿轮图标
- **THEN** 系统 SHALL 切换焦点到已有 Settings tab

### Requirement: Settings Section 导航

Settings 页面 SHALL 包含 section tab 导航。SHALL 支持 General、Display、Notifications、Connection、CLI、Keyboard、Diagnostics、About 八个 section。Connection section SHALL 仅在 Tauri 桌面 host 渲染；CLI section SHALL 在所有平台渲染。

#### Scenario: 默认显示 General section

- **WHEN** Settings 页面首次打开
- **THEN** SHALL 默认显示 General section

#### Scenario: 切换到 CLI section

- **WHEN** 用户点击 CLI section tab
- **THEN** SHALL 显示 CLI 安装状态、版本信息和操作按钮

#### Scenario: standalone 模式隐藏 Connection tab

- **WHEN** Settings 页面在 HTTP standalone 模式下渲染（无 Tauri runtime）
- **THEN** Section 导航 SHALL NOT 渲染 Connection tab
- **AND** 即使 URL 含 `#connection` hash，SHALL 静默回退到 General section

### Requirement: General Section 展示

General section SHALL 展示当前配置值。MVP 阶段 SHALL 至少展示 theme 设置与数据根目录设置。数据根目录设置 SHALL 显示当前 `general.claudeRootPath`；当值为 `null` 时，UI SHALL 明确展示正在使用默认数据根目录。数据根目录的展示文案 SHALL 使用中性表述（“数据根目录”），不硬绑定特定来源名称。

数据根目录设置 SHALL 额外提供一个快速切换控件，其候选来自 `general.recentRoots`（详 [[configuration-management]]）：用户 SHALL 能从该控件一键切换到历史用过的数据根，无需重新手输或重新经文件选择器选目录。快速切换 SHALL 通过既有 `claudeRootPath` 更新路径落地，切换后新的当前值 SHALL 反映在展示控件中。

#### Scenario: 展示当前 theme
- **WHEN** General section 渲染
- **THEN** SHALL 显示当前 theme 值（dark/light/system）

#### Scenario: 展示默认数据根目录
- **WHEN** General section 渲染且 `general.claudeRootPath = null`
- **THEN** SHALL 显示“使用默认数据目录”或等价提示
- **AND** SHALL 显示默认 root 说明，帮助用户理解项目来自默认 `.claude/projects`

#### Scenario: 展示自定义数据根目录
- **WHEN** General section 渲染且 `general.claudeRootPath = "/data/claude-alt"`
- **THEN** SHALL 在输入框或等价控件中显示 `/data/claude-alt`

#### Scenario: 从历史快速切换数据根
- **WHEN** `general.recentRoots` 含至少一个历史路径，用户在快速切换控件中选择其中一项
- **THEN** 系统 SHALL 通过 `claudeRootPath` 更新路径切换到该数据根
- **AND** 切换后展示控件 SHALL 反映新的当前数据根

#### Scenario: 无历史时不阻塞手动输入
- **WHEN** `general.recentRoots` 为空
- **THEN** General section SHALL 仍支持手动输入路径与文件选择器选目录两条既有入口

### Requirement: Notifications Section 展示

Notifications section SHALL 展示通知全局开关和 trigger 列表。所有开关（`enabled` / `soundEnabled` / 每个 trigger 启用状态）MUST 使用 `SettingsToggle` 滑块组件，而非文字按钮，以便用户能一眼分辨开/关状态。trigger 启用态切换 SHALL 通过 `update_config("notifications", { triggers: [...] })` 路径持久化，并依赖 `configuration-management` 的 "Update notifications SHALL accept full triggers replacement" requirement 保证真正落盘与内存同步。

#### Scenario: 展示通知开关

- **WHEN** Notifications section 渲染
- **THEN** SHALL 显示 enabled 和 soundEnabled 的开关状态，使用 `SettingsToggle` 滑块组件

#### Scenario: Toggle 通知开关

- **WHEN** 用户切换 enabled 开关
- **THEN** 系统 SHALL 调用 update_config API 更新 notifications section，成功后更新 UI 状态

#### Scenario: 展示 trigger 列表

- **WHEN** Notifications section 渲染且配置中有 triggers
- **THEN** SHALL 显示 trigger 名称、颜色、启用状态列表；每个 trigger 的启用状态 SHALL 使用 `SettingsToggle` 滑块呈现

#### Scenario: Toggle 单个 trigger 启用态

- **WHEN** 用户切换某个 trigger 的 `SettingsToggle`
- **THEN** 系统 SHALL 乐观更新本地 `config.notifications.triggers[i].enabled`、调用 `update_config("notifications", { triggers: [...] })`
- **AND** 成功后 UI SHALL 保持新状态；失败时 SHALL 重新 `get_config` 回滚并显示错误

### Requirement: 配置加载与错误处理

Settings 页面打开时 SHALL 从后端加载配置。加载失败 SHALL 显示错误提示。用户修改配置时，UI SHALL 先乐观更新本地状态并调用后端；后端失败时 SHALL 重新 `get_config` 回滚并显示错误。

#### Scenario: 配置加载成功
- **WHEN** Settings 页面打开
- **THEN** SHALL 调用 get_config API，显示 loading 状态，成功后渲染配置内容

#### Scenario: 配置加载失败
- **WHEN** get_config API 调用失败
- **THEN** SHALL 显示错误提示，不崩溃

#### Scenario: 通过系统目录选择器保存自定义 Claude root
- **WHEN** 用户在 General section 中点击选择目录并从系统文件管理器选择 `/data/claude-alt`
- **THEN** UI SHALL 调用 `update_config("general", { claudeRootPath: "/data/claude-alt" })`
- **AND** 成功后 UI SHALL 保持该路径为当前值

#### Scenario: 手动输入保存自定义 Claude root
- **WHEN** 用户在 General section 中手动输入绝对路径 `/data/claude-alt` 并保存
- **THEN** UI SHALL 调用 `update_config("general", { claudeRootPath: "/data/claude-alt" })`
- **AND** 成功后 UI SHALL 保持该路径为当前值

#### Scenario: 清空 Claude root 恢复默认
- **WHEN** 用户清空 Claude root 输入并保存或点击恢复默认控件
- **THEN** UI SHALL 调用 `update_config("general", { claudeRootPath: null })`
- **AND** 成功后 UI SHALL 显示默认 Claude root 状态

#### Scenario: 相对路径保存失败并回滚
- **WHEN** 用户输入相对路径 `relative/path` 并保存
- **AND** 后端返回 validation error
- **THEN** UI SHALL 显示错误提示
- **AND** UI SHALL 重新加载配置并恢复到保存前的 `general.claudeRootPath`

### Requirement: 布尔开关视觉规范统一为滑块样式

Settings 页面中所有布尔开关（通用区 `autoExpandAiGroups`、通知区 `enabled` / `soundEnabled`、以及每个 trigger 的启用态）SHALL 使用统一的 `SettingsToggle` 组件——Linear 风格的滑块 Switch。该组件 MUST 表达以下可辨识状态：未启用（灰色 track + 左侧 thumb）、启用（紫色 track + 右侧 thumb）、禁用（整体半透明且不可点击）。组件 API MUST 提供 `enabled: boolean / onChange: (v: boolean) => void / disabled?: boolean` 三个属性，并在按钮元素上设置 `role="switch"` + `aria-checked` 以保证可访问性。

#### Scenario: 启用态显示紫色 track + 右侧 thumb

- **WHEN** `enabled=true`
- **THEN** 组件 SHALL 渲染紫色 track + thumb 位于右侧
- **AND** `aria-checked` SHALL 为 `true`

#### Scenario: 未启用态显示灰色 track + 左侧 thumb

- **WHEN** `enabled=false`
- **THEN** 组件 SHALL 渲染灰色 track + thumb 位于左侧
- **AND** `aria-checked` SHALL 为 `false`

#### Scenario: 点击触发 onChange

- **WHEN** 用户点击组件且 `disabled=false`
- **THEN** 组件 SHALL 调用 `onChange(!enabled)`

#### Scenario: disabled 态不响应点击

- **WHEN** `disabled=true`
- **THEN** 组件 SHALL 渲染 50% 透明度 + 光标 `not-allowed`
- **AND** 点击 SHALL NOT 触发 `onChange`

### Requirement: General section "Use WSL" 按钮

General section SHALL 在 `claudeRootPath` 输入控件下方提供 "Use WSL" 按钮。该按钮 SHALL 仅在 Windows 平台显示。点击按钮 SHALL 触发 `list_wsl_distros` IPC 调用，IPC 返回结构为 `WslDistroScanReport { candidates, distrosWithoutHome }`，UI SHALL 按以下分支处理：

- `candidates.length == 1`：SHALL 自动调用 `update_config("general", { claudeRootPath: candidates[0].claudeRootPath })`，并通过 toast 或 inline 文案提示已切换
- `candidates.length >= 2`：SHALL 显示 distro 选择 modal（apply 阶段新建的共享 `Modal.svelte` 组件），用户选择后 SHALL 调用 `update_config` 应用所选 candidate
- `candidates.length == 0 && distrosWithoutHome.length == 0`：SHALL 在按钮下方显示 inline 文案"未检测到 WSL distro"
- `candidates.length == 0 && distrosWithoutHome.length > 0`：SHALL 在按钮下方显示 inline 文案"检测到 WSL distro 但无法解析 home"，并附 `distrosWithoutHome` 的 distro 名列表（用于排查）
- IPC 调用失败：SHALL 显示 inline 错误提示

distro 选择 modal SHALL 在每个候选项展示 `distro` 名、`claudeRootPath` UNC 路径，并在 `claudeRootExists = false` 时附加视觉提示（例"该 distro 内尚无 Claude 数据"）但**不**禁用选择。

#### Scenario: 非 Windows 平台不显示按钮

- **WHEN** Settings 页面在 macOS 或 Linux 上渲染
- **THEN** General section SHALL NOT 渲染 "Use WSL" 按钮

#### Scenario: Windows 平台单 distro 自动应用

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回单个 candidate `{ distro: "Ubuntu", claudeRootPath: "\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude", ... }`
- **THEN** 系统 SHALL 调用 `update_config("general", { claudeRootPath: "\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude" })`
- **AND** SHALL 在 UI 显示成功提示
- **AND** SHALL NOT 弹出 modal

#### Scenario: Windows 平台多 distro 弹选择

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回 `["Ubuntu", "Debian-12"]` 两个 candidate
- **THEN** SHALL 弹出 distro 选择 modal
- **AND** modal SHALL 列出两个 candidate 的 distro 名与 `claudeRootPath`
- **AND** 用户选定 `Debian-12` 并确认后 SHALL 调用 `update_config("general", { claudeRootPath: "\\\\wsl.localhost\\Debian-12\\..." })`

#### Scenario: distro 内尚无 Claude 数据

- **WHEN** modal 渲染某 candidate 时 `claudeRootExists = false`
- **THEN** SHALL 在该候选行显示视觉提示文案（例"该 distro 内尚无 Claude 数据"）
- **AND** SHALL 仍允许用户选择该候选

#### Scenario: WSL 未检测到

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回 `{ candidates: [], distrosWithoutHome: [] }`
- **THEN** SHALL 在按钮下方显示 inline 文案"未检测到 WSL distro"
- **AND** SHALL NOT 弹出 modal
- **AND** SHALL NOT 调用 `update_config`

#### Scenario: 检测到 distro 但全部 home 解析失败

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` 返回 `{ candidates: [], distrosWithoutHome: ["Ubuntu", "Debian-12"] }`
- **THEN** SHALL 在按钮下方显示 inline 文案"检测到 WSL distro 但无法解析 home"
- **AND** 文案 SHALL 包含 `Ubuntu` 与 `Debian-12` 的 distro 名供用户排查
- **AND** SHALL NOT 弹出 modal
- **AND** SHALL NOT 调用 `update_config`

#### Scenario: IPC 调用失败

- **WHEN** Windows 用户点击 "Use WSL" 按钮
- **AND** `list_wsl_distros` IPC 调用返回错误
- **THEN** SHALL 在按钮下方显示 inline 错误提示
- **AND** SHALL NOT 弹出 modal
- **AND** SHALL NOT 调用 `update_config`

### Requirement: Connection Section 展示与 SSH 连接表单

Connection section SHALL 提供 SSH 连接管理界面。表单字段 SHALL 包含：

- `host`：combobox 控件，下拉联想列表来自 `ssh_get_config_hosts` IPC 调用结果（`~/.ssh/config` 中所有 Host alias）；用户也可手输非 alias 的 hostname
- `port`：默认 22；当用户在 host combobox 选中 alias 后 SHALL 自动调 `ssh_resolve_host` 并填充解析得到的 port
- `username`：同 port，alias 选中后自动填充
- `authMethod`：单选控件，选项 `sshConfig`（默认，使用鉴权候选链） / `password`
- `password`：单行输入框，仅在 `authMethod === "password"` 时可见；密码值 SHALL 仅 in-memory 持有，绝不通过 `ssh_save_last_connection` 持久化

控件区下方 SHALL 显示三个按钮：`Connect` / `Test connection` / `Save as profile`；当 active context 已是 SSH 时 SHALL 同时显示 `Disconnect` 按钮。已保存的 profiles（来自 `ssh.profiles[]`）SHALL 显示为按钮列表，点击一键填充表单。当前连接状态 SHALL 通过 `ConnectionStatusBadge` 组件展示（图标 + 状态文字）。Windows 平台 SHALL 在表单上方显示 inline 提示"v1 Windows 仅支持密码模式或 IdentityFile 直读，命名管道 ssh-agent 计划在 v2 加入"。

#### Scenario: 默认渲染状态

- **WHEN** Connection section 首次渲染且 `ssh.last_connection` 未持久化
- **THEN** 表单 SHALL 渲染空 host / port=22 / 空 username / authMethod="sshConfig"
- **AND** ConnectionStatusBadge SHALL 显示 `disconnected`

#### Scenario: host combobox 联想 ssh config

- **WHEN** 用户聚焦 host 输入框
- **THEN** 系统 SHALL 调 `ssh_get_config_hosts` IPC 拿 alias 列表
- **AND** 输入框下方 SHALL 显示联想下拉，按用户当前输入子串 fuzzy 过滤

#### Scenario: 选中 alias 自动填充

- **WHEN** 用户在 combobox 中选中 alias `myserver`
- **THEN** 前端 SHALL 调 `ssh_resolve_host("myserver")` 取解析结果
- **AND** 解析得到的 port / user / identityFile 字段 SHALL 自动填充表单（用户仍可覆盖）

#### Scenario: password 模式只读 password 字段不持久化

- **WHEN** 用户选 `authMethod = password`
- **AND** 输入密码 `secret` 后点击 Connect
- **THEN** 前端 SHALL 调 `ssh_connect` 携带 password 字段
- **AND** 即使用户后点击 `Save as profile`，前端 SHALL 调 `ssh_save_last_connection` 时 SHALL NOT 携带 password 字段
- **AND** 持久化结果 SHALL 仅含 host / port / username / authMethod 四字段

#### Scenario: Test connection 不切换 active context

- **WHEN** 用户填好表单点击 `Test connection` 且当前 active 是 `Local`
- **THEN** 前端 SHALL 调 `ssh_test_connection` IPC
- **AND** 响应成功后 SHALL 在 inline 区显示"测试成功"提示
- **AND** active context SHALL 仍是 `Local`，未注册新 SSH context

#### Scenario: Connect 成功切换 active context

- **WHEN** 用户填好表单点击 `Connect`
- **THEN** 前端 SHALL 调 `ssh_connect` IPC
- **AND** 响应成功后 ConnectionStatusBadge SHALL 切到 `connected`
- **AND** ContextSwitchOverlay 全屏 loading SHALL 在切换期间短暂出现并在 `context_changed` 事件后退场

#### Scenario: Connect 失败显示 auth chain 诊断

- **WHEN** 用户点击 `Connect` 但鉴权全部失败
- **THEN** 前端 SHALL 从 `ssh_status` 事件 payload 的 `error.attempts[]` 渲染逐源诊断列表
- **AND** 每行 SHALL 含 source 名 + outcome（成功/失败/跳过）+ reason

#### Scenario: Save as profile 持久化（无密码）

- **WHEN** 用户填好表单点击 `Save as profile` 并输入 profile 名 `prod-server`
- **THEN** 前端 SHALL 调 `update_config("ssh", { profiles: [..., { name: "prod-server", host, port, username, authMethod, passwordRequired }] })`
- **AND** 后续 saved profiles 列表 SHALL 渲染该按钮，点击一键填充表单

#### Scenario: Disconnect 按钮回退到 Local

- **WHEN** 当前 active 是 SSH context，用户点击 `Disconnect`
- **THEN** 前端 SHALL 调 `ssh_disconnect` IPC
- **AND** 响应成功后 active context SHALL 切回 `Local`
- **AND** 表单字段保持原值（不清空，方便用户重连）

#### Scenario: Windows 平台显示限制提示

- **WHEN** Connection section 在 Windows 平台渲染
- **THEN** 表单上方 SHALL 显示 inline 提示"v1 Windows 仅支持密码模式或 IdentityFile 直读，命名管道 ssh-agent 计划在 v2 加入"
- **AND** 表单功能照常工作（用户可选 password 模式正常连接）

### Requirement: General Section SHALL render Browser Access subsection in Tauri runtime

General section SHALL 在 Tauri runtime 下额外渲染一个 "Browser Access" 子区块，包含三部分内容：

1. **`SettingsToggle` 切换** "Enable server mode"（受 `httpServer.enabled` 驱动），副文案 SHALL 为 "Start an HTTP server to access the UI from a browser or embed in iframes"
2. **运行状态行**：当 `http_server_status` 返回 `running: true` 时 SHALL 显示绿点 + `Running on http://localhost:{port}` + 一个 "Copy URL" 按钮（点击复制 URL 到剪贴板）；`running: false` 但 `enabled: true` 时 SHALL 显示警告文案（如 "Failed to start: port may be in use"）
3. **端口输入**：可点击编辑当前端口（`SettingsTextInput` 数字输入），保存时 SHALL 调 `update_config` 把 `httpServer.port` 持久化；server 已运行时改端口 SHALL 提示用户需先关再开生效（或后端实现重启逻辑——本 change 只规约 UI 行为，重启与否在实现期决定，**但行为 SHALL 一致可预测**）

整个 "Browser Access" 子区块 SHALL **仅**在 Tauri runtime 渲染（前端通过 `window.__TAURI_INTERNALS__` 检测）；浏览器 runtime 加载时 SHALL 隐藏该子区块——浏览器中的用户已经在用 server，再展示一个开关无意义且会让用户在浏览器里关闭 server 后失联。

切换 toggle 操作 SHALL 调 `http_server_start({ port })`（开启）或 `http_server_stop()`（关闭）IPC；操作进行中（pending）SHALL 把 toggle 设为 disabled 防止重复点击，操作返回错误 SHALL 用 inline 错误提示而非 toast（保留持续可见以便用户改 port）。

#### Scenario: Tauri runtime 默认隐藏 Browser Access 状态行

- **WHEN** Settings General section 在 Tauri runtime 渲染，`httpServer.enabled = false`
- **THEN** "Browser Access" 子区块 SHALL 显示标题 + toggle off 状态 + 端口输入框 + 副文案
- **AND** SHALL **不**显示绿点 / Running URL / Copy 按钮

#### Scenario: Toggle 开启后展示运行 URL

- **WHEN** 用户在 Tauri runtime Settings 中点击 "Enable server mode" toggle，IPC 启动成功
- **THEN** UI SHALL 显示绿点 + `Running on http://localhost:3456`（或当前 port）+ "Copy URL" 按钮
- **AND** toggle SHALL 显示为开启状态

#### Scenario: Copy URL 按钮复制到剪贴板

- **WHEN** server 运行中，用户点击 "Copy URL" 按钮
- **THEN** UI SHALL 把 `http://localhost:{port}` 写入系统剪贴板
- **AND** SHALL 给一个临时视觉反馈（如按钮文案短暂变 "Copied"）

#### Scenario: 启动失败 inline 错误展示

- **WHEN** 用户开 toggle，IPC 返回端口冲突错误
- **THEN** toggle SHALL 自动回到 off 状态
- **AND** 子区块内 SHALL 出现 inline 错误文案描述冲突 + 建议改 port
- **AND** 错误文案 SHALL 保持显示直到用户改 port 或再次尝试（**不**自动消失）

#### Scenario: 浏览器 runtime 隐藏 Browser Access 子区块

- **WHEN** 用户从 Chrome 浏览器加载 Settings 页面，`window.__TAURI_INTERNALS__` 不存在
- **THEN** Settings General section SHALL **不**渲染 "Browser Access" 子区块
- **AND** 其它 General 配置项（theme / Claude root 等）SHALL 正常渲染

### Requirement: Diagnostics tab 暴露 telemetry 快照

Settings 页面 SHALL 在 section 导航中新增 `Diagnostics` tab，与 `General` / `Notifications` 同级。Tab 挂载时 SHALL 调用一次 `getTelemetrySnapshot()` IPC 拿当前快照。

Tab 内容 SHALL 包含四个区域：

1. **顶部仪表盘卡片**（4 个）：cache hit rate、IPC error rate、panic count、SSH 重连次数；数值取自 telemetry snapshot 的 counter（`metadata.cache.hit` / `cdt_api.error` / `cdt_api.warn` / `panic.recovered` / `cdt_ssh.reconnect`）。
2. **延迟分布柱状图**：渲染 `histograms["ipc.list_sessions.duration_ns"]` 与 `histograms["ipc.get_session_detail.duration_ns"]` 的 32 个 power-of-2 bucket；图下方文字标 p50 / p95 / p99 数值，**MUST** 在数值旁注明"power-of-2 bucket upper bound（实际值 ≤ 此值，最坏 2x 偏差）"，避免用户误读为精确测量。
3. **最近 events 列表**：表格渲染 `recentEvents[]`（最多 100 条），列为 timestamp / kind / fields，按 timestamp 倒序。
4. **复制完整 snapshot 按钮**：点击 SHALL 把 `JSON.stringify(snapshot, null, 2)` 写入系统剪贴板并显示 toast"已复制"。

数据获取策略：

- Tab 首次 mount 时拉一次 snapshot；可显示 loading 中间态。
- 提供"刷新"按钮触发再拉一次；按钮按下到数据返回期间 SHALL 保留旧数据展示，避免闪屏。
- SHALL NOT 实现轮询 / 自动刷新——避免抢主线程；用户主动 pull 即可。

Tab 仅读不写，SHALL NOT 暴露任何修改 telemetry 状态的操作。

#### Scenario: 用户打开 Diagnostics tab

- **WHEN** 用户在 Settings 页 sidebar 点击 `Diagnostics` 项
- **THEN** 系统 SHALL 切换到 Diagnostics tab 并调一次 `getTelemetrySnapshot()` IPC
- **AND** SHALL 渲染 4 个仪表盘卡片 + 2 个延迟分布柱状图 + 最近 events 表格 + 复制按钮
- **AND** SHALL 在 1 秒内显示数据（loading 中间态可接受）

#### Scenario: 用户点击复制按钮

- **WHEN** 用户在 Diagnostics tab 顶部点击"复制完整 snapshot"按钮
- **THEN** 系统 SHALL 把 `JSON.stringify(snapshot, null, 2)` 写入系统剪贴板
- **AND** SHALL 显示 toast"已复制"持续 2 秒
- **AND** snapshot JSON SHALL 包含完整 schemaVersion / counters / histograms / recentEvents 字段

#### Scenario: 用户点击刷新按钮

- **WHEN** 用户在 Diagnostics tab 点击刷新按钮
- **THEN** 系统 SHALL 重新调 `getTelemetrySnapshot()` 拿新数据
- **AND** 在新数据返回前 SHALL 保持旧仪表盘 / 柱状图 / events 列表的渲染
- **AND** 新数据到达后 SHALL in-place 替换数值（不经"loading..."中间态）

#### Scenario: tab 仅读不写

- **WHEN** 用户在 Diagnostics tab 任意操作（除复制 / 刷新外）
- **THEN** 系统 SHALL 不提供"重置 counter"或"清空 events"按钮
- **AND** SHALL 不调用任何修改 telemetry 状态的 IPC

### Requirement: Keyboard Shortcuts Section

Settings 页面 SHALL 在 Section 导航中新增独立 tab "键盘快捷键"，与 General / Notifications / Connection 平级。该 tab SHALL 在所有运行模式（含 standalone、SSH、HTTP server）下可见。

Tab 内容 SHALL 按 `keyboard-shortcuts` capability 的 `ShortcutSpec.category`（`global` / `tabs` / `sidebar` / `search` / `session`）分组渲染；每个 category SHALL 用统一的视觉层级（标题字号 / 行间距遵循 `DESIGN.md::The Tool Density Rule`）。每行 SHALL 展示：

- 左：`description`（自然语言中文）
- 中：当前 binding 的可视化表达（mono 字体，遵循 `DESIGN.md::The Machine Information Rule`）
- 右：弱化的"重置默认"按钮（仅当当前 binding 与 default 不一致时启用）

#### Scenario: tab 入口

- **WHEN** 用户打开 Settings 页面
- **THEN** 左侧导航 SHALL 显示 "键盘快捷键" 入口，与 General / Notifications / Connection 平级
- **AND** 默认未选中状态下入口行 SHALL 用 neutral hover bg、不引入 Focus Blue 持久彩色

#### Scenario: 列表分组

- **WHEN** 用户切到 "键盘快捷键" tab
- **THEN** SHALL 按 `global` / `tabs` / `sidebar` / `search` / `session` 顺序渲染 5 个 category 段
- **AND** 每段顶部 SHALL 显示中文 category 名（如 "全局" / "标签页" / "侧栏" / "搜索" / "会话"）
- **AND** 每段下 SHALL 列出该 category 注册的所有 ShortcutSpec 行

### Requirement: Keyboard Shortcut 录键交互

录键控件 SHALL 提供 idle / recording / conflict 三态切换：

- **idle**：neutral surface + 1px border + mono 当前 binding；hover 显示 tooltip "点击修改"
- **recording**：accent 1px border + secondary spinner + placeholder "按下新的快捷键..."；进入 recording 时 SHALL 把全局快捷键 dispatcher suspend，使录键期间已注册的快捷键 SHALL NOT 被触发
- **conflict**：warning 1px border + warning bg + mono 新 binding + 文案 "与 `<other-shortcut-description>` 冲突"；保存按钮 SHALL disabled

录键状态机：

- 进入 recording 后焦点 SHALL trap 在录入器内
- recording 期间 keydown 事件 SHALL `event.preventDefault()` 阻止字符落入 input
- 当一次 keydown 含完整修饰键 + 主键时 SHALL commit binding 并查冲突；冲突非 null → 进 conflict 态、保存 disabled；无冲突 → 切回 idle 显示新 binding，外层 panel 启用保存按钮
- 录键期间用户按 Escape SHALL 取消录入、恢复 idle 显示原 binding，并恢复全局 dispatcher

#### Scenario: 进入 recording 态

- **WHEN** 用户点击 idle 状态的录键控件
- **THEN** 控件 SHALL 切到 recording 态（accent border + spinner）
- **AND** 焦点 SHALL trap 在该控件内
- **AND** 全局快捷键 dispatcher SHALL 被 suspend

#### Scenario: 录入新 binding

- **WHEN** 用户在 recording 态按下 `mod+shift+P`（无冲突）
- **THEN** 控件 SHALL 切回 idle 态，显示 mono `⇧⌘P`（mac）或 `Ctrl+Shift+P`（其他）
- **AND** 全局快捷键 dispatcher SHALL 恢复
- **AND** 外层 ShortcutRow SHALL 反映新值，"重置默认"按钮 SHALL 启用

#### Scenario: 录入冲突 binding

- **WHEN** 用户在 recording 态按下 `mod+B`（已被 `sidebar.toggle` 占用）
- **THEN** 控件 SHALL 切到 conflict 态
- **AND** 文案 SHALL 显示 "与 切换 Sidebar 折叠 (⌘B) 冲突"
- **AND** 保存按钮 SHALL disabled

#### Scenario: Escape 取消录入

- **WHEN** 用户在 recording 态按下 Escape
- **THEN** 控件 SHALL 切回 idle 态、显示原 binding
- **AND** 全局快捷键 dispatcher SHALL 恢复

#### Scenario: 录键期间不触发已注册快捷键

- **WHEN** 用户在 recording 态按下 `mod+B`
- **AND** `mod+B` 已被 `sidebar.toggle` 占用
- **THEN** Sidebar SHALL NOT 切换折叠状态（dispatcher 处于 suspend）
- **AND** 控件 SHALL 进入 conflict 态展示冲突

### Requirement: Keyboard Shortcut 持久化与恢复

修改 SHALL 通过 `Save` 按钮显式提交（不自动保存，不在录键 commit 时 debounce 自动写）；点 Save 时 SHALL 单次 IPC 写入完整 `keyboardShortcuts` HashMap（包含本次 panel 内全部 pendingOverrides），并在 IPC resolved 后一次性把 registry 内存 keymap 切到新值。IPC 失败 SHALL 回滚 pendingOverrides 与 registry，UI 显示 inline 错误。

每行 SHALL 提供"重置默认"按钮（仅当 currentBinding ≠ defaultBinding 时启用），点击 SHALL 把该 ID 的 override 从 `keyboardShortcuts` 配置中移除。Panel 顶部 SHALL 提供"重置全部"按钮，点击 SHALL 弹确认对话框（"将所有快捷键恢复为默认值，已自定义的将丢失"），确认后清空整个 `keyboardShortcuts` HashMap。

#### Scenario: 单条重置默认

- **WHEN** 用户在某行点击"重置默认"按钮
- **THEN** 该 ID 的 override SHALL 从 `keyboardShortcuts` 配置中移除
- **AND** registry 内存 keymap 该 ID SHALL 恢复为 builtin default
- **AND** UI 该行 SHALL 显示 default binding，"重置默认"按钮 SHALL disabled

#### Scenario: 重置全部

- **WHEN** 用户点击"重置全部"按钮并确认
- **THEN** `keyboardShortcuts` 配置 SHALL 被清空
- **AND** registry SHALL 重新走纯 builtin defaults
- **AND** 所有行 SHALL 显示 default binding

#### Scenario: 关闭 Settings 不丢未保存改动

- **WHEN** 用户在录键控件录入新 binding 但未点 Save 就切到其他 Section
- **THEN** Panel 顶部 SHALL 显示未保存提示条 + "保存" / "丢弃" 按钮
- **AND** 用户点击"丢弃"或离开 Settings SHALL 回滚所有 pending 修改

#### Scenario: 保存失败回滚

- **WHEN** 用户点击 Save，IPC 失败
- **THEN** registry 内存 keymap SHALL 回滚到改动前状态
- **AND** UI SHALL 显示 inline 错误："保存失败：<reason>"

### Requirement: CLI Section 状态展示

CLI section SHALL 展示当前 CLI 安装状态。状态来自启动时异步检测的内存缓存结果，打开 Settings 时 SHALL 立即可见（不显示加载态）。

#### Scenario: CLI 未安装

- **WHEN** CLI section 渲染
- **AND** CLI 状态为 `not_installed`
- **THEN** SHALL 显示"安装 CLI vX.Y.Z"按钮
- **AND** SHALL 显示安装目标路径 `~/.local/bin/cdt`
- **AND** 按钮使用 SettingsButton 组件

#### Scenario: CLI 已安装且版本与桌面端一致

- **WHEN** CLI section 渲染
- **AND** CLI 状态为 `installed_current`
- **THEN** SHALL 显示绿色 ✓ + 版本号 + 路径
- **AND** 版本号和路径使用 mono 字体

#### Scenario: CLI 已安装但版本落后

- **WHEN** CLI section 渲染
- **AND** CLI 版本 < 桌面端版本
- **AND** CLI 路径为 `~/.local/bin/cdt`
- **THEN** SHALL 显示当前版本 + "更新到 vX.Y.Z" 按钮

#### Scenario: CLI 安装在受管路径但不在 PATH 中

- **WHEN** CLI section 渲染
- **AND** CLI 状态为 `installed_not_in_path`
- **THEN** SHALL 显示版本 + 路径
- **AND** SHALL 显示 PATH 添加指令（`export PATH="$HOME/.local/bin:$PATH"`）
- **AND** PATH 指令使用 mono 字体 + 可复制

#### Scenario: CLI 由外部管理

- **WHEN** CLI section 渲染
- **AND** CLI 路径不是 `~/.local/bin/cdt`
- **THEN** SHALL 显示版本 + 路径 + "由外部管理" 提示
- **AND** SHALL NOT 显示安装或更新按钮

### Requirement: CLI Section 安装/更新交互

CLI section 的安装/更新按钮 SHALL 提供明确的进行中、成功、失败三态反馈。

#### Scenario: 安装进行中

- **WHEN** 用户点击安装/更新按钮
- **THEN** 按钮 SHALL 变为 disabled 状态
- **AND** SHALL 显示 inline spinner（10×10，Focus Blue）+ "安装中..." 文案
- **AND** SHALL NOT 阻塞 Settings 页面其他交互

#### Scenario: 安装成功

- **WHEN** 安装流程完成且验证通过
- **THEN** SHALL 立即刷新 CLI 状态展示为 `installed_current`
- **AND** 如果 `~/.local/bin` 不在 PATH 中，SHALL 切换到 `installed_not_in_path` 状态展示 PATH 指令

#### Scenario: 安装失败

- **WHEN** 安装流程中任一步骤失败
- **THEN** SHALL 使用 warning 色（Amber）显示错误信息
- **AND** SHALL 显示"重试"按钮
- **AND** SHALL NOT 使用 error red（下载失败是 actionable 状态，非系统错误）

