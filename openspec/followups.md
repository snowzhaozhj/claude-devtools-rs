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

## turn-anchoring（issue #540）

本 change 只修了 turn 锚点（被打断的用户消息现在占一个 turn + 产 user-message injection）。以下三项是同源但独立的遗留，**不阻塞** archive，SHALL 跟踪：

### 保留被打断响应的 partial 内容（放宽 `<synthetic>` 过滤）

被打断的 partial 响应 `model == "<synthetic>"` 当前被 `cdt-parse::noise.rs` 当 `HardNoise(SyntheticAssistant)` 整条过滤，连"有真实 partial 文本的中断响应"也丢。修复后被打断 turn 只显示用户问题、不显示 AI 已产出的 partial 内容。放行需改 noise.rs 分类 + chunk-building 数据模型（被打断 partial 如何承载到 AIChunk），独立风险面。**关闭路径**：评估后开 GitHub issue（label `bug`），或并入 CLI/MCP turn 模型重设计 `redesign-cli-mcp-api`。

### 中断标记错位（`append_interruption_to_last_ai`）

中断标记 `[Request interrupted by user]` 当前被 `cdt-analyze::chunk::builder::append_interruption_to_last_ai` 追加到**最后一个** `AIChunk`——当被打断的 turn 没有 AIChunk 时，标记错位到更早一个 AI 响应上。这是 chunk-building 数据模型问题，独立于 context turn 会计。**关闭路径**：开 GitHub issue（label `bug`）。

### 纯被打断 phase 的 injection 丢失 + PhaseSelector 跳号（pre-existing）

`[AI, compact, User-only, compact, AI]` 这类"phase 内无任何 AI group"的情形：(a) 该 phase 的被打断 injection 无 backfill 目标而丢失；(b) `current_phase_number` 在 compact 处仍递增但空 phase 不 push 进 `phases`，PhaseSelector 出现跳号。二者均 **pre-existing**（本 change 不改 phase push / number 逻辑）。**关闭路径**：评估发生率后决定是否引入"可表示空 phase 的 phase metadata"使编号连续，或开 GitHub issue 记录已知限制。
