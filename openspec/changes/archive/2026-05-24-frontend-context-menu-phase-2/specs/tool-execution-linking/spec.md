## ADDED Requirements

### Requirement: 工具块右键菜单

`BashToolViewer.svelte` / `ReadToolViewer.svelte` / `EditToolViewer.svelte` / `WriteToolViewer.svelte` 等专化查看器 SHALL 通过 `use:contextMenu` action 挂载右键菜单，把工具调用的关键字段（command / output / stderr / file path / diff）暴露为可一键操作的 items。Bash 工具走 `buildBashToolItems` factory；Read/Edit/Write 三种文件工具共享 `buildFileToolItems` factory；items 内容遵循 `frontend-context-menu` capability 的视觉契约（无 icon、separator 分组、shortcut hint）。

#### Scenario: Bash 工具块右键

- **WHEN** 用户在 `BashToolViewer` 根元素上右键
- **THEN** SHALL 弹出含以下 items 的菜单：复制命令 / 复制输出 / 复制 stderr（仅当 ToolExecution 含 error 时） / 在终端打开 cwd / 在浏览器搜索错误信息（仅当 stderr 非空时）
- **AND** 点击"复制命令" SHALL 把 `ToolExecution.input.command` 写入 clipboard

#### Scenario: Bash 工具块在终端打开 cwd

- **WHEN** 用户点击 "在终端打开 cwd"
- **AND** ToolExecution 含 cwd 字段且 cwd 是已存在目录
- **THEN** SHALL 调用 `open_in_terminal(cwd)` IPC（后端按 Settings `terminalApp` 分流）
- **AND** 终端 app SHALL 仅 cd 到 cwd 不执行命令（遵守安全不变量）
- **AND** 失败时 SHALL 弹 toast 显示 `ApiError::ExternalApp` 的 message

#### Scenario: Bash 工具块在浏览器搜索错误

- **WHEN** ToolExecution 含 stderr 且用户点击 "在浏览器搜索错误信息"
- **THEN** SHALL 截取 stderr 首行（≤ 200 字符）作为搜索 query
- **AND** 按 Settings `searchEngine` 拼接 URL 并调 `plugin:opener|open_url`

### Requirement: 文件类工具块右键菜单

`ReadToolViewer` / `EditToolViewer` / `WriteToolViewer` 共享 `buildFileToolItems(exec, ctx)` factory 构造的 items：复制路径 / 复制 diff（仅 Edit/Write）/ 在编辑器打开（含跳行号）/ 在 Finder/Explorer 中显示 / 在终端打开父目录。`pathLabel` 字段 SHALL 由 factory 预处理为 `{ short, full }`，渲染层用 `short` 做 label + `full` 做 `title` tooltip；中段截断保留首段（home 前缀 `~/`）+ 尾段（文件名 + 后缀，最多 30 字符）+ 中间 `…`。

#### Scenario: 文件工具块右键弹出菜单

- **WHEN** 用户在 `ReadToolViewer` / `EditToolViewer` / `WriteToolViewer` 根元素上右键
- **THEN** SHALL 弹出菜单含 "复制路径"、"在编辑器打开"、"在 Finder 中显示"、"在终端打开父目录"
- **AND** Edit/Write 工具菜单 SHALL 额外含 "复制 diff"
- **AND** 路径类 label 显示形如 `~/Rustrove…/contextMenu.svelte.ts` 中段截断形态

#### Scenario: 在编辑器打开（含跳行号）

- **WHEN** 用户点击 "在编辑器打开"
- **AND** ToolExecution 含 `input.file_path` 且 input 含 `offset` 或 `start_line` 等行号字段
- **THEN** SHALL 调用 `open_in_editor(path, line, column)` IPC
- **AND** 后端按 Settings `externalEditor` 调对应 CLI（VS Code: `code --goto path:line:col`、Cursor: `cursor --goto path:line:col`、Zed/Sublime: `<cli> path:line:col`）
- **AND** Settings `external_editor` 为 `system` 时 fallback 到 OS 默认（macOS `open` / Win `start` / Linux `xdg-open`），行号参数忽略

#### Scenario: 在 Finder/Explorer 中显示

- **WHEN** 用户点击 "在 Finder/Explorer 中显示"
- **THEN** SHALL 调 `plugin:opener|reveal_item_in_dir`（或等效现有 IPC）打开 OS 文件管理器并高亮目标文件
- **AND** macOS 走 `open -R <path>` 等价行为；Windows 走 `explorer /select,<path>`；Linux 走 `xdg-open <parent>` fallback（无原生 reveal-and-select 时退化为打开父目录）

#### Scenario: 路径中段截断 + tooltip 完整路径

- **WHEN** factory 处理超过 50 字符的文件路径
- **THEN** SHALL 生成 `pathLabel: { short, full }`，`short` 总长 ≤ 50 字符，`full` 为完整路径
- **AND** `AppContextMenu` 渲染时 SHALL 用 `pathLabel.short` 作为 label 显示文本
- **AND** 给该 item 加 `title={pathLabel.full}` 让 hover 浮 tooltip 显示完整路径

### Requirement: 工具块菜单 dispatch 失败的可见反馈

工具块菜单的所有 IPC dispatch action（`open_in_terminal` / `open_in_editor` / 复制类 / `open_url`）SHALL 在失败时通过应用 toast 系统呈现 `ApiError.message` 的人类可读内容；前端 SHALL **不**默默 swallow 错误，**不**弹原生 alert，**不**仅在 console 输出。

#### Scenario: open_in_editor 调用失败弹 toast

- **WHEN** 用户点击 "在编辑器打开" 但 Settings 配置的 editor CLI 未安装
- **THEN** 后端返回 `ApiError { code: ExternalApp, message: "editor CLI 'code' not found; install VS Code shell command or change Settings" }`
- **AND** 前端 SHALL 弹 toast 显示该 message，引导用户去 Settings 修改

#### Scenario: open_in_terminal 调用失败弹 toast

- **WHEN** 用户点击 "在终端打开" 但 (a) terminal app 未装（如 macOS 选了 Warp 但 Warp.app 不存在）/ (b) Windows path 含 cmd metacharacter 被 ValidationError 拒绝 / (c) Linux 终端 spawn 失败（headless server 无 DE）
- **THEN** 后端返回对应 `ApiError`：(a) `code: ExternalApp, message: "terminal '<App>' is not installed or not found"`、(b) `code: ValidationError, message: "path contains characters unsafe for Windows shell: ..."`、(c) `code: ExternalApp, message: "failed to launch terminal: <reason>"`
- **AND** 前端 SHALL 弹 toast 显示该 message
- **AND** Settings 引导文案"Settings → External Apps 修改 terminal_app"（按 ApiError code 决定是否引导）

#### Scenario: open_url（在浏览器搜索）调用失败弹 toast

- **WHEN** 用户点击 "在浏览器搜索" 但 `plugin:opener|open_url` 失败
- **THEN** 前端 SHALL 弹 toast 显示 "Failed to open browser: <reason>"

#### Scenario: 复制操作失败弹 toast

- **WHEN** `navigator.clipboard.writeText` 因权限或浏览器策略失败抛出
- **THEN** 前端 SHALL 捕获并弹 toast "复制失败：<原因>"
- **AND** 菜单 SHALL 关闭（不卡在 "已复制!" 反馈态）
