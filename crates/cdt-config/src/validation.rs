//! 配置字段校验。
//!
//! 对应 TS `configValidation.ts`。对 `update_config` 的 payload
//! 做分 section 校验，拦截无效值。

use std::collections::HashSet;

use cdt_discover::looks_like_absolute_path;

use crate::error::ConfigError;
use crate::types::{ConfigSection, SearchEngine, SshConfig, SshLastConnection, SshProfile};

/// 校验 HTTP 端口范围 1024–65535。
pub fn validate_http_port(port: u16) -> Result<(), ConfigError> {
    if port < 1024 {
        return Err(ConfigError::validation(
            "httpServer.port must be an integer between 1024 and 65535",
        ));
    }
    // u16 最大 65535，无需检查上限
    Ok(())
}

/// 标准化 `claude_root_path`：空路径 → `None`，绝对路径去尾斜杠。
///
/// 跨平台识别三种绝对路径形式（`Path::is_absolute()` 只认当前平台的风格，
/// Windows 上会拒绝 POSIX `/foo/bar`，但用户可能在 Windows 端配置 SSH 远端
/// POSIX 路径或 WSL 挂载路径 `/mnt/c/...`，必须接受）：
///
/// 1. POSIX：以 `/` 开头
/// 2. Windows 盘符：`[A-Za-z]:` 后接 `/` 或 `\`
/// 3. UNC：以 `\\` 或 `//` 开头
pub fn normalize_claude_root_path(value: Option<&str>) -> Option<String> {
    validate_claude_root_path(value).ok().flatten()
}

/// 校验并标准化 `claude_root_path`，用于用户更新路径。
pub fn validate_claude_root_path(value: Option<&str>) -> Result<Option<String>, ConfigError> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    // `~/`（Windows `~\`）前缀：保留原形不展开（可移植；展开推迟到消费点
    // `cdt_discover::resolve_claude_root_path`）。仅 trim 尾部分隔符。
    if is_tilde_prefixed(trimmed) {
        return Ok(Some(trimmed.trim_end_matches(['/', '\\']).to_owned()));
    }

    if !looks_like_absolute_path(trimmed) {
        return Err(ConfigError::validation(
            "general.claudeRootPath must be an absolute path, a ~/ path, or null",
        ));
    }

    if is_windows_drive_root(trimmed) {
        return Ok(Some(trimmed.to_owned()));
    }

    let stripped = trimmed.trim_end_matches(['/', '\\']);
    if stripped.is_empty() {
        return Ok(Some("/".into()));
    }

    Ok(Some(stripped.to_owned()))
}

fn is_windows_drive_root(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() == 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'/' || bytes[2] == b'\\')
}

/// `~/` / `~\`（Windows）前缀（紧跟分隔符）。`~user/` 具名 home 不算。
fn is_tilde_prefixed(s: &str) -> bool {
    s.starts_with("~/") || s.starts_with("~\\")
}

/// 数据根历史（`general.recentRoots`）条目上限。
pub const MAX_RECENT_ROOTS: usize = 8;

/// MRU 去重键：trim 尾分隔符 + 反斜杠归一正斜杠 + Windows 大小写不敏感。
/// 不做文件系统 canonicalize（详 change `flexible-data-root` 的 `design.md` D6）。
fn recent_root_dedup_key(s: &str) -> String {
    let unified = s.trim_end_matches(['/', '\\']).replace('\\', "/");
    if cfg!(windows) {
        unified.to_lowercase()
    } else {
        unified
    }
}

/// 过滤非法项 + 去重（保留首次出现），用于加载时清洗历史。
#[must_use]
pub fn sanitize_recent_roots(roots: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in roots {
        let Ok(Some(norm)) = validate_claude_root_path(Some(r)) else {
            tracing::warn!(entry = %r, "dropping invalid general.recentRoots entry");
            continue;
        };
        if seen.insert(recent_root_dedup_key(&norm)) {
            out.push(norm);
        }
    }
    out
}

/// 把新数据根加入 MRU 历史：最近在前、去重、过滤非法项、上限截断。
/// `new_root` 应为已 normalize 的合法值（来自 `validate_claude_root_path`）。
#[must_use]
pub fn push_recent_root(existing: &[String], new_root: &str) -> Vec<String> {
    let mut combined = Vec::with_capacity(existing.len() + 1);
    combined.push(new_root.to_owned());
    combined.extend(existing.iter().cloned());
    let mut sanitized = sanitize_recent_roots(&combined);
    sanitized.truncate(MAX_RECENT_ROOTS);
    sanitized
}

/// 校验 section 名是否合法。
pub fn validate_section(section: &str) -> Result<ConfigSection, ConfigError> {
    ConfigSection::from_str_key(section).ok_or_else(|| {
        ConfigError::validation(
            "Section must be one of: notifications, general, display, sessions, httpServer, ssh",
        )
    })
}

/// 校验 `SearchEngine`：仅 `Custom` variant 需要校验 (a) `url_template` 含 `{query}`
/// 占位符；(b) URL scheme ∈ `{http, https}`（拒绝 `javascript:` / `file:` / `data:`
/// / `chrome://` 等危险 scheme，防 XSS-into-opener 路径）。其它 variant 直接通过。
///
/// 详 `openspec/changes/frontend-context-menu-phase-2/design.md::D4` 决策与
/// `openspec/specs/configuration-management/spec.md::Validate configuration fields
/// before persistence` Requirement 的"`SearchEngine.custom` 缺 {query} 占位符拒绝"
/// 与"危险 scheme 拒绝"两个 Scenario。
pub fn validate_search_engine(engine: &SearchEngine) -> Result<(), ConfigError> {
    let SearchEngine::Custom { url_template } = engine else {
        return Ok(());
    };

    if !url_template.contains("{query}") {
        return Err(ConfigError::validation(
            "general.searchEngine.urlTemplate must contain {query} placeholder",
        ));
    }

    // scheme 校验：解析 `<scheme>:` 前缀，必须是 `http` / `https`（大小写不敏感）。
    // 不引入完整 url crate 依赖：本仓 cdt-config 走 minimal deps，单点 prefix 检查
    // 已足够拦截 `javascript:` / `data:` / `file:` / `chrome://` 等 scheme。
    let lower = url_template.trim_start().to_ascii_lowercase();
    let is_http = lower.starts_with("http://");
    let is_https = lower.starts_with("https://");
    if !(is_http || is_https) {
        return Err(ConfigError::validation(
            "general.searchEngine.urlTemplate scheme must be http or https",
        ));
    }
    Ok(())
}

/// 校验 snooze 分钟数（1–1440）。
pub fn validate_snooze_minutes(minutes: u32) -> Result<(), ConfigError> {
    if minutes == 0 || minutes > 1440 {
        return Err(ConfigError::validation(
            "notifications.snoozeMinutes must be between 1 and 1440",
        ));
    }
    Ok(())
}

pub fn validate_ssh_config(config: &SshConfig) -> Result<(), ConfigError> {
    let mut names = HashSet::new();
    for profile in &config.profiles {
        validate_ssh_profile(profile)?;
        let normalized = profile.name.trim();
        if !names.insert(normalized.to_owned()) {
            return Err(ConfigError::validation("ssh.profiles names must be unique"));
        }
    }
    if let Some(last) = &config.last_connection {
        validate_ssh_last_connection(last)?;
    }
    Ok(())
}

fn validate_ssh_profile(profile: &SshProfile) -> Result<(), ConfigError> {
    if profile.name.trim().is_empty() {
        return Err(ConfigError::validation(
            "ssh.profiles[].name must be non-empty",
        ));
    }
    validate_host(&profile.host, "ssh.profiles[].host")?;
    validate_port(profile.port, "ssh.profiles[].port")?;
    validate_username(&profile.username, "ssh.profiles[].username")
}

fn validate_ssh_last_connection(last: &SshLastConnection) -> Result<(), ConfigError> {
    validate_host(&last.host, "ssh.lastConnection.host")?;
    if let Some(port) = last.port {
        validate_port(port, "ssh.lastConnection.port")?;
    }
    if let Some(username) = &last.username {
        validate_username(username, "ssh.lastConnection.username")?;
    }
    Ok(())
}

fn validate_host(host: &str, field: &str) -> Result<(), ConfigError> {
    if host.trim().is_empty() {
        return Err(ConfigError::validation(format!(
            "{field} must be non-empty"
        )));
    }
    Ok(())
}

fn validate_username(username: &str, field: &str) -> Result<(), ConfigError> {
    if username.trim().is_empty() {
        return Err(ConfigError::validation(format!(
            "{field} must be non-empty"
        )));
    }
    Ok(())
}

fn validate_port(port: u16, field: &str) -> Result<(), ConfigError> {
    if port == 0 {
        return Err(ConfigError::validation(format!(
            "{field} must be an integer between 1 and 65535"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_port() {
        assert!(validate_http_port(3456).is_ok());
        assert!(validate_http_port(1024).is_ok());
        assert!(validate_http_port(65535).is_ok());
    }

    #[test]
    fn invalid_port_too_low() {
        assert!(validate_http_port(0).is_err());
        assert!(validate_http_port(80).is_err());
        assert!(validate_http_port(1023).is_err());
    }

    #[test]
    fn normalize_path_absolute() {
        let r = normalize_claude_root_path(Some("/Users/test/.claude"));
        assert_eq!(r, Some("/Users/test/.claude".into()));
    }

    #[test]
    fn normalize_path_trailing_slash() {
        let r = normalize_claude_root_path(Some("/Users/test/"));
        assert_eq!(r, Some("/Users/test".into()));
    }

    #[test]
    fn normalize_path_empty() {
        assert!(normalize_claude_root_path(Some("")).is_none());
        assert!(normalize_claude_root_path(Some("  ")).is_none());
        assert!(normalize_claude_root_path(None).is_none());
    }

    #[test]
    fn normalize_path_relative_rejected() {
        assert!(normalize_claude_root_path(Some("relative/path")).is_none());
        assert!(normalize_claude_root_path(Some("foo\\bar")).is_none());
        // 盘符后缺分隔符也不算合法
        assert!(normalize_claude_root_path(Some("C:")).is_none());
        assert!(normalize_claude_root_path(Some("C:relative")).is_none());
    }

    #[test]
    fn validate_path_relative_rejected() {
        assert!(validate_claude_root_path(Some("relative/path")).is_err());
        assert!(validate_claude_root_path(Some("foo\\bar")).is_err());
        assert!(validate_claude_root_path(Some("C:")).is_err());
        assert!(validate_claude_root_path(Some("C:relative")).is_err());
    }

    #[test]
    fn validate_path_empty_clears() {
        assert_eq!(validate_claude_root_path(Some("  ")).unwrap(), None);
        assert_eq!(validate_claude_root_path(None).unwrap(), None);
    }

    #[test]
    fn validate_path_tilde_accepted_verbatim() {
        assert_eq!(
            validate_claude_root_path(Some("~/.qoder")).unwrap(),
            Some("~/.qoder".into())
        );
        assert_eq!(
            validate_claude_root_path(Some(r"~\.qoder")).unwrap(),
            Some(r"~\.qoder".into())
        );
        // 尾分隔符 trim，原形保留
        assert_eq!(
            validate_claude_root_path(Some("~/.qoder/")).unwrap(),
            Some("~/.qoder".into())
        );
    }

    #[test]
    fn validate_path_named_home_tilde_rejected() {
        assert!(validate_claude_root_path(Some("~alice/data")).is_err());
    }

    #[test]
    fn push_recent_root_dedupes_and_orders_mru() {
        let existing = vec!["/a".to_owned(), "/b".to_owned()];
        // 重选已有项：去重 + 移到最前
        assert_eq!(
            push_recent_root(&existing, "/b"),
            vec!["/b".to_owned(), "/a".to_owned()]
        );
        // 新项前插
        assert_eq!(
            push_recent_root(&existing, "/c"),
            vec!["/c".to_owned(), "/a".to_owned(), "/b".to_owned()]
        );
    }

    #[test]
    fn push_recent_root_enforces_cap() {
        let existing: Vec<String> = (0..MAX_RECENT_ROOTS).map(|i| format!("/p{i}")).collect();
        let out = push_recent_root(&existing, "/new");
        assert_eq!(out.len(), MAX_RECENT_ROOTS);
        assert_eq!(out[0], "/new");
    }

    #[test]
    fn sanitize_recent_roots_filters_invalid_and_dedupes() {
        let roots = vec![
            "/valid".to_owned(),
            "relative/bad".to_owned(),
            "~alice/x".to_owned(),
            "/valid/".to_owned(),
            "~/.qoder".to_owned(),
        ];
        assert_eq!(
            sanitize_recent_roots(&roots),
            vec!["/valid".to_owned(), "~/.qoder".to_owned()]
        );
    }

    #[test]
    fn normalize_path_root() {
        let r = normalize_claude_root_path(Some("/"));
        assert_eq!(r, Some("/".into()));
    }

    #[test]
    fn normalize_path_windows_drive() {
        assert_eq!(
            normalize_claude_root_path(Some(r"C:\Users\alice\.claude")),
            Some(r"C:\Users\alice\.claude".into())
        );
        assert_eq!(
            normalize_claude_root_path(Some("C:/Users/alice/.claude")),
            Some("C:/Users/alice/.claude".into())
        );
    }

    #[test]
    fn normalize_path_windows_trailing_backslash() {
        assert_eq!(
            normalize_claude_root_path(Some(r"C:\Users\alice\")),
            Some(r"C:\Users\alice".into())
        );
    }

    #[test]
    fn normalize_path_windows_drive_root_keeps_separator() {
        assert_eq!(
            normalize_claude_root_path(Some(r"C:\")),
            Some(r"C:\".into())
        );
        assert_eq!(normalize_claude_root_path(Some("D:/")), Some("D:/".into()));
    }

    #[test]
    fn normalize_path_unc() {
        assert_eq!(
            normalize_claude_root_path(Some(r"\\server\share\dir")),
            Some(r"\\server\share\dir".into())
        );
    }

    #[test]
    fn validate_section_valid() {
        assert!(validate_section("notifications").is_ok());
        assert!(validate_section("general").is_ok());
        assert!(validate_section("httpServer").is_ok());
    }

    #[test]
    fn validate_section_invalid() {
        assert!(validate_section("invalid").is_err());
        assert!(validate_section("").is_err());
    }

    #[test]
    fn validate_snooze_valid() {
        assert!(validate_snooze_minutes(1).is_ok());
        assert!(validate_snooze_minutes(30).is_ok());
        assert!(validate_snooze_minutes(1440).is_ok());
    }

    #[test]
    fn validate_snooze_invalid() {
        assert!(validate_snooze_minutes(0).is_err());
        assert!(validate_snooze_minutes(1441).is_err());
    }

    // =========================================================================
    // SearchEngine 校验
    // =========================================================================

    #[test]
    fn validate_search_engine_unit_variants_pass() {
        assert!(validate_search_engine(&SearchEngine::Google).is_ok());
        assert!(validate_search_engine(&SearchEngine::Bing).is_ok());
        assert!(validate_search_engine(&SearchEngine::DuckDuckGo).is_ok());
    }

    #[test]
    fn validate_search_engine_custom_with_query_placeholder_pass() {
        let engine = SearchEngine::Custom {
            url_template: "https://example.com/search?q={query}".into(),
        };
        assert!(validate_search_engine(&engine).is_ok());
    }

    #[test]
    fn validate_search_engine_custom_http_scheme_pass() {
        let engine = SearchEngine::Custom {
            url_template: "http://intranet.local/?q={query}".into(),
        };
        assert!(validate_search_engine(&engine).is_ok());
    }

    #[test]
    fn validate_search_engine_custom_missing_query_rejected() {
        let engine = SearchEngine::Custom {
            url_template: "https://example.com/search".into(),
        };
        let err = validate_search_engine(&engine).unwrap_err();
        assert!(err.to_string().contains("{query}"));
    }

    #[test]
    fn validate_search_engine_custom_javascript_scheme_rejected() {
        let engine = SearchEngine::Custom {
            url_template: "javascript:alert({query})".into(),
        };
        let err = validate_search_engine(&engine).unwrap_err();
        assert!(err.to_string().contains("scheme"));
    }

    #[test]
    fn validate_search_engine_custom_data_scheme_rejected() {
        let engine = SearchEngine::Custom {
            url_template: "data:text/html,{query}".into(),
        };
        assert!(validate_search_engine(&engine).is_err());
    }

    #[test]
    fn validate_search_engine_custom_file_scheme_rejected() {
        let engine = SearchEngine::Custom {
            url_template: "file:///etc/passwd?q={query}".into(),
        };
        assert!(validate_search_engine(&engine).is_err());
    }

    #[test]
    fn validate_search_engine_custom_chrome_scheme_rejected() {
        let engine = SearchEngine::Custom {
            url_template: "chrome://settings/?q={query}".into(),
        };
        assert!(validate_search_engine(&engine).is_err());
    }

    #[test]
    fn validate_search_engine_custom_uppercase_scheme_pass() {
        let engine = SearchEngine::Custom {
            url_template: "HTTPS://example.com/?q={query}".into(),
        };
        assert!(validate_search_engine(&engine).is_ok());
    }

    #[test]
    fn validate_search_engine_custom_leading_whitespace_normalized() {
        let engine = SearchEngine::Custom {
            url_template: "  https://example.com/?q={query}".into(),
        };
        assert!(validate_search_engine(&engine).is_ok());
    }
}
