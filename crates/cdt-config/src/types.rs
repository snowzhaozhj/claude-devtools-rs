//! 配置数据类型。
//!
//! 与 TS `ConfigManager.ts` 的 `AppConfig` 体系对齐。
//! 所有类型 derive `Serialize` / `Deserialize` 以支持 JSON 持久化。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Top-level config
// =============================================================================

/// 应用顶层配置，持久化到 `~/.claude/claude-devtools-config.json`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub notifications: NotificationConfig,
    pub general: GeneralConfig,
    pub display: DisplayConfig,
    pub sessions: SessionsConfig,
    pub ssh: SshPersistConfig,
    pub http_server: HttpServerConfig,
    #[serde(default)]
    pub updater: UpdaterConfig,
}

// =============================================================================
// Notifications
// =============================================================================

/// 通知子系统配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationConfig {
    pub enabled: bool,
    pub sound_enabled: bool,
    pub ignored_regex: Vec<String>,
    pub ignored_repositories: Vec<String>,
    pub snoozed_until: Option<i64>,
    pub snooze_minutes: u32,
    pub include_subagent_errors: bool,
    pub triggers: Vec<NotificationTrigger>,
}

// =============================================================================
// Triggers
// =============================================================================

/// Trigger 内容类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerContentType {
    ToolResult,
    ToolUse,
    Thinking,
    Text,
}

/// Trigger 评估模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    ErrorStatus,
    ContentMatch,
    TokenThreshold,
}

/// Token 类型（用于 `token_threshold` 模式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerTokenType {
    Input,
    Output,
    Total,
}

/// 通知 trigger 配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationTrigger {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub content_type: TriggerContentType,
    pub mode: TriggerMode,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_builtin: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_patterns: Option<Vec<String>>,

    // error_status 模式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_error: Option<bool>,

    // content_match 模式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_pattern: Option<String>,

    // token_threshold 模式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_threshold: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_type: Option<TriggerTokenType>,

    // 仓库范围
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository_ids: Option<Vec<String>>,

    // 显示颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

impl NotificationTrigger {
    /// 是否是内建 trigger。
    pub fn is_builtin(&self) -> bool {
        self.is_builtin.unwrap_or(false)
    }
}

// =============================================================================
// General
// =============================================================================

/// 应用全局设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct GeneralConfig {
    pub launch_at_login: bool,
    pub show_dock_icon: bool,
    pub theme: String,
    pub default_tab: String,
    pub claude_root_path: Option<String>,
    pub auto_expand_ai_groups: bool,
    pub use_native_title_bar: bool,
}

// =============================================================================
// Display
// =============================================================================

/// 展示偏好。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayConfig {
    pub show_timestamps: bool,
    pub compact_mode: bool,
    pub syntax_highlighting: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_sans: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_mono: Option<String>,
}

// =============================================================================
// Sessions
// =============================================================================

/// Session pin/hide 信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedSession {
    pub session_id: String,
    pub pinned_at: i64,
}

/// 隐藏 session 信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiddenSession {
    pub session_id: String,
    pub hidden_at: i64,
}

/// Session 管理配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsConfig {
    pub pinned_sessions: HashMap<String, Vec<PinnedSession>>,
    pub hidden_sessions: HashMap<String, Vec<HiddenSession>>,
}

// =============================================================================
// SSH
// =============================================================================

/// SSH 连接配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectionProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
}

/// SSH 持久化配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshPersistConfig {
    pub last_connection: Option<serde_json::Value>,
    pub auto_reconnect: bool,
    pub profiles: Vec<SshConnectionProfile>,
    pub last_active_context_id: String,
}

// =============================================================================
// HTTP Server
// =============================================================================

/// HTTP 服务器配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpServerConfig {
    pub enabled: bool,
    pub port: u16,
}

// =============================================================================
// Updater
// =============================================================================

/// 自动更新配置。
///
/// `auto_update_check_enabled` 控制启动后台静默检查；关闭时手动「检查更新」按钮仍可用。
/// `skipped_update_version` 记录用户主动「跳过此版本」的目标版本号，启动检查命中即不弹横幅。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterConfig {
    #[serde(default = "default_auto_update_check_enabled")]
    pub auto_update_check_enabled: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped_update_version: Option<String>,
}

impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            auto_update_check_enabled: true,
            skipped_update_version: None,
        }
    }
}

fn default_auto_update_check_enabled() -> bool {
    true
}

// =============================================================================
// Config section key
// =============================================================================

/// 配置 section 标识，用于 `update_config` 的分 section 更新。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSection {
    Notifications,
    General,
    Display,
    Sessions,
    Ssh,
    HttpServer,
    Updater,
}

impl ConfigSection {
    /// 从字符串解析 section 名。
    pub fn from_str_key(s: &str) -> Option<Self> {
        match s {
            "notifications" => Some(Self::Notifications),
            "general" => Some(Self::General),
            "display" => Some(Self::Display),
            "sessions" => Some(Self::Sessions),
            "ssh" => Some(Self::Ssh),
            "httpServer" => Some(Self::HttpServer),
            "updater" => Some(Self::Updater),
            _ => None,
        }
    }
}
