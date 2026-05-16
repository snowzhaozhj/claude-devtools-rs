//! `ConfigManager` 持久化往返与降级行为的 integration 测试。
//!
//! 单测里已覆盖部分场景；本文件作为黑盒级 integration 视角再走一遍：
//! - 写盘 → 新 `ConfigManager` 重读 → 字段一致（多 section）
//! - 损坏 JSON 加载降级 + 备份产出
//! - 旧版 partial JSON（缺 `updater`）合并默认值
//! - 残留 `.tmp` 文件不污染原配置（注：当前 `persist_config` 走 `tokio::fs::write`
//!   直接覆盖，**没有** tmp+rename 的真原子写；此测试验证"残留 .tmp 文件
//!   不会让 `ConfigManager` 错把它当真配置读"的鲁棒性，若未来切到原子写也仍应成立）
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

    // 自定义 trigger 仍在；builtin 三条也仍在（默认合并）
    let by_id: std::collections::HashMap<&str, &NotificationTrigger> = cfg
        .notifications
        .triggers
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();
    assert!(by_id.contains_key("custom-roundtrip"));
    assert!(by_id.contains_key("builtin-tool-result-error"));
    assert!(by_id["custom-roundtrip"].enabled);
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

/// (4) 模拟"上次 save 期间外部进程 crash 留下了 .tmp 残骸"——
/// 当前 `ConfigManager::persist_config` 走 `tokio::fs::write` 直接覆盖，**没有**
/// tmp+rename 真原子写；此测试黑盒验证「目录里的残留 .tmp 文件不会被 `ConfigManager`
/// 误读为真配置」+「原配置内容完整、不被破坏」。
/// 若未来引入真 tmp+rename 原子写，此测试仍应成立（更强语义即更不易回归）。
#[tokio::test]
async fn stray_tmp_artifact_does_not_corrupt_original_config() {
    let (dir, path) = temp_config_path();
    let dir_path = canonical(dir.path());

    // 先写入合法的"老配置"
    let mut mgr = ConfigManager::new(Some(path.clone()));
    mgr.load().await.unwrap();
    mgr.update_general(serde_json::json!({ "theme": "dark" }))
        .await
        .unwrap();
    let original = tokio::fs::read_to_string(&path).await.unwrap();
    assert!(original.contains("\"theme\""), "sanity: original written");

    // 模拟中断：留下 .tmp 残骸（典型路径形态 = `<path>.tmp` / `<path>.tmp.<pid>`）
    let stray_a = dir_path.join("claude-devtools-config.json.tmp");
    let stray_b = dir_path.join("claude-devtools-config.json.tmp.42");
    tokio::fs::write(&stray_a, "GARBAGE_FROM_CRASHED_WRITE")
        .await
        .unwrap();
    tokio::fs::write(&stray_b, "{ \"theme\": \"light\" }")
        .await
        .unwrap();

    // 新 manager 加载——必须只看 `claude-devtools-config.json`，不能被 .tmp 串入
    let mut mgr2 = ConfigManager::new(Some(path.clone()));
    mgr2.load().await.expect("load with stray tmp present");
    let cfg = mgr2.get_config();
    assert_eq!(
        cfg.general.theme, "dark",
        "stray .tmp must not override real config"
    );

    // 原文件内容字节级一致——load 不会改写它
    let after = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(after, original, "original config bytes must be unchanged");

    // .tmp 残骸不应被自动清理（这是后台 housekeeping 的职责，不属于 load 路径）
    assert!(stray_a.exists() && stray_b.exists());
}

/// (5) `add_trigger` / `remove_trigger`（独立 impl 块、非 `DataApi` trait 方法）
/// 往返：新增 → 持久化 → 新 manager 加载 → 字段一致；删除内建 → 拒绝；删除自定义 → 成功
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
    // builtin 仍在
    assert!(
        after_remove
            .notifications
            .triggers
            .iter()
            .any(|t| t.id == "builtin-tool-result-error")
    );
}
