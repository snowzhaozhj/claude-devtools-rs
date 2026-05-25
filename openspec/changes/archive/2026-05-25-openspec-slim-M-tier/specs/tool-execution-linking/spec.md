## MODIFIED Requirements

### Requirement: Pair tool_use with tool_result by id

系统 SHALL 把每个 `tool_use` 块与同 `tool_use_id` 的 `tool_result` 块配对，无视两者间隔多少条消息。配对算法 SHALL 为对输入消息单次遍历的纯同步函数；无匹配的 `tool_use` SHALL 作为 orphan 保留，标记 `output = Missing` 且 `end_ts = None`，不抛错。

输入侧若出现重复 `tool_use_id`（典型流式 rewrite / retry 写入两份 tool_use；或两条 tool_result 共享同 id）SHALL 保留首条遇到者完成配对，后续重复者计入 `ToolLinkingResult.duplicates_dropped` 计数并以 warn 级日志上报。

#### Scenario: Immediate result

- **WHEN** 一个 `tool_use` 紧接的下一条 user 消息含同 id 的 `tool_result`
- **THEN** 这对 SHALL 被链接，并产出含起止时间戳的 `ToolExecution` 记录

#### Scenario: Delayed result

- **WHEN** `tool_use` 后还间隔了若干消息才出现其 `tool_result`
- **THEN** 一旦匹配到对应 result，配对 SHALL 仍然成立

#### Scenario: Duplicate tool_use or result ids tracked via duplicates_dropped

- **WHEN** assistant 侧两个 `tool_use` 共享同一 `tool_use_id`，或两个 `tool_result` 共享同一 `tool_use_id`
- **THEN** 系统 SHALL 保留首条遇到者完成配对，后续重复者 SHALL NOT 触发 panic
- **AND** `ToolLinkingResult.duplicates_dropped` 计数 SHALL 增加，日志 SHALL 以 warn 级上报命中的 `tool_use_id`

#### Scenario: Orphan tool_use has no matching result

- **WHEN** 一条 assistant `tool_use` 在整个 session 中无任何 `tool_result` 与之匹配
- **THEN** 系统 SHALL 产出一条 `ToolExecution` 记录，`output = Missing`、`end_ts = None`、`is_error = false`，SHALL NOT panic

### Requirement: 工具块右键菜单

`Bash / Read / Edit / Write` 等专化工具查看器 SHALL 通过 `frontend-context-menu` capability 的右键菜单契约挂载工具块右键菜单。Bash 工具走 Bash factory（含命令 / 输出 / stderr / cwd 维度），Read / Edit / Write 共享文件工具 factory（含路径 / diff / 行号跳转 / Finder/Explorer 维度）；items 内容遵循 `frontend-context-menu` capability 的视觉契约（无 icon、separator 分组、shortcut hint），右键 IPC dispatch 失败时 SHALL 通过应用 toast 系统呈现 `ApiError.message`，**不**得静默 swallow / 弹原生 alert / 仅 console 输出。

具体 dispatch 行为：

- 复制 / 在终端打开 cwd / 在浏览器搜索错误 / 在编辑器打开（含跳行号）/ 在 Finder/Explorer 中显示 / 在终端打开父目录等 action SHALL 走对应 IPC（`open_in_terminal` / `open_in_editor` / `plugin:opener|*` / clipboard）
- IPC 失败时返回的 `ApiError`（`code` 含 `ExternalApp` / `ValidationError` / `NotFound` 等）SHALL 触发 toast 显示 message，必要时引导用户去 Settings 修改对应配置

#### Scenario: Bash 工具块右键弹出菜单

- **WHEN** 用户在 Bash 工具查看器根元素上右键
- **THEN** SHALL 弹出含以下行为类别的菜单 items：复制命令 / 复制输出 / 复制 stderr（仅当含 error 时） / 在终端打开 cwd / 在浏览器搜索错误信息（仅当 stderr 非空时）

#### Scenario: 文件工具块右键弹出菜单

- **WHEN** 用户在 Read / Edit / Write 工具查看器根元素上右键
- **THEN** SHALL 弹出菜单含 复制路径 / 在编辑器打开（含跳行号） / 在 Finder/Explorer 中显示 / 在终端打开父目录
- **AND** Edit / Write 工具菜单 SHALL 额外含 复制 diff
- **AND** 路径类 label SHALL 走中段截断展示（短形式 + tooltip 完整路径）

#### Scenario: 在编辑器打开（含跳行号）

- **WHEN** 用户点击 "在编辑器打开" 且 ToolExecution input 含 `file_path` 与行号字段（`offset` / `start_line` 等）
- **THEN** SHALL 调用 `open_in_editor` IPC 携带 path / line / column
- **AND** 后端按 Settings 外部编辑器配置分流（VS Code / Cursor / Zed / Sublime / system fallback）

#### Scenario: 工具块菜单 dispatch 失败弹 toast

- **WHEN** 任意右键 IPC dispatch（编辑器 / 终端 / 浏览器搜索 / 复制）失败
- **THEN** 前端 SHALL 弹 toast 显示 `ApiError.message`，**不**默默 swallow，**不**弹原生 alert，**不**仅 console 输出

#### Scenario: 复制操作失败弹 toast

- **WHEN** 浏览器 clipboard 写入因权限或浏览器策略失败抛出
- **THEN** 前端 SHALL 捕获并弹 toast 显示原因
- **AND** 菜单 SHALL 关闭，不卡在 "已复制!" 反馈态
