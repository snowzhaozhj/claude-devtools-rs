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
    pub ssh: SshConfig,
    #[serde(default)]
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
    /// 点击 sidebar 会话项时的默认行为："replace" 替换当前 tab，"new-tab" 总开新 tab。
    /// Cmd/Ctrl + 点击始终翻转该默认（对齐 Chrome 浏览器交互）。
    #[serde(default = "default_session_click_behavior")]
    pub session_click_behavior: String,
}

fn default_session_click_behavior() -> String {
    "replace".to_string()
}

// =============================================================================
// Display
// =============================================================================

/// 时间格式偏好：`"24h"` 24 小时制（默认），`"12h"` 12 小时制（带上午/下午）。
///
/// 详见 `openspec/specs/configuration-management/spec.md` §"Display config exposes
/// time format preference"。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TimeFormat {
    #[default]
    #[serde(rename = "24h")]
    H24,
    #[serde(rename = "12h")]
    H12,
}

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
    /// 时间格式偏好，默认 `TimeFormat::H24`。旧配置文件缺字段时 serde 走 Default。
    #[serde(default)]
    pub time_format: TimeFormat,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SshAuthMethod {
    #[default]
    SshConfig,
    Password,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: SshAuthMethod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshLastConnection {
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default)]
    pub auth_method: SshAuthMethod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_connection: Option<SshLastConnection>,
    pub auto_reconnect: bool,
    pub profiles: Vec<SshProfile>,
}

pub type SshConnectionProfile = SshProfile;
pub type SshPersistConfig = SshConfig;

// =============================================================================
// HTTP Server
// =============================================================================

/// HTTP 服务器配置。
///
/// 老配置文件可能缺整个 `httpServer` 节点或只缺其中一个字段——所有缺失字段
/// 都通过 `#[serde(default = "<fn>")]` 物化为 `enabled=false / port=3456`。
/// 详见 `openspec/specs/configuration-management/spec.md` §"HTTP server enabled
/// / port SHALL be persisted in lockstep with lifecycle"。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpServerConfig {
    #[serde(default = "default_http_server_enabled")]
    pub enabled: bool,
    #[serde(default = "default_http_server_port")]
    pub port: u16,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            enabled: default_http_server_enabled(),
            port: default_http_server_port(),
        }
    }
}

fn default_http_server_enabled() -> bool {
    false
}

fn default_http_server_port() -> u16 {
    3456
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
