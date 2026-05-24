# openspec followups

per-change 改动遗留下来、不阻塞 archive 但 SHALL 跟踪到完成的事项。每条带来源 change + 期望关闭路径（单独 PR / GitHub issue / 跨 OS 手测）。完成后从此文件删除。

## frontend-context-menu-phase-2

### Windows / Linux manual spawn smoke（PR merge 后）

`open_in_terminal` / `open_in_editor` 跨平台 spawn 路径仅 macOS 已真测；Win/Linux 行为靠 Rust unit test (静态 argv 拼接) + IPC contract test (序列化) 兜底，实际 spawn 行为需人工 / runner 矩阵覆盖。SHALL 在 PR merge 后开 GitHub issue（label `bug` + `cross-platform`）跟踪：

**Windows manual checklist：**
- [ ] `wt.exe` 已装：右键 Bash 工具块 → 在终端打开 → Windows Terminal 弹窗口 cd 到 cwd
- [ ] `wt.exe` 未装（Win 10 LTSC）→ fallback 到 PowerShell（`Set-Location -LiteralPath $env:CDT_TARGET_PATH`）
- [ ] PowerShell path 含单引号 `'` → env var 路径 SHALL 正确传递不破解
- [ ] cmd fallback path 含 `&` / `|` / `^` → `ApiError::ValidationError` 返回 + 前端 toast
- [ ] drive letter `C:\Users\foo\bar.ts:42:8` → VS Code `code -g` 跳行号正确（注意 Windows path 含 `:` 与 line 分隔符 `:` 区分）
- [ ] PATH 未注册 `code.cmd` → 前端 toast "editor CLI 'code' not found"

**Linux manual checklist：**
- [ ] Debian/Ubuntu `x-terminal-emulator --working-directory=<path>` 真弹窗口
- [ ] GNOME Terminal `gnome-terminal --working-directory=<path>` 弹
- [ ] KDE Konsole `konsole --workdir <path>` 弹
- [ ] Alacritty `alacritty --working-directory <path>` 弹
- [ ] Wayland session vs X11 session 行为一致
- [ ] `xdg-open` 用 `.spawn()` 非阻塞（不等 editor 关闭）
- [ ] Flatpak / Snap 安装的 VS Code path 不在 PATH → 前端 toast 友好提示

**关闭条件：** 至少一名贡献者跑过 Win + Linux 各自至少一项主流配置（Win = wt 已装 / Linux = Debian + GNOME），结果在 issue 评论留痕。
