//! Teammate 消息的"噪声 / 重复发送"检测。
//!
//! Spec：`openspec/specs/team-coordination-metadata/spec.md`
//! §`Detect operational noise and resend in teammate messages`。
//!
//! 与原版 `claude-devtools/src/renderer/components/chat/items/TeammateMessageItem.tsx`
//! 的 `detectNoise` / `RESEND_PATTERNS` 同算法。

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

/// 与原版同集合：被识别为运维噪声的 message type。
const NOISE_TYPES: &[&str] = &[
    "idle_notification",
    "shutdown_approved",
    "teammate_terminated",
    "shutdown_request",
];

/// `system` teammate 的"短文本视为噪声"阈值（字符数）。
const SYSTEM_SHORT_TEXT_THRESHOLD: usize = 200;

/// resend 关键词正则集（对齐原版 `RESEND_PATTERNS`）。
static RESEND_PATTERNS: LazyLock<[Regex; 5]> = LazyLock::new(|| {
    [
        Regex::new(r"(?i)\bresend").expect("resend regex should compile"),
        Regex::new(r"(?i)\bre-send").expect("re-send regex should compile"),
        Regex::new(r"(?i)\bsent\b.{0,20}\bearlier").expect("sent...earlier regex should compile"),
        Regex::new(r"(?i)\balready\s+sent").expect("already sent regex should compile"),
        Regex::new(r"(?i)\bsent\s+in\s+my\s+previous")
            .expect("sent in my previous regex should compile"),
    ]
});

/// resend 检测扫描 body 的最大前缀长度（字符数）。
const RESEND_BODY_SCAN_PREFIX: usize = 300;

/// 判断一条 teammate 消息是否运维噪声（idle / shutdown / terminated 等）。
///
/// 算法（与原版 TS 完全一致）：
/// 1. `teammate_id == "system"` 且 body 是 JSON 且 `type` 在 [`NOISE_TYPES`] 集合内 → noise。
/// 2. `teammate_id == "system"` 且 body trim 后**非** JSON 且长度 < 200 → noise。
/// 3. `teammate_id != "system"` 但 body 是 JSON 且 `type` 在 [`NOISE_TYPES`] 集合内 → noise。
/// 4. 其它一律 false。
#[must_use]
pub fn detect_noise(body: &str, teammate_id: &str) -> bool {
    let trimmed = body.trim();

    if teammate_id == "system" {
        if trimmed.starts_with('{') {
            if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                if json_type_is_noise(&parsed) {
                    return true;
                }
            }
        }
        // system 的"短文本兜底"：< 200 字符即视为噪声（无论是否 JSON）。
        return trimmed.chars().count() < SYSTEM_SHORT_TEXT_THRESHOLD;
    }

    // 非 system：仅当 body 是 noise-type JSON 时才算噪声。
    if trimmed.starts_with('{') {
        if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            return json_type_is_noise(&parsed);
        }
    }
    false
}

fn json_type_is_noise(parsed: &Value) -> bool {
    parsed
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|t| NOISE_TYPES.contains(&t))
}

/// 判断一条 teammate 消息是否是 resend / 重复发送。
///
/// 命中条件（与原版 TS 完全一致）：
/// - summary 命中 [`RESEND_PATTERNS`] 任一；或
/// - body 前 300 字符命中 [`RESEND_PATTERNS`] 任一。
#[must_use]
pub fn detect_resend(summary: Option<&str>, body: &str) -> bool {
    if let Some(s) = summary {
        if RESEND_PATTERNS.iter().any(|re| re.is_match(s)) {
            return true;
        }
    }
    let scan: String = body.chars().take(RESEND_BODY_SCAN_PREFIX).collect();
    RESEND_PATTERNS.iter().any(|re| re.is_match(&scan))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- detect_noise ----

    #[test]
    fn system_idle_notification_json_is_noise() {
        let body = r#"{"type":"idle_notification","message":"Idle"}"#;
        assert!(detect_noise(body, "system"));
    }

    #[test]
    fn system_short_plain_text_is_noise() {
        // "Heartbeat ack" < 200 字符
        assert!(detect_noise("Heartbeat ack", "system"));
    }

    #[test]
    fn system_long_plain_text_is_not_noise() {
        let body = "x".repeat(SYSTEM_SHORT_TEXT_THRESHOLD + 5);
        assert!(!detect_noise(&body, "system"));
    }

    #[test]
    fn non_system_idle_json_is_noise() {
        let body = r#"{"type":"shutdown_request"}"#;
        assert!(detect_noise(body, "alice"));
    }

    #[test]
    fn non_system_business_message_is_not_noise() {
        assert!(!detect_noise(
            "Working on the task, will report back.",
            "alice"
        ));
    }

    #[test]
    fn non_system_unknown_type_json_is_not_noise() {
        let body = r#"{"type":"some_other_kind"}"#;
        assert!(!detect_noise(body, "alice"));
    }

    // ---- detect_resend ----

    #[test]
    fn summary_match_resend_keyword() {
        assert!(detect_resend(
            Some("Resending the previous message"),
            "body content",
        ));
    }

    #[test]
    fn body_prefix_match_sent_earlier() {
        let body = "Note: this message was sent earlier in my previous reply, but here it is again for clarity.";
        assert!(detect_resend(None, body));
    }

    #[test]
    fn resend_keyword_outside_300_prefix_is_missed() {
        let prefix = "x".repeat(RESEND_BODY_SCAN_PREFIX + 50);
        let body = format!("{prefix} resend this please");
        assert!(!detect_resend(None, &body));
    }

    #[test]
    fn no_resend_signal() {
        assert!(!detect_resend(
            Some("Initial reply"),
            "First time replying with full content.",
        ));
    }

    #[test]
    fn already_sent_pattern() {
        assert!(detect_resend(None, "I've already sent this earlier today."));
    }
}
