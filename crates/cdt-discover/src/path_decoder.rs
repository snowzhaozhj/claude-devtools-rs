//! 编码目录名 / 项目 ID 的纯函数工具。
//!
//! Claude Code 把 "cwd" 编码成 `~/.claude/projects/` 下的目录名。规则：把 `/`
//! 和 `\` 都换成 `-`，强制加 leading `-`。Windows 路径的盘符冒号保留。解码是
//! "每个 leading `-` 换回 `/`" 的 best-effort —— 当路径本身含 `-` 时会歧义，
//! 真实 cwd 必须从 session JSONL 里的 `cwd` 字段读取（由 `ProjectPathResolver`
//! 完成）。Windows 上还识别 legacy 格式 `C--Users-alice-app` 与 WSL mount
//! `/mnt/c/...` 转 `C:/...`。
//!
//! Spec 行为参考：`openspec/specs/project-discovery/spec.md` 的
//! `Decode encoded project paths` / `Encode absolute paths into directory names`
//! / `Scan Claude projects directory` Requirements。

use std::path::{Path, PathBuf};

/// 把绝对路径编码成 `~/.claude/projects/` 下的目录名。
///
/// 规则（对齐 TS `pathDecoder.ts::encodePath`）：
/// 1. 同时把 `/` 与 `\` 替换为 `-`（Windows 路径可能混用两种分隔符）
/// 2. 保留盘符冒号（`C:` 原样）
/// 3. 强制加 leading `-`：若输入本身以分隔符开头替换后已是 `-...` 不再加
///
/// 这是跨 crate 唯一的 encode 实现源；`cdt-config::claude_md` 等调用方 SHALL 通过
/// `cdt_discover::path_decoder::encode_path` 调用，不要再写私有副本。
#[must_use]
pub fn encode_path(absolute_path: &str) -> String {
    if absolute_path.is_empty() {
        return String::new();
    }
    let replaced: String = absolute_path
        .chars()
        .map(|c| if c == '/' || c == '\\' { '-' } else { c })
        .collect();
    if replaced.starts_with('-') {
        replaced
    } else {
        format!("-{replaced}")
    }
}

/// 把编码后的目录名还原成 best-effort 的文件系统路径。
///
/// 识别三种格式（对齐 TS `pathDecoder.ts::decodePath`）：
/// 1. Legacy Windows `C--Users-alice-app` → `C:/Users/alice/app`
/// 2. 新 Windows `-C:-Users-alice-app` → `C:/Users/alice/app`（不加 POSIX `/`）
/// 3. POSIX `-Users-alice-app` → `/Users/alice/app`
///
/// Windows 平台上额外把 `/mnt/c/code` 转 `C:/code`（WSL mount）。
#[must_use]
pub fn decode_path(encoded: &str) -> PathBuf {
    if encoded.is_empty() {
        return PathBuf::new();
    }

    if let Some(legacy) = decode_legacy_windows(encoded) {
        return PathBuf::from(legacy);
    }

    let trimmed = encoded.strip_prefix('-').unwrap_or(encoded);
    let replaced: String = trimmed.replace('-', "/");

    // 新 Windows 格式 `C:/Users/...`：直接返回，不加 POSIX `/` 前缀
    if is_windows_drive_path(&replaced) {
        return PathBuf::from(replaced);
    }

    let absolute = if replaced.starts_with('/') {
        replaced
    } else {
        format!("/{replaced}")
    };

    PathBuf::from(translate_wsl_mount(&absolute))
}

fn decode_legacy_windows(encoded: &str) -> Option<String> {
    let bytes = encoded.as_bytes();
    if bytes.len() < 4 {
        return None;
    }
    let first = bytes[0];
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if bytes[1] != b'-' || bytes[2] != b'-' {
        return None;
    }
    let drive = (first as char).to_ascii_uppercase();
    let rest = &encoded[3..];
    if rest.is_empty() {
        return None;
    }
    let slashed: String = rest.replace('-', "/");
    Some(format!("{drive}:/{slashed}"))
}

fn is_windows_drive_path(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/'
}

/// WSL mount 路径转 Windows 盘符（仅 Windows 平台启用）。
#[cfg(target_os = "windows")]
fn translate_wsl_mount(posix: &str) -> String {
    let bytes = posix.as_bytes();
    if bytes.len() < 7 {
        return posix.to_owned();
    }
    if !posix.starts_with("/mnt/") {
        return posix.to_owned();
    }
    let drive_byte = bytes[5];
    if !drive_byte.is_ascii_alphabetic() {
        return posix.to_owned();
    }
    let after = bytes.get(6);
    if !matches!(after, None | Some(b'/')) {
        return posix.to_owned();
    }
    let drive = (drive_byte as char).to_ascii_uppercase();
    let rest = &posix[6..];
    format!("{drive}:{rest}")
}

#[cfg(not(target_os = "windows"))]
fn translate_wsl_mount(posix: &str) -> String {
    posix.to_owned()
}

/// 跨平台识别绝对路径。
///
/// 标准库 `Path::is_absolute()` 只认当前平台的风格（Windows 上拒绝 POSIX
/// `/foo/bar`、POSIX 上拒绝 Windows `C:\...`），但本项目里多处需要同时接受
/// 两种风格：用户可能在 Windows 端配置 WSL `/mnt/c/...` 或 SSH 远端 POSIX
/// 路径；JSONL 里的 `cwd` 字段来自 Claude Code CLI 的运行环境，Windows CLI
/// 写 `C:\...`，macOS/Linux/WSL CLI 写 POSIX。
///
/// 识别三种形式：
/// 1. POSIX：以 `/` 开头
/// 2. Windows 盘符：`[A-Za-z]:` 后接 `/` 或 `\`
/// 3. UNC：以 `\\` 或 `//` 开头（第 1 条已覆盖 `//`）
#[must_use]
pub fn looks_like_absolute_path(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    if bytes[0] == b'/' || bytes[0] == b'\\' {
        return true;
    }
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// 从任意 project ID 抽出 `baseDir`。
///
/// 历史上同一编码目录下不同 `cwd` 的 session 会被拆分为多个 composite project
/// （id 形如 `{baseDir}::{hash8}`）；现已通过 change `merge-composite-projects`
/// 移除该拆分。本函数保留向后兼容语义：含 `::` 的旧 ID 仍能抽出 `base_dir`，
/// 使配置文件 / 持久化 IPC 缓存里残留的 composite key 在过渡期不致 panic。
#[must_use]
pub fn extract_base_dir(project_id: &str) -> &str {
    match project_id.find("::") {
        Some(idx) => &project_id[..idx],
        None => project_id,
    }
}

/// 从 `Path` 里拿最后一段作为展示名。
#[must_use]
pub fn extract_project_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |s| s.to_string_lossy().into_owned(),
    )
}

/// 从项目目录中 session JSONL 读取 `cwd` 字段来消除 `decode_path` 歧义。
///
/// 按 mtime 倒序尝试多个 JSONL（最新优先），每个只读头部 20 行。
/// 返回 `None` 时调用方应 fallback 到 `extract_project_name(&decode_path(encoded))`。
pub fn resolve_project_name_from_jsonl(project_dir: &Path) -> Option<String> {
    const MAX_ATTEMPTS: usize = 3;

    let mut jsonl_files: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(project_dir).ok()?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        jsonl_files.push((mtime, path));
    }

    jsonl_files.sort_unstable_by_key(|item| std::cmp::Reverse(item.0));

    for (_, jsonl_path) in jsonl_files.iter().take(MAX_ATTEMPTS) {
        if let Some(name) = extract_cwd_from_jsonl_head(jsonl_path) {
            return Some(name);
        }
    }
    None
}

fn extract_cwd_from_jsonl_head(jsonl_path: &Path) -> Option<String> {
    use std::io::{BufRead, BufReader};

    let file = std::fs::File::open(jsonl_path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(20) {
        let Ok(line) = line else { continue };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if let Some(cwd) = value.get("cwd").and_then(|v| v.as_str()) {
            if !cwd.is_empty() {
                return Some(extract_project_name(Path::new(cwd)));
            }
        }
    }
    None
}

/// 是否是合法的 Claude Code 编码目录名。
///
/// 两种合法形式（对齐 TS `pathDecoder.ts::isValidEncodedPath` 的 regex）：
/// - 新格式：以 `-` 开头（POSIX 或 Windows `-C:-...`）
/// - Legacy Windows：`^[A-Za-z]--[A-Za-z0-9_.\s-]+$` 形式（如 `C--Users-foo`）
#[must_use]
pub fn is_valid_encoded_path(name: &str) -> bool {
    if name.starts_with('-') {
        return true;
    }
    let bytes = name.as_bytes();
    if bytes.len() < 4 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b'-' || bytes[2] != b'-' {
        return false;
    }
    let rest = &name[3..];
    // rest 非空且不以 `-` 起头（避免 `C---x` 这种三连字符被误判）
    if rest.is_empty() || rest.starts_with('-') {
        return false;
    }
    // rest 必须是 `[A-Za-z0-9_.\s-]+` 字符集
    rest.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') || c.is_whitespace())
}

#[must_use]
pub fn is_worktree_encoded_path(encoded: &str) -> bool {
    split_worktree_encoded_path(encoded).is_some()
}

pub fn split_worktree_encoded_path(encoded: &str) -> Option<(&str, &str)> {
    encoded
        .split_once("-.claude-worktrees-")
        .or_else(|| encoded.split_once("--claude-worktrees-"))
}

/// `~/.claude/` —— 根据 home 目录动态解析。
#[must_use]
pub fn get_claude_base_path() -> PathBuf {
    home_dir().unwrap_or_else(fallback_home).join(".claude")
}

/// 把 Claude root 配置值解析成绝对基路径：`~/`（Windows `~\`）前缀展开到
/// home，绝对路径原样，`None` 回退默认 `~/.claude`。
///
/// **数据读取侧唯一的 tilde 展开点**——所有消费 `claudeRootPath` 的路径
/// （`projects/` / `todos/` / CLAUDE.md 与 auto-memory 基路径）SHALL 经此函数，
/// 保证一致。配置持久化层保留 `~/` 原形（详 `cdt-config::validate_claude_root_path`）。
#[must_use]
pub fn resolve_claude_root_path(claude_root_path: Option<&Path>) -> PathBuf {
    match claude_root_path {
        Some(p) => expand_tilde_root(p),
        None => get_claude_base_path(),
    }
}

/// `~/` / `~\`（Windows）前缀（紧跟分隔符）展开到 home 目录；其余原样。
/// `~user/` 具名 home 不展开（校验层已拒），与 `mention` 对 `~user` 的口径一致。
fn expand_tilde_root(p: &Path) -> PathBuf {
    if let Some(s) = p.to_str() {
        // 裸 `~`（`~/` / `~\` 经校验 trim 尾分隔符后退化的形态）= home 本身，
        // 避免落到 `p.to_path_buf()` 当相对 `~` 目录静默扫错（silent-failure 二审）。
        if s == "~" {
            return home_dir().unwrap_or_else(fallback_home);
        }
        if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
            return home_dir().unwrap_or_else(fallback_home).join(rest);
        }
    }
    p.to_path_buf()
}

/// Claude root 下的 `projects/`。
#[must_use]
pub fn projects_base_path_for(claude_root_path: Option<&Path>) -> PathBuf {
    resolve_claude_root_path(claude_root_path).join("projects")
}

/// Claude root 下的 `todos/`。
#[must_use]
pub fn todos_base_path_for(claude_root_path: Option<&Path>) -> PathBuf {
    resolve_claude_root_path(claude_root_path).join("todos")
}

/// `~/.claude/projects/` —— 根据 home 目录动态解析。
#[must_use]
pub fn get_projects_base_path() -> PathBuf {
    projects_base_path_for(None)
}

/// `~/.claude/todos/` —— 动态解析。
#[must_use]
pub fn get_todos_base_path() -> PathBuf {
    todos_base_path_for(None)
}

/// Home 目录四级 fallback（对齐 TS `pathDecoder.ts::getHomeDir`）：
/// `HOME` → `USERPROFILE` → `HOMEDRIVE`+`HOMEPATH` → `dirs::home_dir()`。
///
/// 允许 WSL / Git Bash / Cygwin 用户通过 `HOME` 覆盖，同时 Windows native 上
/// 仍能通过 `USERPROFILE` 或 `HOMEDRIVE+HOMEPATH` 定位到 `%USERPROFILE%\.claude\`。
///
/// 跨 crate 共享入口 —— 其他 crate 在解析 `~` 或 `~/.claude/` 基础路径时 SHALL
/// 调用此函数而非直接用 `dirs::home_dir()`，保证 Windows fallback 行为一致。
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    resolve_home_dir(|k| std::env::var(k).ok(), dirs::home_dir)
}

/// 纯函数版 home 解析，供单测注入 env。`env(key)` 返 `Some("")` 视为未设置。
fn resolve_home_dir<E, F>(env: E, fallback: F) -> Option<PathBuf>
where
    E: Fn(&str) -> Option<String>,
    F: FnOnce() -> Option<PathBuf>,
{
    let non_empty = |k: &str| env(k).filter(|v| !v.is_empty());
    if let Some(v) = non_empty("HOME") {
        return Some(PathBuf::from(v));
    }
    if let Some(v) = non_empty("USERPROFILE") {
        return Some(PathBuf::from(v));
    }
    if let (Some(drive), Some(path)) = (non_empty("HOMEDRIVE"), non_empty("HOMEPATH")) {
        return Some(PathBuf::from(format!("{drive}{path}")));
    }
    fallback()
}

fn fallback_home() -> PathBuf {
    // 所有 env 都缺且 `dirs::home_dir()` 返 None 时的兜底：Windows 上用当前盘符根
    // （`C:\`），其他平台用 `/`。比之前无脑返回 `/` 更合理，但实际触发场景极罕见。
    if cfg!(target_os = "windows") {
        PathBuf::from(r"C:\")
    } else {
        PathBuf::from("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 用 `HashMap` 注入 env 的小帮手 —— 避免动进程级 env（workspace `forbid(unsafe_code)`）。
    fn env_from(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k| map.get(k).cloned()
    }

    #[test]
    fn resolve_tilde_root_expands_to_home() {
        let home = home_dir().unwrap_or_else(fallback_home);
        assert_eq!(
            resolve_claude_root_path(Some(Path::new("~/.qoder"))),
            home.join(".qoder")
        );
        // Windows 反斜杠形式等价
        assert_eq!(
            resolve_claude_root_path(Some(Path::new(r"~\.qoder"))),
            home.join(".qoder")
        );
    }

    #[test]
    fn resolve_absolute_root_verbatim() {
        assert_eq!(
            resolve_claude_root_path(Some(Path::new("/data/alt"))),
            PathBuf::from("/data/alt")
        );
    }

    #[test]
    fn resolve_bare_and_slash_tilde_to_home() {
        // 裸 `~`（校验产出）与 `~/` 都 SHALL 展开到 home 本身，不落相对路径
        let home = home_dir().unwrap_or_else(fallback_home);
        assert_eq!(resolve_claude_root_path(Some(Path::new("~"))), home.clone());
        assert_eq!(resolve_claude_root_path(Some(Path::new("~/"))), home);
    }

    #[test]
    fn resolve_named_home_tilde_not_expanded() {
        // `~alice/` 不展开（当字面路径；校验层已拒此形态入 config）
        assert_eq!(
            resolve_claude_root_path(Some(Path::new("~alice/data"))),
            PathBuf::from("~alice/data")
        );
    }

    #[test]
    fn projects_and_todos_share_tilde_expansion() {
        let home = home_dir().unwrap_or_else(fallback_home);
        let p = Path::new("~/.qoder");
        assert_eq!(
            projects_base_path_for(Some(p)),
            home.join(".qoder").join("projects")
        );
        assert_eq!(
            todos_base_path_for(Some(p)),
            home.join(".qoder").join("todos")
        );
    }

    fn none_fallback() -> Option<PathBuf> {
        None
    }

    #[test]
    fn standard_encoded_name_decodes() {
        assert_eq!(
            decode_path("-Users-alice-code-app"),
            PathBuf::from("/Users/alice/code/app")
        );
    }

    #[test]
    fn ambiguous_name_is_best_effort() {
        // `-Users-alice-my-app` 里的中间 `-app` 无从消解，按 spec 返回全 slash 版本。
        assert_eq!(
            decode_path("-Users-alice-my-app"),
            PathBuf::from("/Users/alice/my/app")
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn wsl_mount_path_survives_roundtrip_on_unix() {
        assert_eq!(decode_path("-mnt-c-code"), PathBuf::from("/mnt/c/code"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn wsl_mount_path_translates_on_windows() {
        assert_eq!(decode_path("-mnt-c-code"), PathBuf::from("C:/code"));
    }

    #[test]
    fn new_windows_format_decodes_to_drive_path() {
        assert_eq!(
            decode_path("-C:-Users-alice-app"),
            PathBuf::from("C:/Users/alice/app")
        );
    }

    #[test]
    fn legacy_windows_format_decodes() {
        assert_eq!(
            decode_path("C--Users-alice-app"),
            PathBuf::from("C:/Users/alice/app")
        );
    }

    #[test]
    fn legacy_windows_format_uppercases_drive() {
        assert_eq!(decode_path("d--code-repo"), PathBuf::from("D:/code/repo"));
    }

    #[test]
    fn encode_posix_path() {
        assert_eq!(
            encode_path("/Users/alice/code/app"),
            "-Users-alice-code-app"
        );
    }

    #[test]
    fn encode_windows_backslash_path() {
        assert_eq!(encode_path(r"C:\Users\alice\app"), "-C:-Users-alice-app");
    }

    #[test]
    fn encode_windows_forward_slash_path() {
        assert_eq!(encode_path("C:/Users/alice/app"), "-C:-Users-alice-app");
    }

    #[test]
    fn encode_mixed_separators() {
        assert_eq!(encode_path(r"C:\a/b\c"), "-C:-a-b-c");
    }

    #[test]
    fn encode_empty_input() {
        assert_eq!(encode_path(""), "");
    }

    #[test]
    fn roundtrip_windows_path_forward_slash() {
        let original = "C:/Users/alice/app";
        assert_eq!(decode_path(&encode_path(original)), PathBuf::from(original));
    }

    #[test]
    fn roundtrip_posix_path() {
        let original = "/Users/alice/app";
        assert_eq!(decode_path(&encode_path(original)), PathBuf::from(original));
    }

    #[test]
    fn extract_base_dir_handles_composite_and_plain() {
        assert_eq!(extract_base_dir("-Users-foo::abcd1234"), "-Users-foo");
        assert_eq!(extract_base_dir("-Users-foo"), "-Users-foo");
    }

    #[test]
    fn is_valid_encoded_path_requires_leading_hyphen_or_legacy() {
        assert!(is_valid_encoded_path("-foo"));
        assert!(is_valid_encoded_path("C--Users-foo"));
        assert!(is_valid_encoded_path("d--code"));
        assert!(is_valid_encoded_path("C--a_1.2 3-x")); // 合法字符集（下划线 / 点 / 空格 / 连字符）
        assert!(!is_valid_encoded_path("foo"));
        assert!(!is_valid_encoded_path(""));
        // 三段破折号（legacy rest 以 `-` 起头）不应被误判
        assert!(!is_valid_encoded_path("C---x"));
        // 单字母（只有盘符）无意义
        assert!(!is_valid_encoded_path("C--"));
        // rest 段含非 ASCII 字符应拒绝（Claude Code 的编码目录名只产生 ASCII）
        assert!(!is_valid_encoded_path("A--测试"));
        assert!(!is_valid_encoded_path("B--path/slash")); // rest 段不允许 `/`
        // 盘符必须是字母开头
        assert!(!is_valid_encoded_path("1--foo"));
    }

    #[test]
    fn extract_project_name_returns_last_segment() {
        assert_eq!(
            extract_project_name(&PathBuf::from("/Users/alice/app")),
            "app"
        );
    }

    // --- resolve_home_dir fallback 测试 ---
    //
    // 通过 `env_from` mock 注入测试用 env，避免动进程级 env（workspace
    // `forbid(unsafe_code)` 禁止 `set_var` / `remove_var`）。

    #[test]
    fn resolve_home_dir_prefers_home_over_userprofile() {
        let env = env_from(&[("HOME", "/custom/home"), ("USERPROFILE", r"C:\Users\bob")]);
        assert_eq!(
            resolve_home_dir(env, none_fallback),
            Some(PathBuf::from("/custom/home"))
        );
    }

    #[test]
    fn resolve_home_dir_falls_back_to_userprofile() {
        let env = env_from(&[("USERPROFILE", r"C:\Users\alice")]);
        assert_eq!(
            resolve_home_dir(env, none_fallback),
            Some(PathBuf::from(r"C:\Users\alice"))
        );
    }

    #[test]
    fn resolve_home_dir_falls_back_to_homedrive_homepath() {
        let env = env_from(&[("HOMEDRIVE", "C:"), ("HOMEPATH", r"\Users\alice")]);
        assert_eq!(
            resolve_home_dir(env, none_fallback),
            Some(PathBuf::from(r"C:\Users\alice"))
        );
    }

    #[test]
    fn resolve_home_dir_treats_empty_env_as_missing() {
        let env = env_from(&[("HOME", ""), ("USERPROFILE", r"C:\Users\alice")]);
        assert_eq!(
            resolve_home_dir(env, none_fallback),
            Some(PathBuf::from(r"C:\Users\alice"))
        );
    }

    #[test]
    fn resolve_home_dir_returns_fallback_when_all_env_missing() {
        let env = env_from(&[]);
        let fallback = || Some(PathBuf::from("/fallback/home"));
        assert_eq!(
            resolve_home_dir(env, fallback),
            Some(PathBuf::from("/fallback/home"))
        );
    }

    #[test]
    fn projects_base_path_uses_custom_claude_root() {
        assert_eq!(
            projects_base_path_for(Some(Path::new("/data/claude-alt"))),
            PathBuf::from("/data/claude-alt/projects")
        );
    }

    #[test]
    fn todos_base_path_uses_custom_claude_root() {
        assert_eq!(
            todos_base_path_for(Some(Path::new("/data/claude-alt"))),
            PathBuf::from("/data/claude-alt/todos")
        );
    }

    #[test]
    fn resolve_project_name_from_jsonl_extracts_cwd() {
        let tmp = std::env::temp_dir().join("cdt_test_jsonl_resolve");
        std::fs::create_dir_all(&tmp).unwrap();

        let jsonl_content = r#"{"type":"user","cwd":"/Users/alice/code/claude-devtools-rs","message":{"content":"hi"}}"#;
        std::fs::write(tmp.join("session1.jsonl"), jsonl_content).unwrap();

        let result = resolve_project_name_from_jsonl(&tmp);
        assert_eq!(result, Some("claude-devtools-rs".to_string()));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn resolve_project_name_from_jsonl_returns_none_for_empty_dir() {
        let tmp = std::env::temp_dir().join("cdt_test_jsonl_empty");
        std::fs::create_dir_all(&tmp).unwrap();

        let result = resolve_project_name_from_jsonl(&tmp);
        assert_eq!(result, None);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn resolve_project_name_from_jsonl_returns_none_if_no_cwd() {
        let tmp = std::env::temp_dir().join("cdt_test_jsonl_nocwd");
        std::fs::create_dir_all(&tmp).unwrap();

        let jsonl_content = r#"{"type":"user","message":{"content":"hi"}}"#;
        std::fs::write(tmp.join("session1.jsonl"), jsonl_content).unwrap();

        let result = resolve_project_name_from_jsonl(&tmp);
        assert_eq!(result, None);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn resolve_project_name_from_jsonl_skips_malformed_lines() {
        let tmp = std::env::temp_dir().join("cdt_test_jsonl_malformed");
        std::fs::create_dir_all(&tmp).unwrap();

        let content = "not valid json\n{\"broken\n{\"cwd\":\"/Users/bob/my-project\"}\n";
        std::fs::write(tmp.join("s.jsonl"), content).unwrap();

        let result = resolve_project_name_from_jsonl(&tmp);
        assert_eq!(result, Some("my-project".to_string()));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn resolve_project_name_from_jsonl_falls_through_to_older_file() {
        let tmp = std::env::temp_dir().join("cdt_test_jsonl_fallthrough");
        std::fs::create_dir_all(&tmp).unwrap();

        // Older file has valid cwd
        let old_path = tmp.join("old.jsonl");
        std::fs::write(&old_path, r#"{"cwd":"/home/user/real-project"}"#).unwrap();
        // Set old mtime
        let old_time =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        filetime::set_file_mtime(&old_path, filetime::FileTime::from_system_time(old_time)).ok();

        // Newer file has no cwd (e.g. being written)
        let new_path = tmp.join("new.jsonl");
        std::fs::write(&new_path, "{\"type\":\"system\"}\n").unwrap();

        let result = resolve_project_name_from_jsonl(&tmp);
        assert_eq!(result, Some("real-project".to_string()));

        std::fs::remove_dir_all(&tmp).ok();
    }
}
