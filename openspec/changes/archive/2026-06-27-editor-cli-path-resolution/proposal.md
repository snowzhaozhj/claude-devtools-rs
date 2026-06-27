## Why

桌面 app（Tauri，从 Finder/Dock 启动）的进程 PATH 是 launchd 给的精简版（`/usr/bin:/bin:/usr/sbin:/sbin`），不含 `/usr/local/bin`、`/opt/homebrew/bin`、`~/.cargo/bin` 等用户 CLI 安装目录。`open_in_editor` / `open_in_terminal` 用 bare program name（`Command::new("zed")` 等）spawn 外部 CLI，靠进程 PATH 解析——于是桌面 app 里 `code` / `cursor` / `zed` / `subl` 及 Linux 用户态终端（如 `alacritty`）即使已安装也误报 `ExternalApp: editor CLI 'zed' not found`。终端里能用是因为 shell 的 PATH 完整。

这是 `frontend-context-menu` capability 的 `open_in_editor` / `open_in_terminal` 契约 bug：scenario「编辑器 CLI 未装返 ExternalApp」里「不在 PATH」的判定用错了 PATH——用的是 GUI app 精简 PATH，而非用户真实环境的 PATH。

## What Changes

- 新增 `cdt-api` 内 `path_resolve` 模块：把 bare CLI 名解析成**绝对路径**，解析范围是「增强 PATH」= 当前进程 PATH ∪ 用户 login-shell 真实 PATH ∪ 平台 well-known 目录。
- `open_in_editor` 的 4 个编辑器 CLI（`code`/`cursor`/`zed`/`subl`）与 `open_in_terminal` 的 Linux 终端 emulator 分支，spawn 前先解析绝对路径；命中用绝对路径启动，未命中回退 bare name（保留原 not-found 错误语义）。
- macOS `open` / `xdg-open` / Windows `wt.exe`/`powershell.exe`/`cmd.exe` 不变（系统目录 / Windows GUI app 继承完整 PATH，不受影响）。

## Capabilities

### New Capabilities
<!-- 无新 capability -->

### Modified Capabilities
- `frontend-context-menu`: `open_in_editor` / `open_in_terminal` 的 CLI 解析契约——新增「CLI 在用户 PATH 但不在 GUI app 精简 PATH 时 SHALL 仍解析成功」的行为；保留「解析后仍找不到 → ExternalApp 错误」。

## Impact

- 代码：`crates/cdt-api/src/ipc/path_resolve.rs`（新增）、`crates/cdt-api/src/ipc/external_app.rs`（`build_editor_command` / `build_terminal_command` async 化 + 接入解析）、`crates/cdt-api/src/ipc/mod.rs`（注册 mod）。
- 依赖：新增 `which`（workspace dep，跨平台可执行文件查找，处理 Windows PATHEXT）。
- 子进程：unix 上首次解析触发一次 login-shell spawn（`$SHELL -ilc`，2s timeout，OnceCell 全进程缓存一次），离 hot path，低频用户交互——见 design.md D4 + PR perf 模板。
- 平台：unix（macOS/Linux）行为变化；Windows 行为不变。
