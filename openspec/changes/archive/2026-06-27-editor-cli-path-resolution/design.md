## Context

`open_in_editor` / `open_in_terminal`（`crates/cdt-api/src/ipc/external_app.rs`）用 bare program name spawn 外部 CLI，依赖**进程 PATH** 解析。桌面 app 从 Finder/Dock 启动时进程 PATH 是 launchd 精简版（`/usr/bin:/bin:/usr/sbin:/sbin`），不含 `/usr/local/bin` / `/opt/homebrew/bin` / `~/.cargo/bin` 等用户 CLI 安装目录，导致已安装的 `zed` / `code` / `cursor` / `subl` / `alacritty` 误报 not found。这是 macOS/Linux GUI app 的经典问题（Electron 生态用 `fix-path` / `shell-env`、VS Code 用同款方案解决）。

## Goals / Non-Goals

**Goals:**
- 用户在终端能启动的编辑器/终端 CLI，从桌面 app 右键菜单也能启动。
- 覆盖非标准安装位置（asdf/nvm/fnm/自定义 prefix），不只硬编码几个目录。
- 解析失败时保留原有清晰的 not-found 错误语义。

**Non-Goals:**
- 不改 Windows 行为（Windows GUI app 继承完整 PATH，不受此问题影响）。
- 不解析系统内置命令（`open` / `xdg-open` / `powershell` / `cmd` 在系统目录，恒在精简 PATH 内）。
- 不改 IPC 字段 / payload / 前端契约。

## Decisions

### D1：用 login-shell 真实 PATH（最佳实践），硬编码目录仅作兜底

主路径解析用户 login-shell 的真实 PATH（unix 跑 `$SHELL -ilc` 取 `$PATH`），而非仅硬编码常见目录。理由：login-shell PATH 完全复刻用户终端环境——「用户在命令行能用」即「app 里能用」，天然覆盖 asdf/nvm/fnm/自定义 prefix；硬编码目录覆盖不了这些。这是 VS Code / `fix-path` / `shell-env` 的同款做法。硬编码 well-known 目录退为兜底（见 D3）。

### D2：引入 `which` crate 解析绝对路径，而非依赖 std Command 的 child-env PATH 解析

用 `which::which_in(name, Some(enhanced_path), cwd)` 在增强 PATH 里把 CLI 解析成**绝对路径**，再用绝对路径 spawn。不走「给 Command 设 `.env("PATH", x)` 让 std 自己查」——std 在 unix 上「用父进程 PATH 还是 child-env PATH 解析 program」的行为有歧义，显式 `which_in` 无歧义且跨平台处理 Windows PATHEXT（`code.cmd`）。命中用绝对路径；未命中回退 bare name，让后续 spawn 产生与之前一致的 `NotFound` → ExternalApp 错误。

### D3：well-known 目录作为 login-shell 失败/超时的兜底（defense-in-depth）

增强 PATH = 当前进程 PATH ∪ login-shell PATH ∪ well-known 目录，三者合并保序去重。well-known：macOS `/usr/local/bin` `/opt/homebrew/bin` `/opt/local/bin`；Linux `/usr/local/bin` `/snap/bin` `/var/lib/flatpak/exports/bin`；unix home `~/.local/bin` `~/.cargo/bin` `~/bin`（用 `cdt_discover::home_dir()` 满足 Windows 兼容硬约束，虽 home 段 `#[cfg(unix)]`）。即使 login-shell spawn 超时，常见安装位置仍能命中。

### D4：增强 PATH 用 `OnceCell` 全进程缓存一次

login-shell spawn 是子进程，但增强 PATH 在 `tokio::sync::OnceCell` 里全进程只构建一次，后续解析零额外 spawn。`perf.md`「有文件可读绝不 spawn 子进程」不适用：login-shell 解析出的交互式 PATH **没有文件可读**（是 source rc 文件后的产物），且这条路径是低频用户交互（点「在编辑器打开」），离 startup / IPC / 算法 hot path。2s timeout 防止 rc 文件慢启动卡住调用方。

### D5：只接入受影响的 CLI

接入 `build_editor_command` 的 4 个编辑器（`code`/`cursor`/`zed`/`subl`）+ `build_terminal_command` 的 Linux emulator（`x-terminal-emulator`/`gnome-terminal`/`konsole`/`alacritty`）。macOS `open -a`、Windows `wt.exe`/`powershell.exe`/`cmd.exe`、system `open`/`xdg-open` 不接入——它们在系统目录或 Windows 继承完整 PATH，恒可解析。

## Risks / Trade-offs

- **login-shell `-i` 可能慢或挂**（重 rc 文件 / 等待 tty）：用 2s `timeout` + `stdin(Stdio::null())` 兜底；超时退回 well-known 目录，zed 等仍可解析。
- **rc 文件往 stdout 喷噪声**（`echo` / fastfetch）：用 sentinel `__CDT_PATH_START__...__CDT_PATH_END__` 包裹 `printf`，只提取标记间内容（`shell-env`/`fix-path` 同款）。
- **全进程缓存 = 进程生命周期内不刷新**：用户改 PATH 后需重启 app 才生效——可接受（PATH 变更本就低频，且 well-known 兜底覆盖新增标准目录）。
- **`ends_with("code")` 类断言极端误配**（装了 `my-special-code`）：实际 CLI 名固定，风险可接受。

**这个设计最先会在哪断？** login-shell 解析在某些非常规 shell 配置（非 zsh/bash 的 `$SHELL`、`-i` 不被支持、rc 文件 `set -e` 提前退出）下返回 None——但此时 well-known + 当前进程 PATH 仍兜底，最坏退化到「只认标准目录」，不会比现状更差。**扩展瓶颈**：未来要支持更多编辑器只需在 `build_editor_command` 加分支 + 一个 `resolve_program` 调用，无结构性瓶颈。
