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
async fn augmented_path() -> &'static OsString {
    AUGMENTED_PATH
        .get_or_init(|| async { build_augmented_path().await })
        .await
}

async fn build_augmented_path() -> OsString {
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
}

/// 合并 PATH 条目：按平台分隔符拼接，**保序去重**（`HashSet` 记首次出现路径）。
fn merge_paths(entries: impl IntoIterator<Item = PathBuf>) -> OsString {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let unique: Vec<PathBuf> = entries
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect();
    // join_paths 在 Unix 路径含 `:` 时可能失败（极罕见）；失败回退空串
    std::env::join_paths(unique).unwrap_or_default()
}

/// 通过 login-shell 获取用户真实 PATH（仅 Unix）。
///
/// 用 sentinel `__CDT_PATH_START__...__CDT_PATH_END__` 包裹，过滤 rc 文件
/// 往 stdout 喷出的噪声（shell-env/fix-path 标准做法）。
/// 2 秒超时 + 非 0 退出 + spawn 失败 → 返回 `None`（best-effort）。
#[cfg(unix)]
async fn login_shell_path() -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt;
    use std::process::Stdio;

    // 常量提到函数顶部（clippy::items_after_statements）
    const SCRIPT: &str = "printf '__CDT_PATH_START__%s__CDT_PATH_END__' \"$PATH\"";
    const START: &[u8] = b"__CDT_PATH_START__";
    const END: &[u8] = b"__CDT_PATH_END__";

    let shell = std::env::var_os("SHELL")?;

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::process::Command::new(&shell)
            .arg("-ilc")
            .arg(SCRIPT)
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .ok()? // timeout Elapsed → None
    .ok()?; // IO spawn error → None

    if !output.status.success() {
        return None;
    }

    let bytes = &output.stdout;
    let start_pos = bytes.windows(START.len()).position(|w| w == START)?;
    let content_start = start_pos + START.len();
    let rest = &bytes[content_start..];
    let end_pos = rest.windows(END.len()).position(|w| w == END)?;

    Some(OsString::from_vec(rest[..end_pos].to_vec()))
}

/// 平台 well-known 可执行目录列表（不依赖 PATH / login-shell）。
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
    #[cfg(unix)]
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
}
