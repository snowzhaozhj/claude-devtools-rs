//! 配置数据类型。
//!
//! 与 TS `ConfigManager.ts` 的 `AppConfig` 体系对齐。
//! 所有类型 derive `Serialize` / `Deserialize` 以支持 JSON 持久化。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 磁盘加载路径用的"宽松"枚举反序列化：未知字面值（如老版本写过的
/// `"theme": "auto"`）fallback 到 `Default`，并 `tracing::warn!` 一行让 dev
/// 知情。**不**让整个 `AppConfig` 反序列化失败 → `merge_with_defaults`
/// 整体回退到默认值的回归（codex PR #285 review 指出的 blocking 路径）。
///
/// `update_*` 路径走 `PartialXConfig` 仍用 serde 默认严格 deserialize，
/// 拒绝非法值（保 spec configuration-management 字段校验 Scenario）。
fn deserialize_lenient_enum<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: serde::de::DeserializeOwned + Default,
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match serde_json::from_value::<T>(value.clone()) {
        Ok(v) => Ok(v),
        Err(e) => {
            tracing::warn!(
                value = %value,
                error = %e,
                "Unknown enum variant in stored config; falling back to Default"
            );
            Ok(T::default())
        }
    }
}

/// 宽容反序列化字符串数组：非字符串项（数字 / 对象 / null）静默跳过，而非让整个
/// `AppConfig` 反序列化失败回退默认。codex 二审：`recentRoots` 含非字符串项（如
/// `[.., 42]`）若走严格 `Vec<String>` 会连累 `httpServer.port` 等无关字段一起丢。
/// 保留的字符串项由 `sanitize_recent_roots` 在 load 时进一步按合法性过滤。
fn deserialize_lenient_string_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .filter_map(|v| match v {
            serde_json::Value::String(s) => Some(s),
            _ => None,
        })
        .collect())
}

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
    /// 用户自定义键盘快捷键映射：`actionId` → key combo（如 `"sidebar.toggle" → "mod+shift+b"`）。
    /// 空 `HashMap` 在 IPC / 文件序列化为 `{}`（无 `skip_serializing_if`），让前端 customization 层
    /// 永远拿到稳定 shape；详 `openspec/specs/configuration-management/spec.md` & `keyboard-shortcuts/spec.md`。
    #[serde(default)]
    pub keyboard_shortcuts: HashMap<String, String>,
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

/// 主题偏好。
///
/// 序列化形态保持原 `theme: "dark" | "light" | "system"` 字符串契约
/// （`#[serde(rename_all = "kebab-case")]` 单词产出小写）；前端 / 旧
/// 配置文件不感知 enum 化。详 `crates/cdt-config/src/manager.rs::PartialGeneralConfig`
/// 决策：String → typed enum 让 serde 接管字面量校验。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    Dark,
    Light,
    #[default]
    System,
}

/// 启动默认 tab 偏好。序列化为 `"dashboard" | "last-session"`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultTab {
    #[default]
    Dashboard,
    LastSession,
}

/// 点击 sidebar 会话项的默认行为。序列化为 `"replace" | "new-tab"`。
/// Cmd/Ctrl + 点击始终翻转该默认（对齐 Chrome 浏览器交互）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SessionClickBehavior {
    #[default]
    Replace,
    NewTab,
}

/// 应用全局设置。
///
/// 三个 enum 字段（`theme` / `default_tab` / `session_click_behavior`）走 lenient
/// deserialize：磁盘配置含历史版本写过的未知字面量（如 `theme: "auto"`）时
/// fallback 到 `Default`，**不**让整个 `AppConfig` 反序列化失败 → `merge_with_defaults`
/// 整体回退到默认。`update_*` 路径走 `PartialGeneralConfig` 仍是严格 enum
/// 反序列化（拒绝非法值），保 spec configuration-management 校验契约。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct GeneralConfig {
    pub launch_at_login: bool,
    pub show_dock_icon: bool,
    #[serde(default, deserialize_with = "deserialize_lenient_enum")]
    pub theme: Theme,
    #[serde(default, deserialize_with = "deserialize_lenient_enum")]
    pub default_tab: DefaultTab,
    pub claude_root_path: Option<String>,
    /// 用户切换过的数据根历史（MRU），供 Settings 快速切换下拉。后端在写入
    /// 非 null `claude_root_path` 时自动 append（去重 + MRU + 上限）；前端只读，
    /// 不通过 `update_general` 直接更新。详 configuration-management spec。
    #[serde(default, deserialize_with = "deserialize_lenient_string_vec")]
    pub recent_roots: Vec<String>,
    pub auto_expand_ai_groups: bool,
    pub use_native_title_bar: bool,
    /// 点击 sidebar 会话项时的默认行为："replace" 替换当前 tab，"new-tab" 总开新 tab。
    /// Cmd/Ctrl + 点击始终翻转该默认（对齐 Chrome 浏览器交互）。
    #[serde(default, deserialize_with = "deserialize_lenient_enum")]
    pub session_click_behavior: SessionClickBehavior,
    /// 用户偏好外部编辑器（用于"在编辑器打开"右键菜单 IPC `open_in_editor`）。
    /// 默认 `System`：走 OS 默认 app（macOS `open` / Win `start` / Linux `xdg-open`）。
    /// 详 `openspec/specs/configuration-management/spec.md` §"持久化外部编辑器偏好"。
    #[serde(default, deserialize_with = "deserialize_lenient_enum")]
    pub external_editor: ExternalEditor,
    /// 用户偏好搜索引擎（用于"在浏览器搜索"右键菜单 action）。
    /// 默认 `Google`；`Custom { url_template }` 中 `url_template` SHALL 含 `{query}`
    /// 占位符且 scheme ∈ `{http, https}`（详 §"持久化浏览器搜索引擎偏好"）。
    #[serde(default, deserialize_with = "deserialize_lenient_enum")]
    pub search_engine: SearchEngine,
    /// 用户偏好终端 app（用于"在终端打开"右键菜单 IPC `open_in_terminal`）。
    /// 统一跨平台 enum：配置可在不同平台间同步，不匹配当前 OS 时运行时
    /// `tracing::warn!` + fallback 到平台默认（不报错）。详 §"持久化首选终端 app"。
    #[serde(default, deserialize_with = "deserialize_lenient_enum")]
    pub terminal_app: TerminalApp,
}

// =============================================================================
// External editor / search engine / terminal app（Phase 2 右键菜单基础设施）
// =============================================================================

/// 用户偏好外部编辑器。
///
/// 用于 IPC `open_in_editor` 后端按 enum 白名单 dispatch CLI（绝不接受前端传 editor
/// 名以防 RCE）。详 `openspec/changes/frontend-context-menu-phase-2/design.md::D2` 决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExternalEditor {
    /// 走 OS 默认 app：macOS `open <path>` / Win `start "" "<path>"` / Linux `xdg-open`。
    /// 不支持跳行号（line/column 参数被忽略）。
    #[default]
    System,
    /// `code --goto path:line:col`
    VsCode,
    /// `cursor --goto path:line:col`（Cursor fork 兼容 VS Code CLI 形态）
    Cursor,
    /// `zed path:line:col`
    Zed,
    /// `subl path:line:col`
    Sublime,
}

/// 用户偏好浏览器搜索引擎。
///
/// internally-tagged enum：JSON 序列化为 `{ "type": "google" }` /
/// `{ "type": "custom", "urlTemplate": "https://example.com/search?q={query}" }`。
/// `Custom.url_template` SHALL 满足两条：(a) 含 `{query}` 占位符；
/// (b) URL scheme ∈ `{http, https}`。详 `design.md::D3` 决策。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SearchEngine {
    /// `https://www.google.com/search?q=<encoded>`
    #[default]
    Google,
    /// `https://www.bing.com/search?q=<encoded>`
    Bing,
    /// `https://duckduckgo.com/?q=<encoded>`
    DuckDuckGo,
    /// 用户自定义 URL 模板，含 `{query}` 占位符。
    Custom {
        #[serde(rename = "urlTemplate")]
        url_template: String,
    },
}

/// 用户偏好终端 app。
///
/// 统一扁平 enum：跨平台并集 10 个 variant，配置文件可跨 OS 同步——运行时
/// `open_in_terminal` 根据 `cfg!(target_os)` 判断与当前 OS 不匹配时
/// `tracing::warn!` + fallback 到平台默认终端，**不**返回错误。
/// `serde(rename_all = "snake_case")` 输出形态：`Terminal` → `"terminal"`、
/// `ITerm` → `"i_term"`（注意不是 `"iterm"`）。详 `design.md::D3` 决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TerminalApp {
    // ---- macOS ----
    /// macOS 内置 Terminal.app（默认值；`open -a Terminal <path>`）
    #[default]
    Terminal,
    /// iTerm2（`open -a iTerm <path>`）
    ITerm,
    /// Warp（`open -a Warp <path>`）
    Warp,
    // ---- Windows ----
    /// Windows Terminal（`wt.exe -d <path>`）
    WindowsTerminal,
    /// `cmd.exe /K cd /d <path>`（fallback；path 含 cmd metacharacters 会被拒绝）
    Cmd,
    /// `powershell.exe -NoExit -Command "Set-Location -LiteralPath $env:CDT_TARGET_PATH"`
    PowerShell,
    // ---- Linux ----
    /// Debian alternatives 系统统一入口（`x-terminal-emulator --working-directory <path>`）
    XTerminalEmulator,
    /// `gnome-terminal --working-directory=<path>`
    GnomeTerminal,
    /// `konsole --workdir <path>`
    Konsole,
    /// `alacritty --working-directory <path>`
    Alacritty,
}

impl TerminalApp {
    /// 当前平台默认终端，用于跨平台不匹配 fallback。
    #[must_use]
    pub fn platform_default() -> Self {
        if cfg!(target_os = "macos") {
            Self::Terminal
        } else if cfg!(target_os = "windows") {
            Self::WindowsTerminal
        } else {
            Self::XTerminalEmulator
        }
    }

    /// 当前平台合法 `TerminalApp` 集合（`list_available_terminals` IPC 用）。
    #[must_use]
    pub fn available_for_current_platform() -> Vec<Self> {
        if cfg!(target_os = "macos") {
            vec![Self::Terminal, Self::ITerm, Self::Warp]
        } else if cfg!(target_os = "windows") {
            vec![Self::WindowsTerminal, Self::Cmd, Self::PowerShell]
        } else {
            vec![
                Self::XTerminalEmulator,
                Self::GnomeTerminal,
                Self::Konsole,
                Self::Alacritty,
            ]
        }
    }

    /// 该 variant 是否在当前 OS 合法（用于运行时 mismatch 判定）。
    #[must_use]
    pub fn is_available_on_current_platform(self) -> bool {
        Self::available_for_current_platform().contains(&self)
    }
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
    KeyboardShortcuts,
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
            "keyboardShortcuts" => Some(Self::KeyboardShortcuts),
            _ => None,
        }
    }
}
