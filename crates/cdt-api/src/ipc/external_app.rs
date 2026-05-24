//! 外部应用交互 IPC：`open_in_terminal` / `open_in_editor` / `list_available_terminals`。
//!
//! 行为契约：`openspec/specs/frontend-context-menu/spec.md` 的三个 Requirement
//! - `open_in_terminal IPC 契约`
//! - `open_in_editor IPC 契约`
//! - `list_available_terminals IPC 契约`
//!
//! 设计决策：`openspec/changes/frontend-context-menu-phase-2/design.md::D1` ~ `D5`。
//!
//! 安全不变量（硬约束）：
//! - 入参 SHALL **不**接受任意 shell command 字符串，仅接受 path / line / column
//! - macOS / Linux / Windows Terminal 一律 OS-level argv 传参（`Command::new(exe).arg(path)`）
//! - Windows PowerShell / cmd fallback 把 path 走 `CDT_TARGET_PATH` env var，
//!   命令内仅引用 `$env:CDT_TARGET_PATH` / `%CDT_TARGET_PATH%`，**严禁**拼字符串
//! - cmd fallback 在 path 含 `&|<>^()%!'"\n` 等 cmd metacharacters 时直接拒绝
//!   返回 `ApiError::ValidationError`（cmd parser 在 env var 展开后仍 re-tokenize
//!   ，无法 100% 安全）
//! - Editor / Terminal 可执行文件白名单完全封闭在本模块内，前端**无法**指定任意程序

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use cdt_config::{ConfigManager, ExternalEditor, TerminalApp};
use cdt_discover::looks_like_absolute_path;
use tokio::process::Command;
use tokio::sync::Mutex;

use super::error::ApiError;

// =============================================================================
// 公开 IPC 函数
// =============================================================================

/// 在用户偏好终端 app 中打开目录（仅 cd，不执行任何用户命令）。
///
/// 详 `frontend-context-menu/spec.md::open_in_terminal IPC 契约`。
pub async fn open_in_terminal(
    path: &str,
    config_mgr: &Mutex<ConfigManager>,
) -> Result<(), ApiError> {
    let canonical = validate_and_canonicalize_path(path).await?;
    let dir = ensure_directory(&canonical).await?;

    // 读取当前 terminal_app 设置，按当前 OS 判断是否需要 fallback
    let configured = {
        let guard = config_mgr.lock().await;
        guard.get_config().general.terminal_app
    };
    let terminal = if configured.is_available_on_current_platform() {
        configured
    } else {
        let fallback = TerminalApp::platform_default();
        tracing::warn!(
            configured = ?configured,
            fallback = ?fallback,
            current_os = std::env::consts::OS,
            "terminal_app not available on current OS; falling back to platform default"
        );
        fallback
    };

    let cmd = build_terminal_command(terminal, &dir)?;
    spawn_command(cmd, "terminal")
}

/// 在用户偏好编辑器中打开文件（含可选行号 / 列号）。
///
/// 详 `frontend-context-menu/spec.md::open_in_editor IPC 契约`。
pub async fn open_in_editor(
    path: &str,
    line: Option<u32>,
    column: Option<u32>,
    config_mgr: &Mutex<ConfigManager>,
) -> Result<(), ApiError> {
    let canonical = validate_and_canonicalize_path(path).await?;

    let editor = {
        let guard = config_mgr.lock().await;
        guard.get_config().general.external_editor
    };

    let cmd = build_editor_command(editor, &canonical, line, column);
    spawn_command(cmd, "editor")
}

/// 当前平台支持的 `TerminalApp` 列表（`snake_case` 序列化值），供 Settings dropdown 过滤。
///
/// 详 `frontend-context-menu/spec.md::list_available_terminals IPC 契约`。
#[must_use]
pub fn list_available_terminals() -> Vec<String> {
    TerminalApp::available_for_current_platform()
        .into_iter()
        .map(|app| {
            // 用 serde 输出与 IPC 契约一致的 snake_case 字符串（如 ITerm → "i_term"）
            let v = serde_json::to_value(app).expect("TerminalApp serializes to value");
            v.as_str()
                .expect("TerminalApp serializes to JSON string")
                .to_owned()
        })
        .collect()
}

// =============================================================================
// Path 校验：绝对路径 + canonicalize（拒绝相对 / 不存在 / traversal）
// =============================================================================

/// 校验 path 并 canonicalize 解析符号链接 / `..` / 存在性。
///
/// 详 `design.md::D4` 决策的 path 校验 5 步策略 (1)-(2)。
async fn validate_and_canonicalize_path(path: &str) -> Result<PathBuf, ApiError> {
    if !looks_like_absolute_path(path) {
        return Err(ApiError::validation("path must be absolute"));
    }
    tokio::fs::canonicalize(path)
        .await
        .map_err(|e| ApiError::not_found(format!("path does not exist: {path} ({e})")))
}

/// 文件路径自动取父目录（仅 `open_in_terminal` 用）。
///
/// 详 `design.md::D4` 决策的 path 校验 5 步策略 (3)。
async fn ensure_directory(canonical: &Path) -> Result<PathBuf, ApiError> {
    let metadata = tokio::fs::metadata(canonical)
        .await
        .map_err(|e| ApiError::not_found(format!("path metadata failed: {e}")))?;
    if metadata.is_dir() {
        Ok(canonical.to_path_buf())
    } else {
        canonical
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| ApiError::validation("file path has no parent directory"))
    }
}

// =============================================================================
// Terminal command 构造（按 TerminalApp dispatch 平台命令）
// =============================================================================

/// 构造打开终端的 `Command`。
///
/// 详 `design.md::D1` 跨平台 spawn 映射表。
fn build_terminal_command(app: TerminalApp, dir: &Path) -> Result<Command, ApiError> {
    let mut cmd = match app {
        // -------- macOS（OS-argv，零注入）--------
        TerminalApp::Terminal => {
            let mut c = Command::new("open");
            c.arg("-a").arg("Terminal").arg(dir);
            c
        }
        TerminalApp::ITerm => {
            let mut c = Command::new("open");
            c.arg("-a").arg("iTerm").arg(dir);
            c
        }
        TerminalApp::Warp => {
            let mut c = Command::new("open");
            c.arg("-a").arg("Warp").arg(dir);
            c
        }
        // -------- Windows --------
        TerminalApp::WindowsTerminal => {
            let mut c = Command::new("wt.exe");
            c.arg("-d").arg(dir);
            c
        }
        TerminalApp::PowerShell => {
            // path 通过 env var 传入，命令内只引用 `$env:CDT_TARGET_PATH`，
            // path 完全不进 PowerShell parser
            let mut c = Command::new("powershell.exe");
            c.args([
                "-NoExit",
                "-Command",
                "Set-Location -LiteralPath $env:CDT_TARGET_PATH",
            ]);
            c.env("CDT_TARGET_PATH", dir.as_os_str());
            c
        }
        TerminalApp::Cmd => {
            // cmd 在 env var 展开后仍 re-tokenize；path 含 cmd metacharacters 时拒绝
            reject_cmd_unsafe_path(dir)?;
            let mut c = Command::new("cmd.exe");
            c.args(["/K", "cd /d \"%CDT_TARGET_PATH%\""]);
            c.env("CDT_TARGET_PATH", dir.as_os_str());
            c
        }
        // -------- Linux --------
        TerminalApp::XTerminalEmulator => {
            let mut c = Command::new("x-terminal-emulator");
            c.arg("--working-directory").arg(dir);
            c
        }
        TerminalApp::GnomeTerminal => {
            // gnome-terminal 用 `=` 形式传参（不接受空格分隔）
            let mut arg: OsString = OsString::from("--working-directory=");
            arg.push(dir.as_os_str());
            let mut c = Command::new("gnome-terminal");
            c.arg(arg);
            c
        }
        TerminalApp::Konsole => {
            let mut c = Command::new("konsole");
            c.arg("--workdir").arg(dir);
            c
        }
        TerminalApp::Alacritty => {
            let mut c = Command::new("alacritty");
            c.arg("--working-directory").arg(dir);
            c
        }
    };
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    Ok(cmd)
}

/// cmd.exe 在 `%VAR%` 展开后仍会 re-tokenize 命令字符串——path 含 cmd
/// metacharacters 时无法 100% 安全（即使加引号也可能被 `^` 转义穿透）。
/// 直接拒绝路径，引导用户重命名。详 `design.md::D1` 安全实现细则。
fn reject_cmd_unsafe_path(path: &Path) -> Result<(), ApiError> {
    const UNSAFE_CHARS: &[char] = &[
        '&', '|', '<', '>', '^', '(', ')', '%', '!', '\'', '"', '\n', '\r',
    ];
    let path_str = path.to_string_lossy();
    if let Some(c) = path_str.chars().find(|c| UNSAFE_CHARS.contains(c)) {
        return Err(ApiError::validation(format!(
            "path contains characters unsafe for Windows cmd shell: '{c}'. \
             Rename the directory or switch to PowerShell / Windows Terminal."
        )));
    }
    Ok(())
}

// =============================================================================
// Editor command 构造（按 ExternalEditor dispatch CLI + 跳行号）
// =============================================================================

/// 构造打开编辑器的 `Command`。
///
/// 详 `design.md::D2` Editor CLI 映射表。`line: None` 时省略行号后缀，
/// `column: None` 时省略列号（保留 line）。Windows drive letter colon 与
/// `--goto path:line:col` 冲突由各 editor parser 智能识别 drive letter。
fn build_editor_command(
    editor: ExternalEditor,
    canonical: &Path,
    line: Option<u32>,
    column: Option<u32>,
) -> Command {
    let mut cmd = match editor {
        ExternalEditor::System => return build_system_open_command(canonical),
        ExternalEditor::VsCode => goto_command("code", canonical, line, column),
        ExternalEditor::Cursor => goto_command("cursor", canonical, line, column),
        ExternalEditor::Zed => path_with_loc_command("zed", canonical, line, column),
        ExternalEditor::Sublime => path_with_loc_command("subl", canonical, line, column),
    };
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd
}

/// `code --goto <path>:<line>:<col>` 形式（VS Code / Cursor）。
///
/// `--goto` 在没有 line 时省略整个 flag；line 存在 column 不存在时只跟 line。
/// VS Code / Cursor parser 已知支持 Windows drive letter colon 智能识别
/// （详 `design.md::D2` Windows drive letter colon 说明）。
fn goto_command(cli: &str, path: &Path, line: Option<u32>, column: Option<u32>) -> Command {
    let mut cmd = Command::new(cli);
    if let Some(l) = line {
        cmd.arg("--goto");
        cmd.arg(format_goto_target(path, l, column));
    } else {
        cmd.arg(path);
    }
    cmd
}

/// `zed <path>:<line>:<col>` / `subl <path>:<line>:<col>` 形式。
fn path_with_loc_command(
    cli: &str,
    path: &Path,
    line: Option<u32>,
    column: Option<u32>,
) -> Command {
    let mut cmd = Command::new(cli);
    if let Some(l) = line {
        cmd.arg(format_goto_target(path, l, column));
    } else {
        cmd.arg(path);
    }
    cmd
}

/// 拼接 `<path>:<line>` 或 `<path>:<line>:<col>` 形式的 `OsString`。
///
/// 用 `OsString::push` 处理非 UTF-8 path（macOS / Linux 合法），避免
/// `to_string_lossy` 截断；Windows 上 `to_string_lossy()` 等价于直接转。
fn format_goto_target(path: &Path, line: u32, column: Option<u32>) -> OsString {
    let mut out: OsString = path.as_os_str().to_owned();
    out.push(":");
    out.push(line.to_string());
    if let Some(c) = column {
        out.push(":");
        out.push(c.to_string());
    }
    out
}

/// `System` fallback：走 OS 默认打开应用，忽略 line / column。
fn build_system_open_command(path: &Path) -> Command {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(path);
        c
    } else if cfg!(target_os = "windows") {
        // `cmd /C start "" "<path>"` —— 第一个空字符串是窗口标题占位，避免
        // path 被 `start` 误识别为标题。path 走 OS-argv 由 OS quoting 处理。
        let mut c = Command::new("cmd.exe");
        c.arg("/C").arg("start").arg("").arg(path);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd
}

// =============================================================================
// Spawn helper：非阻塞启动子进程，错误归类为 ApiErrorCode::ExternalApp
// =============================================================================

/// 非阻塞 spawn 子进程，立即返回——不等待目标 app 退出。
///
/// 详 `design.md::D2` 决策的 fallback 链 step 5："spawn 用 `.spawn()`，
/// 不等待 editor 进程退出"。`tokio::process::Command::spawn` 是同步调用，
/// 故本函数无需 `async`；调用方在 async fn 内直接调即可。
fn spawn_command(mut cmd: Command, kind: &str) -> Result<(), ApiError> {
    // 拿到可执行文件名用于 error message（用 OsString 避免非 UTF-8 panic）
    let program: OsString = cmd.as_std().get_program().to_owned();
    let program_lossy = program.to_string_lossy().into_owned();

    match cmd.spawn() {
        Ok(child) => {
            // 非阻塞模式：spawn 一个 reaper task 等子进程退出，避免 zombie；
            // 调用方不阻塞，立即返回 Ok(())。
            tokio::spawn(async move {
                let mut child = child;
                let _ = child.wait().await;
            });
            Ok(())
        }
        Err(e) => {
            // ErrorKind::NotFound = CLI 不在 PATH（最常见）
            if matches!(e.kind(), std::io::ErrorKind::NotFound) {
                Err(ApiError::external_app(format!(
                    "{kind} CLI '{program_lossy}' not found; install or change Settings ({e})"
                )))
            } else {
                Err(ApiError::external_app(format!(
                    "failed to launch {kind} '{program_lossy}': {e}"
                )))
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use tempfile::TempDir;

    /// 构造一个临时 `ConfigManager` 持有指定的 `external_editor` / `terminal_app` 设置。
    async fn config_with(
        external_editor: ExternalEditor,
        terminal_app: TerminalApp,
    ) -> (TempDir, Mutex<ConfigManager>) {
        let dir = TempDir::new().expect("tempdir");
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.expect("load");
        mgr.update_general(serde_json::json!({
            "externalEditor": format!("{}",
                serde_json::to_value(external_editor).unwrap().as_str().unwrap()),
            "terminalApp": format!("{}",
                serde_json::to_value(terminal_app).unwrap().as_str().unwrap()),
        }))
        .await
        .expect("update general");
        (dir, Mutex::new(mgr))
    }

    // -------- Path 校验 --------

    #[tokio::test]
    async fn open_in_terminal_rejects_relative_path() {
        let (_d, cfg) = config_with(ExternalEditor::System, TerminalApp::Terminal).await;
        let err = open_in_terminal("relative/path", &cfg).await.unwrap_err();
        assert_eq!(err.code, super::super::error::ApiErrorCode::ValidationError);
        assert!(err.message.contains("absolute"));
    }

    #[tokio::test]
    async fn open_in_terminal_rejects_nonexistent_path() {
        let (_d, cfg) = config_with(ExternalEditor::System, TerminalApp::Terminal).await;
        let err = open_in_terminal("/nonexistent/foo/bar/baz/zzz", &cfg)
            .await
            .unwrap_err();
        assert_eq!(err.code, super::super::error::ApiErrorCode::NotFound);
    }

    #[tokio::test]
    async fn open_in_editor_rejects_relative_path() {
        let (_d, cfg) = config_with(ExternalEditor::VsCode, TerminalApp::Terminal).await;
        let err = open_in_editor("relative/foo.rs", Some(1), None, &cfg)
            .await
            .unwrap_err();
        assert_eq!(err.code, super::super::error::ApiErrorCode::ValidationError);
    }

    // -------- ensure_directory: 文件 → 父目录降级 --------

    #[tokio::test]
    async fn ensure_directory_returns_self_for_dir() {
        let dir = TempDir::new().unwrap();
        let canonical = tokio::fs::canonicalize(dir.path()).await.unwrap();
        let result = ensure_directory(&canonical).await.unwrap();
        assert_eq!(result, canonical);
    }

    #[tokio::test]
    async fn ensure_directory_falls_back_to_parent_for_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        tokio::fs::write(&file, "hello").await.unwrap();
        let canonical = tokio::fs::canonicalize(&file).await.unwrap();
        let result = ensure_directory(&canonical).await.unwrap();
        let expected_parent = tokio::fs::canonicalize(dir.path()).await.unwrap();
        assert_eq!(result, expected_parent);
    }

    // -------- list_available_terminals --------

    #[test]
    fn list_available_terminals_per_platform() {
        let list = list_available_terminals();
        assert!(!list.is_empty(), "expected at least one terminal");

        if cfg!(target_os = "macos") {
            assert_eq!(list, vec!["terminal", "i_term", "warp"]);
        } else if cfg!(target_os = "windows") {
            assert_eq!(list, vec!["windows_terminal", "cmd", "power_shell"]);
        } else {
            assert_eq!(
                list,
                vec![
                    "x_terminal_emulator",
                    "gnome_terminal",
                    "konsole",
                    "alacritty"
                ]
            );
        }
    }

    #[test]
    fn list_available_terminals_iterm_serializes_as_i_term() {
        // codex 二审重点：snake_case 对 ITerm 输出 i_term 不是 iterm
        if cfg!(target_os = "macos") {
            let list = list_available_terminals();
            assert!(list.contains(&"i_term".to_string()), "got: {list:?}");
            assert!(!list.contains(&"iterm".to_string()), "got: {list:?}");
        }
    }

    // -------- build_terminal_command: argv 拼接验证 --------

    fn argv_strings(cmd: &Command) -> Vec<String> {
        cmd.as_std()
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    fn program_string(cmd: &Command) -> String {
        cmd.as_std().get_program().to_string_lossy().into_owned()
    }

    fn env_var(cmd: &Command, key: &str) -> Option<String> {
        cmd.as_std().get_envs().find_map(|(k, v)| {
            if k == OsStr::new(key) {
                v.map(|val| val.to_string_lossy().into_owned())
            } else {
                None
            }
        })
    }

    #[test]
    fn build_terminal_macos_terminal_uses_open_a() {
        let dir = Path::new("/Users/foo/project");
        let cmd = build_terminal_command(TerminalApp::Terminal, dir).unwrap();
        assert_eq!(program_string(&cmd), "open");
        assert_eq!(
            argv_strings(&cmd),
            vec!["-a", "Terminal", "/Users/foo/project"]
        );
    }

    #[test]
    fn build_terminal_macos_iterm_uses_iterm_app_name() {
        let dir = Path::new("/Users/foo/project");
        let cmd = build_terminal_command(TerminalApp::ITerm, dir).unwrap();
        assert_eq!(
            argv_strings(&cmd),
            vec!["-a", "iTerm", "/Users/foo/project"]
        );
    }

    #[test]
    fn build_terminal_windows_terminal_uses_wt_d() {
        let dir = Path::new("C:\\Users\\foo");
        let cmd = build_terminal_command(TerminalApp::WindowsTerminal, dir).unwrap();
        assert_eq!(program_string(&cmd), "wt.exe");
        assert_eq!(argv_strings(&cmd), vec!["-d", "C:\\Users\\foo"]);
    }

    #[test]
    fn build_terminal_powershell_passes_path_via_env_var_not_argv() {
        // 安全不变量验证：path **不**进 PowerShell parser，命令内仅引用 env var
        let dir = Path::new("C:\\path with spaces & special'chars");
        let cmd = build_terminal_command(TerminalApp::PowerShell, dir).unwrap();
        assert_eq!(program_string(&cmd), "powershell.exe");
        assert_eq!(
            argv_strings(&cmd),
            vec![
                "-NoExit",
                "-Command",
                "Set-Location -LiteralPath $env:CDT_TARGET_PATH"
            ]
        );
        // path 在 env var 而不在 argv
        assert_eq!(
            env_var(&cmd, "CDT_TARGET_PATH").as_deref(),
            Some("C:\\path with spaces & special'chars")
        );
    }

    #[test]
    fn build_terminal_cmd_safe_path_uses_env_var() {
        let dir = Path::new("C:\\Users\\foo");
        let cmd = build_terminal_command(TerminalApp::Cmd, dir).unwrap();
        assert_eq!(program_string(&cmd), "cmd.exe");
        assert_eq!(
            argv_strings(&cmd),
            vec!["/K", "cd /d \"%CDT_TARGET_PATH%\""]
        );
        assert_eq!(
            env_var(&cmd, "CDT_TARGET_PATH").as_deref(),
            Some("C:\\Users\\foo")
        );
    }

    #[test]
    fn build_terminal_cmd_rejects_metachar_ampersand() {
        // codex 二审重点：cmd metacharacter 拒绝
        let dir = Path::new("C:\\foo&bar");
        let err = build_terminal_command(TerminalApp::Cmd, dir).unwrap_err();
        assert_eq!(err.code, super::super::error::ApiErrorCode::ValidationError);
        assert!(err.message.contains("unsafe"));
        assert!(err.message.contains('&'));
    }

    #[test]
    fn build_terminal_cmd_rejects_metachar_pipe() {
        let dir = Path::new("C:\\foo|bar");
        let err = build_terminal_command(TerminalApp::Cmd, dir).unwrap_err();
        assert!(err.message.contains('|'));
    }

    #[test]
    fn build_terminal_cmd_rejects_metachar_caret() {
        let dir = Path::new("C:\\foo^bar");
        let err = build_terminal_command(TerminalApp::Cmd, dir).unwrap_err();
        assert!(err.message.contains('^'));
    }

    #[test]
    fn build_terminal_cmd_rejects_metachar_percent() {
        let dir = Path::new("C:\\foo%bar%");
        let err = build_terminal_command(TerminalApp::Cmd, dir).unwrap_err();
        assert!(err.message.contains('%'));
    }

    #[test]
    fn build_terminal_cmd_rejects_metachar_quote() {
        let dir = Path::new("C:\\foo\"bar");
        let err = build_terminal_command(TerminalApp::Cmd, dir).unwrap_err();
        assert!(err.message.contains('"'));
    }

    #[test]
    fn build_terminal_cmd_rejects_newline() {
        let dir = Path::new("C:\\foo\nbar");
        let err = build_terminal_command(TerminalApp::Cmd, dir).unwrap_err();
        assert!(err.message.contains("unsafe"));
    }

    #[test]
    fn build_terminal_powershell_accepts_metachars_in_env_var() {
        // PowerShell 走 env var 不进 shell parser，metachars 安全
        let dir = Path::new("/path with & | special chars");
        let cmd = build_terminal_command(TerminalApp::PowerShell, dir).unwrap();
        assert_eq!(
            env_var(&cmd, "CDT_TARGET_PATH").as_deref(),
            Some("/path with & | special chars")
        );
    }

    #[test]
    fn build_terminal_linux_x_terminal_emulator_argv() {
        let dir = Path::new("/home/foo");
        let cmd = build_terminal_command(TerminalApp::XTerminalEmulator, dir).unwrap();
        assert_eq!(program_string(&cmd), "x-terminal-emulator");
        assert_eq!(argv_strings(&cmd), vec!["--working-directory", "/home/foo"]);
    }

    #[test]
    fn build_terminal_linux_gnome_terminal_uses_equals_form() {
        let dir = Path::new("/home/foo");
        let cmd = build_terminal_command(TerminalApp::GnomeTerminal, dir).unwrap();
        assert_eq!(program_string(&cmd), "gnome-terminal");
        assert_eq!(argv_strings(&cmd), vec!["--working-directory=/home/foo"]);
    }

    #[test]
    fn build_terminal_linux_konsole_uses_workdir_flag() {
        let dir = Path::new("/home/foo");
        let cmd = build_terminal_command(TerminalApp::Konsole, dir).unwrap();
        assert_eq!(program_string(&cmd), "konsole");
        assert_eq!(argv_strings(&cmd), vec!["--workdir", "/home/foo"]);
    }

    #[test]
    fn build_terminal_linux_alacritty() {
        let dir = Path::new("/home/foo");
        let cmd = build_terminal_command(TerminalApp::Alacritty, dir).unwrap();
        assert_eq!(program_string(&cmd), "alacritty");
        assert_eq!(argv_strings(&cmd), vec!["--working-directory", "/home/foo"]);
    }

    // -------- build_editor_command: argv 拼接验证 --------

    #[test]
    fn build_editor_vs_code_with_line_col() {
        let path = Path::new("/foo/bar.rs");
        let cmd = build_editor_command(ExternalEditor::VsCode, path, Some(42), Some(8));
        assert_eq!(program_string(&cmd), "code");
        assert_eq!(argv_strings(&cmd), vec!["--goto", "/foo/bar.rs:42:8"]);
    }

    #[test]
    fn build_editor_vs_code_line_only() {
        let path = Path::new("/foo/bar.rs");
        let cmd = build_editor_command(ExternalEditor::VsCode, path, Some(42), None);
        assert_eq!(argv_strings(&cmd), vec!["--goto", "/foo/bar.rs:42"]);
    }

    #[test]
    fn build_editor_vs_code_no_line_omits_goto() {
        let path = Path::new("/foo/bar.rs");
        let cmd = build_editor_command(ExternalEditor::VsCode, path, None, None);
        assert_eq!(argv_strings(&cmd), vec!["/foo/bar.rs"]);
    }

    #[test]
    fn build_editor_vs_code_no_line_with_column_still_omits_goto() {
        // line 缺失但 column 给了——column 也忽略（design.md::D2 fallback step 4）
        let path = Path::new("/foo/bar.rs");
        let cmd = build_editor_command(ExternalEditor::VsCode, path, None, Some(8));
        assert_eq!(argv_strings(&cmd), vec!["/foo/bar.rs"]);
    }

    #[test]
    fn build_editor_cursor_uses_cursor_cli() {
        let path = Path::new("/foo/bar.rs");
        let cmd = build_editor_command(ExternalEditor::Cursor, path, Some(10), Some(5));
        assert_eq!(program_string(&cmd), "cursor");
        assert_eq!(argv_strings(&cmd), vec!["--goto", "/foo/bar.rs:10:5"]);
    }

    #[test]
    fn build_editor_zed_no_goto_flag() {
        let path = Path::new("/foo/bar.rs");
        let cmd = build_editor_command(ExternalEditor::Zed, path, Some(10), Some(5));
        assert_eq!(program_string(&cmd), "zed");
        // zed 不用 --goto，path:line:col 直接拼
        assert_eq!(argv_strings(&cmd), vec!["/foo/bar.rs:10:5"]);
    }

    #[test]
    fn build_editor_sublime_no_goto_flag() {
        let path = Path::new("/foo/bar.rs");
        let cmd = build_editor_command(ExternalEditor::Sublime, path, Some(10), None);
        assert_eq!(program_string(&cmd), "subl");
        assert_eq!(argv_strings(&cmd), vec!["/foo/bar.rs:10"]);
    }

    #[test]
    fn build_editor_vs_code_windows_drive_letter_path_keeps_colon() {
        // codex 二审重点：Windows drive letter `C:\foo:42:8` —— VS Code parser
        // 已知支持智能识别 drive letter colon（详 design.md::D2 段）。后端
        // 责任仅是正确拼接 argv 让 parser 看到完整字符串。
        let path = Path::new("C:\\foo\\bar.rs");
        let cmd = build_editor_command(ExternalEditor::VsCode, path, Some(42), Some(8));
        assert_eq!(program_string(&cmd), "code");
        // 这里关键：drive letter `C:` 后跟 `\` 不被 VS Code parser 误判为 line；
        // 只有末尾两段数字 `:42:8` 才被识别为 line/col
        assert_eq!(argv_strings(&cmd), vec!["--goto", "C:\\foo\\bar.rs:42:8"]);
    }

    #[test]
    fn build_editor_vs_code_windows_drive_letter_path_line_only() {
        let path = Path::new("C:\\Program Files\\test.rs");
        let cmd = build_editor_command(ExternalEditor::VsCode, path, Some(7), None);
        assert_eq!(
            argv_strings(&cmd),
            vec!["--goto", "C:\\Program Files\\test.rs:7"]
        );
    }

    #[test]
    fn build_editor_system_macos_uses_open() {
        if !cfg!(target_os = "macos") {
            return;
        }
        let path = Path::new("/foo/bar.rs");
        let cmd = build_editor_command(ExternalEditor::System, path, Some(42), Some(8));
        assert_eq!(program_string(&cmd), "open");
        // System fallback 忽略 line/column
        assert_eq!(argv_strings(&cmd), vec!["/foo/bar.rs"]);
    }

    #[test]
    fn build_editor_system_linux_uses_xdg_open() {
        if !cfg!(target_os = "linux") {
            return;
        }
        let path = Path::new("/foo/bar.rs");
        let cmd = build_editor_command(ExternalEditor::System, path, None, None);
        assert_eq!(program_string(&cmd), "xdg-open");
        assert_eq!(argv_strings(&cmd), vec!["/foo/bar.rs"]);
    }

    // -------- format_goto_target --------

    #[test]
    fn format_goto_target_path_only_with_line() {
        let out = format_goto_target(Path::new("/foo/bar.rs"), 42, None);
        assert_eq!(out, OsStr::new("/foo/bar.rs:42"));
    }

    #[test]
    fn format_goto_target_path_with_line_and_col() {
        let out = format_goto_target(Path::new("/foo/bar.rs"), 42, Some(8));
        assert_eq!(out, OsStr::new("/foo/bar.rs:42:8"));
    }
}
