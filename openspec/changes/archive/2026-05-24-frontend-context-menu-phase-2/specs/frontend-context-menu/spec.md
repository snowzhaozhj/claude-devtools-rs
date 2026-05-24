## ADDED Requirements

### Requirement: 文本选区菜单（window-level handler）

应用 SHALL 注册 window-level `contextmenu` 监听器作为 surface-level `use:contextMenu` 与全局兜底之间的中间层（Layer 2）：当 surface 未拦截事件且 `window.getSelection()?.toString().length > 0` 时，SHALL `e.preventDefault()` 阻止系统菜单并弹出选区专属菜单（含"复制选中文本"、"复制为引用 Markdown"、"在浏览器搜索"等 items）。Layer 2 SHALL 在 `installGlobalContextMenuFallback()` 之前注册以保证执行顺序；handler 内 SHALL 跳过 `<input>` / `<textarea>` / `[contenteditable="true"]` / `[data-allow-native-context]` 元素让浏览器原生菜单接管。

#### Scenario: 选中文本后右键空白区弹选区菜单

- **WHEN** 用户先 drag-select 一段文本，再在选区附近未挂 `use:contextMenu` 的位置右键
- **THEN** Layer 2 handler SHALL 检测 `selection.toString().length > 0`
- **AND** `e.preventDefault()` 阻止系统菜单
- **AND** 弹出 `AppContextMenu`，items 由 `buildSelectionItems(selectionText, ctx)` 构造
- **AND** 全局兜底 Layer 3 SHALL 检测 `e.defaultPrevented === true` 并 skip 自身处理

#### Scenario: 选中文本后右键已挂 use:contextMenu 的元素

- **WHEN** 用户先选中一段文本，再在挂了 `use:contextMenu` 的 surface 元素（如 `.user-bubble`）上右键
- **THEN** Layer 1 surface action SHALL 优先触发并 `stopPropagation`
- **AND** Layer 2 SHALL **不**触发（事件不冒泡到 window）
- **AND** Surface factory（如 `buildUserMessageItems`）SHALL 检测有选区并在首段首项前动态插入"复制选中文本"item
- **AND** 用户感知：弹 surface 菜单 + 含"复制选中文本"项

#### Scenario: 无选区时 Layer 2 跳过

- **WHEN** 用户在未挂 `use:contextMenu` 的位置右键且 `selection.toString().length === 0`
- **THEN** Layer 2 SHALL 跳过（不弹选区菜单不 preventDefault）
- **AND** Layer 3 全局兜底 SHALL 接管 `e.preventDefault()`，不弹任何菜单

#### Scenario: 选中文本后右键 input/textarea 走原生菜单

- **WHEN** 用户在 `<input>` / `<textarea>` / `[contenteditable]` 元素内选中文本后右键
- **THEN** Layer 2 SHALL 通过 `target.closest('input, textarea, [contenteditable], [data-allow-native-context]')` 检测并跳过
- **AND** 浏览器原生菜单 SHALL 正常弹出（粘贴 / 拼写检查 / 朗读等）

#### Scenario: HMR 重复注册幂等

- **WHEN** Vite HMR 触发模块重载，`installSelectionContextMenu()` 再次被调用
- **THEN** SHALL 通过 `__cdtSelectionMenuInstalled` window sentinel flag 检测已注册并跳过
- **AND** window 上仅存在一个 selection contextmenu listener

### Requirement: menu-items 函数库

应用 SHALL 在 `ui/src/lib/contextMenu/menu-items.ts` 提供按 surface 拆分的 factory 函数，每个返回 `ContextMenuItem[]`：`buildUserMessageItems` / `buildAssistantMessageItems` / `buildBashToolItems` / `buildFileToolItems` / `buildWorktreeChipItems` / `buildProjectCardItems` / `buildSelectionItems`。所有 factory 接受统一上下文 `MenuItemContext`（含 `sessionId` / `projectId` / `settings` / `dispatch` 五个 IPC 调用闭包 + `selectionText: string` 当前选区文本快照），让 item.action 自包含——factory 内**不**直接 import `api.ts`、**不**直接读 DOM（含 `window.getSelection()` / `document.activeElement`），所有 IPC 走 `ctx.dispatch.{copyToClipboard,openInEditor,openInTerminal,revealInDir,openUrl}` 间接调用以便 vitest mock，所有运行时浏览器状态 SHALL 通过 `ctx` 字段传入。Factory SHALL 是纯函数：给定输入 → 确定输出，不持有外部状态、不读 DOM。**调用方** SHALL 在 `oncontextmenu` 触发瞬间预先读 `window.getSelection()?.toString() ?? ""` 后通过 `ctx.selectionText` 传入 factory，统一 selection 快照源避免 factory 内部读 DOM 的 jsdom / SSR / 测试稳定性问题。

#### Scenario: factory 返回纯数据

- **WHEN** vitest 测试调用 `buildUserMessageItems(chunk, mockCtx)`，`mockCtx` 含 `selectionText: ""` + mock dispatch
- **THEN** 返回值 SHALL 是 `ContextMenuItem[]`
- **AND** items 内的 `action` 闭包仅引用 `ctx.dispatch.*` 与 chunk 字段，不调真 IPC
- **AND** mock dispatch 后调用 action SHALL 只触发 mock 函数，不发真 IPC
- **AND** 单测 SHALL **不**需要 jsdom 的 `window.getSelection` polyfill

#### Scenario: separator 自动插入按 kind 分组

- **WHEN** factory 返回的 items 含相邻 `kind` 不同的 item（如 `kind: "copy"` 后跟 `kind: "navigate"`）
- **THEN** factory 内部 SHALL 在 kind 切换处插入 `{ separator: true }`
- **AND** factory SHALL trim 首尾孤立 separator（首项为 separator 或末项为 separator 时去除）

#### Scenario: 有选区时融合"复制选中文本"

- **WHEN** 调用方在 oncontextmenu 触发瞬间读 `window.getSelection()?.toString() ?? ""` 取得 `selectionText` 长度 > 0 后调 `buildUserMessageItems(chunk, { ...ctx, selectionText })`
- **THEN** factory SHALL 在首段（kind=copy）首项前动态插入"复制选中文本"item（`shortcut: "⌘C"`）
- **AND** 该 item 的 action 调 `ctx.dispatch.copyToClipboard(ctx.selectionText)`

#### Scenario: 无选区时不插入选区项

- **WHEN** 调用方传入 `ctx.selectionText === ""`
- **THEN** factory SHALL **不**插入"复制选中文本"item
- **AND** 返回 items 与 `selectionText` 为空字符串时调用结果一致（确定性纯函数）

### Requirement: ContextMenuItem 类型扩展

`ui/src/lib/contextMenu/types.ts` 内的 `ContextMenuItem` 类型 SHALL 扩展四个 optional 字段：`shortcut?: string`（右侧灰色快捷键 hint，仅 display 不绑定真实快捷键）/ `submenu?: ContextMenuItem[]`（二级菜单数组，有值时 `action` 与 `shortcut` 忽略）/ `kind?: "copy" | "navigate" | "external"`（语义分类，factory 内部 separator 插入用，AppContextMenu 不消费）/ `pathLabel?: { short: string; full: string }`（路径中段截断形态，渲染层用 `short` 做 label + `full` 做 `title` tooltip）。所有新字段 SHALL 是 optional，Phase 1 已落地的 Sidebar / Tab 右键菜单 SHALL 无需改动即兼容。

#### Scenario: shortcut 字段渲染

- **WHEN** `ContextMenuItem` 含 `shortcut: "⌘C"`
- **THEN** `AppContextMenu` 渲染时 SHALL 在 item 行内右对齐显示 `⌘C` 文本
- **AND** 文字颜色 SHALL 为 `--color-text-muted`，字体为 `var(--font-mono)` `11px` `400`
- **AND** 无 shortcut 字段时该位置留空（不渲染空 placeholder）

#### Scenario: submenu 字段渲染 chevron

- **WHEN** `ContextMenuItem` 含非空 `submenu` 数组
- **THEN** `AppContextMenu` SHALL 在 item 行内右对齐渲染 `›` chevron 指示器
- **AND** 该 item 的 `action` 与 `shortcut` 字段 SHALL 被忽略（不渲染 shortcut hint，不调 action）
- **AND** chevron 与 shortcut hint 互斥（同 item 不会同时渲染）

#### Scenario: pathLabel 字段渲染中段截断

- **WHEN** `ContextMenuItem` 含 `pathLabel: { short: "在编辑器打开 ~/Rustro…/menu-items.ts", full: "在编辑器打开 /Users/zhao/Rustrover/Project/.../menu-items.ts" }`
- **THEN** `AppContextMenu` SHALL 用 `pathLabel.short` 作为 label 显示文本（覆盖 `label` 字段）
- **AND** SHALL 加 `title={pathLabel.full}` 让 hover 浮 tooltip 显示完整路径

#### Scenario: kind 字段不渲染

- **WHEN** `ContextMenuItem` 含 `kind: "copy"`
- **THEN** `AppContextMenu` SHALL **不**消费 `kind` 字段（无视觉变化）
- **AND** factory 内部按 kind 决定 separator 插入位置

### Requirement: AppContextMenu submenu 渲染

`AppContextMenu` SHALL 扩展支持 submenu 渲染：检测 item.submenu 非空时挂 `›` chevron + 进入 hover 状态后 200ms 弹出二级菜单（同样通过 mount 到 `document.body`）；ArrowRight SHALL 即时打开 submenu + focus 进 submenu 首项；ArrowLeft SHALL 关闭 submenu + focus 还回 parent；Esc SHALL 关闭整棵菜单树；submenu 视觉规格与父菜单完全相同（同 bg / border / radius / shadow），不做层级递进。submenu 渲染深度 SHALL 限制为 ≤ 2（Phase 2 仅用一层 submenu）。

#### Scenario: hover 200ms 打开 submenu

- **WHEN** 用户鼠标 hover 含 submenu 的 item 持续 200ms
- **THEN** SHALL 在 parent item 右侧弹出 submenu 浮层
- **AND** parent item SHALL 保持 `.cm-item-active` bg 锁定直到 submenu 关闭
- **AND** viewport 右边距不足时 submenu SHALL 翻转到左侧展开

#### Scenario: ArrowRight 即时打开 + focus 进 submenu

- **WHEN** 用户键盘导航至含 submenu 的 active item，按 ArrowRight
- **THEN** submenu SHALL 立即弹出（无 200ms 延迟）
- **AND** focus SHALL 进入 submenu 首项

#### Scenario: ArrowLeft 关闭 submenu + focus 回 parent

- **WHEN** submenu 已打开且 focus 在 submenu 内某项，用户按 ArrowLeft
- **THEN** submenu SHALL 关闭
- **AND** focus SHALL 还回 parent item

#### Scenario: Esc 关闭整棵菜单树

- **WHEN** submenu 已打开，用户按 Esc
- **THEN** submenu 与 parent 菜单 SHALL 同时关闭
- **AND** focus SHALL 还回 trigger 元素

#### Scenario: submenu 视觉与父菜单完全一致

- **WHEN** submenu 渲染
- **THEN** SHALL 复用父菜单的 `--color-surface` bg + `1px solid --color-border-emphasis` + `8px` radius + `4px` padding + `0 4px 16px rgba(0, 0, 0, 0.15)` shadow
- **AND** SHALL **不**加深 bg 或追加额外 shadow（遵守 `DESIGN.md::§1 Overview` flat + tonal layering 原则）

#### Scenario: 渲染深度上限 2

- **WHEN** 调用方传入 nested submenu 三层以上
- **THEN** AppContextMenu SHALL 在 depth 2 后忽略后续 submenu 字段
- **AND** depth=2 的 item 即使含 submenu 也按 leaf item 渲染（不显示 chevron 不弹三级菜单）

### Requirement: AppContextMenu 视觉规格扩展

`AppContextMenu` Phase 2 视觉规格 SHALL 在 Phase 1 基础上扩展：(a) `min-width: 200px` / `max-width: 320px`，超长 label 用 `text-overflow: ellipsis` 截断；(b) shortcut hint 行内右对齐 + `--color-text-muted` + `var(--font-mono)` `11px`；(c) submenu chevron 行内右对齐与 shortcut hint 互斥渲染；(d) keyboard active state 维持 Phase 1 outline `2px solid rgba(59, 130, 246, 0.15)` + `--tool-item-hover-bg`，不做对比度调整。Phase 2 SHALL **不**引入 icon 渲染（Phase 1 D3 决策延续）也 **不**引入 danger item 视觉首落地。

#### Scenario: max-width 320px 截断长 label

- **WHEN** `ContextMenuItem.label` 长度超过 320px 渲染宽度
- **THEN** label SHALL 通过 CSS `white-space: nowrap` + `overflow: hidden` + `text-overflow: ellipsis` 末段截断
- **AND** 路径类 item 用 `pathLabel` 字段提前 JS 中段截断（CSS 末段截断作 fallback）

#### Scenario: 暗色模式视觉规格不变

- **WHEN** 应用切到暗色主题
- **THEN** AppContextMenu SHALL 用 `--dark-surface: #1e1e1c` + `1px solid --dark-border-emphasis: #4f4e4a` + 同级 shadow
- **AND** submenu 视觉与父菜单完全相同（不加深 bg）

### Requirement: open_in_terminal IPC 契约

应用 SHALL 暴露 `open_in_terminal` Tauri command（HTTP 镜像同名）：入参 `{ path: String }`（绝对路径）；返回 `Result<(), ApiError>`。command handler 在后端 SHALL 校验 `path` 是绝对路径 + canonicalize 解析（拒绝相对路径与不存在路径），从 `ConfigManager` 读取 `terminalApp` 设置后按平台 dispatch 子进程：macOS 走 `Command::new("open").args(["-a", "Terminal"]).arg(path)` OS-level argv（零注入）；Windows `wt.exe` 走 OS-argv `Command::new("wt.exe").args(["-d"]).arg(path)`；Windows PowerShell / cmd fallback SHALL 把 `path` 通过环境变量 `CDT_TARGET_PATH` 传入，命令字符串内仅引用 `$env:CDT_TARGET_PATH` / `%CDT_TARGET_PATH%`，**严禁**把 path 拼进命令字符串以避免 shell parser 解释 metacharacters；Linux 走 `<term-app> --working-directory <path>` 等价命令的 OS-argv 形态。**安全不变量**：command 入参 SHALL **不**接受任意 shell command 字符串，仅接受 path；macOS / Linux / Windows Terminal 一律 OS-argv 传参；Windows cmd fallback 在 path 含 `&` / `|` / `<` / `>` / `^` / `(` / `)` / `%` / `!` / `'` / `"` / 换行等 cmd metacharacters 时 SHALL 直接拒绝返回 `ApiError::ValidationError`（cmd parser 在 env var 展开后仍 re-tokenize，无法 100% 安全）。

#### Scenario: macOS 调用 Terminal

- **WHEN** macOS 上 Settings `terminalApp = "terminal"`，前端调 `invoke('open_in_terminal', { path: '/Users/foo/project' })`
- **THEN** 后端 SHALL 调 `std::process::Command::new("open").arg("-a").arg("Terminal").arg("/Users/foo/project").spawn()`
- **AND** Terminal.app SHALL 弹窗口 cd 到目标目录
- **AND** 返回 `Ok(())`

#### Scenario: Windows 三级 fallback

- **WHEN** Windows 上 Settings `terminalApp = "windows_terminal"` 且 `wt.exe` 在 PATH
- **THEN** 后端优先尝试 `Command::new("wt.exe").arg("-d").arg(path).spawn()`
- **AND** spawn 失败（wt.exe 未装）SHALL fallback 到 `Command::new("powershell.exe").args(["-NoExit", "-Command", "Set-Location -LiteralPath ..."]).spawn()`
- **AND** PowerShell 也失败 SHALL fallback 到 `Command::new("cmd.exe").args(["/K", "cd /d", path]).spawn()`
- **AND** 三级全失败返回 `ApiError::ExternalApp("failed to launch terminal: <reason>")`

#### Scenario: 相对路径拒绝

- **WHEN** 前端调 `invoke('open_in_terminal', { path: 'relative/path' })`
- **THEN** 后端 SHALL 通过 `cdt_discover::looks_like_absolute_path` 校验失败
- **AND** 返回 `ApiError::ValidationError("path must be absolute")`
- **AND** **不** spawn 任何子进程

#### Scenario: 不存在路径返回 NotFound

- **WHEN** 前端调 `invoke('open_in_terminal', { path: '/nonexistent/foo' })`
- **THEN** 后端 `tokio::fs::canonicalize` SHALL 失败
- **AND** 返回 `ApiError::NotFound("path does not exist: /nonexistent/foo")`

#### Scenario: 文件路径自动取父目录

- **WHEN** 前端调 `invoke('open_in_terminal', { path: '/Users/foo/file.txt' })` 且 path 是文件
- **THEN** 后端 `metadata.is_dir()` 失败后 SHALL 取 `canonicalized.parent()` 降级到目录
- **AND** 终端 app 打开 `/Users/foo`

#### Scenario: 跨平台 terminalApp 不匹配 fallback

- **WHEN** macOS 上 Settings `terminalApp = "windows_terminal"`（用户跨平台同步配置过来）
- **THEN** 后端 SHALL `tracing::warn!` 记录 mismatch
- **AND** fallback 到 macOS 平台默认 `TerminalApp::Terminal` 继续 spawn
- **AND** **不**返回错误

### Requirement: open_in_editor IPC 契约

应用 SHALL 暴露 `open_in_editor` Tauri command（HTTP 镜像同名）：入参 `{ path: String, line: Option<u32>, column: Option<u32> }`；返回 `Result<(), ApiError>`。command handler SHALL 校验 path（同 `open_in_terminal`），从 `ConfigManager` 读取 `externalEditor` 后按白名单 dispatch CLI：`vs_code` → `code --goto path:line:col`、`cursor` → `cursor --goto path:line:col`、`zed` → `zed path:line:col`、`sublime` → `subl path:line:col`、`system` → 走 OS 默认（macOS `open` / Win `start` / Linux `xdg-open`，行号参数忽略）。`line` 为 `None` 时 SHALL 省略行号后缀；CLI 不存在 SHALL 返回 `ApiError::ExternalApp` 引导用户去 Settings 修改。

#### Scenario: VS Code 跳行号

- **WHEN** Settings `externalEditor = "vs_code"`，前端调 `invoke('open_in_editor', { path: '/foo/bar.rs', line: 42, column: 8 })`
- **THEN** 后端 SHALL 调 `Command::new("code").arg("--goto").arg("/foo/bar.rs:42:8").spawn()`
- **AND** VS Code SHALL 打开文件并跳到行 42 列 8

#### Scenario: 行号缺失时省略后缀

- **WHEN** 前端调 `invoke('open_in_editor', { path: '/foo/bar.rs', line: null, column: null })`
- **THEN** 后端 SHALL 调 `Command::new("code").arg("/foo/bar.rs").spawn()`（无 `--goto`）
- **AND** 不附加 `:line:col` 后缀

#### Scenario: System fallback OS 默认

- **WHEN** Settings `externalEditor = "system"`，前端调 `invoke('open_in_editor', { path: '/foo/bar.rs', line: 42, column: 8 })`
- **THEN** 后端 SHALL 走 macOS `open /foo/bar.rs` / Win `cmd /C start "" "/foo/bar.rs"` / Linux `xdg-open /foo/bar.rs`
- **AND** `line` / `column` 参数 SHALL 被忽略（OS 默认 app 不一定支持跳行号）

#### Scenario: editor CLI 未装返回 ExternalApp

- **WHEN** Settings `externalEditor = "cursor"` 但 `cursor` CLI 不在 PATH
- **THEN** 后端 spawn 失败 SHALL 返回 `ApiError::ExternalApp("editor CLI 'cursor' not found; install Cursor shell command or change Settings")`
- **AND** 前端 SHALL 弹 toast 显示该 message 引导用户

#### Scenario: spawn 非阻塞

- **WHEN** 后端调 `Command::new(...).spawn()` 启动 editor
- **THEN** SHALL 立即返回 `Ok(())` 不等待 editor 进程退出
- **AND** editor 进程 SHALL 独立于本应用生命周期运行

### Requirement: list_available_terminals IPC 契约

应用 SHALL 暴露 `list_available_terminals` Tauri command（HTTP 镜像同名）：入参空；返回 `Vec<String>`，元素为当前 OS 合法 `TerminalApp` 枚举的 snake_case 序列化值。前端 Settings dropdown 用此 IPC 过滤选项，避免在 macOS 显示 Windows / Linux 终端。

#### Scenario: 当前平台过滤

- **WHEN** 前端调 `invoke('list_available_terminals')`
- **THEN** 后端 SHALL 按 `cfg!(target_os)` 返回当前平台合法集合：
- **AND** macOS 返回 `["terminal", "i_term", "warp"]`
- **AND** Windows 返回 `["windows_terminal", "cmd", "power_shell"]`
- **AND** Linux 返回 `["x_terminal_emulator", "gnome_terminal", "konsole", "alacritty"]`
