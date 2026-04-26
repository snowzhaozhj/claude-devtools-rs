//! IPC contract tests: 守护 `LocalDataApi` 公开方法的 JSON 序列化形状契约。
//!
//! 对应 spec `frontend-test-pyramid` § "Rust IPC contract test 守护字段形状"。
//!
//! 每个 `LocalDataApi` 公开方法（与 `src-tauri/src/lib.rs::invoke_handler!`
//! 列表中的 Tauri command 1:1 对应）SHALL 至少有一个用例断言：
//! - 顶层字段名是 camelCase（不是 `snake_case`）
//! - `xxxOmitted` flag 字段命名遵循 `<原字段>Omitted` 规范
//! - `#[serde(tag = "...")]` 的 internally-tagged enum tag 值与 spec 一致
//! - `#[serde(skip_serializing_if = "Option::is_none")]` 字段在 None 时不出现
//!
//! ⚠️ 修改 `src-tauri/src/lib.rs` 的 `invoke_handler!` 列表时
//! MUST 同步更新 `EXPECTED_TAURI_COMMANDS` 常量。

use std::sync::Arc;

use cdt_api::{
    ConfigUpdateRequest, DataApi, LocalDataApi, PaginatedRequest, PaginatedResponse, ProjectInfo,
    ProjectSessionPrefs, SearchRequest, SessionSummary,
};
use cdt_config::{
    ConfigManager, NotificationManager, NotificationTrigger, TriggerContentType, TriggerMode,
};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use chrono::{TimeZone, Utc};
use serde_json::json;
use tempfile::TempDir;

use cdt_core::chunk::{
    AIChunk, AssistantResponse, Chunk, ChunkMetrics, CompactChunk, SemanticStep, SlashCommand,
    SystemChunk, TeammateMessage, UserChunk,
};
use cdt_core::context::{
    ClaudeMdContextInjection, ClaudeMdScope, ContextInjection, MentionedFileInjection,
    TaskCoordinationInjection, ThinkingTextInjection, ToolOutputInjection, UserMessageInjection,
};
use cdt_core::message::{ImageSource, MessageContent};
use cdt_core::process::Process;
use cdt_core::tool_execution::{ToolExecution, ToolOutput};

// =============================================================================
// 共享常量与 setup helper
// =============================================================================

/// 与 `src-tauri/src/lib.rs` 的 `invoke_handler!` 完全对齐的 Tauri command 列表。
/// 长度断言 + 命名断言通过 `expected_tauri_commands_count_is_22` 用例守护。
pub const EXPECTED_TAURI_COMMANDS: &[&str] = &[
    "list_projects",
    "list_sessions",
    "get_session_detail",
    "get_subagent_trace",
    "get_image_asset",
    "get_tool_output",
    "search_sessions",
    "get_config",
    "update_config",
    "get_notifications",
    "mark_notification_read",
    "delete_notification",
    "mark_all_notifications_read",
    "clear_notifications",
    "add_trigger",
    "remove_trigger",
    "read_agent_configs",
    "pin_session",
    "unpin_session",
    "hide_session",
    "unhide_session",
    "get_project_session_prefs",
];

/// 构造一个最小可用的 `LocalDataApi` 用于 contract test。
///
/// `TempDir` 必须由调用方持有所有权直到测试结束，避免 `ConfigManager`
/// 持有的路径在 drop 后变成悬空。
pub async fn setup_api() -> (Arc<LocalDataApi>, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    let scanner = ProjectScanner::new(
        Arc::new(LocalFileSystemProvider::new()),
        projects_base.clone(),
    );
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.expect("config load");
    let notif_mgr = NotificationManager::new(None);
    let ssh_mgr = SshConnectionManager::new();

    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);
    (Arc::new(api), tmp)
}

fn ts() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 11, 0, 0, 0).unwrap()
}

// =============================================================================
// Meta 测：command 列表完整性
// =============================================================================

#[test]
fn expected_tauri_commands_count_is_22() {
    assert_eq!(
        EXPECTED_TAURI_COMMANDS.len(),
        22,
        "EXPECTED_TAURI_COMMANDS 长度变化时 SHALL 同步更新 src-tauri/src/lib.rs::invoke_handler! \
         以及本文件常量；当前 src-tauri 注册 22 个 Tauri command"
    );
}

#[test]
fn expected_tauri_commands_have_no_duplicates() {
    let mut sorted = EXPECTED_TAURI_COMMANDS.to_vec();
    sorted.sort_unstable();
    let original_len = sorted.len();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        original_len,
        "EXPECTED_TAURI_COMMANDS MUST 不含重复项"
    );
}

#[tokio::test]
async fn setup_api_constructs_without_panic() {
    let (_api, _tmp) = setup_api().await;
}

// =============================================================================
// Schema-level: ProjectInfo / SessionSummary / PaginatedResponse / 其他基础 struct
// =============================================================================

#[test]
fn project_info_serializes_camelcase() {
    let p = ProjectInfo {
        id: "test".into(),
        path: "/tmp/foo".into(),
        display_name: "Test Project".into(),
        session_count: 5,
    };
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["displayName"], json!("Test Project"));
    assert_eq!(json["sessionCount"], json!(5));
    assert!(
        json.get("display_name").is_none(),
        "MUST 不出现 snake_case 字段名"
    );
    assert!(json.get("session_count").is_none());
}

#[test]
fn session_summary_serializes_camelcase_with_optional_title() {
    let s = SessionSummary {
        session_id: "sess-1".into(),
        project_id: "proj-1".into(),
        timestamp: 1_700_000_000,
        message_count: 12,
        title: Some("hello".into()),
        is_ongoing: true,
    };
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["sessionId"], json!("sess-1"));
    assert_eq!(json["projectId"], json!("proj-1"));
    assert_eq!(json["messageCount"], json!(12));
    assert_eq!(json["isOngoing"], json!(true));
    assert_eq!(json["title"], json!("hello"));

    // Skeleton variant (title=None)
    let skeleton = SessionSummary {
        title: None,
        is_ongoing: false,
        ..s
    };
    let json = serde_json::to_value(&skeleton).unwrap();
    assert_eq!(json["title"], json!(null), "Option<String> None → null");
    assert_eq!(json["isOngoing"], json!(false));
}

#[test]
fn paginated_response_serializes_camelcase() {
    let p: PaginatedResponse<SessionSummary> = PaginatedResponse {
        items: vec![],
        next_cursor: Some("cur-1".into()),
        total: 100,
    };
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["items"], json!([]));
    assert_eq!(json["nextCursor"], json!("cur-1"));
    assert_eq!(json["total"], json!(100));
    assert!(json.get("next_cursor").is_none());
}

#[test]
fn search_request_serializes_camelcase() {
    let r = SearchRequest {
        query: "foo".into(),
        project_id: Some("p1".into()),
        session_id: None,
    };
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["query"], json!("foo"));
    assert_eq!(json["projectId"], json!("p1"));
    assert!(json.get("project_id").is_none());
}

#[test]
fn config_update_request_serializes_camelcase() {
    let r = ConfigUpdateRequest {
        section: "notifications".into(),
        data: json!({ "enabled": true }),
    };
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["section"], json!("notifications"));
    assert_eq!(json["data"], json!({ "enabled": true }));
}

#[test]
fn project_session_prefs_serializes_camelcase() {
    let p = ProjectSessionPrefs {
        pinned: vec!["s1".into()],
        hidden: vec![],
    };
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["pinned"], json!(["s1"]));
    assert_eq!(json["hidden"], json!([]));
}

// =============================================================================
// Schema-level: Chunk enum tag + 各 chunk 类型
// =============================================================================

#[test]
fn chunk_enum_user_tag_is_user() {
    let chunk = Chunk::User(UserChunk {
        uuid: "u1".into(),
        timestamp: ts(),
        duration_ms: None,
        content: MessageContent::Text("hi".into()),
        metrics: ChunkMetrics::default(),
    });
    let json = serde_json::to_value(&chunk).unwrap();
    assert_eq!(json["kind"], json!("user"), "Chunk::User → kind: user");
}

#[test]
fn chunk_enum_ai_tag_is_ai_not_assistant() {
    let chunk = Chunk::Ai(AIChunk {
        timestamp: ts(),
        duration_ms: None,
        responses: vec![],
        metrics: ChunkMetrics::default(),
        semantic_steps: vec![],
        tool_executions: vec![],
        subagents: vec![],
        slash_commands: vec![],
        teammate_messages: vec![],
    });
    let json = serde_json::to_value(&chunk).unwrap();
    assert_eq!(
        json["kind"],
        json!("ai"),
        "Chunk::Ai → kind: ai（不是 assistant）"
    );
}

#[test]
fn chunk_enum_system_and_compact_tags() {
    let s = Chunk::System(SystemChunk {
        uuid: "s1".into(),
        timestamp: ts(),
        duration_ms: None,
        content_text: "init".into(),
        metrics: ChunkMetrics::default(),
    });
    let c = Chunk::Compact(CompactChunk {
        uuid: "c1".into(),
        timestamp: ts(),
        duration_ms: None,
        summary_text: "summary".into(),
        metrics: ChunkMetrics::default(),
    });
    assert_eq!(serde_json::to_value(&s).unwrap()["kind"], json!("system"));
    assert_eq!(serde_json::to_value(&c).unwrap()["kind"], json!("compact"));
}

#[test]
fn ai_chunk_serializes_camelcase_fields() {
    let chunk = AIChunk {
        timestamp: ts(),
        duration_ms: Some(100),
        responses: vec![],
        metrics: ChunkMetrics::default(),
        semantic_steps: vec![],
        tool_executions: vec![],
        subagents: vec![],
        slash_commands: vec![SlashCommand {
            name: "/commit".into(),
            message: None,
            args: None,
            message_uuid: "mu1".into(),
            timestamp: ts(),
            instructions: None,
        }],
        teammate_messages: vec![TeammateMessage {
            uuid: "tm1".into(),
            teammate_id: "alice".into(),
            color: Some("blue".into()),
            summary: None,
            body: "hello".into(),
            timestamp: ts(),
            reply_to_tool_use_id: None,
            token_count: Some(42),
            is_noise: false,
            is_resend: false,
        }],
    };
    let json = serde_json::to_value(&chunk).unwrap();
    assert_eq!(json["durationMs"], json!(100));
    assert!(json.get("duration_ms").is_none());
    assert!(json["semanticSteps"].is_array());
    assert!(json["toolExecutions"].is_array());
    assert!(json["slashCommands"].is_array());
    assert!(json["teammateMessages"].is_array());
    assert!(json["subagents"].is_array());

    // SlashCommand 内部
    let sc = &json["slashCommands"][0];
    assert_eq!(sc["messageUuid"], json!("mu1"));
    assert_eq!(sc["name"], json!("/commit"));

    // TeammateMessage 内部 + Option None 字段被 skip
    let tm = &json["teammateMessages"][0];
    assert_eq!(tm["teammateId"], json!("alice"));
    assert_eq!(tm["isNoise"], json!(false));
    assert_eq!(tm["isResend"], json!(false));
    assert_eq!(tm["tokenCount"], json!(42));
    assert!(
        tm.get("summary").is_none(),
        "Option<String> None + skip_serializing_if MUST 不出现"
    );
    assert!(tm.get("replyToToolUseId").is_none());
}

#[test]
fn ai_chunk_empty_teammate_messages_omitted() {
    let chunk = AIChunk {
        timestamp: ts(),
        duration_ms: None,
        responses: vec![],
        metrics: ChunkMetrics::default(),
        semantic_steps: vec![],
        tool_executions: vec![],
        subagents: vec![],
        slash_commands: vec![],
        teammate_messages: vec![],
    };
    let json = serde_json::to_value(&chunk).unwrap();
    assert!(
        json.get("teammateMessages").is_none(),
        "空 teammate_messages SHALL 被 skip_serializing_if 去掉"
    );
}

#[test]
fn semantic_step_enum_tags_are_snake_case() {
    let steps = vec![
        SemanticStep::Thinking {
            text: "think".into(),
            timestamp: ts(),
        },
        SemanticStep::Text {
            text: "say".into(),
            timestamp: ts(),
        },
        SemanticStep::ToolExecution {
            tool_use_id: "tu1".into(),
            tool_name: "Bash".into(),
            timestamp: ts(),
        },
    ];
    let json = serde_json::to_value(&steps).unwrap();
    assert_eq!(json[0]["kind"], json!("thinking"));
    assert_eq!(json[1]["kind"], json!("text"));
    assert_eq!(json[2]["kind"], json!("tool_execution"));
    assert_eq!(json[2]["toolUseId"], json!("tu1"));
    assert_eq!(json[2]["toolName"], json!("Bash"));
}

// =============================================================================
// Schema-level: omit flag 命名规范
// =============================================================================

#[test]
fn assistant_response_content_omitted_field_name() {
    let r = AssistantResponse {
        uuid: "a1".into(),
        timestamp: ts(),
        content: MessageContent::Text(String::new()),
        tool_calls: vec![],
        usage: None,
        model: None,
        content_omitted: true,
    };
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["contentOmitted"], json!(true));
    assert!(
        json.get("content_omitted").is_none(),
        "MUST 不出现 snake_case"
    );
    assert!(
        json.get("responseContentOmitted").is_none(),
        "MUST 不出现命名变体"
    );
}

#[test]
fn tool_execution_output_omitted_field_name() {
    let exec = ToolExecution {
        tool_use_id: "tu1".into(),
        tool_name: "Bash".into(),
        input: json!({}),
        output: ToolOutput::Text {
            text: String::new(),
        },
        is_error: false,
        start_ts: ts(),
        end_ts: None,
        source_assistant_uuid: "a1".into(),
        result_agent_id: None,
        teammate_spawn: None,
        output_omitted: true,
        output_bytes: Some(1024),
    };
    let json = serde_json::to_value(&exec).unwrap();
    assert_eq!(json["outputOmitted"], json!(true));
    assert_eq!(json["outputBytes"], json!(1024));
    assert!(json.get("output_omitted").is_none());
    assert!(
        json.get("toolOutputOmitted").is_none(),
        "MUST 不出现命名变体"
    );
}

#[test]
fn image_source_data_omitted_field_name() {
    let img = ImageSource {
        kind: "base64".into(),
        media_type: "image/png".into(),
        data: String::new(),
        data_omitted: true,
    };
    let json = serde_json::to_value(&img).unwrap();
    assert_eq!(json["dataOmitted"], json!(true));
    // ImageSource 例外：`type` / `media_type` 保留 snake_case 与 Anthropic JSONL 一致
    assert_eq!(json["type"], json!("base64"));
    assert_eq!(json["media_type"], json!("image/png"));
    assert!(json.get("data_omitted").is_none());
    assert!(json.get("imageOmitted").is_none(), "MUST 不出现命名变体");
}

#[test]
fn process_messages_omitted_field_name() {
    let p = Process {
        session_id: "sub-1".into(),
        root_task_description: None,
        spawn_ts: ts(),
        end_ts: None,
        metrics: ChunkMetrics::default(),
        team: None,
        subagent_type: Some("code-reviewer".into()),
        messages: vec![],
        main_session_impact: None,
        is_ongoing: false,
        duration_ms: None,
        parent_task_id: None,
        description: None,
        header_model: None,
        last_isolated_tokens: 0,
        is_shutdown_only: false,
        messages_omitted: true,
    };
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["messagesOmitted"], json!(true));
    assert_eq!(json["sessionId"], json!("sub-1"));
    assert_eq!(json["subagentType"], json!("code-reviewer"));
    assert!(json.get("messages_omitted").is_none());
    assert!(
        json.get("subagentMessagesOmitted").is_none(),
        "MUST 不出现命名变体"
    );
}

// =============================================================================
// Schema-level: ContextInjection 6 个 category（kebab-case internally-tagged）
// =============================================================================

#[test]
fn context_injection_claude_md_category() {
    let inj = ContextInjection::ClaudeMd(ClaudeMdContextInjection {
        id: "cm1".into(),
        path: "/p/CLAUDE.md".into(),
        display_name: "CLAUDE.md".into(),
        scope: ClaudeMdScope::Project,
        estimated_tokens: 100,
        first_seen_turn_index: 0,
    });
    let json = serde_json::to_value(&inj).unwrap();
    assert_eq!(json["category"], json!("claude-md"));
    assert_eq!(json["displayName"], json!("CLAUDE.md"));
    assert_eq!(json["estimatedTokens"], json!(100));
    assert!(
        json.get("ClaudeMd").is_none(),
        "internally-tagged MUST 不是 externally-tagged 形式"
    );
}

#[test]
fn context_injection_all_six_categories_kebab_case() {
    let cases = [
        (
            ContextInjection::MentionedFile(MentionedFileInjection {
                id: "m1".into(),
                path: "/p/file.rs".into(),
                display_name: "file.rs".into(),
                estimated_tokens: 10,
                first_seen_turn_index: 0,
                first_seen_in_group: "g1".into(),
                exists: true,
            }),
            "mentioned-file",
        ),
        (
            ContextInjection::ToolOutput(ToolOutputInjection {
                id: "t1".into(),
                turn_index: 0,
                ai_group_id: "g1".into(),
                estimated_tokens: 50,
                tool_count: 1,
                tool_breakdown: vec![],
            }),
            "tool-output",
        ),
        (
            ContextInjection::ThinkingText(ThinkingTextInjection {
                id: "th1".into(),
                turn_index: 0,
                ai_group_id: "g1".into(),
                estimated_tokens: 5,
                breakdown: vec![],
            }),
            "thinking-text",
        ),
        (
            ContextInjection::TaskCoordination(TaskCoordinationInjection {
                id: "tc1".into(),
                turn_index: 0,
                ai_group_id: "g1".into(),
                estimated_tokens: 20,
                breakdown: vec![],
            }),
            "task-coordination",
        ),
        (
            ContextInjection::UserMessage(UserMessageInjection {
                id: "u1".into(),
                turn_index: 0,
                ai_group_id: "g1".into(),
                estimated_tokens: 2,
                text_preview: "hi".into(),
            }),
            "user-message",
        ),
    ];
    for (inj, expected_category) in cases {
        let json = serde_json::to_value(&inj).unwrap();
        assert_eq!(
            json["category"],
            json!(expected_category),
            "category tag mismatch for {expected_category}"
        );
    }
}

// =============================================================================
// Schema-level: NotificationTrigger（add_trigger / remove_trigger 入参）
// =============================================================================

#[test]
fn notification_trigger_serializes_camelcase_with_omitted_options() {
    let t = NotificationTrigger {
        id: "trig-1".into(),
        name: "On Error".into(),
        enabled: true,
        content_type: TriggerContentType::ToolResult,
        mode: TriggerMode::ErrorStatus,
        tool_name: Some("Bash".into()),
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
    let json = serde_json::to_value(&t).unwrap();
    assert_eq!(json["id"], json!("trig-1"));
    assert_eq!(json["contentType"], json!("tool_result"));
    assert_eq!(json["mode"], json!("error_status"));
    assert_eq!(json["toolName"], json!("Bash"));
    assert_eq!(json["requireError"], json!(true));
    assert!(
        json.get("isBuiltin").is_none(),
        "Option None + skip_serializing_if MUST 不出现"
    );
    assert!(json.get("matchField").is_none());
    assert!(json.get("tokenThreshold").is_none());
    assert!(json.get("color").is_none());
}

// =============================================================================
// API-level: 22 个 Tauri command 端到端调用
// =============================================================================

#[tokio::test]
async fn list_projects_returns_camelcase_array() {
    let (api, _tmp) = setup_api().await;
    let projects = api.list_projects().await.unwrap();
    let json = serde_json::to_value(&projects).unwrap();
    assert!(json.is_array(), "list_projects SHALL 返回 array");
    // 空 setup → 空 array
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_sessions_returns_paginated_response_shape() {
    let (api, _tmp) = setup_api().await;
    let resp = api
        .list_sessions(
            "any-project",
            &PaginatedRequest {
                page_size: 10,
                cursor: None,
            },
        )
        .await
        .unwrap();
    let json = serde_json::to_value(&resp).unwrap();
    assert!(json["items"].is_array());
    assert!(json["total"].is_number());
    // nextCursor 是 Option<String>，setup 为空时 None → null
    assert!(json["nextCursor"].is_null());
    assert!(json.get("next_cursor").is_none(), "MUST 不出现 snake_case");
}

#[tokio::test]
async fn list_sessions_sync_returns_paginated_response_shape() {
    // list_sessions_sync 是 LocalDataApi 公开方法（HTTP 路径用），不在 Tauri command
    // 列表中，但仍需契约守护。
    let (api, _tmp) = setup_api().await;
    let resp = api
        .list_sessions_sync(
            "any-project",
            &PaginatedRequest {
                page_size: 10,
                cursor: None,
            },
        )
        .await
        .unwrap();
    let json = serde_json::to_value(&resp).unwrap();
    assert!(json["items"].is_array());
    assert!(json["total"].is_number());
}

#[tokio::test]
async fn get_session_detail_missing_session_returns_error() {
    let (api, _tmp) = setup_api().await;
    let result = api.get_session_detail("ghost-project", "ghost-sess").await;
    assert!(
        result.is_err(),
        "找不到 session 时 SHALL 返回 ApiError 而非 panic"
    );
}

#[tokio::test]
async fn get_subagent_trace_missing_returns_empty_array() {
    let (api, _tmp) = setup_api().await;
    // LocalDataApi::get_subagent_trace 设计上找不到 root/subagent 时 SHALL Ok
    // 返空 array（见 cdt-api/src/ipc/local.rs 实现），不抛 Err / 不 panic
    let value = api
        .get_subagent_trace("ghost-root", "ghost-sub")
        .await
        .expect("get_subagent_trace SHALL Ok 即使找不到");
    assert!(value.is_array(), "返回值 SHALL 是 JSON array");
    assert_eq!(value.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn get_image_asset_invalid_block_returns_data_uri() {
    let (api, _tmp) = setup_api().await;
    // get_image_asset 设计上对无效 block_id 返回空 data: URI 的 Ok（见 cdt-api
    // 现有测试 `get_image_asset_invalid_block_id_returns_empty_data_uri`）
    let uri = api
        .get_image_asset("ghost-root", "ghost-sess", "ghost-block")
        .await
        .expect("invalid block_id SHALL Ok empty data URI 而非 Err");
    assert!(
        uri.starts_with("data:") || uri.is_empty(),
        "返回值 SHALL 是 data: URI 字符串，实际：{uri}"
    );
}

#[tokio::test]
async fn get_tool_output_missing_returns_tool_output_enum() {
    let (api, _tmp) = setup_api().await;
    // 找不到时 SHALL Ok ToolOutput::Missing（见 cdt-api 现有测试
    // `get_tool_output_returns_missing_when_jsonl_not_exist`）
    let output = api
        .get_tool_output("ghost-root", "ghost-sess", "ghost-tu")
        .await
        .expect("找不到 tool output 时 SHALL Ok Missing 而非 Err");
    let json = serde_json::to_value(&output).unwrap();
    assert!(json.is_object(), "ToolOutput SHALL 序列化为 object");
    let kind = json["kind"].as_str().expect("kind 字段 SHALL 存在");
    assert!(
        matches!(kind, "text" | "structured" | "missing"),
        "kind SHALL 是 text/structured/missing 之一，实际：{kind}"
    );
    // ghost id 必走 Missing 分支
    assert_eq!(kind, "missing");
}

#[tokio::test]
async fn search_with_missing_project_id_returns_path_not_found_err() {
    let (api, _tmp) = setup_api().await;
    let req = SearchRequest {
        query: "hello".into(),
        project_id: Some("ghost-project".into()),
        session_id: None,
    };
    // 真实契约：project_id 对应目录不存在时 SHALL Err（"path not found"），
    // UI 应保证 project_id 有效。这不同于 list_projects 等"读已知存储"语义。
    let err = api
        .search(&req)
        .await
        .expect_err("search 对不存在的 project_id SHALL Err");
    let msg = err.to_string();
    assert!(
        msg.contains("path not found"),
        "Err message SHALL 含 'path not found'，实际：{msg}"
    );
}

#[tokio::test]
async fn search_without_project_id_returns_validation_err() {
    let (api, _tmp) = setup_api().await;
    let req = SearchRequest {
        query: "hello".into(),
        project_id: None,
        session_id: None,
    };
    // 契约（cdt-api/src/ipc/local.rs::search）：缺 project_id SHALL Err validation
    let err = api
        .search(&req)
        .await
        .expect_err("search 无 project_id SHALL Err validation");
    let msg = err.to_string();
    assert!(
        msg.contains("project_id is required"),
        "Err message SHALL 含 'project_id is required'，实际：{msg}"
    );
}

#[tokio::test]
async fn get_config_returns_camelcase_top_level_sections() {
    let (api, _tmp) = setup_api().await;
    let config = api.get_config().await.unwrap();
    assert!(config.is_object(), "get_config SHALL 返回 object");
    let obj = config.as_object().unwrap();
    // AppConfig 顶层 sections（camelCase 后）
    for key in ["notifications", "general", "display", "sessions", "ssh"] {
        assert!(obj.contains_key(key), "顶层 section MUST 含 {key}");
    }
    // httpServer 是 camelCase 后的形式
    assert!(
        obj.contains_key("httpServer"),
        "http_server section SHALL 序列化为 httpServer"
    );
    assert!(!obj.contains_key("http_server"), "MUST 不出现 snake_case");

    // notifications.triggers 是数组
    assert!(config["notifications"]["triggers"].is_array());
    // notifications.soundEnabled 是 camelCase
    assert!(config["notifications"]["soundEnabled"].is_boolean());
}

#[tokio::test]
async fn update_config_with_invalid_section_returns_error() {
    let (api, _tmp) = setup_api().await;
    let req = ConfigUpdateRequest {
        section: "nonexistent_section".into(),
        data: json!({ "foo": "bar" }),
    };
    let result = api.update_config(&req).await;
    assert!(
        result.is_err(),
        "无效 section SHALL 返回 ApiError 而非 panic"
    );
}

#[tokio::test]
async fn get_notifications_returns_object_shape() {
    let (api, _tmp) = setup_api().await;
    let result = api.get_notifications(50, 0).await.unwrap();
    // GetNotificationsResult { notifications: Vec<...>, totalCount: usize }
    let json = serde_json::to_value(&result).unwrap();
    assert!(json["notifications"].is_array());
    assert!(json["totalCount"].is_number());
    assert!(json.get("total_count").is_none(), "MUST 不出现 snake_case");
}

#[tokio::test]
async fn mark_notification_read_returns_bool() {
    let (api, _tmp) = setup_api().await;
    let result = api.mark_notification_read("ghost-id").await.unwrap();
    let json = serde_json::to_value(result).unwrap();
    assert!(json.is_boolean(), "mark_notification_read SHALL 返回 bool");
    // 不存在的 id → false
    assert_eq!(json, json!(false));
}

#[tokio::test]
async fn delete_notification_returns_bool() {
    let (api, _tmp) = setup_api().await;
    let result = api.delete_notification("ghost-id").await.unwrap();
    let json = serde_json::to_value(result).unwrap();
    assert_eq!(json, json!(false));
}

#[tokio::test]
async fn mark_all_notifications_read_returns_unit() {
    let (api, _tmp) = setup_api().await;
    api.mark_all_notifications_read().await.unwrap();
    // () → null（Tauri command 包装层会 serialize 此 Result，验证序列化形态）
    let json = serde_json::to_value(()).unwrap();
    assert_eq!(json, json!(null), "() SHALL 序列化为 null");
}

#[tokio::test]
async fn clear_notifications_returns_count() {
    let (api, _tmp) = setup_api().await;
    let count = api.clear_notifications(None).await.unwrap();
    let json = serde_json::to_value(count).unwrap();
    assert!(json.is_number(), "clear_notifications SHALL 返回 usize");
    assert_eq!(json, json!(0));
}

#[tokio::test]
async fn add_trigger_returns_value_shape() {
    let (api, _tmp) = setup_api().await;
    let trigger = NotificationTrigger {
        id: "test-trigger-1".into(), // 必须非空（contract: "Trigger ID is required"）
        name: "Test Trigger".into(),
        enabled: true,
        content_type: TriggerContentType::ToolResult,
        mode: TriggerMode::ErrorStatus,
        tool_name: None,
        is_builtin: Some(false),
        ignore_patterns: None,
        require_error: Some(true),
        match_field: None,
        match_pattern: None,
        token_threshold: None,
        token_type: None,
        repository_ids: None,
        color: None,
    };
    // valid trigger（含非空 id） SHALL Ok 返回 JSON value
    let value = api
        .add_trigger(trigger)
        .await
        .expect("valid NotificationTrigger SHALL Ok");
    assert!(
        value.is_object() || value.is_array() || value.is_string(),
        "add_trigger SHALL 返回 JSON object/array/string，实际：{value:?}"
    );
}

#[tokio::test]
async fn add_trigger_without_id_returns_validation_err() {
    let (api, _tmp) = setup_api().await;
    let trigger = NotificationTrigger {
        id: String::new(), // 触发 "Trigger ID is required" 校验
        name: "Test".into(),
        enabled: true,
        content_type: TriggerContentType::ToolResult,
        mode: TriggerMode::ErrorStatus,
        tool_name: None,
        is_builtin: Some(false),
        ignore_patterns: None,
        require_error: Some(true),
        match_field: None,
        match_pattern: None,
        token_threshold: None,
        token_type: None,
        repository_ids: None,
        color: None,
    };
    let err = api
        .add_trigger(trigger)
        .await
        .expect_err("空 id 的 trigger SHALL Err validation");
    let msg = err.to_string();
    assert!(
        msg.contains("Trigger ID is required"),
        "Err message SHALL 含 'Trigger ID is required'，实际：{msg}"
    );
}

#[tokio::test]
async fn remove_trigger_with_unknown_id_returns_not_found_err() {
    let (api, _tmp) = setup_api().await;
    // 真实契约：remove 不存在的 id SHALL Err（trigger manager 不是 idempotent）
    let err = api
        .remove_trigger("ghost-trig")
        .await
        .expect_err("remove 不存在 trigger SHALL Err not found");
    let msg = err.to_string();
    assert!(
        msg.contains("not found"),
        "Err message SHALL 含 'not found'，实际：{msg}"
    );
}

#[tokio::test]
async fn read_agent_configs_returns_array() {
    let (api, _tmp) = setup_api().await;
    let configs = api.read_agent_configs().await.unwrap();
    let json = serde_json::to_value(&configs).unwrap();
    assert!(
        json.is_array(),
        "read_agent_configs SHALL 返回 AgentConfig array"
    );
}

#[tokio::test]
async fn pin_unpin_session_round_trip() {
    let (api, _tmp) = setup_api().await;
    api.pin_session("p1", "s1").await.unwrap();
    let prefs = api.get_project_session_prefs("p1").await.unwrap();
    let json = serde_json::to_value(&prefs).unwrap();
    assert_eq!(json["pinned"], json!(["s1"]));
    assert_eq!(json["hidden"], json!([]));

    api.unpin_session("p1", "s1").await.unwrap();
    let prefs = api.get_project_session_prefs("p1").await.unwrap();
    let json = serde_json::to_value(&prefs).unwrap();
    assert_eq!(json["pinned"], json!([]));
}

#[tokio::test]
async fn hide_unhide_session_round_trip() {
    let (api, _tmp) = setup_api().await;
    api.hide_session("p1", "s1").await.unwrap();
    let prefs = api.get_project_session_prefs("p1").await.unwrap();
    let json = serde_json::to_value(&prefs).unwrap();
    assert_eq!(json["hidden"], json!(["s1"]));

    api.unhide_session("p1", "s1").await.unwrap();
    let prefs = api.get_project_session_prefs("p1").await.unwrap();
    let json = serde_json::to_value(&prefs).unwrap();
    assert_eq!(json["hidden"], json!([]));
}

#[tokio::test]
async fn get_project_session_prefs_empty_project_returns_default_shape() {
    let (api, _tmp) = setup_api().await;
    let prefs = api
        .get_project_session_prefs("never-touched")
        .await
        .unwrap();
    let json = serde_json::to_value(&prefs).unwrap();
    assert_eq!(json["pinned"], json!([]));
    assert_eq!(json["hidden"], json!([]));
    assert!(
        json.as_object()
            .unwrap()
            .keys()
            .all(|k| k == "pinned" || k == "hidden"),
        "ProjectSessionPrefs SHALL 只含 pinned/hidden 两个 key"
    );
}
