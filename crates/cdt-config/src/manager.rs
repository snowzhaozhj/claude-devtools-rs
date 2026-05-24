//! `ConfigManager` —— 配置加载、保存、更新、合并。
//!
//! 对应 TS `ConfigManager.ts`。核心职责：
//! - 从磁盘加载 JSON 配置
//! - 损坏文件自动备份（修复 TS impl-bug）
//! - partial config 与默认值合并
//! - 分 section 更新 + 持久化
//! - Session pin/unpin、hide/unhide、snooze

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::defaults::default_config;
use crate::error::ConfigError;
use crate::trigger::{TriggerManager, merge_triggers, validate_trigger};
use crate::types::{
    AppConfig, HiddenSession, NotificationTrigger, PinnedSession, SshConfig, SshLastConnection,
};
use crate::types::{ExternalEditor, SearchEngine, TerminalApp};
use crate::validation::{
    normalize_claude_root_path, validate_claude_root_path, validate_http_port,
    validate_search_engine, validate_snooze_minutes, validate_ssh_config,
};

/// 默认配置文件路径：`~/.claude/claude-devtools-config.json`。
fn default_config_path() -> PathBuf {
    let home = cdt_discover::home_dir().unwrap_or_else(|| PathBuf::from("."));
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
    ///
    /// 加载完成后会同步执行 `migrate_composite_ids` 一次性数据迁移：把
    /// `pinned_sessions` / `hidden_sessions` `HashMap` 中含 `::` 形式的旧
    /// composite `project_id` key 折叠为 `base_dir`（详见
    /// `configuration-management::Migrate composite project IDs in pinned sessions on load`
    /// Requirement）。迁移失败 / 写盘失败 SHALL 通过 `warn!` 记录但**不阻塞**启动。
    pub async fn load(&mut self) -> Result<(), ConfigError> {
        self.config = self.load_from_disk().await?;
        let needs_write = migrate_composite_ids(&mut self.config);
        if needs_write {
            if let Err(e) = self.backup_pre_merge_composite().await {
                tracing::warn!(
                    error = %e,
                    "Failed to back up config before composite migration; continuing"
                );
            }
            if let Err(e) = self.persist_config(&self.config).await {
                tracing::warn!(
                    error = %e,
                    "Failed to persist composite-folded config; will retry on next load"
                );
            }
        }
        self.trigger_manager = TriggerManager::new(self.config.notifications.triggers.clone());
        Ok(())
    }

    /// 迁移前备份原配置文件到 `<path>.pre-merge-composite.bak`（覆盖已存在的）。
    async fn backup_pre_merge_composite(&self) -> Result<(), ConfigError> {
        if !tokio::fs::try_exists(&self.config_path)
            .await
            .unwrap_or(false)
        {
            return Ok(());
        }
        let backup_path = PathBuf::from(format!(
            "{}.pre-merge-composite.bak",
            self.config_path.display()
        ));
        tokio::fs::copy(&self.config_path, &backup_path)
            .await
            .map_err(|e| ConfigError::io(&backup_path, e))?;
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

        let tmp_path = self
            .config_path
            .with_extension(format!("{}.tmp", std::process::id()));
        let content = serde_json::to_string_pretty(config)?;

        let write_result: Result<(), ConfigError> = async {
            let file = tokio::fs::File::create(&tmp_path)
                .await
                .map_err(|e| ConfigError::io(&tmp_path, e))?;
            let mut file = tokio::io::BufWriter::new(file);
            tokio::io::AsyncWriteExt::write_all(&mut file, content.as_bytes())
                .await
                .map_err(|e| ConfigError::io(&tmp_path, e))?;
            tokio::io::AsyncWriteExt::flush(&mut file)
                .await
                .map_err(|e| ConfigError::io(&tmp_path, e))?;
            file.into_inner()
                .sync_all()
                .await
                .map_err(|e| ConfigError::io(&tmp_path, e))?;
            tokio::fs::rename(&tmp_path, &self.config_path)
                .await
                .map_err(|e| ConfigError::io(&self.config_path, e))?;
            Ok(())
        }
        .await;

        if write_result.is_err() {
            let _ = tokio::fs::remove_file(&tmp_path).await;
        }
        write_result?;

        tracing::info!("Config saved");
        Ok(())
    }

    async fn commit_next_config(&mut self, next: AppConfig) -> Result<AppConfig, ConfigError> {
        self.persist_config(&next).await?;
        self.trigger_manager = TriggerManager::new(next.notifications.triggers.clone());
        self.config = next;
        Ok(self.get_config())
    }

    // =========================================================================
    // Config updates
    // =========================================================================

    /// 更新 notifications section。
    pub async fn update_notifications(
        &mut self,
        updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let mut next = self.config.clone();
        if let Some(obj) = updates.as_object() {
            for (k, v) in obj {
                match k.as_str() {
                    "enabled" => {
                        if let Some(b) = v.as_bool() {
                            next.notifications.enabled = b;
                        }
                    }
                    "soundEnabled" => {
                        if let Some(b) = v.as_bool() {
                            next.notifications.sound_enabled = b;
                        }
                    }
                    "includeSubagentErrors" => {
                        if let Some(b) = v.as_bool() {
                            next.notifications.include_subagent_errors = b;
                        }
                    }
                    "snoozeMinutes" => {
                        if let Some(n) = v.as_u64() {
                            let minutes = u32::try_from(n)
                                .map_err(|_| ConfigError::validation("snoozeMinutes overflow"))?;
                            validate_snooze_minutes(minutes)?;
                            next.notifications.snooze_minutes = minutes;
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
                        next.notifications.triggers = list;
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
        self.commit_next_config(next).await
    }

    /// 更新 general section。
    pub async fn update_general(
        &mut self,
        updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let mut candidate = self.config.general.clone();
        if let Some(obj) = updates.as_object() {
            for (k, v) in obj {
                match k.as_str() {
                    "launchAtLogin" => {
                        if let Some(b) = v.as_bool() {
                            candidate.launch_at_login = b;
                        }
                    }
                    "showDockIcon" => {
                        if let Some(b) = v.as_bool() {
                            candidate.show_dock_icon = b;
                        }
                    }
                    "theme" => {
                        if let Some(s) = v.as_str() {
                            match s {
                                "dark" | "light" | "system" => {
                                    s.clone_into(&mut candidate.theme);
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
                                    s.clone_into(&mut candidate.default_tab);
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
                        let raw = if v.is_null() {
                            None
                        } else {
                            Some(v.as_str().ok_or_else(|| {
                                ConfigError::validation(
                                    "general.claudeRootPath must be a string or null",
                                )
                            })?)
                        };
                        candidate.claude_root_path = validate_claude_root_path(raw)?;
                    }
                    "autoExpandAiGroups" => {
                        if let Some(b) = v.as_bool() {
                            candidate.auto_expand_ai_groups = b;
                        }
                    }
                    "useNativeTitleBar" => {
                        if let Some(b) = v.as_bool() {
                            candidate.use_native_title_bar = b;
                        }
                    }
                    "sessionClickBehavior" => {
                        if let Some(s) = v.as_str() {
                            match s {
                                "replace" | "new-tab" => {
                                    s.clone_into(&mut candidate.session_click_behavior);
                                }
                                _ => {
                                    return Err(ConfigError::validation(
                                        "general.sessionClickBehavior must be one of: replace, new-tab",
                                    ));
                                }
                            }
                        }
                    }
                    "externalEditor" => {
                        // serde 严格枚举校验：合法值 system / vs_code / cursor / zed / sublime；
                        // invalid 值（如 "vim"）反序列化失败 → ValidationError。
                        let editor: ExternalEditor =
                            serde_json::from_value(v.clone()).map_err(|e| {
                                ConfigError::validation(format!(
                                    "general.externalEditor must be one of: system, vs_code, cursor, zed, sublime ({e})"
                                ))
                            })?;
                        candidate.external_editor = editor;
                    }
                    "searchEngine" => {
                        // internally-tagged enum 反序列化 + Custom variant 额外校验
                        // ({query} 占位符 + scheme http/https）。详 design.md::D3。
                        let engine: SearchEngine =
                            serde_json::from_value(v.clone()).map_err(|e| {
                                ConfigError::validation(format!(
                                    "general.searchEngine must be one of: \
                                     {{type:'google'|'bing'|'duck_duck_go'}} or \
                                     {{type:'custom', urlTemplate:'<URL with {{query}}>'}} ({e})"
                                ))
                            })?;
                        validate_search_engine(&engine)?;
                        candidate.search_engine = engine;
                    }
                    "terminalApp" => {
                        // 统一扁平 enum：跨平台合法集合并集；不匹配当前 OS 时**不**报错，
                        // 仅 tracing::warn 提示 + 运行时 fallback（详 design.md::D3 + spec
                        // configuration-management `terminalApp 跨平台值不报错` Scenario）。
                        let app: TerminalApp = serde_json::from_value(v.clone()).map_err(|e| {
                            ConfigError::validation(format!(
                                "general.terminalApp must be one of: terminal, i_term, warp, \
                                     windows_terminal, cmd, power_shell, x_terminal_emulator, \
                                     gnome_terminal, konsole, alacritty ({e})"
                            ))
                        })?;
                        if !app.is_available_on_current_platform() {
                            tracing::warn!(
                                terminal_app = ?app,
                                current_os = std::env::consts::OS,
                                "general.terminalApp set to a value not available on the current platform; \
                                 will fall back to platform default at open_in_terminal call site"
                            );
                        }
                        candidate.terminal_app = app;
                    }
                    other => {
                        // 未知字段拒绝（spec configuration-management `未知字段拒绝`
                        // Scenario）。其它 update_xxx 仍是 warn-and-ignore 兼容；本节
                        // 由 Phase 2 收紧——前端 Settings 改完应感知后端字段名漂移。
                        return Err(ConfigError::validation(format!(
                            "Unknown general config key: '{other}'"
                        )));
                    }
                }
            }
        }

        let mut next = self.config.clone();
        next.general = candidate;
        self.commit_next_config(next).await
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

        let mut next = self.config.clone();
        if let Some(obj) = updates.as_object() {
            let mut candidate = next.display.clone();
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
                    "timeFormat" => {
                        let Some(s) = v.as_str() else {
                            return Err(ConfigError::validation(
                                "display.timeFormat must be a string: \"24h\" or \"12h\"",
                            ));
                        };
                        candidate.time_format = match s {
                            "24h" => crate::types::TimeFormat::H24,
                            "12h" => crate::types::TimeFormat::H12,
                            _ => {
                                return Err(ConfigError::validation(
                                    "display.timeFormat must be one of: 24h, 12h",
                                ));
                            }
                        };
                    }
                    other => {
                        tracing::warn!(key = %other, "unknown display update key ignored");
                    }
                }
            }
            next.display = candidate;
        }
        self.commit_next_config(next).await
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
        let mut next = self.config.clone();
        if let Some(obj) = updates.as_object() {
            for (k, v) in obj {
                match k.as_str() {
                    "autoUpdateCheckEnabled" => {
                        if let Some(b) = v.as_bool() {
                            next.updater.auto_update_check_enabled = b;
                        }
                    }
                    "skippedUpdateVersion" => {
                        if v.is_null() {
                            next.updater.skipped_update_version = None;
                        } else if let Some(s) = v.as_str() {
                            next.updater.skipped_update_version = Some(s.to_owned());
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
        self.commit_next_config(next).await
    }

    /// 仅写 `httpServer.enabled` 字段，**不**触碰 `port`。
    ///
    /// `server-mode` 的 `http_server_stop` IPC 用此方法把 `enabled=false`
    /// 持久化，让下次启动时 `port` 保留上次成功值（详
    /// `openspec/specs/configuration-management/spec.md` §"HTTP server enabled
    /// / port SHALL be persisted in lockstep with lifecycle"）。
    pub async fn set_http_server_enabled(
        &mut self,
        enabled: bool,
    ) -> Result<AppConfig, ConfigError> {
        let mut next = self.config.clone();
        next.http_server.enabled = enabled;
        self.commit_next_config(next).await
    }

    /// 仅写 `httpServer.port` 字段，先经 `validate_http_port` 校验。
    ///
    /// `server-mode` 的 `http_server_start` IPC 在 bind 成功后调此方法
    /// 持久化用户选的端口（不重置 `enabled`，由调用方按需另调
    /// `set_http_server_enabled`）。
    pub async fn set_http_server_port(&mut self, port: u16) -> Result<AppConfig, ConfigError> {
        validate_http_port(port)?;
        let mut next = self.config.clone();
        next.http_server.port = port;
        self.commit_next_config(next).await
    }

    /// 更新 `httpServer` section。
    pub async fn update_http_server(
        &mut self,
        updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let mut next = self.config.clone();
        if let Some(obj) = updates.as_object() {
            for (k, v) in obj {
                match k.as_str() {
                    "enabled" => {
                        if let Some(b) = v.as_bool() {
                            next.http_server.enabled = b;
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
                            next.http_server.port = port;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.commit_next_config(next).await
    }

    /// 整体替换 `keyboard_shortcuts` 映射（同 `notifications.triggers` 的整体替换语义）。
    ///
    /// `updates` SHALL 是一个 JSON object，键为 `actionId`、值为非空 key combo 字符串。
    /// 空 object 等价于"清空所有自定义快捷键，回退默认"。详
    /// `openspec/specs/configuration-management/spec.md::keyboardShortcuts.update`。
    pub async fn update_keyboard_shortcuts(
        &mut self,
        updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let new_map: HashMap<String, String> = serde_json::from_value(updates).map_err(|e| {
            ConfigError::validation(format!(
                "keyboardShortcuts must be a Record<string, string>: {e}"
            ))
        })?;
        for (action_id, combo) in &new_map {
            if action_id.trim().is_empty() {
                return Err(ConfigError::validation(
                    "keyboardShortcuts: actionId must be a non-empty string",
                ));
            }
            if combo.trim().is_empty() {
                return Err(ConfigError::validation(format!(
                    "keyboardShortcuts.{action_id}: combo must be a non-empty string"
                )));
            }
        }
        let mut next = self.config.clone();
        next.keyboard_shortcuts = new_map;
        self.commit_next_config(next).await
    }

    pub async fn update_ssh(
        &mut self,
        updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let mut candidate = self.config.ssh.clone();
        if let Some(obj) = updates.as_object() {
            for (k, v) in obj {
                match k.as_str() {
                    "profiles" => {
                        candidate.profiles = serde_json::from_value(v.clone()).map_err(|e| {
                            ConfigError::validation(format!(
                                "ssh.profiles must be an array of SSH profiles: {e}"
                            ))
                        })?;
                    }
                    "lastConnection" => {
                        candidate.last_connection = if v.is_null() {
                            None
                        } else {
                            Some(serde_json::from_value(v.clone()).map_err(|e| {
                                ConfigError::validation(format!(
                                    "ssh.lastConnection must be an SSH connection object: {e}"
                                ))
                            })?)
                        };
                    }
                    "autoReconnect" => {
                        if let Some(b) = v.as_bool() {
                            candidate.auto_reconnect = b;
                        }
                    }
                    other => tracing::warn!(key = %other, "unknown ssh update key ignored"),
                }
            }
        }
        validate_ssh_config(&candidate)?;

        let mut next = self.config.clone();
        next.ssh = candidate;
        self.commit_next_config(next).await
    }

    pub async fn save_ssh_last_connection(
        &mut self,
        last_connection: SshLastConnection,
    ) -> Result<AppConfig, ConfigError> {
        let mut next = self.config.clone();
        next.ssh.last_connection = Some(last_connection);
        validate_ssh_config(&next.ssh)?;
        self.commit_next_config(next).await
    }

    pub fn get_ssh_last_connection(&self) -> Option<SshLastConnection> {
        self.config.ssh.last_connection.clone()
    }

    pub fn get_ssh_config(&self) -> SshConfig {
        self.config.ssh.clone()
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
        let mut trigger_manager = self.trigger_manager.clone();
        let mut next = self.config.clone();
        next.notifications.triggers = trigger_manager.add(trigger)?;
        self.commit_next_config(next).await
    }

    pub async fn remove_trigger(&mut self, trigger_id: &str) -> Result<AppConfig, ConfigError> {
        let mut trigger_manager = self.trigger_manager.clone();
        let mut next = self.config.clone();
        next.notifications.triggers = trigger_manager.remove(trigger_id)?;
        self.commit_next_config(next).await
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

        let mut next = self.config.clone();
        next.notifications.snoozed_until = Some(snoozed_until);
        self.commit_next_config(next).await
    }

    /// 清除 snooze。
    pub async fn clear_snooze(&mut self) -> Result<AppConfig, ConfigError> {
        let mut next = self.config.clone();
        next.notifications.snoozed_until = None;
        self.commit_next_config(next).await
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
        let mut next = self.config.clone();
        let pins = next
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
        self.commit_next_config(next).await.map(|_| ())
    }

    /// Unpin 一个 session。
    pub async fn unpin_session(
        &mut self,
        project_id: &str,
        session_id: &str,
    ) -> Result<(), ConfigError> {
        let mut next = self.config.clone();
        if let Some(pins) = next.sessions.pinned_sessions.get_mut(project_id) {
            pins.retain(|p| p.session_id != session_id);
            if pins.is_empty() {
                next.sessions.pinned_sessions.remove(project_id);
            }
            self.commit_next_config(next).await?;
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
        let mut next = self.config.clone();
        let hidden = next
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
        self.commit_next_config(next).await.map(|_| ())
    }

    /// 取消隐藏 session。
    pub async fn unhide_session(
        &mut self,
        project_id: &str,
        session_id: &str,
    ) -> Result<(), ConfigError> {
        let mut next = self.config.clone();
        if let Some(hidden) = next.sessions.hidden_sessions.get_mut(project_id) {
            hidden.retain(|h| h.session_id != session_id);
            if hidden.is_empty() {
                next.sessions.hidden_sessions.remove(project_id);
            }
            self.commit_next_config(next).await?;
        }
        Ok(())
    }

    // =========================================================================
    // Reset / reload
    // =========================================================================

    /// 重置为默认配置。
    pub async fn reset_to_defaults(&mut self) -> Result<AppConfig, ConfigError> {
        self.commit_next_config(default_config()).await
    }

    /// 从磁盘重新加载。
    pub async fn reload(&mut self) -> Result<AppConfig, ConfigError> {
        self.load().await?;
        Ok(self.get_config())
    }
}

/// 一次性折叠 `pinned_sessions` / `hidden_sessions` `HashMap` 里残留的 composite
/// `project_id`（含 `::`）为 `base_dir`，并按 `(session_id, pinned_at / hidden_at)`
/// 去重保留**时间戳最早**的条目。返回是否真的发生了折叠（决定是否需要写盘）。
///
/// Spec：`configuration-management::Migrate composite project IDs in pinned sessions on load`。
/// 本函数 SHALL 是**幂等**的——纯粹基于 input 重写，不依赖任何"已迁移"标志位。
pub(crate) fn migrate_composite_ids(config: &mut AppConfig) -> bool {
    let pinned_changed = fold_composite_keys(
        &mut config.sessions.pinned_sessions,
        |a: &PinnedSession, b: &PinnedSession| a.pinned_at <= b.pinned_at,
        |a: &PinnedSession, b: &PinnedSession| a.session_id == b.session_id,
    );
    let hidden_changed = fold_composite_keys(
        &mut config.sessions.hidden_sessions,
        |a: &HiddenSession, b: &HiddenSession| a.hidden_at <= b.hidden_at,
        |a: &HiddenSession, b: &HiddenSession| a.session_id == b.session_id,
    );
    pinned_changed || hidden_changed
}

fn fold_composite_keys<T, EarlierFn, SameSessionFn>(
    map: &mut std::collections::HashMap<String, Vec<T>>,
    is_earlier: EarlierFn,
    same_session: SameSessionFn,
) -> bool
where
    T: Clone,
    EarlierFn: Fn(&T, &T) -> bool,
    SameSessionFn: Fn(&T, &T) -> bool,
{
    let composite_keys: Vec<String> = map.keys().filter(|k| k.contains("::")).cloned().collect();
    if composite_keys.is_empty() {
        return false;
    }
    for key in composite_keys {
        let Some(entries) = map.remove(&key) else {
            continue;
        };
        let base_dir = key
            .split_once("::")
            .map(|(b, _)| b.to_string())
            .unwrap_or(key);
        let target = map.entry(base_dir).or_default();
        for new_entry in entries {
            if let Some(existing) = target
                .iter_mut()
                .find(|existing| same_session(existing, &new_entry))
            {
                if is_earlier(&new_entry, existing) {
                    *existing = new_entry;
                }
            } else {
                target.push(new_entry);
            }
        }
    }
    true
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
    async fn migrate_composite_ids_folds_pinned_sessions() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // 写入含两个 composite key（同 base_dir 不同 hash）的配置：
        // - "-Users-foo-repo::abcd1234" 含 s1 (pinnedAt=1000)
        // - "-Users-foo-repo::ef567890" 含 s2 (pinnedAt=2000) + s1 (pinnedAt=500)
        // fold 后 base_dir "-Users-foo-repo" 应含 s1(500, 取最早) + s2(2000)
        let body = serde_json::json!({
            "sessions": {
                "pinnedSessions": {
                    "-Users-foo-repo::abcd1234": [
                        { "sessionId": "s1", "pinnedAt": 1000 }
                    ],
                    "-Users-foo-repo::ef567890": [
                        { "sessionId": "s2", "pinnedAt": 2000 },
                        { "sessionId": "s1", "pinnedAt": 500 }
                    ]
                },
                "hiddenSessions": {}
            }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&body).unwrap())
            .await
            .unwrap();

        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();
        let config = mgr.get_config();

        // composite key 已不存在
        assert!(
            !config
                .sessions
                .pinned_sessions
                .contains_key("-Users-foo-repo::abcd1234")
        );
        assert!(
            !config
                .sessions
                .pinned_sessions
                .contains_key("-Users-foo-repo::ef567890")
        );
        let pins = config
            .sessions
            .pinned_sessions
            .get("-Users-foo-repo")
            .expect("base_dir key must exist after fold");
        assert_eq!(pins.len(), 2, "去重后应有 2 条 session: {pins:?}");
        let s1 = pins.iter().find(|p| p.session_id == "s1").unwrap();
        let s2 = pins.iter().find(|p| p.session_id == "s2").unwrap();
        assert_eq!(s1.pinned_at, 500, "s1 应保留 pinned_at 最早");
        assert_eq!(s2.pinned_at, 2000);

        // 备份文件应已生成
        let backup = PathBuf::from(format!("{}.pre-merge-composite.bak", path.display()));
        assert!(backup.exists(), "备份文件应存在");

        // 写回内容已 fold（再次 load 不应再次触发 fold）
        let raw = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(!raw.contains("::"), "落盘后 key 不含 `::`: {raw}");

        // 幂等性：再次 load 不会修改落盘内容
        let mtime_before = tokio::fs::metadata(&path)
            .await
            .unwrap()
            .modified()
            .unwrap();
        // 删掉 backup 让第二次如果触发就重新生成
        tokio::fs::remove_file(&backup).await.unwrap();
        let mut mgr2 = ConfigManager::new(Some(path.clone()));
        mgr2.load().await.unwrap();
        let mtime_after = tokio::fs::metadata(&path)
            .await
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(mtime_before, mtime_after, "幂等 load 不应改 mtime");
        assert!(!backup.exists(), "幂等 load 不应再产生 backup");
    }

    #[tokio::test]
    async fn migrate_composite_ids_does_not_touch_repository_ids() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let body = serde_json::json!({
            "notifications": {
                "triggers": [
                    {
                        "id": "t1",
                        "name": "T1",
                        "enabled": true,
                        "contentType": "tool_result",
                        "mode": "error_status",
                        "repositoryIds": ["/Users/foo/repo/.git", "-with::colon-ok-since-not-our-key"]
                    }
                ]
            },
            "sessions": {
                "pinnedSessions": {
                    "-Users-foo-repo::abcd1234": [{ "sessionId": "s1", "pinnedAt": 1 }]
                },
                "hiddenSessions": {}
            }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&body).unwrap())
            .await
            .unwrap();

        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();
        let config = mgr.get_config();

        // pinned_sessions fold 生效
        assert!(
            config
                .sessions
                .pinned_sessions
                .contains_key("-Users-foo-repo")
        );
        // triggers.repository_ids 字节不变
        let trigger = config
            .notifications
            .triggers
            .iter()
            .find(|t| t.id == "t1")
            .expect("custom trigger t1");
        let repo_ids = trigger.repository_ids.as_ref().expect("repository_ids set");
        assert_eq!(
            repo_ids.as_slice(),
            &[
                "/Users/foo/repo/.git".to_string(),
                "-with::colon-ok-since-not-our-key".to_string()
            ]
        );
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
    async fn http_server_start_persists_enabled_true_and_port() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        // server-mode 的 http_server_start IPC：成功 bind 后顺序调两个 setter
        mgr.set_http_server_port(3500).await.unwrap();
        mgr.set_http_server_enabled(true).await.unwrap();

        // 重 load 验证持久化往返
        let mut mgr2 = ConfigManager::new(Some(path));
        mgr2.load().await.unwrap();
        let cfg = mgr2.get_config();
        assert!(cfg.http_server.enabled);
        assert_eq!(cfg.http_server.port, 3500);
    }

    #[tokio::test]
    async fn http_server_stop_writes_enabled_false_only_preserves_port() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        mgr.set_http_server_port(3500).await.unwrap();
        mgr.set_http_server_enabled(true).await.unwrap();

        mgr.set_http_server_enabled(false).await.unwrap();

        let mut mgr2 = ConfigManager::new(Some(path));
        mgr2.load().await.unwrap();
        let cfg = mgr2.get_config();
        assert!(!cfg.http_server.enabled, "stop SHALL 写 enabled=false");
        assert_eq!(
            cfg.http_server.port, 3500,
            "stop SHALL 保留上次成功 port 不重置为默认"
        );
    }

    #[tokio::test]
    async fn http_server_start_failed_validation_does_not_persist() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        // 模拟 server-mode 的 http_server_start：先校验 port，超范围直接 return Err，
        // 不调 set_http_server_enabled / set_http_server_port → 持久化保持原值
        let err = mgr.set_http_server_port(80).await.unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));

        let cfg = mgr.get_config();
        assert!(!cfg.http_server.enabled);
        assert_eq!(cfg.http_server.port, 3456);
    }

    #[tokio::test]
    async fn http_server_config_missing_in_old_file_uses_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // 老配置：完全没 httpServer 字段
        tokio::fs::write(
            &path,
            r#"{"display":{"showTimestamps":false,"compactMode":false,"syntaxHighlighting":true}}"#,
        )
        .await
        .unwrap();

        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();
        let cfg = mgr.get_config();
        assert!(!cfg.http_server.enabled, "缺字段默认 enabled=false");
        assert_eq!(cfg.http_server.port, 3456, "缺字段默认 port=3456");
    }

    #[tokio::test]
    async fn http_server_partial_section_missing_one_field_uses_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // 老配置：httpServer 节点存在但只有 enabled 缺 port
        tokio::fs::write(&path, r#"{"httpServer":{"enabled":true}}"#)
            .await
            .unwrap();

        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();
        let cfg = mgr.get_config();
        assert!(cfg.http_server.enabled);
        assert_eq!(cfg.http_server.port, 3456, "缺 port 字段用默认 3456");
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

    // =========================================================================
    // Phase 2: external_editor / search_engine / terminal_app
    // 详 openspec/changes/frontend-context-menu-phase-2/specs/configuration-management/spec.md
    // =========================================================================

    async fn setup_general_test_manager() -> (tempfile::TempDir, ConfigManager) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();
        (dir, mgr)
    }

    #[tokio::test]
    async fn general_external_editor_default_is_system() {
        let (_d, mgr) = setup_general_test_manager().await;
        assert_eq!(
            mgr.get_config().general.external_editor,
            ExternalEditor::System
        );
    }

    #[tokio::test]
    async fn general_external_editor_round_trip_vs_code() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let result = mgr
            .update_general(serde_json::json!({ "externalEditor": "vs_code" }))
            .await
            .unwrap();
        assert_eq!(result.general.external_editor, ExternalEditor::VsCode);

        // serde 输出 camelCase + snake_case enum
        let json = serde_json::to_value(&result.general).unwrap();
        assert_eq!(json["externalEditor"], serde_json::json!("vs_code"));
    }

    #[tokio::test]
    async fn general_external_editor_invalid_value_rejected() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let err = mgr
            .update_general(serde_json::json!({ "externalEditor": "vim" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("externalEditor"));
        // 已存值未变
        assert_eq!(
            mgr.get_config().general.external_editor,
            ExternalEditor::System
        );
    }

    #[tokio::test]
    async fn general_search_engine_default_is_google() {
        let (_d, mgr) = setup_general_test_manager().await;
        assert_eq!(mgr.get_config().general.search_engine, SearchEngine::Google);
    }

    #[tokio::test]
    async fn general_search_engine_serializes_internally_tagged() {
        let (_d, mgr) = setup_general_test_manager().await;
        let json = serde_json::to_value(&mgr.get_config().general).unwrap();
        // internally tagged enum 序列化为 { "type": "google" }
        assert_eq!(
            json["searchEngine"],
            serde_json::json!({ "type": "google" })
        );
    }

    #[tokio::test]
    async fn general_search_engine_round_trip_duck_duck_go() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let result = mgr
            .update_general(serde_json::json!({
                "searchEngine": { "type": "duck_duck_go" }
            }))
            .await
            .unwrap();
        assert_eq!(result.general.search_engine, SearchEngine::DuckDuckGo);
    }

    #[tokio::test]
    async fn general_search_engine_round_trip_custom() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let result = mgr
            .update_general(serde_json::json!({
                "searchEngine": {
                    "type": "custom",
                    "urlTemplate": "https://kagi.com/search?q={query}"
                }
            }))
            .await
            .unwrap();
        let SearchEngine::Custom { url_template } = &result.general.search_engine else {
            panic!("expected Custom variant");
        };
        assert_eq!(url_template, "https://kagi.com/search?q={query}");

        // round-trip 序列化形态
        let json = serde_json::to_value(&result.general).unwrap();
        assert_eq!(
            json["searchEngine"],
            serde_json::json!({
                "type": "custom",
                "urlTemplate": "https://kagi.com/search?q={query}"
            })
        );
    }

    #[tokio::test]
    async fn general_search_engine_custom_missing_query_placeholder_rejected() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let err = mgr
            .update_general(serde_json::json!({
                "searchEngine": {
                    "type": "custom",
                    "urlTemplate": "https://example.com/search"
                }
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("{query}"));
        assert_eq!(mgr.get_config().general.search_engine, SearchEngine::Google);
    }

    #[tokio::test]
    async fn general_search_engine_custom_javascript_scheme_rejected() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let err = mgr
            .update_general(serde_json::json!({
                "searchEngine": {
                    "type": "custom",
                    "urlTemplate": "javascript:alert({query})"
                }
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("scheme"));
    }

    #[tokio::test]
    async fn general_search_engine_invalid_tag_rejected() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let err = mgr
            .update_general(serde_json::json!({
                "searchEngine": { "type": "yahoo" }
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("searchEngine"));
    }

    #[tokio::test]
    async fn general_terminal_app_default_is_terminal() {
        let (_d, mgr) = setup_general_test_manager().await;
        assert_eq!(mgr.get_config().general.terminal_app, TerminalApp::Terminal);
    }

    #[tokio::test]
    async fn general_terminal_app_iterm_serializes_as_i_term() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let result = mgr
            .update_general(serde_json::json!({ "terminalApp": "i_term" }))
            .await
            .unwrap();
        assert_eq!(result.general.terminal_app, TerminalApp::ITerm);

        // 注意 serde rename_all = "snake_case" 对 ITerm 输出 "i_term" 不是 "iterm"
        let json = serde_json::to_value(&result.general).unwrap();
        assert_eq!(json["terminalApp"], serde_json::json!("i_term"));
    }

    #[tokio::test]
    async fn general_terminal_app_cross_platform_value_accepted_no_error() {
        // 统一扁平 enum：跨平台值都合法（仅运行时 warn + fallback，不返回错误）
        let (_d, mut mgr) = setup_general_test_manager().await;
        // 在任何平台上接受跨平台 enum 值；只 warn
        let result = mgr
            .update_general(serde_json::json!({ "terminalApp": "konsole" }))
            .await
            .unwrap();
        assert_eq!(result.general.terminal_app, TerminalApp::Konsole);
    }

    #[tokio::test]
    async fn general_terminal_app_invalid_value_rejected() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let err = mgr
            .update_general(serde_json::json!({ "terminalApp": "fish" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("terminalApp"));
    }

    #[tokio::test]
    async fn general_unknown_field_rejected() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let err = mgr
            .update_general(serde_json::json!({ "unknownField": "value" }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Unknown general config key"));
    }

    #[tokio::test]
    async fn general_three_new_fields_persist_to_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        mgr.update_general(serde_json::json!({
            "externalEditor": "vs_code",
            "searchEngine": {
                "type": "custom",
                "urlTemplate": "https://example.com/?q={query}"
            },
            "terminalApp": "i_term"
        }))
        .await
        .unwrap();

        // 重新 load 验证持久化
        let mut mgr2 = ConfigManager::new(Some(path));
        mgr2.load().await.unwrap();
        let cfg = mgr2.get_config();
        assert_eq!(cfg.general.external_editor, ExternalEditor::VsCode);
        assert_eq!(cfg.general.terminal_app, TerminalApp::ITerm);
        let SearchEngine::Custom { url_template } = &cfg.general.search_engine else {
            panic!("expected Custom variant after reload");
        };
        assert_eq!(url_template, "https://example.com/?q={query}");
    }

    #[tokio::test]
    async fn general_legacy_config_missing_three_fields_uses_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // 写入老配置：general 段没有三新字段
        let body = serde_json::json!({
            "general": {
                "launchAtLogin": false,
                "showDockIcon": true,
                "theme": "system",
                "defaultTab": "dashboard",
                "claudeRootPath": null,
                "autoExpandAiGroups": false,
                "useNativeTitleBar": false
            }
        });
        tokio::fs::write(&path, serde_json::to_string(&body).unwrap())
            .await
            .unwrap();

        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();
        let cfg = mgr.get_config();
        // serde(default) 让旧配置兼容
        assert_eq!(cfg.general.external_editor, ExternalEditor::System);
        assert_eq!(cfg.general.search_engine, SearchEngine::Google);
        assert_eq!(cfg.general.terminal_app, TerminalApp::Terminal);
    }
}
