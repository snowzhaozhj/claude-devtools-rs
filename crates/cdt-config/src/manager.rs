//! `ConfigManager` —— 配置加载、保存、更新、合并。
//!
//! 对应 TS `ConfigManager.ts`。核心职责：
//! - 从磁盘加载 JSON 配置
//! - 损坏文件自动备份（修复 TS impl-bug）
//! - partial config 与默认值合并
//! - 分 section 更新 + 持久化
//! - Session pin/unpin、hide/unhide、snooze

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::defaults::default_config;
use crate::error::ConfigError;
use crate::trigger::{TriggerManager, merge_triggers, validate_trigger};
use crate::types::{AppConfig, HiddenSession, NotificationTrigger, PinnedSession};
use crate::validation::{normalize_claude_root_path, validate_http_port, validate_snooze_minutes};

/// 默认配置文件路径：`~/.claude/claude-devtools-config.json`。
fn default_config_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".claude").join("claude-devtools-config.json")
}

/// 配置管理器。
pub struct ConfigManager {
    config: AppConfig,
    config_path: PathBuf,
    trigger_manager: TriggerManager,
}

impl ConfigManager {
    /// 创建 `ConfigManager`（不加载磁盘文件）。
    pub fn new(config_path: Option<PathBuf>) -> Self {
        let path = config_path.unwrap_or_else(default_config_path);
        let config = default_config();
        let trigger_manager = TriggerManager::new(config.notifications.triggers.clone());
        Self {
            config,
            config_path: path,
            trigger_manager,
        }
    }

    /// 从磁盘异步加载配置。
    pub async fn load(&mut self) -> Result<(), ConfigError> {
        self.config = self.load_from_disk().await?;
        self.trigger_manager = TriggerManager::new(self.config.notifications.triggers.clone());
        Ok(())
    }

    /// 获取当前配置的副本。
    pub fn get_config(&self) -> AppConfig {
        self.config.clone()
    }

    /// 获取配置文件路径。
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    // =========================================================================
    // Config loading
    // =========================================================================

    async fn load_from_disk(&self) -> Result<AppConfig, ConfigError> {
        // 文件不存在 → 使用默认值
        match tokio::fs::metadata(&self.config_path).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!("No config file found, using defaults");
                return Ok(default_config());
            }
            Err(e) => return Err(ConfigError::io(&self.config_path, e)),
        }

        let content = tokio::fs::read_to_string(&self.config_path)
            .await
            .map_err(|e| ConfigError::io(&self.config_path, e))?;

        match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(parsed) => {
                let partial: AppConfig = Self::merge_with_defaults(&parsed);
                Ok(partial)
            }
            Err(e) => {
                // 损坏文件：备份 → 加载默认
                tracing::warn!(
                    path = %self.config_path.display(),
                    error = %e,
                    "Config file corrupted, backing up and loading defaults"
                );
                self.backup_corrupted_file().await?;
                Ok(default_config())
            }
        }
    }

    /// 备份损坏文件：重命名为 `<path>.bak.<timestamp_ms>`。
    async fn backup_corrupted_file(&self) -> Result<(), ConfigError> {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let backup_path = PathBuf::from(format!("{}.bak.{ts}", self.config_path.display()));
        tracing::warn!(
            backup = %backup_path.display(),
            "Backing up corrupted config file"
        );
        tokio::fs::rename(&self.config_path, &backup_path)
            .await
            .map_err(|e| ConfigError::io(&backup_path, e))?;
        Ok(())
    }

    /// 把 partial JSON 与默认值合并。
    fn merge_with_defaults(parsed: &serde_json::Value) -> AppConfig {
        let defaults = default_config();
        let default_val = serde_json::to_value(&defaults).unwrap_or_default();

        let merged = deep_merge(&default_val, parsed);

        let mut config: AppConfig =
            serde_json::from_value(merged).unwrap_or_else(|_| defaults.clone());

        // 特殊处理 triggers merge
        if let Some(loaded_triggers) = parsed.get("notifications").and_then(|n| n.get("triggers")) {
            if let Ok(loaded) =
                serde_json::from_value::<Vec<NotificationTrigger>>(loaded_triggers.clone())
            {
                let default_triggers = crate::defaults::default_triggers();
                config.notifications.triggers = merge_triggers(&loaded, &default_triggers);
            }
        }

        // 标准化 `claudeRootPath`
        config.general.claude_root_path =
            normalize_claude_root_path(config.general.claude_root_path.as_deref());

        config
    }

    // =========================================================================
    // Config saving
    // =========================================================================

    /// 保存当前配置到磁盘。
    pub async fn save(&self) -> Result<(), ConfigError> {
        self.persist_config(&self.config).await
    }

    async fn persist_config(&self, config: &AppConfig) -> Result<(), ConfigError> {
        if let Some(parent) = self.config_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ConfigError::io(parent, e))?;
        }

        let content = serde_json::to_string_pretty(config)?;
        tokio::fs::write(&self.config_path, content)
            .await
            .map_err(|e| ConfigError::io(&self.config_path, e))?;

        tracing::info!("Config saved");
        Ok(())
    }

    // =========================================================================
    // Config updates
    // =========================================================================

    /// 更新 notifications section。
    pub async fn update_notifications(
        &mut self,
        updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        if let Some(obj) = updates.as_object() {
            for (k, v) in obj {
                match k.as_str() {
                    "enabled" => {
                        if let Some(b) = v.as_bool() {
                            self.config.notifications.enabled = b;
                        }
                    }
                    "soundEnabled" => {
                        if let Some(b) = v.as_bool() {
                            self.config.notifications.sound_enabled = b;
                        }
                    }
                    "includeSubagentErrors" => {
                        if let Some(b) = v.as_bool() {
                            self.config.notifications.include_subagent_errors = b;
                        }
                    }
                    "snoozeMinutes" => {
                        if let Some(n) = v.as_u64() {
                            let minutes = u32::try_from(n)
                                .map_err(|_| ConfigError::validation("snoozeMinutes overflow"))?;
                            validate_snooze_minutes(minutes)?;
                            self.config.notifications.snooze_minutes = minutes;
                        }
                    }
                    "triggers" => {
                        let list: Vec<NotificationTrigger> = serde_json::from_value(v.clone())
                            .map_err(|e| {
                                ConfigError::validation(format!(
                                    "triggers must be an array of NotificationTrigger: {e}"
                                ))
                            })?;
                        for t in &list {
                            let r = validate_trigger(t);
                            if !r.valid {
                                return Err(ConfigError::validation(format!(
                                    "Invalid trigger \"{}\": {}",
                                    t.id,
                                    r.errors.join(", ")
                                )));
                            }
                        }
                        self.config.notifications.triggers.clone_from(&list);
                        self.trigger_manager.set_triggers(list);
                    }
                    other => {
                        tracing::warn!(
                            key = %other,
                            "unknown notifications update key ignored"
                        );
                    }
                }
            }
        }
        self.save().await?;
        Ok(self.get_config())
    }

    /// 更新 general section。
    pub async fn update_general(
        &mut self,
        updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        if let Some(obj) = updates.as_object() {
            for (k, v) in obj {
                match k.as_str() {
                    "launchAtLogin" => {
                        if let Some(b) = v.as_bool() {
                            self.config.general.launch_at_login = b;
                        }
                    }
                    "showDockIcon" => {
                        if let Some(b) = v.as_bool() {
                            self.config.general.show_dock_icon = b;
                        }
                    }
                    "theme" => {
                        if let Some(s) = v.as_str() {
                            match s {
                                "dark" | "light" | "system" => {
                                    s.clone_into(&mut self.config.general.theme);
                                }
                                _ => {
                                    return Err(ConfigError::validation(
                                        "general.theme must be one of: dark, light, system",
                                    ));
                                }
                            }
                        }
                    }
                    "defaultTab" => {
                        if let Some(s) = v.as_str() {
                            match s {
                                "dashboard" | "last-session" => {
                                    s.clone_into(&mut self.config.general.default_tab);
                                }
                                _ => {
                                    return Err(ConfigError::validation(
                                        "general.defaultTab must be one of: dashboard, last-session",
                                    ));
                                }
                            }
                        }
                    }
                    "claudeRootPath" => {
                        let raw = v.as_str();
                        self.config.general.claude_root_path = normalize_claude_root_path(raw);
                    }
                    "autoExpandAIGroups" => {
                        if let Some(b) = v.as_bool() {
                            self.config.general.auto_expand_ai_groups = b;
                        }
                    }
                    "useNativeTitleBar" => {
                        if let Some(b) = v.as_bool() {
                            self.config.general.use_native_title_bar = b;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.save().await?;
        Ok(self.get_config())
    }

    /// 更新 display section。
    ///
    /// 校验语义为整次原子：先在 `candidate` 副本上应用所有字段并校验，全部通过才写入
    /// `self.config.display` 并 save；任一字段非法则整次返回 error，已存值不变。
    /// 字符串字段（`fontSans` / `fontMono`）trim 后空白或 JSON `null` 归一化为 `None`，
    /// 长度 > 500 字符拒绝。
    pub async fn update_display(
        &mut self,
        updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        const FONT_FAMILY_MAX_LEN: usize = 500;

        if let Some(obj) = updates.as_object() {
            let mut candidate = self.config.display.clone();
            for (k, v) in obj {
                match k.as_str() {
                    "showTimestamps" => {
                        if let Some(b) = v.as_bool() {
                            candidate.show_timestamps = b;
                        }
                    }
                    "compactMode" => {
                        if let Some(b) = v.as_bool() {
                            candidate.compact_mode = b;
                        }
                    }
                    "syntaxHighlighting" => {
                        if let Some(b) = v.as_bool() {
                            candidate.syntax_highlighting = b;
                        }
                    }
                    "fontSans" => {
                        candidate.font_sans =
                            normalize_font_family_field(v, "fontSans", FONT_FAMILY_MAX_LEN)?;
                    }
                    "fontMono" => {
                        candidate.font_mono =
                            normalize_font_family_field(v, "fontMono", FONT_FAMILY_MAX_LEN)?;
                    }
                    other => {
                        tracing::warn!(key = %other, "unknown display update key ignored");
                    }
                }
            }
            self.config.display = candidate;
        }
        self.save().await?;
        Ok(self.get_config())
    }

    /// 更新 updater section。
    ///
    /// 支持字段：
    /// - `autoUpdateCheckEnabled: bool` —— 启动后台自动检查开关
    /// - `skippedUpdateVersion: string | null` —— null 清空、字符串写入
    pub async fn update_updater(
        &mut self,
        updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        if let Some(obj) = updates.as_object() {
            for (k, v) in obj {
                match k.as_str() {
                    "autoUpdateCheckEnabled" => {
                        if let Some(b) = v.as_bool() {
                            self.config.updater.auto_update_check_enabled = b;
                        }
                    }
                    "skippedUpdateVersion" => {
                        if v.is_null() {
                            self.config.updater.skipped_update_version = None;
                        } else if let Some(s) = v.as_str() {
                            self.config.updater.skipped_update_version = Some(s.to_owned());
                        }
                    }
                    other => {
                        tracing::warn!(
                            key = %other,
                            "unknown updater update key ignored"
                        );
                    }
                }
            }
        }
        self.save().await?;
        Ok(self.get_config())
    }

    /// 更新 `httpServer` section。
    pub async fn update_http_server(
        &mut self,
        updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        if let Some(obj) = updates.as_object() {
            for (k, v) in obj {
                match k.as_str() {
                    "enabled" => {
                        if let Some(b) = v.as_bool() {
                            self.config.http_server.enabled = b;
                        }
                    }
                    "port" => {
                        if let Some(n) = v.as_u64() {
                            let port = u16::try_from(n).map_err(|_| {
                                ConfigError::validation(
                                    "httpServer.port must be an integer between 1024 and 65535",
                                )
                            })?;
                            validate_http_port(port)?;
                            self.config.http_server.port = port;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.save().await?;
        Ok(self.get_config())
    }

    // =========================================================================
    // Trigger management (delegated)
    // =========================================================================

    pub fn get_triggers(&self) -> Vec<NotificationTrigger> {
        self.trigger_manager.get_all()
    }

    pub fn get_enabled_triggers(&self) -> Vec<NotificationTrigger> {
        self.trigger_manager.get_enabled()
    }

    pub async fn add_trigger(
        &mut self,
        trigger: NotificationTrigger,
    ) -> Result<AppConfig, ConfigError> {
        self.config.notifications.triggers = self.trigger_manager.add(trigger)?;
        self.save().await?;
        Ok(self.get_config())
    }

    pub async fn remove_trigger(&mut self, trigger_id: &str) -> Result<AppConfig, ConfigError> {
        self.config.notifications.triggers = self.trigger_manager.remove(trigger_id)?;
        self.save().await?;
        Ok(self.get_config())
    }

    // =========================================================================
    // Snooze management
    // =========================================================================

    /// 设置 snooze，单位分钟。
    pub async fn set_snooze(&mut self, minutes: Option<u32>) -> Result<AppConfig, ConfigError> {
        let m = minutes.unwrap_or(self.config.notifications.snooze_minutes);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let snoozed_until = i64::try_from(now_ms).unwrap_or(i64::MAX) + i64::from(m) * 60 * 1000;

        self.config.notifications.snoozed_until = Some(snoozed_until);
        self.save().await?;
        Ok(self.get_config())
    }

    /// 清除 snooze。
    pub async fn clear_snooze(&mut self) -> Result<AppConfig, ConfigError> {
        self.config.notifications.snoozed_until = None;
        self.save().await?;
        Ok(self.get_config())
    }

    /// 检查是否处于 snooze（自动清除过期的）。
    pub fn is_snoozed(&mut self) -> bool {
        let Some(until) = self.config.notifications.snoozed_until else {
            return false;
        };

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let now = i64::try_from(now_ms).unwrap_or(i64::MAX);

        if now >= until {
            self.config.notifications.snoozed_until = None;
            return false;
        }
        true
    }

    // =========================================================================
    // Session pin management
    // =========================================================================

    /// Pin 一个 session。
    pub async fn pin_session(
        &mut self,
        project_id: &str,
        session_id: &str,
    ) -> Result<(), ConfigError> {
        let pins = self
            .config
            .sessions
            .pinned_sessions
            .entry(project_id.to_owned())
            .or_default();

        if pins.iter().any(|p| p.session_id == session_id) {
            return Ok(());
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        pins.insert(
            0,
            PinnedSession {
                session_id: session_id.to_owned(),
                pinned_at: i64::try_from(now_ms).unwrap_or(i64::MAX),
            },
        );
        self.save().await
    }

    /// Unpin 一个 session。
    pub async fn unpin_session(
        &mut self,
        project_id: &str,
        session_id: &str,
    ) -> Result<(), ConfigError> {
        if let Some(pins) = self.config.sessions.pinned_sessions.get_mut(project_id) {
            pins.retain(|p| p.session_id != session_id);
            if pins.is_empty() {
                self.config.sessions.pinned_sessions.remove(project_id);
            }
            self.save().await?;
        }
        Ok(())
    }

    // =========================================================================
    // Session hide management
    // =========================================================================

    /// 隐藏一个 session。
    pub async fn hide_session(
        &mut self,
        project_id: &str,
        session_id: &str,
    ) -> Result<(), ConfigError> {
        let hidden = self
            .config
            .sessions
            .hidden_sessions
            .entry(project_id.to_owned())
            .or_default();

        if hidden.iter().any(|h| h.session_id == session_id) {
            return Ok(());
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        hidden.insert(
            0,
            HiddenSession {
                session_id: session_id.to_owned(),
                hidden_at: i64::try_from(now_ms).unwrap_or(i64::MAX),
            },
        );
        self.save().await
    }

    /// 取消隐藏 session。
    pub async fn unhide_session(
        &mut self,
        project_id: &str,
        session_id: &str,
    ) -> Result<(), ConfigError> {
        if let Some(hidden) = self.config.sessions.hidden_sessions.get_mut(project_id) {
            hidden.retain(|h| h.session_id != session_id);
            if hidden.is_empty() {
                self.config.sessions.hidden_sessions.remove(project_id);
            }
            self.save().await?;
        }
        Ok(())
    }

    // =========================================================================
    // Reset / reload
    // =========================================================================

    /// 重置为默认配置。
    pub async fn reset_to_defaults(&mut self) -> Result<AppConfig, ConfigError> {
        self.config = default_config();
        self.trigger_manager = TriggerManager::new(self.config.notifications.triggers.clone());
        self.save().await?;
        Ok(self.get_config())
    }

    /// 从磁盘重新加载。
    pub async fn reload(&mut self) -> Result<AppConfig, ConfigError> {
        self.load().await?;
        Ok(self.get_config())
    }
}

/// 把传入的 JSON 值归一化为 `Option<String>` 用于 font-family 类字段：
/// - `null` → `None`
/// - 字符串 trim 后为空 → `None`
/// - 字符串长度（trim 后）> `max_len` → validation error
/// - 非字符串 / 非 null → validation error
/// - 其余 → `Some(s.trim())`
fn normalize_font_family_field(
    value: &serde_json::Value,
    field_name: &str,
    max_len: usize,
) -> Result<Option<String>, ConfigError> {
    if value.is_null() {
        return Ok(None);
    }
    let Some(s) = value.as_str() else {
        return Err(ConfigError::validation(format!(
            "display.{field_name} must be a string or null"
        )));
    };
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > max_len {
        return Err(ConfigError::validation(format!(
            "display.{field_name} must be <= {max_len} characters"
        )));
    }
    Ok(Some(trimmed.to_owned()))
}

/// 递归合并两个 JSON value（`base` 为默认值，`overlay` 为已加载值）。
fn deep_merge(base: &serde_json::Value, overlay: &serde_json::Value) -> serde_json::Value {
    match (base, overlay) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            let mut merged = b.clone();
            for (k, v) in o {
                let base_val = b.get(k).unwrap_or(&serde_json::Value::Null);
                merged.insert(k.clone(), deep_merge(base_val, v));
            }
            serde_json::Value::Object(merged)
        }
        // overlay 优先
        (_, overlay) => overlay.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn first_launch_no_config_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        let config = mgr.get_config();
        assert!(config.notifications.enabled);
        assert_eq!(config.http_server.port, 3456);
    }

    #[tokio::test]
    async fn corrupted_config_creates_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        tokio::fs::write(&path, "not json{{{").await.unwrap();

        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        // 原文件应该被重命名
        assert!(!path.exists());

        // 备份文件应该存在
        let mut found_backup = false;
        let mut entries = tokio::fs::read_dir(dir.path()).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("config.json.bak.") {
                found_backup = true;
                let content = tokio::fs::read_to_string(entry.path()).await.unwrap();
                assert_eq!(content, "not json{{{");
            }
        }
        assert!(found_backup, "backup file should exist");

        // 应该加载了默认值
        let config = mgr.get_config();
        assert!(config.notifications.enabled);
    }

    #[tokio::test]
    async fn partial_config_merged_with_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // 只写 httpServer.port
        tokio::fs::write(&path, r#"{"httpServer":{"port":9999}}"#)
            .await
            .unwrap();

        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        let config = mgr.get_config();
        assert_eq!(config.http_server.port, 9999);
        // 其他字段应该是默认值
        assert!(config.notifications.enabled);
        assert_eq!(config.general.theme, "system");
    }

    #[tokio::test]
    async fn update_http_port_validation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));

        // 有效端口
        let result = mgr
            .update_http_server(serde_json::json!({"port": 8080}))
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().http_server.port, 8080);

        // 无效端口
        let result = mgr
            .update_http_server(serde_json::json!({"port": 80}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn pin_unpin_session() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));

        mgr.pin_session("proj1", "sess1").await.unwrap();
        let config = mgr.get_config();
        assert_eq!(config.sessions.pinned_sessions["proj1"].len(), 1);

        // 重复 pin 不增加
        mgr.pin_session("proj1", "sess1").await.unwrap();
        let config = mgr.get_config();
        assert_eq!(config.sessions.pinned_sessions["proj1"].len(), 1);

        mgr.unpin_session("proj1", "sess1").await.unwrap();
        let config = mgr.get_config();
        assert!(!config.sessions.pinned_sessions.contains_key("proj1"));
    }

    #[tokio::test]
    async fn hide_unhide_session() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));

        mgr.hide_session("proj1", "sess1").await.unwrap();
        let config = mgr.get_config();
        assert_eq!(config.sessions.hidden_sessions["proj1"].len(), 1);

        mgr.unhide_session("proj1", "sess1").await.unwrap();
        let config = mgr.get_config();
        assert!(!config.sessions.hidden_sessions.contains_key("proj1"));
    }

    #[test]
    fn deep_merge_preserves_overlay() {
        let base = serde_json::json!({"a": 1, "b": {"c": 2, "d": 3}});
        let overlay = serde_json::json!({"b": {"c": 99}});
        let result = deep_merge(&base, &overlay);
        assert_eq!(result["a"], 1);
        assert_eq!(result["b"]["c"], 99);
        assert_eq!(result["b"]["d"], 3);
    }

    #[tokio::test]
    async fn update_notifications_persists_triggers() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        let new_trigger = serde_json::json!({
            "id": "custom-42",
            "name": "My custom",
            "enabled": true,
            "contentType": "tool_result",
            "mode": "error_status",
            "requireError": true,
        });
        let updates = serde_json::json!({ "triggers": [new_trigger] });

        let result = mgr.update_notifications(updates).await.unwrap();

        assert_eq!(result.notifications.triggers.len(), 1);
        assert_eq!(result.notifications.triggers[0].id, "custom-42");

        let enabled = mgr.get_enabled_triggers();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, "custom-42");

        let disk = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(disk.contains("custom-42"));
    }

    #[tokio::test]
    async fn update_notifications_rejects_invalid_trigger() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        let before_count = mgr.get_config().notifications.triggers.len();
        let before_enabled = mgr.get_enabled_triggers().len();

        let bad = serde_json::json!({
            "id": "",
            "name": "",
            "enabled": true,
            "contentType": "tool_result",
            "mode": "error_status",
        });
        let updates = serde_json::json!({ "triggers": [bad] });

        let err = mgr.update_notifications(updates).await.unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));

        assert_eq!(
            mgr.get_config().notifications.triggers.len(),
            before_count,
            "triggers array must not be partially mutated on validation failure"
        );
        assert_eq!(
            mgr.get_enabled_triggers().len(),
            before_enabled,
            "TriggerManager must not be mutated on validation failure"
        );
    }

    #[tokio::test]
    async fn updater_default_enabled_and_no_skipped_version() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        let cfg = mgr.get_config();
        assert!(cfg.updater.auto_update_check_enabled);
        assert!(cfg.updater.skipped_update_version.is_none());
    }

    #[tokio::test]
    async fn updater_partial_config_missing_fields_uses_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // 老配置：完全没有 updater 字段
        tokio::fs::write(&path, r#"{"httpServer":{"port":9999}}"#)
            .await
            .unwrap();

        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        let cfg = mgr.get_config();
        assert!(
            cfg.updater.auto_update_check_enabled,
            "缺字段必须默认启用自动检查"
        );
        assert!(cfg.updater.skipped_update_version.is_none());
        assert_eq!(cfg.http_server.port, 9999, "其他字段不应被覆盖");
    }

    #[tokio::test]
    async fn updater_set_auto_check_disabled_persists() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        mgr.update_updater(serde_json::json!({ "autoUpdateCheckEnabled": false }))
            .await
            .unwrap();

        // 重 load 验证持久化
        let mut mgr2 = ConfigManager::new(Some(path));
        mgr2.load().await.unwrap();
        assert!(!mgr2.get_config().updater.auto_update_check_enabled);
    }

    #[tokio::test]
    async fn updater_skipped_version_set_then_clear() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        mgr.update_updater(serde_json::json!({ "skippedUpdateVersion": "0.3.0" }))
            .await
            .unwrap();
        assert_eq!(
            mgr.get_config().updater.skipped_update_version.as_deref(),
            Some("0.3.0")
        );

        mgr.update_updater(serde_json::json!({ "skippedUpdateVersion": null }))
            .await
            .unwrap();
        assert!(mgr.get_config().updater.skipped_update_version.is_none());

        // 序列化时缺省字段不应出现
        let disk = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(!disk.contains("skippedUpdateVersion"));
    }

    #[tokio::test]
    async fn updater_unknown_key_ignored() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        let result = mgr
            .update_updater(serde_json::json!({ "fooBar": 123, "autoUpdateCheckEnabled": false }))
            .await
            .unwrap();
        assert!(!result.updater.auto_update_check_enabled);
    }

    #[tokio::test]
    async fn display_font_fields_default_to_none_on_first_launch() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("fresh.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        let cfg = mgr.get_config();
        assert!(cfg.display.font_sans.is_none());
        assert!(cfg.display.font_mono.is_none());
    }

    #[tokio::test]
    async fn display_font_fields_forward_compatible_with_old_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // 老配置：display 段缺失 fontSans / fontMono 字段
        tokio::fs::write(
            &path,
            r#"{"display":{"showTimestamps":false,"compactMode":true,"syntaxHighlighting":true}}"#,
        )
        .await
        .unwrap();

        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        let cfg = mgr.get_config();
        assert!(!cfg.display.show_timestamps, "已有字段保留");
        assert!(cfg.display.compact_mode, "已有字段保留");
        assert!(cfg.display.font_sans.is_none(), "缺字段视为 None");
        assert!(cfg.display.font_mono.is_none(), "缺字段视为 None");
    }

    #[tokio::test]
    async fn display_set_custom_font_persists_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        let custom = "\"JetBrains Mono\", monospace";
        mgr.update_display(serde_json::json!({ "fontMono": custom }))
            .await
            .unwrap();

        // 重 load 验证持久化往返
        let mut mgr2 = ConfigManager::new(Some(path));
        mgr2.load().await.unwrap();
        assert_eq!(mgr2.get_config().display.font_mono.as_deref(), Some(custom));
    }

    #[tokio::test]
    async fn display_whitespace_value_normalizes_to_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        // 先设一个值，再设全空白
        mgr.update_display(serde_json::json!({ "fontSans": "Arial" }))
            .await
            .unwrap();
        mgr.update_display(serde_json::json!({ "fontSans": "   " }))
            .await
            .unwrap();

        assert!(mgr.get_config().display.font_sans.is_none());
        // 持久化层不应包含 fontSans 键（None 序列化省略）
        let disk = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(!disk.contains("fontSans"));
    }

    #[tokio::test]
    async fn display_explicit_null_clears_value() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        mgr.update_display(serde_json::json!({ "fontSans": "Arial" }))
            .await
            .unwrap();
        assert!(mgr.get_config().display.font_sans.is_some());

        mgr.update_display(serde_json::json!({ "fontSans": null }))
            .await
            .unwrap();
        assert!(mgr.get_config().display.font_sans.is_none());
    }

    #[tokio::test]
    async fn display_oversized_font_value_rejected_and_keeps_old() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        let original = "\"Fira Code\", monospace";
        mgr.update_display(serde_json::json!({ "fontMono": original }))
            .await
            .unwrap();

        let huge = "x".repeat(501);
        let err = mgr
            .update_display(serde_json::json!({ "fontMono": huge }))
            .await
            .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));

        assert_eq!(
            mgr.get_config().display.font_mono.as_deref(),
            Some(original),
            "已存值在校验失败时保持不变"
        );
    }

    #[tokio::test]
    async fn display_atomic_batch_rejects_all_on_partial_invalid() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        // 先确保两个字段都有已知值
        mgr.update_display(serde_json::json!({
            "fontSans": "Arial",
            "fontMono": "Menlo",
        }))
        .await
        .unwrap();

        // 同次 update：fontSans 合法，fontMono 超长 → 整次拒绝
        let huge = "x".repeat(501);
        let err = mgr
            .update_display(serde_json::json!({
                "fontSans": "\"JetBrains Mono\", monospace",
                "fontMono": huge,
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));

        let cfg = mgr.get_config();
        assert_eq!(
            cfg.display.font_sans.as_deref(),
            Some("Arial"),
            "fontSans 合法但因 fontMono 失败也不能写入（原子性）"
        );
        assert_eq!(
            cfg.display.font_mono.as_deref(),
            Some("Menlo"),
            "fontMono 失败保持原值"
        );
    }

    #[tokio::test]
    async fn display_reset_to_defaults_clears_font_overrides() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        mgr.update_display(serde_json::json!({
            "fontSans": "\"JetBrains Mono\", monospace",
            "fontMono": "Menlo",
        }))
        .await
        .unwrap();
        assert!(mgr.get_config().display.font_sans.is_some());

        mgr.reset_to_defaults().await.unwrap();
        let cfg = mgr.get_config();
        assert!(cfg.display.font_sans.is_none());
        assert!(cfg.display.font_mono.is_none());
    }

    #[tokio::test]
    async fn update_notifications_warn_on_unknown_key() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        let updates = serde_json::json!({ "fooBar": 123, "enabled": false });
        let result = mgr.update_notifications(updates).await.unwrap();

        assert!(!result.notifications.enabled);
    }
}
