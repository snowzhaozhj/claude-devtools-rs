//! `ConfigManager` 持久化往返与降级行为的 integration 测试。
//!
//! 单测里已覆盖部分场景；本文件作为黑盒级 integration 视角再走一遍：
//! - 写盘 → 新 `ConfigManager` 重读 → 字段一致（多 section）
//! - 损坏 JSON 加载降级 + 备份产出
//! - 旧版 partial JSON（缺 `updater`）合并默认值
//! - save 写入中断（合法 JSON 被截断成 prefix）→ load 走备份 + 默认值降级
//! - `add_trigger` / `remove_trigger`（非 trait 公开方法）往返后字段一致

use std::path::{Path, PathBuf};

use cdt_config::types::NotificationTrigger;
use cdt_config::{ConfigError, ConfigManager, TriggerContentType, TriggerMode};
use tempfile::TempDir;

/// macOS 上 `TempDir` 返回 `/var/...` 但 `tokio::fs::canonicalize` 返回 `/private/var/...`；
/// 涉及路径字符串比较时统一走 canonicalize。
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn temp_config_path() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = canonical(dir.path()).join("claude-devtools-config.json");
    (dir, path)
}

#[tokio::test]
async fn failed_general_save_does_not_mutate_in_memory_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = canonical(dir.path()).join("config.json");
    let mut mgr = ConfigManager::new(Some(config_path));
    mgr.load().await.expect("load defaults from missing file");

    std::fs::remove_dir_all(dir.path()).expect("remove parent dir before save");
    std::fs::write(dir.path(), b"not a directory").expect("replace parent with file");

    let before = mgr.get_config();
    let custom_root = canonical(Path::new("/"));
    let err = mgr
        .update_general(serde_json::json!({
            "theme": "dark",
            "claudeRootPath": custom_root.to_string_lossy(),
        }))
        .await
        .expect_err("writing config under a file parent must fail");
    assert!(matches!(err, ConfigError::Io { .. }));

    let after = mgr.get_config();
    assert_eq!(after.general.theme, before.general.theme);
    assert_eq!(
        after.general.claude_root_path,
        before.general.claude_root_path
    );
}

#[tokio::test]
async fn failed_general_save_preserves_existing_config_file() {
    let (_dir, path) = temp_config_path();
    let mut mgr = ConfigManager::new(Some(path.clone()));
    mgr.load().await.expect("load defaults from missing file");
    mgr.update_general(serde_json::json!({ "theme": "light" }))
        .await
        .expect("write baseline config");
    let baseline = std::fs::read_to_string(&path).expect("read baseline config");

    let tmp_path = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::create_dir(&tmp_path).expect("block temp file creation");

    let err = mgr
        .update_general(serde_json::json!({ "theme": "dark" }))
        .await
        .expect_err("temp file creation must fail");
    assert!(matches!(err, ConfigError::Io { .. }));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read config after failed save"),
        baseline
    );
    assert_eq!(mgr.get_config().general.theme, "light");
}

/// (1) 多 section 写盘 → 新 manager 重读 → 字段一致
#[tokio::test]
async fn round_trip_general_display_updater_triggers_persists_after_reload() {
    let (_dir, path) = temp_config_path();

    // 第一个 manager：写入若干 section
    let mut mgr = ConfigManager::new(Some(path.clone()));
    mgr.load().await.expect("first load on empty path");

    mgr.update_general(serde_json::json!({
        "theme": "dark",
        "autoExpandAiGroups": true,
        "sessionClickBehavior": "new-tab",
    }))
    .await
    .expect("update general");

    mgr.update_display(serde_json::json!({
        "compactMode": true,
        "fontMono": "\"JetBrains Mono\", monospace",
    }))
    .await
    .expect("update display");

    mgr.update_updater(serde_json::json!({
        "autoUpdateCheckEnabled": false,
        "skippedUpdateVersion": "0.4.2",
    }))
    .await
    .expect("update updater");

    let custom = NotificationTrigger {
        id: "custom-roundtrip".into(),
        name: "Round-trip Trigger".into(),
        enabled: true,
        content_type: TriggerContentType::ToolResult,
        mode: TriggerMode::ErrorStatus,
        tool_name: None,
        is_builtin: None,
        ignore_patterns: None,
        require_error: Some(true),
        match_field: None,
        match_pattern: None,
        token_threshold: None,
        token_type: None,
        repository_ids: None,
        color: None,
    };
    mgr.add_trigger(custom).await.expect("add custom trigger");

    // 显式 drop 第一个 manager，避免误以为是同实例内存命中
    drop(mgr);

    // 第二个 manager：仅从磁盘加载，验证字段全部保留
    let mut mgr2 = ConfigManager::new(Some(path));
    mgr2.load().await.expect("second load");
    let cfg = mgr2.get_config();

    assert_eq!(cfg.general.theme, "dark");
    assert!(cfg.general.auto_expand_ai_groups);
    assert_eq!(cfg.general.session_click_behavior, "new-tab");

    assert!(cfg.display.compact_mode);
    assert_eq!(
        cfg.display.font_mono.as_deref(),
        Some("\"JetBrains Mono\", monospace")
    );

    assert!(!cfg.updater.auto_update_check_enabled);
    assert_eq!(cfg.updater.skipped_update_version.as_deref(), Some("0.4.2"));

    // 自定义 trigger 仍在；defaults.rs::default_triggers 列出的全部 3 个 builtin 必须仍在
    // （任一缺失说明 default-merge 路径回归）
    let by_id: std::collections::HashMap<&str, &NotificationTrigger> = cfg
        .notifications
        .triggers
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();
    assert!(by_id.contains_key("custom-roundtrip"));
    let expected_builtins = builtin_trigger_ids();
    assert!(
        !expected_builtins.is_empty(),
        "defaults should declare at least one builtin"
    );
    for builtin_id in &expected_builtins {
        assert!(
            by_id.contains_key(builtin_id.as_str()),
            "builtin trigger {builtin_id} must survive partial-config merge"
        );
    }
    assert!(by_id["custom-roundtrip"].enabled);
}

/// 运行时拉取 `defaults::default_triggers()` 里所有 `is_builtin` 为真的 id。
/// 不硬编码常量——defaults.rs 增删 builtin 时本测试自动同步覆盖面。
fn builtin_trigger_ids() -> Vec<String> {
    cdt_config::defaults::default_triggers()
        .into_iter()
        .filter(NotificationTrigger::is_builtin)
        .map(|t| t.id)
        .collect()
}

/// (2) 损坏 JSON → 不 panic，加载默认值，原文件被重命名为 `<path>.bak.<ts>`
#[tokio::test]
async fn corrupted_json_falls_back_to_defaults_without_panic() {
    let (dir, path) = temp_config_path();

    tokio::fs::write(&path, "{ this is :: not json ][")
        .await
        .expect("write corrupted");

    let mut mgr = ConfigManager::new(Some(path.clone()));
    // 关键断言：不 panic，返回 Ok
    let result = mgr.load().await;
    assert!(result.is_ok(), "corrupted JSON must not error: {result:?}");

    // 原文件被备份重命名
    assert!(!path.exists(), "corrupted file should be renamed away");
    let mut entries = tokio::fs::read_dir(canonical(dir.path())).await.unwrap();
    let mut backup_count = 0usize;
    while let Some(entry) = entries.next_entry().await.unwrap() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("claude-devtools-config.json.bak.") {
            backup_count += 1;
            let body = tokio::fs::read_to_string(entry.path()).await.unwrap();
            assert_eq!(body, "{ this is :: not json ][");
        }
    }
    assert_eq!(backup_count, 1, "should produce exactly one backup file");

    // 内存中是默认值
    let cfg = mgr.get_config();
    assert!(cfg.notifications.enabled);
    assert_eq!(cfg.http_server.port, 3456);
    assert!(cfg.updater.auto_update_check_enabled);
}

/// (3) 老配置缺 `updater` section → 合并后 updater 用 default 填上，其他字段保留
#[tokio::test]
async fn legacy_config_missing_updater_section_merged_with_defaults() {
    let (_dir, path) = temp_config_path();

    // 模拟一份 v0.2.x 时代的老配置：完全没有 `updater` key
    let legacy = serde_json::json!({
        "httpServer": { "enabled": true, "port": 17000 },
        "general": { "theme": "light" },
        "display": { "compactMode": true },
    });
    tokio::fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap())
        .await
        .expect("write legacy");

    let mut mgr = ConfigManager::new(Some(path));
    mgr.load().await.expect("load legacy");
    let cfg = mgr.get_config();

    // 老字段保留
    assert_eq!(cfg.http_server.port, 17000);
    assert_eq!(cfg.general.theme, "light");
    assert!(cfg.display.compact_mode);

    // updater 整段缺失 → 用 default
    assert!(
        cfg.updater.auto_update_check_enabled,
        "missing updater section must default-enable auto check"
    );
    assert!(cfg.updater.skipped_update_version.is_none());

    // 其他 default 字段也应该补齐（deep_merge）
    assert!(cfg.notifications.enabled);
    assert_eq!(cfg.notifications.snooze_minutes, 30);
}

/// (4) 模拟"save 写到一半进程崩溃，留下被截断的 config 文件"——
/// 当前 `ConfigManager::persist_config`（manager.rs:152-160）是 `tokio::fs::write` 直接
/// 覆盖、**没有** tmp+rename 真原子写；这意味着崩溃在写入中途真的能让 config 文件被
/// 截断成不完整 JSON（例如 `{"general":{"theme":"da`）。本测试模拟这个真实回归场景：
/// 先 save 一份合法配置 → 把它截断成不完整 JSON → reload 必须走损坏降级路径
/// （备份原残骸 + 内存恢复 default + 不 panic），保证下次启动仍可用。
///
/// 与测试 (2) 的区别：(2) 是写入根本无法解析的字符串；本测试是 **合法 JSON 被切断
/// 成 prefix**，覆盖 `serde_json::from_str` 对截断输入的真实失败路径。
#[tokio::test]
async fn save_interrupted_truncated_config_falls_back_to_defaults() {
    let (dir, path) = temp_config_path();

    // 先用真 save 写入一份合法的 baseline，确保 path 上是 well-formed JSON
    let mut mgr = ConfigManager::new(Some(path.clone()));
    mgr.load().await.unwrap();
    mgr.update_general(serde_json::json!({ "theme": "dark" }))
        .await
        .unwrap();
    let original = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(
        original.contains("\"theme\""),
        "sanity: baseline save wrote full json"
    );
    drop(mgr);

    // 模拟写入中断：把合法 JSON 截断到一半（前 24 字节），形成不完整 JSON
    let trunc_to = 24.min(original.len() / 2);
    let truncated = original.as_bytes()[..trunc_to].to_vec();
    assert!(
        serde_json::from_slice::<serde_json::Value>(&truncated).is_err(),
        "sanity: truncated prefix must not parse as JSON"
    );
    tokio::fs::write(&path, &truncated)
        .await
        .expect("simulate save crash truncation");

    // reload —— 必须走损坏降级路径
    let mut mgr2 = ConfigManager::new(Some(path.clone()));
    let result = mgr2.load().await;
    assert!(
        result.is_ok(),
        "truncated config must not error out: {result:?}"
    );

    // 原残骸已被重命名为 backup（manager.rs:99-114）
    assert!(!path.exists(), "truncated file must be renamed to backup");
    let mut backup_count = 0usize;
    let mut entries = tokio::fs::read_dir(canonical(dir.path())).await.unwrap();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("claude-devtools-config.json.bak.") {
            backup_count += 1;
            let body = tokio::fs::read(entry.path()).await.unwrap();
            assert_eq!(
                body, truncated,
                "backup must contain the truncated bytes verbatim"
            );
        }
    }
    assert_eq!(backup_count, 1, "exactly one backup file expected");

    // 内存中已恢复 default，下次操作仍可用
    let cfg = mgr2.get_config();
    assert!(cfg.notifications.enabled);
    assert_eq!(cfg.http_server.port, 3456);
    assert_eq!(
        cfg.general.theme, "system",
        "must reset to default theme, not keep stale 'dark'"
    );

    // 后续 save 必须成功，证明 manager 处于可用状态
    mgr2.update_general(serde_json::json!({ "theme": "light" }))
        .await
        .expect("post-fallback save must succeed");
    let after = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(after.contains("\"light\""));
}

/// (5) `add_trigger` / `remove_trigger`（独立 impl 块、非 `DataApi` trait 方法）
/// 往返：新增 → 持久化 → 新 manager 加载 → 字段一致；删除内建 → 拒绝；删除自定义 → 成功
#[tokio::test]
async fn failed_trigger_save_does_not_mutate_trigger_manager() {
    let (_dir, path) = temp_config_path();
    let mut mgr = ConfigManager::new(Some(path.clone()));
    mgr.load().await.unwrap();
    let before_count = mgr.get_triggers().len();

    let tmp_path = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::create_dir(&tmp_path).expect("block temp file creation");

    let trigger = NotificationTrigger {
        id: "custom-failed-save".into(),
        name: "Failed Save".into(),
        enabled: true,
        content_type: TriggerContentType::ToolResult,
        mode: TriggerMode::ErrorStatus,
        tool_name: None,
        is_builtin: None,
        ignore_patterns: None,
        require_error: Some(true),
        match_field: None,
        match_pattern: None,
        token_threshold: None,
        token_type: None,
        repository_ids: None,
        color: None,
    };

    let err = mgr
        .add_trigger(trigger)
        .await
        .expect_err("blocked temp path must fail save");
    assert!(matches!(err, ConfigError::Io { .. }));
    assert_eq!(mgr.get_triggers().len(), before_count);
    assert!(
        mgr.get_triggers()
            .iter()
            .all(|t| t.id != "custom-failed-save")
    );
}

#[tokio::test]
async fn trigger_crud_round_trip_through_independent_methods() {
    let (_dir, path) = temp_config_path();

    let mut mgr = ConfigManager::new(Some(path.clone()));
    mgr.load().await.unwrap();

    let custom = NotificationTrigger {
        id: "custom-perf-spike".into(),
        name: "Perf Spike".into(),
        enabled: true,
        content_type: TriggerContentType::ToolResult,
        mode: TriggerMode::TokenThreshold,
        tool_name: None,
        is_builtin: None,
        ignore_patterns: None,
        require_error: None,
        match_field: None,
        match_pattern: None,
        token_threshold: Some(12_000),
        token_type: Some(cdt_config::types::TriggerTokenType::Total),
        repository_ids: None,
        color: Some("red".into()),
    };
    mgr.add_trigger(custom.clone())
        .await
        .expect("add custom trigger");

    // 重复 add 同 id 应被 TriggerManager 拒绝
    let dup_err = mgr.add_trigger(custom.clone()).await.unwrap_err();
    assert!(matches!(dup_err, ConfigError::Validation(_)));

    // 内建 trigger 不可删
    let builtin_err = mgr.remove_trigger("builtin-tool-result-error").await;
    assert!(
        matches!(builtin_err, Err(ConfigError::Validation(_))),
        "builtin triggers must not be removable: got {builtin_err:?}"
    );

    drop(mgr);

    // 新 manager 从磁盘加载，验证 custom trigger 仍在且字段一致
    let mut mgr2 = ConfigManager::new(Some(path.clone()));
    mgr2.load().await.unwrap();
    let after_add = mgr2.get_config();
    let found = after_add
        .notifications
        .triggers
        .iter()
        .find(|t| t.id == "custom-perf-spike")
        .expect("custom trigger persisted");
    assert_eq!(found.name, "Perf Spike");
    assert!(found.enabled);
    assert_eq!(found.token_threshold, Some(12_000));
    assert_eq!(found.color.as_deref(), Some("red"));

    // 删除自定义 trigger → 持久化 → 新 manager 看不到
    mgr2.remove_trigger("custom-perf-spike")
        .await
        .expect("remove custom trigger");
    drop(mgr2);

    let mut mgr3 = ConfigManager::new(Some(path));
    mgr3.load().await.unwrap();
    let after_remove = mgr3.get_config();
    assert!(
        after_remove
            .notifications
            .triggers
            .iter()
            .all(|t| t.id != "custom-perf-spike"),
        "removed trigger must not reappear after reload"
    );
    // 全部 builtin 仍在（动态从 defaults 拉取 id，不止抽查一条）
    let surviving_ids: std::collections::HashSet<&str> = after_remove
        .notifications
        .triggers
        .iter()
        .map(|t| t.id.as_str())
        .collect();
    for builtin_id in &builtin_trigger_ids() {
        assert!(
            surviving_ids.contains(builtin_id.as_str()),
            "builtin {builtin_id} must survive custom-trigger removal"
        );
    }
}

// =============================================================================
// keyboardShortcuts (configuration-management spec delta · 2026-05-23)
// =============================================================================

/// (K1) 默认配置：`keyboardShortcuts` 字段在磁盘上序列化为空 object `{}`，
/// **而非** 缺失或 `null`——前端 customization 层依赖稳定 shape。
/// 见 `openspec/specs/configuration-management/spec.md::keyboardShortcuts.serialize-empty`。
#[tokio::test]
async fn empty_keyboard_shortcuts_serializes_as_object_in_disk_json() {
    let (_dir, path) = temp_config_path();
    let mut mgr = ConfigManager::new(Some(path.clone()));
    mgr.load().await.expect("load defaults");
    // 触发首次写盘（任意 update 都行；这里走 general 不动 keyboardShortcuts）
    mgr.update_general(serde_json::json!({ "theme": "dark" }))
        .await
        .expect("write baseline");

    let raw = tokio::fs::read_to_string(&path).await.expect("read disk");
    let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse disk json");
    let shortcuts = parsed
        .get("keyboardShortcuts")
        .expect("keyboardShortcuts must be present (camelCase, not omitted)");
    assert!(
        shortcuts.is_object(),
        "keyboardShortcuts must serialize as object, got {shortcuts:?}"
    );
    assert_eq!(
        shortcuts.as_object().unwrap().len(),
        0,
        "default keyboardShortcuts must be empty object"
    );
    // 确认 snake_case 字段名没有泄漏到磁盘
    assert!(
        parsed.get("keyboard_shortcuts").is_none(),
        "snake_case must not appear on disk: {raw}"
    );
}

/// (K2) 整体替换语义（同 `notifications.triggers`）：
/// `update_keyboard_shortcuts({"sidebar.toggle": "mod+shift+b"})` → 持久化 → 新 manager 重读 → 字段一致；
/// 后续 `update_keyboard_shortcuts({"foo": "ctrl+x"})` SHALL **替换**而非合并。
#[tokio::test]
async fn keyboard_shortcuts_round_trip_and_whole_replace() {
    let (_dir, path) = temp_config_path();
    let mut mgr = ConfigManager::new(Some(path.clone()));
    mgr.load().await.expect("load defaults");
    assert!(mgr.get_config().keyboard_shortcuts.is_empty());

    // 第一次写入
    mgr.update_keyboard_shortcuts(serde_json::json!({
        "sidebar.toggle": "mod+shift+b",
        "command-palette.open": "mod+k",
    }))
    .await
    .expect("set initial shortcuts");

    drop(mgr);

    // 新 manager 重读，确认两条都在
    let mut mgr2 = ConfigManager::new(Some(path.clone()));
    mgr2.load().await.expect("reload");
    let cfg = mgr2.get_config();
    assert_eq!(cfg.keyboard_shortcuts.len(), 2);
    assert_eq!(
        cfg.keyboard_shortcuts
            .get("sidebar.toggle")
            .map(String::as_str),
        Some("mod+shift+b")
    );
    assert_eq!(
        cfg.keyboard_shortcuts
            .get("command-palette.open")
            .map(String::as_str),
        Some("mod+k")
    );

    // 整体替换：只传一条 → 旧两条全部消失
    mgr2.update_keyboard_shortcuts(serde_json::json!({
        "foo": "ctrl+x",
    }))
    .await
    .expect("whole replace");

    let cfg2 = mgr2.get_config();
    assert_eq!(
        cfg2.keyboard_shortcuts.len(),
        1,
        "whole replace must drop old entries"
    );
    assert_eq!(
        cfg2.keyboard_shortcuts.get("foo").map(String::as_str),
        Some("ctrl+x")
    );
    assert!(!cfg2.keyboard_shortcuts.contains_key("sidebar.toggle"));

    // 空 object 替换 → 清空
    mgr2.update_keyboard_shortcuts(serde_json::json!({}))
        .await
        .expect("clear via empty object");
    assert!(mgr2.get_config().keyboard_shortcuts.is_empty());
}

/// (K3) 老配置缺 `keyboardShortcuts` 字段 → 加载后等价于空 HashMap，其他字段保留。
#[tokio::test]
async fn legacy_config_missing_keyboard_shortcuts_defaults_to_empty_map() {
    let (_dir, path) = temp_config_path();

    // v0.5.x 之前的配置完全没有 `keyboardShortcuts` key
    let legacy = serde_json::json!({
        "general": { "theme": "light" },
        "httpServer": { "enabled": true, "port": 17000 },
    });
    tokio::fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap())
        .await
        .expect("write legacy");

    let mut mgr = ConfigManager::new(Some(path));
    mgr.load().await.expect("load legacy");
    let cfg = mgr.get_config();

    assert!(
        cfg.keyboard_shortcuts.is_empty(),
        "missing keyboardShortcuts must default to empty map"
    );
    // 确认旧字段保留
    assert_eq!(cfg.general.theme, "light");
    assert_eq!(cfg.http_server.port, 17000);
}

/// (K4) 非法输入拒绝：非对象、键空串、值空串 SHALL 返回 `ConfigError::Validation`，
/// 且**不**修改内存状态。
#[tokio::test]
async fn keyboard_shortcuts_rejects_invalid_inputs() {
    let (_dir, path) = temp_config_path();
    let mut mgr = ConfigManager::new(Some(path));
    mgr.load().await.unwrap();

    // 先注入一条合法 entry，作为"非法调用不应破坏旧状态"的基线
    mgr.update_keyboard_shortcuts(serde_json::json!({
        "sidebar.toggle": "mod+shift+b",
    }))
    .await
    .unwrap();

    // 非对象（数组）
    let err = mgr
        .update_keyboard_shortcuts(serde_json::json!(["mod+x"]))
        .await
        .expect_err("array must be rejected");
    assert!(matches!(err, ConfigError::Validation(_)));

    // 值类型错（数字）
    let err = mgr
        .update_keyboard_shortcuts(serde_json::json!({ "foo": 42 }))
        .await
        .expect_err("non-string value must be rejected");
    assert!(matches!(err, ConfigError::Validation(_)));

    // 空键
    let err = mgr
        .update_keyboard_shortcuts(serde_json::json!({ "": "mod+x" }))
        .await
        .expect_err("empty actionId must be rejected");
    assert!(matches!(err, ConfigError::Validation(_)));

    // 空值
    let err = mgr
        .update_keyboard_shortcuts(serde_json::json!({ "foo": "" }))
        .await
        .expect_err("empty combo must be rejected");
    assert!(matches!(err, ConfigError::Validation(_)));

    // 旧状态未被破坏
    let cfg = mgr.get_config();
    assert_eq!(cfg.keyboard_shortcuts.len(), 1);
    assert_eq!(
        cfg.keyboard_shortcuts
            .get("sidebar.toggle")
            .map(String::as_str),
        Some("mod+shift+b")
    );
}
