//! 默认配置值。

use std::collections::HashMap;

use crate::types::{
    AppConfig, DisplayConfig, GeneralConfig, HttpServerConfig, NotificationConfig,
    NotificationTrigger, SessionsConfig, SshPersistConfig, TriggerContentType, TriggerMode,
    TriggerTokenType, UpdaterConfig,
};

/// 默认 trigger 列表（内建，不可删除）。
pub fn default_triggers() -> Vec<NotificationTrigger> {
    vec![
        NotificationTrigger {
            id: "builtin-bash-command".into(),
            name: ".env File Access Alert".into(),
            enabled: false,
            content_type: TriggerContentType::ToolUse,
            mode: TriggerMode::ContentMatch,
            match_pattern: Some("/.env".into()),
            is_builtin: Some(true),
            color: Some("red".into()),
            tool_name: None,
            ignore_patterns: None,
            require_error: None,
            match_field: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
        },
        NotificationTrigger {
            id: "builtin-tool-result-error".into(),
            name: "Tool Result Error".into(),
            enabled: false,
            content_type: TriggerContentType::ToolResult,
            mode: TriggerMode::ErrorStatus,
            require_error: Some(true),
            ignore_patterns: Some(vec![
                r"The user doesn't want to proceed with this tool use\.".into(),
                r"\[Request interrupted by user for tool use\]".into(),
            ]),
            is_builtin: Some(true),
            color: Some("orange".into()),
            tool_name: None,
            match_field: None,
            match_pattern: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
        },
        NotificationTrigger {
            id: "builtin-high-token-usage".into(),
            name: "High Token Usage".into(),
            enabled: false,
            content_type: TriggerContentType::ToolResult,
            mode: TriggerMode::TokenThreshold,
            token_threshold: Some(8000),
            token_type: Some(TriggerTokenType::Total),
            color: Some("yellow".into()),
            is_builtin: Some(true),
            tool_name: None,
            ignore_patterns: None,
            require_error: None,
            match_field: None,
            match_pattern: None,
            repository_ids: None,
        },
    ]
}

/// 默认忽略 regex。
fn default_ignored_regex() -> Vec<String> {
    vec![r"The user doesn't want to proceed with this tool use\.".into()]
}

/// 创建默认 `AppConfig`。
pub fn default_config() -> AppConfig {
    AppConfig {
        notifications: NotificationConfig {
            enabled: true,
            sound_enabled: true,
            ignored_regex: default_ignored_regex(),
            ignored_repositories: Vec::new(),
            snoozed_until: None,
            snooze_minutes: 30,
            include_subagent_errors: true,
            triggers: default_triggers(),
        },
        general: GeneralConfig {
            launch_at_login: false,
            show_dock_icon: true,
            theme: "system".into(),
            default_tab: "dashboard".into(),
            claude_root_path: None,
            auto_expand_ai_groups: false,
            use_native_title_bar: false,
        },
        display: DisplayConfig {
            show_timestamps: true,
            compact_mode: false,
            syntax_highlighting: true,
        },
        sessions: SessionsConfig {
            pinned_sessions: HashMap::new(),
            hidden_sessions: HashMap::new(),
        },
        ssh: SshPersistConfig {
            last_connection: None,
            auto_reconnect: false,
            profiles: Vec::new(),
            last_active_context_id: "local".into(),
        },
        http_server: HttpServerConfig {
            enabled: false,
            port: 3456,
        },
        updater: UpdaterConfig::default(),
    }
}
