## ADDED Requirements

### Requirement: 外部 CLI PATH 解析

`open_in_editor` / `open_in_terminal` 在 spawn 外部编辑器 / 终端 CLI（`code` / `cursor` / `zed` / `subl` 及 Linux 终端 emulator）前，SHALL 在「增强 PATH」内把 bare CLI 名解析成绝对路径再启动。增强 PATH SHALL 合并：当前进程 PATH、用户 login-shell 真实 PATH（unix）、平台 well-known 安装目录三者（保序去重）。这解决桌面 app 从 Finder/Dock 启动时进程 PATH 为 launchd 精简版（不含 `/usr/local/bin` / `/opt/homebrew/bin` / `~/.cargo/bin` 等）导致已安装 CLI 误报 not found 的问题。

系统内置命令（macOS `open`、Linux `xdg-open`、Windows `wt.exe` / `powershell.exe` / `cmd.exe`）SHALL 不参与解析——它们恒在精简 PATH 可达范围内。增强 PATH SHALL 全进程仅构建一次并缓存，login-shell 解析 SHALL 为 best-effort（超时 / 失败时退回当前进程 PATH + well-known 目录兜底，不阻断启动）。

#### Scenario: CLI 在用户 PATH 但不在 GUI app 精简 PATH 时仍解析成功

- **WHEN** 配置的编辑器 CLI（如 `zed`）已安装在 `/usr/local/bin` 等用户目录，但桌面 app 进程的精简 PATH 不含该目录
- **THEN** 后端 SHALL 在增强 PATH（含 login-shell PATH / well-known 目录）内解析出该 CLI 的绝对路径
- **AND** SHALL 用绝对路径 spawn，编辑器正常打开，不报 not found

#### Scenario: 解析后仍找不到才回退 not-found 语义

- **WHEN** 配置的 CLI 在增强 PATH（当前进程 PATH + login-shell PATH + well-known 目录）内均不存在
- **THEN** 后端 SHALL 回退用 bare name spawn，由 spawn 失败返回 ExternalApp 错误引导用户安装或改 Settings
- **AND** 错误 message 中的 CLI 名 SHALL 为原始 bare name（非绝对路径）

#### Scenario: login-shell 解析失败时 well-known 目录兜底

- **WHEN** login-shell PATH 解析超时或失败（非常规 shell / rc 文件异常）
- **THEN** 增强 PATH SHALL 仍含当前进程 PATH 与平台 well-known 目录
- **AND** 装在标准位置（如 macOS `/usr/local/bin`）的 CLI SHALL 仍能解析成功
