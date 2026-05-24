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

    if !looks_like_absolute_path(trimmed) {
        return Err(ConfigError::validation(
            "general.claudeRootPath must be an absolute path or null",
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
