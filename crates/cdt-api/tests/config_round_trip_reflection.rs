//! Reflection-based round-trip tests for `ConfigManager::update_<section>`.
//!
//! `ConfigManager::update_<section>` 是手写白名单 dispatch（`match k.as_str() { ... _ => {} }`），
//! 漏 match 分支会静默 drop 该字段。`CLAUDE.md` 的「`ConfigManager::update_<section>` 是
//! 手写白名单 dispatch」条目要求**每个**新增字段 SHALL 同步加 match 分支 + 一条
//! `ipc_contract` round-trip 测试。
//!
//! 本测试用**反射 + hardcoded `expected_keys` 双向校验**的方式守护这条契约：
//!
//! 1. 序列化 default `GeneralConfig` / `DisplayConfig` / `UpdaterConfig`，对 Option 字段
//!    先 set `Some(dummy)` 触发 `skip_serializing_if = "Option::is_none"` 失活，确保
//!    serde JSON 完整暴露所有 field name。
//! 2. 比对 JSON 的 keys 与 hardcoded `EXPECTED_<SECTION>_KEYS`——若 struct 加字段忘记
//!    同步本测试，序列化 keys ⊋ expected，断言失败提示开发者：
//!    (a) 在本测试加 case；
//!    (b) 在 `cdt-config/src/manager.rs::update_<section>` 加 match 分支。
//! 3. 对每个 `expected_key` 用一个 `alt_value` 调 `update_<section>`，重读 readback
//!    断言 `readback[key] == alt_value`。漏 match 分支 → readback 不变 → 断言失败。
//! 4. 对 enum 字段额外断言：传入非法字面量 → `update_<section>` 返回 `Err`。
//!
//! 一句话：守护「struct 加字段 + manager 漏分支」的常见悄悄失效。

use std::collections::HashSet;

use cdt_config::ConfigManager;
use serde_json::{Value, json};
use tempfile::TempDir;

async fn setup_manager() -> (ConfigManager, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let mut mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    mgr.load().await.expect("config load");
    (mgr, tmp)
}

fn json_keys(v: &Value) -> HashSet<String> {
    v.as_object()
        .expect("section must serialize as JSON object")
        .keys()
        .cloned()
        .collect()
}

fn expected_keys(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

// =============================================================================
// GeneralConfig (8 字段，无 skip_serializing_if)
// =============================================================================

const GENERAL_EXPECTED_KEYS: &[&str] = &[
    "launchAtLogin",
    "showDockIcon",
    "theme",
    "defaultTab",
    "claudeRootPath",
    // 数据根 MRU 历史（change flexible-data-root）：后端派生只读字段，前端
    // 不经 update_general 更新，故无对应 update round-trip；仅需 get_config 暴露。
    "recentRoots",
    "autoExpandAiGroups",
    "useNativeTitleBar",
    "sessionClickBehavior",
    // Phase 2 frontend-context-menu-phase-2：右键菜单 IPC + Settings dropdown
    "externalEditor",
    "searchEngine",
    "terminalApp",
];

#[tokio::test]
async fn general_config_field_set_matches_expected() {
    let (mgr, _tmp) = setup_manager().await;
    let json = serde_json::to_value(&mgr.get_config().general).unwrap();
    let actual = json_keys(&json);
    let expected = expected_keys(GENERAL_EXPECTED_KEYS);
    assert_eq!(
        actual, expected,
        "GeneralConfig 字段集变化时 SHALL 同步：\n  (a) 本测试 GENERAL_EXPECTED_KEYS\n  \
         (b) cdt-config/src/manager.rs::update_general 的 match 分支\n  \
         (c) crates/cdt-api/tests/ipc_contract.rs round-trip 测试\n  \
         否则 SettingsView 看似生效，重启后丢失"
    );
}

#[tokio::test]
async fn general_config_all_fields_round_trip() {
    let (mut mgr, _tmp) = setup_manager().await;

    let cases: &[(&str, Value)] = &[
        ("launchAtLogin", json!(true)),
        ("showDockIcon", json!(false)),
        ("theme", json!("dark")),
        ("defaultTab", json!("last-session")),
        ("claudeRootPath", json!("/tmp/cdt-test-claude-root")),
        ("autoExpandAiGroups", json!(true)),
        ("useNativeTitleBar", json!(true)),
        ("sessionClickBehavior", json!("new-tab")),
        // Phase 2 三新字段：扁平 enum + internally-tagged enum
        ("externalEditor", json!("vs_code")),
        (
            "searchEngine",
            json!({ "type": "custom", "urlTemplate": "https://example.com/?q={query}" }),
        ),
        ("terminalApp", json!("i_term")),
    ];

    let case_keys: HashSet<String> = cases.iter().map(|(k, _)| (*k).to_owned()).collect();
    // recentRoots 是后端派生只读字段（append on claudeRootPath update），不经
    // update_general 直接写，故不参与 round-trip（change flexible-data-root）。
    let readonly_derived: HashSet<String> = ["recentRoots".to_owned()].into_iter().collect();
    let expected_updatable: HashSet<String> = expected_keys(GENERAL_EXPECTED_KEYS)
        .difference(&readonly_derived)
        .cloned()
        .collect();
    assert_eq!(
        case_keys, expected_updatable,
        "round-trip cases 与 GENERAL_EXPECTED_KEYS（除只读派生字段）不一致——加可更新字段时两处都要同步"
    );

    for (key, alt) in cases {
        mgr.update_general(json!({ *key: alt.clone() }))
            .await
            .unwrap_or_else(|e| panic!("update_general({key}={alt}) 应成功，实际: {e:?}"));
        let after = serde_json::to_value(&mgr.get_config().general).unwrap();
        assert_eq!(
            &after[*key], alt,
            "update_general 漏 match 分支 for `{key}`：写入 {alt} 后 readback 是 {}",
            after[*key]
        );
    }
}

#[tokio::test]
async fn general_config_rejects_non_string_claude_root_path() {
    let (mut mgr, _tmp) = setup_manager().await;
    mgr.update_general(json!({ "claudeRootPath": "/tmp/cdt-test-claude-root" }))
        .await
        .expect("set initial root");

    let res = mgr.update_general(json!({ "claudeRootPath": 42 })).await;
    assert!(
        res.is_err(),
        "update_general MUST reject non-string claudeRootPath values"
    );
    assert_eq!(
        mgr.get_config().general.claude_root_path.as_deref(),
        Some("/tmp/cdt-test-claude-root")
    );
}

#[tokio::test]
async fn general_config_rejects_invalid_enum_values() {
    let (mut mgr, _tmp) = setup_manager().await;

    let invalid_cases: &[(&str, Value)] = &[
        ("theme", json!("purple")),
        ("defaultTab", json!("inbox")),
        ("sessionClickBehavior", json!("drag")),
        // Phase 2 enum 校验
        ("externalEditor", json!("vim")),
        ("terminalApp", json!("fish")),
    ];

    for (key, bad) in invalid_cases {
        let res = mgr.update_general(json!({ *key: bad.clone() })).await;
        assert!(
            res.is_err(),
            "update_general MUST 拒绝非法 `{key}` 值 {bad}；实际接受"
        );
    }
}

// =============================================================================
// DisplayConfig (6 字段；font_sans / font_mono 有 skip_serializing_if = "Option::is_none")
// =============================================================================

const DISPLAY_EXPECTED_KEYS: &[&str] = &[
    "showTimestamps",
    "compactMode",
    "syntaxHighlighting",
    "fontSans",
    "fontMono",
    "timeFormat",
];

#[tokio::test]
async fn display_config_field_set_matches_expected() {
    let (mut mgr, _tmp) = setup_manager().await;
    // font_sans / font_mono 在 default 下是 None，skip_serializing_if 会让它们消失。
    // 先 set Some(...) 触发完整序列化，再断言 keys 完整。
    mgr.update_display(json!({ "fontSans": "X", "fontMono": "Y" }))
        .await
        .expect("update_display(font_*=Some(...))");
    let json = serde_json::to_value(&mgr.get_config().display).unwrap();
    let actual = json_keys(&json);
    let expected = expected_keys(DISPLAY_EXPECTED_KEYS);
    assert_eq!(
        actual, expected,
        "DisplayConfig 字段集变化时 SHALL 同步本测试 + manager.rs::update_display match 分支"
    );
}

#[tokio::test]
async fn display_config_all_fields_round_trip() {
    let (mut mgr, _tmp) = setup_manager().await;

    let cases: &[(&str, Value)] = &[
        ("showTimestamps", json!(false)),
        ("compactMode", json!(true)),
        ("syntaxHighlighting", json!(false)),
        ("fontSans", json!("Arial")),
        ("fontMono", json!("Menlo")),
        ("timeFormat", json!("12h")),
    ];

    let case_keys: HashSet<String> = cases.iter().map(|(k, _)| (*k).to_owned()).collect();
    assert_eq!(
        case_keys,
        expected_keys(DISPLAY_EXPECTED_KEYS),
        "round-trip cases 与 DISPLAY_EXPECTED_KEYS 不一致"
    );

    for (key, alt) in cases {
        mgr.update_display(json!({ *key: alt.clone() }))
            .await
            .unwrap_or_else(|e| panic!("update_display({key}={alt}) 应成功，实际: {e:?}"));
        let after = serde_json::to_value(&mgr.get_config().display).unwrap();
        assert_eq!(
            &after[*key], alt,
            "update_display 漏 match 分支 for `{key}`：写入 {alt} 后 readback 是 {}",
            after[*key]
        );
    }
}

#[tokio::test]
async fn display_config_rejects_overlong_font_family() {
    let (mut mgr, _tmp) = setup_manager().await;
    // FONT_FAMILY_MAX_LEN = 500（见 manager.rs::update_display 上的硬约束）
    let huge = "A".repeat(501);
    let res = mgr.update_display(json!({ "fontSans": huge })).await;
    assert!(
        res.is_err(),
        "update_display MUST 拒绝 fontSans 长度 > 500，实际接受"
    );
}

// =============================================================================
// UpdaterConfig (2 字段；skipped_update_version 有 skip_serializing_if = "Option::is_none")
// =============================================================================

const UPDATER_EXPECTED_KEYS: &[&str] = &["autoUpdateCheckEnabled", "skippedUpdateVersion"];

#[tokio::test]
async fn updater_config_field_set_matches_expected() {
    let (mut mgr, _tmp) = setup_manager().await;
    mgr.update_updater(json!({ "skippedUpdateVersion": "0.0.0" }))
        .await
        .expect("update_updater set Some");
    let json = serde_json::to_value(&mgr.get_config().updater).unwrap();
    let actual = json_keys(&json);
    let expected = expected_keys(UPDATER_EXPECTED_KEYS);
    assert_eq!(
        actual, expected,
        "UpdaterConfig 字段集变化时 SHALL 同步本测试 + manager.rs::update_updater match 分支"
    );
}

#[tokio::test]
async fn updater_config_all_fields_round_trip() {
    let (mut mgr, _tmp) = setup_manager().await;

    let cases: &[(&str, Value)] = &[
        ("autoUpdateCheckEnabled", json!(false)),
        ("skippedUpdateVersion", json!("9.9.9")),
    ];

    let case_keys: HashSet<String> = cases.iter().map(|(k, _)| (*k).to_owned()).collect();
    assert_eq!(
        case_keys,
        expected_keys(UPDATER_EXPECTED_KEYS),
        "round-trip cases 与 UPDATER_EXPECTED_KEYS 不一致"
    );

    for (key, alt) in cases {
        mgr.update_updater(json!({ *key: alt.clone() }))
            .await
            .unwrap_or_else(|e| panic!("update_updater({key}={alt}) 应成功，实际: {e:?}"));
        let after = serde_json::to_value(&mgr.get_config().updater).unwrap();
        assert_eq!(
            &after[*key], alt,
            "update_updater 漏 match 分支 for `{key}`：写入 {alt} 后 readback 是 {}",
            after[*key]
        );
    }
}

#[tokio::test]
async fn updater_skipped_version_round_trips_null_to_none() {
    let (mut mgr, _tmp) = setup_manager().await;
    mgr.update_updater(json!({ "skippedUpdateVersion": "1.2.3" }))
        .await
        .unwrap();
    let mid = mgr.get_config().updater.skipped_update_version.clone();
    assert_eq!(mid.as_deref(), Some("1.2.3"));
    mgr.update_updater(json!({ "skippedUpdateVersion": Value::Null }))
        .await
        .unwrap();
    let cleared = mgr.get_config().updater.skipped_update_version.clone();
    assert!(
        cleared.is_none(),
        "update_updater MUST 把 null 归一化为 None，实际 {cleared:?}"
    );
}
