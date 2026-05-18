## Why

Windows 用户大量在 WSL 内运行 Claude Code，session 文件落在 `\\wsl.localhost\<distro>\home\<user>\.claude\projects\` 下；TS 原版（`../claude-devtools/src/main/ipc/config.ts::listWslDistros`）已实现"扫描本机 WSL distro → 让用户选 distro → 把 `claudeRootPath` 设成对应 UNC 路径"的工作流，**Rust 端口至今缺这条路径**——Windows 用户要手填 UNC 路径才能用 WSL 内的数据。本 change 补齐这一 Windows 平台特异能力，与 TS 原版行为对齐。

## What Changes

- 新增 `wsl-distro-discovery` capability：在 Windows 平台上枚举本机已安装的 WSL distro，解析每个 distro 的 `$HOME` 与 `~/.claude` UNC 候选路径，返回给前端供用户选择
- IPC 层新增 1 个命令 `list_wsl_distros`，返回 `WslDistroScanReport { candidates: Vec<WslDistroCandidate>, distrosWithoutHome: Vec<String> }`：
  - `candidates`：解 home 成功的 distro，每个含 `distro` 名 / `homePath`（distro 内绝对路径，例 `/home/alice`）/ `claudeRootPath`（外部 UNC 形式，例 `\\wsl.localhost\Ubuntu\home\alice\.claude`）/ `claudeRootExists`（UNC 是否可访问标记）
  - `distrosWithoutHome`：枚举到但解 home 失败（含 USERNAME fallback）的 distro 名，用于前端区分"无 WSL"和"有 WSL 但全失败"
- 非 Windows 平台 `list_wsl_distros` SHALL 返回 `{ candidates: [], distrosWithoutHome: [] }`（不算错误）
- 命令枚举 fallback 链：依次尝试 `--list --quiet` / `-l -q` / `-l` 三套参数；wsl.exe 路径 candidate 顺序为 `%WINDIR%\System32\wsl.exe` / `%WINDIR%\Sysnative\wsl.exe` / `wsl.exe`
- 解码 `wsl.exe` stdout 兼容 UTF-16 LE（含 BOM 检测 + heuristic）与 UTF-8（ASCII 输出常见路径），全局 strip NUL 字节
- 解 home 失败时回退到 `/home/<windows-USERNAME>` —— 与 TS 原版一致
- Settings UI General section `claudeRootPath` 输入框下方新增 "Use WSL" 按钮：仅在 Windows 平台显示；点击触发 `list_wsl_distros`；返回单 distro 时直接填入 `claudeRootPath`；多 distro 时弹 modal 让用户选；返回 `candidates` 空 + `distrosWithoutHome` 空时 inline 提示"未检测到 WSL distro"；`candidates` 空 + `distrosWithoutHome` 非空时 inline 提示"检测到 WSL distro 但无法解析 home（…）"
- 选定 distro 后通过现有 `update_config("general", { claudeRootPath })` 持久化 —— **不引入新配置字段、不引入新运行时 context、不扩 `FsKind` / `ActiveContext`**

## Capabilities

### New Capabilities
- `wsl-distro-discovery`: Windows 平台枚举本机 WSL distro 并返回每个 distro 的 `~/.claude` UNC 候选路径，封装 `wsl.exe -l -q` 调用、UTF-16 LE 输出解码、`wsl -d <distro> -- sh -lc 'printf %s "$HOME"'` 解 home 三类子任务

### Modified Capabilities
- `settings-ui`: General section 新增 Windows-only "Use WSL" 按钮 + distro 选择 modal 行为；按钮点击 → 调 `list_wsl_distros` → 单结果直填 / 多结果选择 / 空结果 inline 提示

## Impact

- 受影响代码：
  - `crates/cdt-discover/src/wsl.rs`（**新文件**，`#[cfg(target_os = "windows")]` gate；非 Windows 走 stub 返回空）
  - `crates/cdt-discover/src/lib.rs`（导出 `wsl` 模块 + 公开类型）
  - `crates/cdt-api/src/ipc/`（新 IPC command + handler）
  - `src-tauri/src/lib.rs::invoke_handler!`（注册新命令）
  - `src-tauri/capabilities/default.json`（授权新命令）
  - `ui/src/routes/SettingsView.svelte`（"Use WSL" 按钮 + IPC 调用）
  - `ui/src/lib/components/`（distro 选择 modal —— 优先复用现有 modal 基础设施，不另起组件）
  - `openspec/specs/wsl-distro-discovery/spec.md`（新主 spec）
  - `openspec/changes/wsl-distro-scan/specs/settings-ui/spec.md`（增量加 General section 的 WSL 按钮 requirement）
- 受影响 IPC 协议：新增 `list_wsl_distros` command；payload schema 走 `cdt-api` IPC contract test 同步
- 平台依赖：仅 Windows；macOS / Linux 平台保留 stub 返回空，IPC 协议保持单形态
- 不引入新 crate（最薄方案，详见 design.md D1）
- 不修改 `configuration-management` 主 spec：UNC 路径作为合法 absolute path 完全在现有 `claudeRootPath` 持久化契约内
- 测试：`cdt-discover` 加 mocked output 解析单测（UTF-16 LE / BOM / 空列表 / 命令失败 4 种 case）；UI 加 vitest mockIPC 验按钮行为；Windows-only 集成手测发版前 smoke
