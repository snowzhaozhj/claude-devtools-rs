//! 配置字段校验。
//!
//! 对应 TS `configValidation.ts`。对 `update_config` 的 payload
//! 做分 section 校验，拦截无效值。

use std::path::Path;

use crate::error::ConfigError;
use crate::types::ConfigSection;

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

/// 标准化 `claude_root_path`：空/非绝对路径 → `None`，去尾斜杠。
pub fn normalize_claude_root_path(value: Option<&str>) -> Option<String> {
    let raw = value?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return None;
    }

    // 去尾斜杠（保留根目录 `/`）
    let s = trimmed.to_owned();
    let stripped = s.trim_end_matches(['/', '\\']);
    if stripped.is_empty() {
        return Some("/".into());
    }

    Some(stripped.to_owned())
}

/// 校验 section 名是否合法。
pub fn validate_section(section: &str) -> Result<ConfigSection, ConfigError> {
    ConfigSection::from_str_key(section).ok_or_else(|| {
        ConfigError::validation(
            "Section must be one of: notifications, general, display, sessions, httpServer, ssh",
        )
    })
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
    }

    #[test]
    fn normalize_path_root() {
        let r = normalize_claude_root_path(Some("/"));
        assert_eq!(r, Some("/".into()));
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
}
