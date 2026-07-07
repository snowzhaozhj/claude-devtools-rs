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

use serde::Deserialize;

use crate::defaults::default_config;
use crate::error::ConfigError;
use crate::trigger::{TriggerManager, merge_triggers, validate_trigger};
use crate::types::{
    AppConfig, DefaultTab, HiddenSession, NotificationTrigger, PinnedSession, SessionClickBehavior,
    SshConfig, SshLastConnection, SshProfile, Theme, TimeFormat,
};
use crate::types::{ExternalEditor, SearchEngine, TerminalApp};
use crate::validation::{
    normalize_claude_root_path, push_recent_root, sanitize_recent_roots, validate_claude_root_path,
    validate_http_port, validate_search_engine, validate_snooze_minutes, validate_ssh_config,
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
    /// 单调递增版本号；每次 `commit_next_config` 成功 +1。客户端可在 partial body
    /// 里加 `_version` 字段做 optimistic concurrency check 防 last-write-wins
    /// （缺省 = skip 校验，向后兼容；session-local，新建实例从 0 起）。
    version: u64,
    /// CLI `--root` 临时覆盖数据根：独立于 `config`，**不进** `persist_config`
    /// （后者只序列化 `self.config`），保证 serve 模式下任何 `update_*` 落盘都
    /// 不含 override（change flexible-data-root D3/F3 + code-reviewer 二审：原
    /// 方案把 override 写进 `config` 会经 `update_*` 的整份 persist 泄漏到磁盘）。
    root_override: Option<String>,
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
            version: 0,
            root_override: None,
        }
    }

    /// 当前 config 版本号。每次 `update_*` 成功 +1（session-local，不持久化）。
    /// 客户端拿到此值后可在下一次 `update_*` partial body 里加 `_version` 字段做
    /// optimistic concurrency check：传入值与当前不一致 → `ConfigError::Validation`。
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
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
        // 加载时过滤 recentRoots 非法项 + 去重（F7）；清洗后的值随下次 update 落盘。
        self.config.general.recent_roots = sanitize_recent_roots(&self.config.general.recent_roots);
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

    /// 设置 CLI `--root` 临时数据根覆盖：存入独立 `root_override` 字段，**不**碰
    /// `self.config`，故 `persist_config` / 任何 `update_*` 落盘都不含 override
    /// （change `flexible-data-root` D3/F3）。`root` 支持 `~/` 原形，展开推迟到消费点。
    pub fn set_claude_root_override(&mut self, root: &str) {
        self.root_override = Some(root.to_owned());
    }

    /// 数据根解析优先级：CLI `--root` override > 持久化 `claudeRootPath` > 默认。
    /// 所有读侧消费者（projects / todos / `claude_base` 派生）SHALL 用此方法而非
    /// 直接读 `config.general.claude_root_path`，让 `--root` 覆盖生效且不落盘。
    #[must_use]
    pub fn effective_claude_root(&self) -> Option<&str> {
        self.root_override
            .as_deref()
            .or(self.config.general.claude_root_path.as_deref())
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
        // version 仅在持久化成功后递增——失败时客户端用旧 version 重试 partial。
        // wrapping_add 防理论上的 overflow（实际 u64 一辈子用不完）。
        self.version = self.version.wrapping_add(1);
        Ok(self.get_config())
    }

    /// 校验 client 传入的 `_version` 是否匹配当前 server 版本；缺省（None）= skip
    /// 校验，保留向后兼容。spec：configuration-management 防 last-write-wins。
    fn check_version(&self, expected: Option<u64>) -> Result<(), ConfigError> {
        let Some(expected) = expected else {
            return Ok(());
        };
        if expected != self.version {
            return Err(ConfigError::validation(format!(
                "Config version mismatch: client expected {expected}, server is at {}. \
                 Re-fetch and retry.",
                self.version
            )));
        }
        Ok(())
    }

    // =========================================================================
    // Config updates
    // =========================================================================

    /// 更新 notifications section。
    pub async fn update_notifications(
        &mut self,
        mut updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let expected_version = extract_version(&mut updates)?;
        self.check_version(expected_version)?;

        // typed Partial deserialize：失败 → ConfigError::Validation。serde 默认
        // ignore unknown fields，保 update_notifications_warn_on_unknown_key 兼容。
        warn_unknown_keys(
            "notifications",
            &updates,
            &[
                "enabled",
                "soundEnabled",
                "includeSubagentErrors",
                "snoozeMinutes",
                "triggers",
            ],
        );
        let partial: PartialNotificationConfig = deserialize_partial("notifications", updates)?;

        let mut next = self.config.clone();
        if let Some(v) = partial.enabled {
            next.notifications.enabled = v;
        }
        if let Some(v) = partial.sound_enabled {
            next.notifications.sound_enabled = v;
        }
        if let Some(v) = partial.include_subagent_errors {
            next.notifications.include_subagent_errors = v;
        }
        if let Some(minutes) = partial.snooze_minutes {
            validate_snooze_minutes(minutes)?;
            next.notifications.snooze_minutes = minutes;
        }
        if let Some(triggers) = partial.triggers {
            for t in &triggers {
                let r = validate_trigger(t);
                if !r.valid {
                    return Err(ConfigError::validation(format!(
                        "Invalid trigger \"{}\": {}",
                        t.id,
                        r.errors.join(", ")
                    )));
                }
            }
            next.notifications.triggers = triggers;
        }

        self.commit_next_config(next).await
    }

    /// 更新 general section。
    ///
    /// 严格未知字段（`#[serde(deny_unknown_fields)]`）：未列出 key 反序列化失败 →
    /// `Unknown general config key: '<field>'`（spec configuration-management
    /// `未知字段拒绝` Scenario）。其它 `update_*` 仍是 ignore unknown 兼容前端。
    pub async fn update_general(
        &mut self,
        mut updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let expected_version = extract_version(&mut updates)?;
        self.check_version(expected_version)?;

        let partial: PartialGeneralConfig = deserialize_partial("general", updates)?;

        let mut candidate = self.config.general.clone();
        if let Some(v) = partial.launch_at_login {
            candidate.launch_at_login = v;
        }
        if let Some(v) = partial.show_dock_icon {
            candidate.show_dock_icon = v;
        }
        if let Some(v) = partial.theme {
            candidate.theme = v;
        }
        if let Some(v) = partial.default_tab {
            candidate.default_tab = v;
        }
        if let Some(opt) = partial.claude_root_path {
            candidate.claude_root_path = validate_claude_root_path(opt.as_deref())?;
            // 写入非 null 数据根时 append MRU 历史（去重 + 上限 + 过滤非法）。
            if let Some(root) = &candidate.claude_root_path {
                candidate.recent_roots = push_recent_root(&candidate.recent_roots, root);
            }
        }
        if let Some(v) = partial.auto_expand_ai_groups {
            candidate.auto_expand_ai_groups = v;
        }
        if let Some(v) = partial.use_native_title_bar {
            candidate.use_native_title_bar = v;
        }
        if let Some(v) = partial.session_click_behavior {
            candidate.session_click_behavior = v;
        }
        if let Some(v) = partial.external_editor {
            candidate.external_editor = v;
        }
        if let Some(engine) = partial.search_engine {
            // internally-tagged enum 反序列化已过；Custom variant 额外校验
            // ({query} 占位符 + scheme http/https），详 design.md::D3。
            validate_search_engine(&engine)?;
            candidate.search_engine = engine;
        }
        if let Some(app) = partial.terminal_app {
            // 统一扁平 enum：跨平台合法集合并集；不匹配当前 OS 时**不**报错，
            // 仅 tracing::warn 提示 + 运行时 fallback（spec configuration-management
            // `terminalApp 跨平台值不报错` Scenario）。
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
        mut updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        const FONT_FAMILY_MAX_LEN: usize = 500;

        let expected_version = extract_version(&mut updates)?;
        self.check_version(expected_version)?;

        warn_unknown_keys(
            "display",
            &updates,
            &[
                "showTimestamps",
                "compactMode",
                "syntaxHighlighting",
                "fontSans",
                "fontMono",
                "timeFormat",
            ],
        );
        let partial: PartialDisplayConfig = deserialize_partial("display", updates)?;

        let mut next = self.config.clone();
        let mut candidate = next.display.clone();
        if let Some(v) = partial.show_timestamps {
            candidate.show_timestamps = v;
        }
        if let Some(v) = partial.compact_mode {
            candidate.compact_mode = v;
        }
        if let Some(v) = partial.syntax_highlighting {
            candidate.syntax_highlighting = v;
        }
        if let Some(value) = partial.font_sans {
            candidate.font_sans =
                normalize_font_family_value(value, "fontSans", FONT_FAMILY_MAX_LEN)?;
        }
        if let Some(value) = partial.font_mono {
            candidate.font_mono =
                normalize_font_family_value(value, "fontMono", FONT_FAMILY_MAX_LEN)?;
        }
        if let Some(format) = partial.time_format {
            candidate.time_format = format;
        }
        next.display = candidate;
        self.commit_next_config(next).await
    }

    /// 更新 updater section。
    ///
    /// 支持字段：
    /// - `autoUpdateCheckEnabled: bool` —— 启动后台自动检查开关
    /// - `skippedUpdateVersion: string | null` —— null 清空、字符串写入
    pub async fn update_updater(
        &mut self,
        mut updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let expected_version = extract_version(&mut updates)?;
        self.check_version(expected_version)?;

        warn_unknown_keys(
            "updater",
            &updates,
            &["autoUpdateCheckEnabled", "skippedUpdateVersion"],
        );
        let partial: PartialUpdaterConfig = deserialize_partial("updater", updates)?;

        let mut next = self.config.clone();
        if let Some(v) = partial.auto_update_check_enabled {
            next.updater.auto_update_check_enabled = v;
        }
        if let Some(opt) = partial.skipped_update_version {
            next.updater.skipped_update_version = opt;
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
        mut updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let expected_version = extract_version(&mut updates)?;
        self.check_version(expected_version)?;

        warn_unknown_keys("httpServer", &updates, &["enabled", "port"]);
        let partial: PartialHttpServerConfig = deserialize_partial("httpServer", updates)?;

        let mut next = self.config.clone();
        if let Some(v) = partial.enabled {
            next.http_server.enabled = v;
        }
        if let Some(port) = partial.port {
            validate_http_port(port)?;
            next.http_server.port = port;
        }
        self.commit_next_config(next).await
    }

    /// 整体替换 `keyboard_shortcuts` 映射（同 `notifications.triggers` 的整体替换语义）。
    ///
    /// `updates` SHALL 是一个 JSON object，键为 `actionId`、值为非空 key combo 字符串；
    /// 客户端可附加 `_version: <u64>` 做 optimistic concurrency check（缺省 skip）。
    /// 空 object 等价于"清空所有自定义快捷键，回退默认"。详
    /// `openspec/specs/configuration-management/spec.md::keyboardShortcuts.update`。
    pub async fn update_keyboard_shortcuts(
        &mut self,
        mut updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let expected_version = extract_version(&mut updates)?;
        self.check_version(expected_version)?;

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
        mut updates: serde_json::Value,
    ) -> Result<AppConfig, ConfigError> {
        let expected_version = extract_version(&mut updates)?;
        self.check_version(expected_version)?;

        warn_unknown_keys(
            "ssh",
            &updates,
            &["profiles", "lastConnection", "autoReconnect"],
        );
        let partial: PartialSshConfig = deserialize_partial("ssh", updates)?;

        let mut candidate = self.config.ssh.clone();
        if let Some(profiles) = partial.profiles {
            candidate.profiles = profiles;
        }
        if let Some(last) = partial.last_connection {
            candidate.last_connection = last;
        }
        if let Some(v) = partial.auto_reconnect {
            candidate.auto_reconnect = v;
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

/// 把已 typed deserialize 的 `Option<String>`（可能为 null/empty/whitespace）归一化
/// 为 `Option<String>` 用于 font-family 类字段：
/// - `None`（即客户端传入 `null`）→ `None`
/// - 字符串 trim 后为空 → `None`
/// - 字符串长度（trim 后）> `max_len` → validation error
/// - 其余 → `Some(s.trim())`
fn normalize_font_family_value(
    value: Option<String>,
    field_name: &str,
    max_len: usize,
) -> Result<Option<String>, ConfigError> {
    let Some(s) = value else {
        return Ok(None);
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

// =========================================================================
// Partial<XConfig> structs + 通用 helpers（typed update_* 反序列化路径）
//
// 设计目标（详 `crates/CLAUDE.md::ConfigManager::update_<section> 是手写白名单
// dispatch`）：把 7 个 `update_*` 的"手写 match k.as_str() → 字面量校验"统一
// 改成"typed PartialXConfig deserialize → 应用 Some-字段 → 后置 validation"。
// 字段类型由 serde 直接校验，enum 字面量集合由 `Theme/DefaultTab/...` 等类型
// 表达，加字段时 struct 改一处即可（不再有"漏 match 分支静默 drop"风险）。
//
// `_version: Option<u64>` optimistic concurrency check：从 JSON Value 中
// 单独抽取（`extract_version`），不放进 PartialXConfig 字段——这样
// `update_keyboard_shortcuts`（HashMap 直接 deserialize）也能复用同一机制。
// =========================================================================

/// 从 partial JSON object 中抽取并移除 `_version` 字段。返回客户端期望的版本号
/// （`None` = 客户端未带，跳过 concurrency check 保前向兼容）。
fn extract_version(updates: &mut serde_json::Value) -> Result<Option<u64>, ConfigError> {
    let Some(obj) = updates.as_object_mut() else {
        return Ok(None);
    };
    let Some(v) = obj.remove("_version") else {
        return Ok(None);
    };
    if v.is_null() {
        return Ok(None);
    }
    v.as_u64()
        .map(Some)
        .ok_or_else(|| ConfigError::validation("_version must be a non-negative integer or null"))
}

/// 对于不走 `deny_unknown_fields` 的 lenient partial struct，在 deserialize 前
/// 扫描 input object keys，未在 `known_keys` 内的发 `tracing::warn!` 诊断信号。
/// 恢复 PR #285 之前的行为（issue #290）。
fn warn_unknown_keys(section: &str, updates: &serde_json::Value, known_keys: &[&str]) {
    let Some(obj) = updates.as_object() else {
        return;
    };
    for key in obj.keys() {
        if !known_keys.contains(&key.as_str()) {
            tracing::warn!(
                section = section,
                key = key.as_str(),
                "Unknown config key in partial update (ignored)"
            );
        }
    }
}

/// 用 `serde_path_to_error` 反序列化 typed Partial：错误信息携带字段路径
/// （如 `external_editor: unknown variant 'vim'...`），便于前端定位问题字段。
/// `deny_unknown_fields` 触发的 "unknown field `xxx`" 转写成历史兼容的
/// `"Unknown <section> config key: '<field>'"`（对应
/// `general_unknown_field_rejected` 测试契约）。
fn deserialize_partial<T>(section: &str, updates: serde_json::Value) -> Result<T, ConfigError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_path_to_error::deserialize::<_, T>(updates).map_err(|err| {
        let inner_msg = err.inner().to_string();
        if let Some(field) = parse_unknown_field(&inner_msg) {
            return ConfigError::validation(format!("Unknown {section} config key: '{field}'"));
        }
        let path = err.path().to_string();
        if path.is_empty() || path == "." {
            ConfigError::validation(format!("Invalid {section} config update: {inner_msg}"))
        } else {
            // serde_path_to_error 给出的 path 是 snake_case（struct 字段名）；
            // 把它映射回前端使用的 camelCase（与 ConfigError 历史输出形态对齐）。
            let camel_path = snake_path_to_camel(&path);
            ConfigError::validation(format!(
                "Invalid {section} config update at `{camel_path}`: {inner_msg}"
            ))
        }
    })
}

/// 从 serde 错误文本解析 "unknown field `xxx`" 中的字段名。
fn parse_unknown_field(msg: &str) -> Option<String> {
    let start = msg.find("unknown field `")?;
    let rest = &msg[start + "unknown field `".len()..];
    let end = rest.find('`')?;
    Some(rest[..end].to_owned())
}

/// 把 `serde_path_to_error` 输出的 `snake_case` 字段路径（如
/// `external_editor` / `search_engine.url_template`）转成前端 `camelCase`
/// （`externalEditor` / `searchEngine.urlTemplate`）便于错误信息直接定位 IPC payload。
fn snake_path_to_camel(path: &str) -> String {
    path.split('.')
        .map(snake_to_camel_segment)
        .collect::<Vec<_>>()
        .join(".")
}

fn snake_to_camel_segment(seg: &str) -> String {
    let mut out = String::with_capacity(seg.len());
    let mut upper_next = false;
    for ch in seg.chars() {
        if ch == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// 三态 Option 反序列化：absent → `None` / null → `Some(None)` / value → `Some(Some(v))`。
/// 用于 `claude_root_path` / `font_sans` / `font_mono` / `skipped_update_version`
/// / `last_connection` 等需要区分"未传"（保留旧值）与"显式清空"（写 null）的字段。
/// `clippy::option_option` 推荐改 enum 但本场景三态正是 Option<Option<T>> 的
/// 经典语义；改 enum 反而失去 serde 默认 Option 处理能力。
#[allow(clippy::option_option)]
fn deserialize_double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartialNotificationConfig {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    sound_enabled: Option<bool>,
    #[serde(default)]
    include_subagent_errors: Option<bool>,
    #[serde(default)]
    snooze_minutes: Option<u32>,
    #[serde(default)]
    triggers: Option<Vec<NotificationTrigger>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::option_option)]
struct PartialGeneralConfig {
    #[serde(default)]
    launch_at_login: Option<bool>,
    #[serde(default)]
    show_dock_icon: Option<bool>,
    #[serde(default)]
    theme: Option<Theme>,
    #[serde(default)]
    default_tab: Option<DefaultTab>,
    /// 三态：absent / null（清空）/ Some(s)。详 `deserialize_double_option`。
    #[serde(default, deserialize_with = "deserialize_double_option")]
    claude_root_path: Option<Option<String>>,
    #[serde(default)]
    auto_expand_ai_groups: Option<bool>,
    #[serde(default)]
    use_native_title_bar: Option<bool>,
    #[serde(default)]
    session_click_behavior: Option<SessionClickBehavior>,
    #[serde(default)]
    external_editor: Option<ExternalEditor>,
    #[serde(default)]
    search_engine: Option<SearchEngine>,
    #[serde(default)]
    terminal_app: Option<TerminalApp>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::option_option)]
struct PartialDisplayConfig {
    #[serde(default)]
    show_timestamps: Option<bool>,
    #[serde(default)]
    compact_mode: Option<bool>,
    #[serde(default)]
    syntax_highlighting: Option<bool>,
    /// 三态：absent / null（清空）/ Some(s)。`normalize_font_family_value`
    /// 接管 trim/length 校验。
    #[serde(default, deserialize_with = "deserialize_double_option")]
    font_sans: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    font_mono: Option<Option<String>>,
    #[serde(default)]
    time_format: Option<TimeFormat>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::option_option)]
struct PartialUpdaterConfig {
    #[serde(default)]
    auto_update_check_enabled: Option<bool>,
    /// 三态：absent / null（清空）/ Some(s)。
    #[serde(default, deserialize_with = "deserialize_double_option")]
    skipped_update_version: Option<Option<String>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartialHttpServerConfig {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    port: Option<u16>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::option_option)]
struct PartialSshConfig {
    #[serde(default)]
    profiles: Option<Vec<SshProfile>>,
    /// 三态：absent / null（清空）/ Some(连接对象）。
    #[serde(default, deserialize_with = "deserialize_double_option")]
    last_connection: Option<Option<SshLastConnection>>,
    #[serde(default)]
    auto_reconnect: Option<bool>,
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
    async fn set_claude_root_override_does_not_leak_to_disk_even_through_update() {
        // CLI `--root` 覆盖：走独立 root_override 字段，不进 self.config，故任何
        // update_* 的整份 persist 都不泄漏 override（change flexible-data-root D3/F3
        // + code-reviewer 二审：serve 模式 PATCH /api/config 曾会把 override 落盘）。
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path.clone()));
        mgr.load().await.unwrap();

        mgr.set_claude_root_override("~/.qoder");
        // override 经 effective_claude_root 生效，但不进 config
        assert_eq!(mgr.effective_claude_root(), Some("~/.qoder"));
        assert_eq!(
            mgr.get_config().general.claude_root_path,
            None,
            "override MUST NOT 进 config（否则 update_* 会 persist 泄漏）"
        );

        // 关键回归：override 生效期间 update 无关字段 → 触发整份 persist
        mgr.update_general(serde_json::json!({ "theme": "dark" }))
            .await
            .unwrap();

        let mut mgr2 = ConfigManager::new(Some(path));
        mgr2.load().await.unwrap();
        assert_eq!(
            mgr2.get_config().general.claude_root_path,
            None,
            "update_* persist 后磁盘 claudeRootPath SHALL 仍不含 --root override"
        );
        assert_eq!(
            mgr2.get_config().general.theme,
            Theme::Dark,
            "无关字段 theme SHALL 正常落盘"
        );
    }

    #[tokio::test]
    async fn load_tolerates_non_string_recent_roots_without_dropping_other_fields() {
        // codex 二审：recentRoots 含非字符串项若走严格 Vec<String> 会让整份 config
        // 反序列化失败回退默认，丢掉 httpServer.port 等无关字段。lenient 反序列化
        // 跳过坏项，保留其余配置。
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let body = serde_json::json!({
            "general": {
                "launchAtLogin": false,
                "showDockIcon": true,
                "theme": "dark",
                "defaultTab": "sessions",
                "claudeRootPath": "/tmp/cdt-root",
                "recentRoots": ["/tmp/cdt-root", 42, "relative/bad"],
                "autoExpandAiGroups": false,
                "sessionClickBehavior": "replace"
            },
            "httpServer": { "enabled": true, "port": 4567 }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&body).unwrap())
            .await
            .unwrap();

        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();
        let cfg = mgr.get_config();

        // 无关字段保留（未整份回默认）
        assert_eq!(cfg.http_server.port, 4567);
        assert_eq!(cfg.general.theme, Theme::Dark);
        assert_eq!(
            cfg.general.claude_root_path.as_deref(),
            Some("/tmp/cdt-root")
        );
        // 非字符串项跳过 + 非法字符串项（相对路径）被 sanitize 过滤
        assert_eq!(cfg.general.recent_roots, vec!["/tmp/cdt-root".to_owned()]);
    }

    #[tokio::test]
    async fn override_takes_precedence_over_patched_claude_root_path() {
        // override 生效时 PATCH claudeRootPath：effective_claude_root 仍返回 override
        // （运行时 reconfigure 用 effective，不被 PATCH 绕过）——codex 二审。
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();

        mgr.set_claude_root_override("/tmp/override");
        mgr.update_general(serde_json::json!({ "claudeRootPath": "/tmp/patched" }))
            .await
            .unwrap();

        assert_eq!(
            mgr.effective_claude_root(),
            Some("/tmp/override"),
            "override SHALL 优先于 PATCH 的 claudeRootPath（运行时 reconfigure 用它）"
        );
        assert_eq!(
            mgr.get_config().general.claude_root_path.as_deref(),
            Some("/tmp/patched"),
            "PATCH 值仍写入 config 持久化层，只是 effective 读侧用 override"
        );
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
        assert_eq!(config.general.theme, Theme::System);
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

    // =========================================================================
    // Lenient enum fallback for stored config（codex PR #285 review blocking 修复）
    //
    // 老 / 外部修改的 config.json 含未知 enum 字面量（如 `theme: "auto"`）时，
    // 走 `deserialize_lenient_enum`：该字段 fallback 到 Default + warn，**不**让
    // 整个 AppConfig deserialize 失败 → merge_with_defaults 整体回退到默认。
    // =========================================================================

    #[tokio::test]
    async fn load_unknown_theme_value_falls_back_to_default_and_keeps_other_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        // 写入一份合法配置但 theme 是未知值（模拟老版本 / 外部工具改过）
        let body = serde_json::json!({
            "general": {
                "launchAtLogin": true,
                "showDockIcon": false,
                "theme": "auto",
                "defaultTab": "dashboard",
                "claudeRootPath": null,
                "autoExpandAiGroups": true,
                "useNativeTitleBar": true,
                "sessionClickBehavior": "replace"
            },
            "httpServer": { "enabled": true, "port": 17000 }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&body).unwrap())
            .await
            .unwrap();

        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();
        let cfg = mgr.get_config();

        // theme 单字段 fallback 到 Default
        assert_eq!(cfg.general.theme, Theme::System);
        // 同 section 其它字段保留（不再"整体回退到默认"）
        assert!(cfg.general.launch_at_login);
        assert!(!cfg.general.show_dock_icon);
        assert!(cfg.general.auto_expand_ai_groups);
        assert!(cfg.general.use_native_title_bar);
        // 其它 section 也保留
        assert!(cfg.http_server.enabled);
        assert_eq!(cfg.http_server.port, 17000);
    }

    #[tokio::test]
    async fn load_unknown_default_tab_value_falls_back_keeps_theme() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let body = serde_json::json!({
            "general": {
                "launchAtLogin": false,
                "showDockIcon": true,
                "theme": "dark",
                "defaultTab": "imaginary-future-tab",
                "claudeRootPath": null,
                "autoExpandAiGroups": false,
                "useNativeTitleBar": false,
                "sessionClickBehavior": "new-tab"
            }
        });
        tokio::fs::write(&path, serde_json::to_string_pretty(&body).unwrap())
            .await
            .unwrap();

        let mut mgr = ConfigManager::new(Some(path));
        mgr.load().await.unwrap();
        let cfg = mgr.get_config();
        // 仅 default_tab 字段 fallback；其余合法 enum 字段保留
        assert_eq!(cfg.general.default_tab, DefaultTab::Dashboard);
        assert_eq!(cfg.general.theme, Theme::Dark);
        assert_eq!(
            cfg.general.session_click_behavior,
            SessionClickBehavior::NewTab
        );
    }

    #[tokio::test]
    async fn update_general_strict_rejects_unknown_enum_after_load_fallback() {
        // load 路径 lenient 但 update 路径仍严格。spec configuration-management
        // `Validate configuration fields before persistence` Scenario 守护。
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let body = serde_json::json!({
            "general": {
                "theme": "auto",
                "defaultTab": "dashboard",
                "sessionClickBehavior": "replace",
                "launchAtLogin": false,
                "showDockIcon": true,
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
        // load fallback 后 theme = System
        assert_eq!(mgr.get_config().general.theme, Theme::System);

        // update_general 仍拒绝非法 theme
        let err = mgr
            .update_general(serde_json::json!({ "theme": "auto" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert_eq!(mgr.get_config().general.theme, Theme::System);
    }

    // =========================================================================
    // _version 字段：optimistic concurrency check（防 last-write-wins）
    //
    // 客户端 2 个并发 tab 改 settings 时，A 的 stale partial 不应该覆盖 B 的
    // commit。partial 里带 `_version` = 拿配置时的版本号；不匹配 → Err。
    // 缺省（不带 `_version`）= skip 校验，向后兼容老前端。
    // =========================================================================

    #[tokio::test]
    async fn version_starts_at_zero_and_increments_on_each_commit() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        assert_eq!(mgr.version(), 0, "新 manager SHALL 从 0 起");

        mgr.update_general(serde_json::json!({ "theme": "dark" }))
            .await
            .unwrap();
        assert_eq!(mgr.version(), 1);

        mgr.update_display(serde_json::json!({ "compactMode": true }))
            .await
            .unwrap();
        assert_eq!(mgr.version(), 2);

        mgr.update_updater(serde_json::json!({ "autoUpdateCheckEnabled": false }))
            .await
            .unwrap();
        assert_eq!(mgr.version(), 3);
    }

    #[tokio::test]
    async fn version_does_not_increment_on_validation_failure() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let _ = mgr
            .update_general(serde_json::json!({ "theme": "dark" }))
            .await
            .unwrap();
        assert_eq!(mgr.version(), 1);

        // 非法值：partial deserialize 失败 → version 不增
        let err = mgr
            .update_general(serde_json::json!({ "theme": "bogus" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert_eq!(mgr.version(), 1, "非法值拒绝后 version 保持不变");
    }

    #[tokio::test]
    async fn version_check_passes_when_client_matches_server() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        // 客户端读到 v=0，立即提交 partial 带 _version=0 SHALL 成功
        mgr.update_general(serde_json::json!({
            "_version": 0,
            "theme": "light",
        }))
        .await
        .unwrap();
        assert_eq!(mgr.version(), 1);
        assert_eq!(mgr.get_config().general.theme, Theme::Light);
    }

    #[tokio::test]
    async fn version_check_rejects_stale_client_version() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        // server 先 advance 到 v=2
        mgr.update_general(serde_json::json!({ "theme": "dark" }))
            .await
            .unwrap();
        mgr.update_general(serde_json::json!({ "theme": "light" }))
            .await
            .unwrap();
        assert_eq!(mgr.version(), 2);

        // 客户端 stale 拿着 v=0 提交 → 拒绝
        let err = mgr
            .update_general(serde_json::json!({
                "_version": 0,
                "theme": "system",
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(
            err.to_string().contains("Config version mismatch"),
            "Err message SHALL 含 'Config version mismatch'，实际：{err}"
        );
        // 已存值未变 + version 未增
        assert_eq!(mgr.get_config().general.theme, Theme::Light);
        assert_eq!(mgr.version(), 2);
    }

    #[tokio::test]
    async fn version_absent_skips_check_for_backward_compat() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        mgr.update_general(serde_json::json!({ "theme": "dark" }))
            .await
            .unwrap();
        // 老前端不传 _version → skip check，正常提交
        mgr.update_general(serde_json::json!({ "theme": "light" }))
            .await
            .unwrap();
        assert_eq!(mgr.get_config().general.theme, Theme::Light);
        assert_eq!(mgr.version(), 2);
    }

    #[tokio::test]
    async fn version_null_skips_check() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        mgr.update_general(serde_json::json!({
            "_version": serde_json::Value::Null,
            "theme": "dark",
        }))
        .await
        .unwrap();
        assert_eq!(mgr.get_config().general.theme, Theme::Dark);
    }

    #[tokio::test]
    async fn version_check_works_across_all_seven_update_methods() {
        // 所有 update_* SHALL 共享同一 version counter（一处 commit 让其它 update
        // 的 stale _version 失效）
        let (_d, mut mgr) = setup_general_test_manager().await;

        // v=0 → update_general → v=1
        mgr.update_general(serde_json::json!({ "theme": "dark" }))
            .await
            .unwrap();
        assert_eq!(mgr.version(), 1);

        // 客户端持有 v=0 试图 update_display 应该被拒
        let err = mgr
            .update_display(serde_json::json!({
                "_version": 0,
                "compactMode": true,
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));

        // 客户端 update_keyboard_shortcuts 也共享 version
        let err = mgr
            .update_keyboard_shortcuts(serde_json::json!({
                "_version": 0,
                "sidebar.toggle": "mod+b",
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));

        // 拿 v=1 重试 update_keyboard_shortcuts 成功
        mgr.update_keyboard_shortcuts(serde_json::json!({
            "_version": 1,
            "sidebar.toggle": "mod+b",
        }))
        .await
        .unwrap();
        assert_eq!(mgr.version(), 2);
    }

    #[tokio::test]
    async fn version_non_integer_value_rejected() {
        let (_d, mut mgr) = setup_general_test_manager().await;
        let err = mgr
            .update_general(serde_json::json!({
                "_version": "not-a-number",
                "theme": "dark",
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ConfigError::Validation(_)));
        assert!(err.to_string().contains("_version"));
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
