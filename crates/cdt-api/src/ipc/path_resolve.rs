//! 「增强 PATH」解析层：把 bare CLI 名（如 `code`/`zed`）解析成绝对路径。
//!
//! 职责：收集当前进程 PATH + login-shell 真实 PATH + 平台 well-known 目录，合并去重后
//! 作为搜索范围，调 `which_in` 找到可执行文件的绝对路径。桌面 app 从 Finder/Dock
//! 启动时进程 PATH 是 launchd 精简版，不含 `/opt/homebrew/bin` / `~/.cargo/bin` 等
//! 用户安装目录，本层在首次调用时一次性构建「增强 PATH」并缓存于 `OnceCell`。

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use tokio::sync::OnceCell;

static AUGMENTED_PATH: OnceCell<OsString> = OnceCell::const_new();

/// 把 bare CLI 名解析成绝对路径。
///
/// 命中时返回绝对路径 `OsString`；未命中时回退返回 `OsString::from(name)`，
/// 保留现有 "not found" 错误语义——调用方不处理此处的 "not found"。
pub(super) async fn resolve_program(name: &str) -> OsString {
    let path = augmented_path().await;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    resolve_in(name, path.as_os_str(), &cwd)
}

/// `which_in` 的薄封装：命中返回绝对路径 `OsString`，未命中返回 bare name。
fn resolve_in(name: &str, search_path: &OsStr, cwd: &Path) -> OsString {
    which::which_in(name, Some(search_path), cwd)
        .map_or_else(|_| OsString::from(name), PathBuf::into_os_string)
}

/// 全进程唯一「增强 PATH」，首次调用时构建后缓存。
///
/// 构建逻辑内联在 `async {}` 闭包里（而非独立 `async fn`）：Windows 上 login-shell
/// 那段 `.await` 被 `#[cfg(unix)]` 去掉后整个闭包无 await，但 `clippy::unused_async`
/// 只查 `async fn` 不查 async block，故内联可避免 Windows 上的 unused-async 报错。
async fn augmented_path() -> &'static OsString {
    AUGMENTED_PATH
        .get_or_init(|| async {
            let mut entries: Vec<PathBuf> = Vec::new();

            // 1. 当前进程 PATH（launchd 精简版，但总比没有好）
            if let Some(p) = std::env::var_os("PATH") {
                entries.extend(std::env::split_paths(&p));
            }

            // 2. login-shell 真实 PATH（仅 Unix；Windows GUI app 继承完整 PATH，跳过）
            #[cfg(unix)]
            if let Some(shell_path) = login_shell_path().await {
                entries.extend(std::env::split_paths(&shell_path));
            }

            // 3. 平台 well-known 目录兜底（不依赖 PATH / login-shell）
            entries.extend(well_known_dirs());

            merge_paths(entries)
        })
        .await
}

/// 合并 PATH 条目：按平台分隔符拼接，**保序去重**（`HashSet` 记首次出现路径）。
///
/// 三道过滤（顺序）：
/// 1. **只保留绝对路径**——丢弃 `.` / 相对条目，杜绝解析到 cwd 下同名程序（安全扩面）。
///    刻意用 `Path::is_absolute()` 而非 `cdt_discover::looks_like_absolute_path`：本函数
///    处理的是**当前运行平台**的真实本地 PATH 条目（进程 PATH / login-shell PATH(仅 unix)
///    / 当前平台 well-known 目录），不是 JSONL `cwd` / SSH / WSL 来的跨平台编码字符串；
///    跨平台版会在 Windows 上反而放进不可用的 POSIX `/foo` 条目。divergence 见 tasks.md。
/// 2. **保序去重**。
/// 3. **逐条剔除无法 `join` 的非法条目**（含平台分隔符，如 Unix 路径含 `:`）——避免
///    单个坏条目让 `join_paths` 整体失败、回退空串后丢光所有目录。
fn merge_paths(entries: impl IntoIterator<Item = PathBuf>) -> OsString {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let unique: Vec<PathBuf> = entries
        .into_iter()
        .filter(|p| p.is_absolute())
        .filter(|p| seen.insert(p.clone()))
        .filter(|p| std::env::join_paths(std::iter::once(p)).is_ok())
        .collect();
    // 每个条目已单独验证可 join → 整体 join 必成功。debug_assert 锁这个不变量：
    // 若未来有人删掉第 3 道 filter，debug 构建会炸出来，而非把"一个坏条目"无声
    // 放大成"PATH 全空 → 所有编辑器打不开"。
    let joined = std::env::join_paths(&unique);
    debug_assert!(
        joined.is_ok(),
        "merge_paths: 逐条过滤后整体 join 仍失败，必有未过滤的非法条目"
    );
    joined.unwrap_or_default()
}

/// 通过 login-shell 获取用户真实 PATH（仅 Unix）。
///
/// 读 `$SHELL` 后委托 `shell_path_via`。`$SHELL` 未设 → `None`。
#[cfg(unix)]
async fn login_shell_path() -> Option<OsString> {
    let Some(shell) = std::env::var_os("SHELL") else {
        tracing::debug!(
            reason = "shell_unset",
            "login-shell PATH 解析跳过；退回 well-known 目录"
        );
        return None;
    };
    shell_path_via(&shell).await
}

/// 跑 `<shell> -ilc 'printf <START>$PATH<END>'` 取交互式 PATH（shell-env/fix-path 思路）。
///
/// 三处稳健性（对应 codex 二审）：
/// - **随机 sentinel**：marker 含 pid + 纳秒 token，PATH 内容不可能撞上 → 杜绝 PATH
///   值含 END 字面量导致的提前截断；解析仍取**最后一个** START，跳过 rc 文件噪声。
/// - **显式超时回收**：2s 超时后显式 `start_kill()` + `wait()`，不依赖 `kill_on_drop`
///   的 best-effort orphan reaper，杜绝遗留 / zombie 子进程。
/// - 先 `read_to_end` 排空 stdout 再 `wait()`：持续消费管道，rc 文件喷 >64KB 也不会
///   写阻塞死锁（真撑满则被 2s 超时兜底）。
///
/// 任一失败（spawn / 超时 / 非 0 退出 / sentinel 缺失 / IO）→ `None`（best-effort，
/// 调用方有 well-known 目录 + 进程 PATH 兜底）。每条失败带 `reason` 落 `debug!`：
/// 这正是「用户编辑器装在非标准目录 + login-shell 解析失败 → 误报 not found」的诊断
/// 入口，按 `crates/CLAUDE.md::日志级别纪律` 用 `debug!`（best-effort 降级是常态噪声，
/// 非异常）——`cdt -vv` 可拉出 root cause，不污染默认输出。
#[cfg(unix)]
async fn shell_path_via(shell: &OsStr) -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt;
    use std::process::Stdio;
    use tokio::io::AsyncReadExt;

    let token = {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        format!("{:x}_{nanos:x}", std::process::id())
    };
    let start = format!("__CDT_{token}_S__");
    let end = format!("__CDT_{token}_E__");
    let script = format!("printf '{start}%s{end}' \"$PATH\"");

    let mut child = match tokio::process::Command::new(shell)
        .arg("-ilc")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true) // 兜底：意外 drop 路径仍杀子进程
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            tracing::debug!(shell = ?shell, reason = "spawn_failed", error = %e, "login-shell PATH 解析失败");
            return None;
        }
    };

    let Some(mut stdout) = child.stdout.take() else {
        tracing::debug!(reason = "no_stdout", "login-shell PATH 解析失败");
        return None;
    };

    let drain = async {
        let mut buf = Vec::new();
        stdout.read_to_end(&mut buf).await.ok()?;
        let status = child.wait().await.ok()?;
        Some((buf, status))
    };

    let (bytes, status) = match tokio::time::timeout(std::time::Duration::from_secs(2), drain).await
    {
        Ok(Some(pair)) => pair,
        Ok(None) => {
            tracing::debug!(reason = "io_error", "login-shell PATH 解析失败");
            return None;
        }
        Err(_) => {
            // 超时：显式 kill + reap，不留子进程
            let _ = child.start_kill();
            let _ = child.wait().await;
            tracing::debug!(shell = ?shell, reason = "timeout", "login-shell PATH 解析超时（2s）；退回 well-known 目录");
            return None;
        }
    };

    if !status.success() {
        tracing::debug!(shell = ?shell, reason = "nonzero_exit", code = ?status.code(), "login-shell PATH 解析失败");
        return None;
    }

    let Some(content) = extract_sentinel(&bytes, start.as_bytes(), end.as_bytes()) else {
        tracing::debug!(reason = "sentinel_missing", "login-shell PATH 解析失败");
        return None;
    };
    Some(OsString::from_vec(content))
}

/// 取 stdout 里**最后一个** `start` 标记之后、其后**首个** `end` 标记之前的字节。
///
/// 取最后一个 start 跳过 rc 文件在真实 `printf` 之前喷出的 start 字样噪声；
/// 任一 marker 缺失 → `None`。
#[cfg(unix)]
fn extract_sentinel(bytes: &[u8], start: &[u8], end: &[u8]) -> Option<Vec<u8>> {
    let start_pos = bytes.windows(start.len()).rposition(|w| w == start)?;
    let rest = &bytes[start_pos + start.len()..];
    let end_pos = rest.windows(end.len()).position(|w| w == end)?;
    Some(rest[..end_pos].to_vec())
}

/// 平台 well-known 可执行目录列表（不依赖 PATH / login-shell）。
///
/// Windows 返回空——GUI app 继承完整 PATH，无需兜底；空分支单独返回也避免
/// `let mut dirs` 在 Windows 下三个 cfg 块全空触发 `unused_mut`。
#[cfg(not(unix))]
fn well_known_dirs() -> Vec<PathBuf> {
    Vec::new()
}

/// 平台 well-known 可执行目录列表（不依赖 PATH / login-shell）。
#[cfg(unix)]
fn well_known_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/opt/local/bin"));
    }

    #[cfg(target_os = "linux")]
    {
        dirs.push(PathBuf::from("/usr/local/bin"));
        dirs.push(PathBuf::from("/snap/bin"));
        dirs.push(PathBuf::from("/var/lib/flatpak/exports/bin"));
    }

    // Unix home-based 目录（走 cdt_discover::home_dir() Windows 兼容四级 fallback）
    if let Some(home) = cdt_discover::home_dir() {
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join(".cargo/bin"));
        dirs.push(home.join("bin"));
    }

    dirs
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use tempfile::TempDir;

    // -------- merge_paths 保序去重 --------

    #[cfg(unix)]
    #[test]
    fn merge_paths_deduplicates_preserving_insertion_order() {
        let entries = vec![
            PathBuf::from("/usr/bin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"), // 重复
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"), // 重复
        ];
        let result = merge_paths(entries);
        let paths: Vec<PathBuf> = std::env::split_paths(&result).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/usr/bin"),
                PathBuf::from("/usr/local/bin"),
                PathBuf::from("/opt/homebrew/bin"),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn merge_paths_drops_relative_and_dot_entries() {
        // 相对条目（`.` / 相对路径）SHALL 被过滤，杜绝 cwd 同名程序注入
        let entries = vec![
            PathBuf::from("."),
            PathBuf::from("relative/bin"),
            PathBuf::from("/usr/local/bin"),
        ];
        let paths: Vec<PathBuf> = std::env::split_paths(&merge_paths(entries)).collect();
        assert_eq!(paths, vec![PathBuf::from("/usr/local/bin")]);
    }

    #[cfg(windows)]
    #[test]
    fn merge_paths_drops_relative_keeps_drive_absolute_on_windows() {
        // Windows：相对条目过滤，盘符绝对路径保留
        let entries = vec![
            PathBuf::from("."),
            PathBuf::from(r"relative\bin"),
            PathBuf::from(r"C:\Tools\bin"),
        ];
        let paths: Vec<PathBuf> = std::env::split_paths(&merge_paths(entries)).collect();
        assert_eq!(paths, vec![PathBuf::from(r"C:\Tools\bin")]);
    }

    #[cfg(unix)]
    #[test]
    fn merge_paths_skips_unjoinable_entry_keeps_rest() {
        // 含分隔符 `:` 的非法条目 SHALL 被单独剔除，其余目录保留（不全量清空）
        let entries = vec![
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/Users/a:b/.local/bin"), // 含 `:`，join_paths 单条会失败
            PathBuf::from("/opt/homebrew/bin"),
        ];
        let paths: Vec<PathBuf> = std::env::split_paths(&merge_paths(entries)).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/usr/local/bin"),
                PathBuf::from("/opt/homebrew/bin"),
            ]
        );
    }

    #[test]
    fn merge_paths_empty_input_returns_empty() {
        let result = merge_paths(std::iter::empty::<PathBuf>());
        assert!(result.is_empty());
    }

    // -------- resolve_in 命中 --------

    #[cfg(unix)]
    #[test]
    fn resolve_in_returns_absolute_path_when_hit() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let exe = dir.path().join("cdt_test_cli_hit");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        let mut perms = std::fs::metadata(&exe).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&exe, perms).unwrap();

        let search = std::env::join_paths([dir.path()]).unwrap();
        let cwd = dir.path();
        let result = resolve_in("cdt_test_cli_hit", &search, cwd);

        let p = Path::new(&result);
        assert!(p.is_absolute(), "expected absolute path, got: {result:?}");
        assert_eq!(p.file_name(), Some(OsStr::new("cdt_test_cli_hit")));
    }

    // -------- resolve_in 未命中 --------

    #[test]
    fn resolve_in_returns_bare_name_when_miss() {
        // 空 search_path 保证找不到任何程序
        let empty = std::env::join_paths::<_, PathBuf>([]).unwrap();
        let cwd = Path::new(".");
        let result = resolve_in("nonexistent_cli_xyz_abc_999", &empty, cwd);
        assert_eq!(result, OsString::from("nonexistent_cli_xyz_abc_999"));
    }

    // -------- login_shell_path 烟雾测试（CI 容错）--------

    #[cfg(unix)]
    #[tokio::test]
    async fn login_shell_path_returns_some_or_none_gracefully() {
        // SHELL 存在时期望 Some 且含 ≥1 目录；SHELL 不存在 / 超时 / 报错 → None，都接受
        let result = login_shell_path().await;
        if let Some(path) = result {
            let paths: Vec<_> = std::env::split_paths(&path).collect();
            assert!(
                !paths.is_empty(),
                "login_shell_path returned non-empty OsString but split gives empty vec"
            );
        }
        // None 也接受
    }

    // -------- extract_sentinel 边界 --------

    #[cfg(unix)]
    #[test]
    fn extract_sentinel_takes_content_between_markers() {
        let out = b"PREFIX<S>/usr/local/bin:/opt/homebrew/bin<E>SUFFIX";
        let got = extract_sentinel(out, b"<S>", b"<E>").unwrap();
        assert_eq!(got, b"/usr/local/bin:/opt/homebrew/bin");
    }

    #[cfg(unix)]
    #[test]
    fn extract_sentinel_uses_last_start_skipping_rc_noise() {
        // rc 文件在真实 printf 前喷出含 START 的噪声：取最后一个 START
        let out = b"<S>noise-from-rc\n<S>/real/path<E>";
        let got = extract_sentinel(out, b"<S>", b"<E>").unwrap();
        assert_eq!(got, b"/real/path");
    }

    #[cfg(unix)]
    #[test]
    fn extract_sentinel_returns_none_on_missing_marker() {
        assert!(extract_sentinel(b"no markers here", b"<S>", b"<E>").is_none());
        assert!(extract_sentinel(b"<S>only start", b"<S>", b"<E>").is_none());
        assert!(extract_sentinel(b"only end<E>", b"<S>", b"<E>").is_none());
    }

    // -------- shell_path_via 超时回收 --------

    #[cfg(unix)]
    #[tokio::test]
    async fn shell_path_via_times_out_on_slow_shell_without_hanging() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::Instant;

        // 临时"慢 shell"：忽略 -ilc 参数，sleep 远超 2s 超时
        let dir = TempDir::new().unwrap();
        let slow = dir.path().join("slow-shell");
        std::fs::write(&slow, "#!/bin/sh\nsleep 30\n").unwrap();
        let mut perms = std::fs::metadata(&slow).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&slow, perms).unwrap();

        let begin = Instant::now();
        let result = shell_path_via(slow.as_os_str()).await;
        let elapsed = begin.elapsed();

        assert!(result.is_none(), "slow shell SHALL time out to None");
        // 2s 超时 + kill/reap，应在远小于 sleep 30 的时间内返回（给宽松上限抗 CI 抖动）
        assert!(
            elapsed.as_secs() < 10,
            "SHALL return shortly after 2s timeout, took {elapsed:?}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shell_path_via_returns_path_via_real_shell() {
        // 成功链路：真 shell 跑 `-ilc 'printf <S>$PATH<E>'` → spawn → 读 stdout →
        // 提取 sentinel → Some(PATH)。CI unix runner 必有 /bin/sh，进程 PATH 必非空。
        let result = shell_path_via(OsStr::new("/bin/sh")).await;
        let path = result.expect("/bin/sh SHALL resolve to Some(PATH)");
        let dirs: Vec<_> = std::env::split_paths(&path).collect();
        assert!(
            !dirs.is_empty(),
            "resolved login-shell PATH SHALL be non-empty"
        );
    }

    // -------- well_known_dirs 平台兜底 --------

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn well_known_dirs_includes_usr_local_bin() {
        // Scenario「login-shell 失败 well-known 目录兜底」依赖 /usr/local/bin 在列表内
        assert!(
            well_known_dirs()
                .iter()
                .any(|p| p == Path::new("/usr/local/bin")),
            "well_known_dirs SHALL include /usr/local/bin on macOS/Linux"
        );
    }
}
