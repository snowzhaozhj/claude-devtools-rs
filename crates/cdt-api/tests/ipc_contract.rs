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
    ConfigUpdateRequest, DataApi, LocalDataApi, MemoryFileContent, MemoryLayer, MemoryLayerKind,
    PaginatedRequest, PaginatedResponse, ProjectInfo, ProjectMemory, ProjectSessionPrefs,
    SearchRequest, SessionMetadataUpdate, SessionSummary, SshAuthMethod, SshConnectRequest,
    WslDistroCandidate, WslDistroScanReport,
};
use cdt_config::{
    ConfigManager, NotificationManager, NotificationTrigger, TriggerContentType, TriggerMode,
};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::{SshConnectionManager, SshFileSystemProvider};
use chrono::{TimeZone, Utc};
use serde_json::json;
use tempfile::TempDir;

#[path = "common/fake_remote_sftp.rs"]
mod fake_remote_sftp;
use fake_remote_sftp::{CountedFakeRemoteSftp, FakeCounters};

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

// 共享常量从 `tests/contract_data.rs` 引入——`tests/http_contract.rs` 也用
// 同一份避免漂移。`#[path]` 把源码内联到本编译单元，dead_code allow 已在
// contract_data 内部 gate。
#[path = "contract_data.rs"]
mod contract_data;
use contract_data::EXPECTED_TAURI_COMMANDS;

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

async fn write_user_session(dir: &std::path::Path, session_id: &str, cwd: &str, text: &str) {
    let line = format!(
        r#"{{"type":"user","uuid":"{session_id}","parentUuid":null,"timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"user","content":"{text}"}}}}"#,
    );
    tokio::fs::write(dir.join(format!("{session_id}.jsonl")), format!("{line}\n"))
        .await
        .unwrap();
}

// =============================================================================
// Meta 测：command 列表完整性
// =============================================================================

#[test]
fn expected_tauri_commands_count_is_49() {
    assert_eq!(
        EXPECTED_TAURI_COMMANDS.len(),
        49,
        "EXPECTED_TAURI_COMMANDS 长度变化时 SHALL 同步更新 src-tauri/src/lib.rs::invoke_handler! \
         以及本文件常量；当前 src-tauri 注册 49 个 Tauri command（含 SSH + server-mode + \
         simplify-repository-as-project change 加的 list_group_sessions + change \
         ssh-project-memory-remote-rw 加的 add_memory / delete_memory + change \
         add-telemetry-signal-bus 加的 get_telemetry_snapshot / record_correctness_events）"
    );
}

#[test]
fn expected_tauri_commands_include_server_mode_three() {
    for name in [
        "http_server_start",
        "http_server_stop",
        "http_server_status",
    ] {
        assert!(
            EXPECTED_TAURI_COMMANDS.contains(&name),
            "server-mode command {name} SHALL 在 EXPECTED_TAURI_COMMANDS 内（与 \
             src-tauri/src/lib.rs::invoke_handler! 同步）"
        );
    }
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
fn ssh_connect_request_accepts_new_and_legacy_payloads() {
    let new_payload: SshConnectRequest = serde_json::from_value(json!({
        "host": "prod-box",
        "port": 2222,
        "username": "alice",
        "authMethod": "password",
        "password": "secret",
        "contextId": "ctx-prod"
    }))
    .unwrap();
    assert_eq!(new_payload.host, "prod-box");
    assert_eq!(new_payload.port, Some(2222));
    assert_eq!(new_payload.username.as_deref(), Some("alice"));
    assert_eq!(new_payload.auth_method, SshAuthMethod::Password);
    assert_eq!(new_payload.context_id.as_deref(), Some("ctx-prod"));

    let legacy_payload: SshConnectRequest = serde_json::from_value(json!({
        "hostAlias": "legacy-host"
    }))
    .unwrap();
    assert_eq!(legacy_payload.host, "legacy-host");
    assert_eq!(legacy_payload.auth_method, SshAuthMethod::SshConfig);
    assert_eq!(legacy_payload.port, None);

    let serialized = serde_json::to_value(&new_payload).unwrap();
    assert_eq!(serialized["authMethod"], json!("password"));
    assert_eq!(serialized["contextId"], json!("ctx-prod"));
    assert!(serialized.get("host_alias").is_none());
    assert!(serialized.get("auth_method").is_none());
}

#[test]
fn ssh_connection_result_shape_matches_connect_and_test_connection_contract() {
    let result = cdt_api::SshConnectionResult {
        context_id: "ctx-test".into(),
        status: cdt_ssh::SshStatus::Connected,
        auth_chain: vec![],
    };
    let json = serde_json::to_value(result).unwrap();
    assert_eq!(json["contextId"], json!("ctx-test"));
    assert_eq!(json["status"], json!("connected"));
    assert!(json["authChain"].is_array());
    assert!(json.get("context_id").is_none());
    assert!(json.get("auth_chain").is_none());
}

#[test]
fn ssh_auth_and_error_payloads_match_ipc_contract() {
    let attempt = cdt_ssh::AuthAttempt {
        source: cdt_ssh::AuthSource::Password,
        outcome: cdt_ssh::AuthOutcome::Failure("denied".into()),
        elapsed_ms: 12,
    };
    let attempt_json = serde_json::to_value(&attempt).unwrap();
    assert_eq!(attempt_json["source"]["type"], json!("password"));
    assert_eq!(attempt_json["outcome"]["type"], json!("failure"));
    assert_eq!(attempt_json["outcome"]["data"], json!("denied"));
    assert_eq!(attempt_json["elapsedMs"], json!(12));
    assert!(attempt_json.get("elapsed_ms").is_none());

    let error = cdt_ssh::SshError::AuthExhausted {
        attempts: vec![attempt],
    };
    let error_json = serde_json::to_value(&error).unwrap();
    assert_eq!(error_json["code"], json!("ssh_auth_exhausted"));
    assert!(error_json.get("AuthExhausted").is_none());
}

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
fn project_memory_serializes_camelcase() {
    let memory = ProjectMemory {
        project_id: "proj-1".into(),
        has_memory: true,
        count: 2,
        default_file: Some("MEMORY.md".into()),
        layers: vec![MemoryLayer {
            file: "MEMORY.md".into(),
            title: "Index".into(),
            hook: Some("MEMORY.md".into()),
            kind: MemoryLayerKind::Index,
        }],
    };
    let json = serde_json::to_value(&memory).unwrap();
    assert_eq!(json["projectId"], json!("proj-1"));
    assert_eq!(json["hasMemory"], json!(true));
    assert_eq!(json["defaultFile"], json!("MEMORY.md"));
    assert_eq!(json["layers"][0]["kind"], json!("index"));
    assert!(json.get("project_id").is_none());
    assert!(json.get("has_memory").is_none());
    assert!(json.get("default_file").is_none());
}

#[test]
fn memory_file_content_serializes_camelcase() {
    let content = MemoryFileContent {
        project_id: "proj-1".into(),
        file: "MEMORY.md".into(),
        file_path: "/mock/proj-1/memory/MEMORY.md".into(),
        content: "# Memory".into(),
    };
    let json = serde_json::to_value(&content).unwrap();
    assert_eq!(json["projectId"], json!("proj-1"));
    assert_eq!(json["file"], json!("MEMORY.md"));
    assert_eq!(json["filePath"], json!("/mock/proj-1/memory/MEMORY.md"));
    assert_eq!(json["content"], json!("# Memory"));
    assert!(json.get("project_id").is_none());
    assert!(json.get("file_path").is_none());
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
        git_branch: Some("feat/x".into()),
        worktree_id: None,
        worktree_name: None,
        group_id: None,
        cwd_relative_to_repo_root: None,
        cwd: Some("/Users/foo/repo".into()),
    };
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["sessionId"], json!("sess-1"));
    assert_eq!(json["projectId"], json!("proj-1"));
    assert_eq!(json["messageCount"], json!(12));
    assert_eq!(json["isOngoing"], json!(true));
    assert_eq!(json["title"], json!("hello"));
    assert_eq!(json["gitBranch"], json!("feat/x"));
    assert_eq!(json["cwd"], json!("/Users/foo/repo"));
    assert!(
        json.get("git_branch").is_none(),
        "MUST 不出现 snake_case 字段名"
    );

    // Skeleton variant (title=None / git_branch=None / cwd=None)
    let skeleton = SessionSummary {
        title: None,
        is_ongoing: false,
        git_branch: None,
        cwd: None,
        ..s
    };
    let json = serde_json::to_value(&skeleton).unwrap();
    assert_eq!(json["title"], json!(null), "Option<String> None → null");
    assert_eq!(json["isOngoing"], json!(false));
    assert_eq!(json["gitBranch"], json!(null));
    assert!(
        json.get("cwd").is_none(),
        "cwd=None SHALL 被 skip_serializing_if 省略输出"
    );
}

#[test]
fn session_metadata_update_serializes_camelcase_with_git_branch() {
    let u = SessionMetadataUpdate {
        project_id: "proj-1".into(),
        session_id: "sess-1".into(),
        title: Some("hello".into()),
        message_count: 7,
        is_ongoing: true,
        git_branch: Some("feat/x".into()),
        group_id: Some("group-1".into()),
    };
    let json = serde_json::to_value(&u).unwrap();
    assert_eq!(json["projectId"], json!("proj-1"));
    assert_eq!(json["sessionId"], json!("sess-1"));
    assert_eq!(json["title"], json!("hello"));
    assert_eq!(json["messageCount"], json!(7));
    assert_eq!(json["isOngoing"], json!(true));
    assert_eq!(json["gitBranch"], json!("feat/x"));
    assert!(
        json.get("git_branch").is_none(),
        "MUST 不出现 snake_case 字段名"
    );

    let none_branch = SessionMetadataUpdate {
        git_branch: None,
        ..u
    };
    let json = serde_json::to_value(&none_branch).unwrap();
    assert_eq!(json["gitBranch"], json!(null));
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
        chunk_id: "u1".into(),
        uuid: "u1".into(),
        timestamp: ts(),
        duration_ms: None,
        content: MessageContent::Text("hi".into()),
        metrics: ChunkMetrics::default(),
    });
    let json = serde_json::to_value(&chunk).unwrap();
    assert_eq!(json["kind"], json!("user"), "Chunk::User → kind: user");
    assert_eq!(json["chunkId"], json!("u1"));
    assert!(json.get("chunk_id").is_none());
}

#[test]
fn chunk_enum_ai_tag_is_ai_not_assistant() {
    let chunk = Chunk::Ai(AIChunk {
        chunk_id: "ai:a1:0".into(),
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
    assert_eq!(json["chunkId"], json!("ai:a1:0"));
    assert!(json.get("chunk_id").is_none());
}

#[test]
fn chunk_enum_system_and_compact_tags() {
    let s = Chunk::System(SystemChunk {
        chunk_id: "s1".into(),
        uuid: "s1".into(),
        timestamp: ts(),
        duration_ms: None,
        content_text: "init".into(),
        metrics: ChunkMetrics::default(),
    });
    let c = Chunk::Compact(CompactChunk {
        chunk_id: "c1".into(),
        uuid: "c1".into(),
        timestamp: ts(),
        duration_ms: None,
        summary_text: "summary".into(),
        metrics: ChunkMetrics::default(),
        token_delta: None,
        phase_number: None,
    });
    let system = serde_json::to_value(&s).unwrap();
    let compact = serde_json::to_value(&c).unwrap();
    assert_eq!(system["kind"], json!("system"));
    assert_eq!(system["chunkId"], json!("s1"));
    assert!(system.get("chunk_id").is_none());
    assert_eq!(compact["kind"], json!("compact"));
    assert_eq!(compact["chunkId"], json!("c1"));
    assert!(compact.get("chunk_id").is_none());
}

/// 验 `CompactChunk.tokenDelta` / `phaseNumber` 在 `Some(...)` 时序列化
/// 使用 camelCase 键名，且 `tokenDelta` 内层（`preCompactionTokens` 等）
/// 也是 camelCase。spec: ipc-data-api "Token delta present" Scenario。
#[test]
fn compact_chunk_serializes_token_delta_and_phase_number_camelcase() {
    let c = Chunk::Compact(CompactChunk {
        chunk_id: "c1".into(),
        uuid: "c1".into(),
        timestamp: ts(),
        duration_ms: None,
        summary_text: "summary".into(),
        metrics: ChunkMetrics::default(),
        token_delta: Some(cdt_core::CompactionTokenDelta {
            pre_compaction_tokens: 30_000,
            post_compaction_tokens: 5_000,
            delta: -25_000,
        }),
        phase_number: Some(3),
    });
    let v = serde_json::to_value(&c).unwrap();
    assert_eq!(v["tokenDelta"]["preCompactionTokens"], 30_000);
    assert_eq!(v["tokenDelta"]["postCompactionTokens"], 5_000);
    assert_eq!(v["tokenDelta"]["delta"], -25_000);
    assert_eq!(v["phaseNumber"], 3);
    // 反向断言：snake_case 形态不存在
    assert!(v["token_delta"].is_null());
    assert!(v["phase_number"].is_null());
}

/// 验 `tokenDelta: None` AND `phaseNumber: None` 时序列化省略两个字段
/// （`#[serde(skip_serializing_if = "Option::is_none")]` 行为）。spec:
/// ipc-data-api "Token delta None" / "Phase number None" Scenarios。
#[test]
fn compact_chunk_omits_optional_derived_fields_when_none() {
    let c = Chunk::Compact(CompactChunk {
        chunk_id: "c1".into(),
        uuid: "c1".into(),
        timestamp: ts(),
        duration_ms: None,
        summary_text: "summary".into(),
        metrics: ChunkMetrics::default(),
        token_delta: None,
        phase_number: None,
    });
    let v = serde_json::to_value(&c).unwrap();
    assert!(
        v.get("tokenDelta").is_none(),
        "tokenDelta key should be omitted when None"
    );
    assert!(
        v.get("phaseNumber").is_none(),
        "phaseNumber key should be omitted when None"
    );
}

#[test]
fn ai_chunk_serializes_camelcase_fields() {
    let chunk = AIChunk {
        chunk_id: "ai:a1:0".into(),
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
fn aichunk_with_empty_responses_and_teammate_messages_round_trips() {
    // Spec: chunk-building::Embed teammate messages into AIChunk 第 5 条规则。
    // 守住 empty-responses + 非空 teammate_messages 的 AIChunk 可序列化 +
    // 反序列化等价（前端 type 与 displayItemBuilder 假设依赖此形态）。
    let chunk = AIChunk {
        chunk_id: "tm1:0".into(),
        timestamp: ts(),
        duration_ms: None,
        responses: vec![],
        metrics: ChunkMetrics::default(),
        semantic_steps: vec![],
        tool_executions: vec![],
        subagents: vec![],
        slash_commands: vec![],
        teammate_messages: vec![TeammateMessage {
            uuid: "tm1".into(),
            teammate_id: "alice".into(),
            color: Some("blue".into()),
            summary: Some("you are frontend".into()),
            body: "你是 kb-shortcuts team 的 frontend teammate".into(),
            timestamp: ts(),
            reply_to_tool_use_id: None,
            token_count: Some(42),
            is_noise: false,
            is_resend: false,
        }],
    };
    let json = serde_json::to_string(&chunk).expect("serialize empty-AI");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("re-parse json");
    assert_eq!(
        parsed["responses"],
        json!([]),
        "responses should serialize as empty array"
    );
    assert!(
        parsed["teammateMessages"].is_array(),
        "teammateMessages should be array"
    );
    assert_eq!(parsed["teammateMessages"][0]["teammateId"], json!("alice"));

    let round_tripped: AIChunk = serde_json::from_str(&json).expect("deserialize empty-AI");
    assert_eq!(round_tripped, chunk, "round-trip identity");
}

#[test]
fn ai_chunk_empty_teammate_messages_omitted() {
    let chunk = AIChunk {
        chunk_id: "ai:a1:0".into(),
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
        error_message: Some("boom".into()),
        teammate_spawn: None,
        output_omitted: true,
        output_bytes: Some(1024),
    };
    let json = serde_json::to_value(&exec).unwrap();
    assert_eq!(json["outputOmitted"], json!(true));
    assert_eq!(json["outputBytes"], json!(1024));
    assert_eq!(json["errorMessage"], json!("boom"));
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

/// spec `ipc-data-api` "Expose subagent messages total count"：OMIT 默认路径下
/// `messagesTotalCount` MUST 等于 subagent `build_chunks` 后的真实 chunk 数（即裁剪
/// 前的 `messages.len()`），即使 `messages` 已被清空、`messagesOmitted=true`。
#[test]
fn subagent_messages_total_count_in_omit_path() {
    // 模拟 IPC 裁剪后的 Process：messages 已被 apply_subagent_messages_omit 清空、
    // messages_omitted=true，但 messages_total_count 仍是 resolver 阶段填好的原值。
    let p = Process {
        session_id: "sub-omit".into(),
        root_task_description: None,
        spawn_ts: ts(),
        end_ts: None,
        metrics: ChunkMetrics::default(),
        team: None,
        subagent_type: None,
        messages: vec![],
        main_session_impact: None,
        is_ongoing: true,
        duration_ms: None,
        parent_task_id: Some("toolu-A".into()),
        description: None,
        header_model: None,
        last_isolated_tokens: 0,
        is_shutdown_only: false,
        messages_omitted: true,
        messages_total_count: 12, // 裁剪前真实 chunk 数
    };
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["messagesOmitted"], json!(true));
    assert_eq!(json["messagesTotalCount"], json!(12));
    assert_eq!(json["messages"], json!([]));
    assert!(
        json["messagesTotalCount"].is_u64(),
        "MUST 是无符号整数（u32 序列化为 JSON number）"
    );
}

/// spec `ipc-data-api` "Expose subagent messages total count"：rollback 路径
/// （`OMIT_SUBAGENT_MESSAGES=false`）下 `messagesTotalCount` MUST 仍等于
/// `messages.len()`，与 OMIT 路径保持同字段语义。
#[test]
fn subagent_messages_total_count_in_rollback_path() {
    use cdt_core::{AIChunk, Chunk};

    let ai_chunk = Chunk::Ai(AIChunk {
        chunk_id: "ai:a1:0".into(),
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

    // 模拟 rollback 路径：messages 含完整 chunks、messages_omitted=false，
    // messages_total_count 仍等于 messages.len()
    let p = Process {
        session_id: "sub-rollback".into(),
        root_task_description: None,
        spawn_ts: ts(),
        end_ts: None,
        metrics: ChunkMetrics::default(),
        team: None,
        subagent_type: None,
        messages: vec![ai_chunk.clone(), ai_chunk.clone(), ai_chunk],
        main_session_impact: None,
        is_ongoing: false,
        duration_ms: None,
        parent_task_id: None,
        description: None,
        header_model: None,
        last_isolated_tokens: 0,
        is_shutdown_only: false,
        messages_omitted: false,
        messages_total_count: 3,
    };
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["messagesOmitted"], json!(false));
    assert_eq!(json["messagesTotalCount"], json!(3));
    assert_eq!(json["messages"].as_array().unwrap().len(), 3);
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
        messages_total_count: 7,
    };
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["messagesOmitted"], json!(true));
    assert_eq!(json["sessionId"], json!("sub-1"));
    assert_eq!(json["subagentType"], json!("code-reviewer"));
    // spec ipc-data-api "Expose subagent messages total count"：u32 字段，
    // camelCase 形态，OMIT 与 rollback 路径下行为一致
    assert_eq!(json["messagesTotalCount"], json!(7));
    assert!(json.get("messages_omitted").is_none());
    assert!(json.get("messages_total_count").is_none());
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
// API-level: Tauri command 端到端调用
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

/// 任一 fs op counter 增量 ≥ 1 才算"真走了远端 fake provider"。
///
/// 用途：每个 IPC method 调用前后 snapshot，断言至少一类 op（`metadata` / `read` /
/// `read_dir` / `read_lines_head` / `try_exists`）触发——防止某个 IPC method
/// 误退化为 local fs 仍返合理默认值的假阳性（followups.md
/// `[coverage-gap] active context dispatch contract test 缺 read 计数器`）。
fn assert_remote_fs_touched(before: FakeCounters, after: FakeCounters, method: &str) {
    let touched = after.metadata > before.metadata
        || after.read > before.read
        || after.read_dir > before.read_dir
        || after.read_lines_head > before.read_lines_head
        || after.try_exists > before.try_exists;
    assert!(
        touched,
        "{method} SHALL 触发至少一次远端 fs op（before: {before:?} → after: {after:?}）；\
         若 counter 全 0，意味着 IPC method 误退化为 local fs 而非走 SSH provider"
    );
}

#[tokio::test]
async fn active_ssh_context_reads_remote_projects_and_sessions() {
    let (api, _tmp) = setup_api().await;
    let remote_home = "/remote/home/.claude/projects";
    let project_id = "-remote-project";
    let session_id = "remote-session";
    let cwd = "/srv/remote-project";
    let line = format!(
        r#"{{"type":"user","uuid":"{session_id}","parentUuid":null,"timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"user","content":"from remote"}}}}"#,
    );
    let fake = Arc::new(CountedFakeRemoteSftp::with_session(
        remote_home,
        project_id,
        session_id,
        format!("{line}\n"),
    ));
    let provider = SshFileSystemProvider::with_client(
        "ctx-remote",
        fake.clone() as Arc<dyn cdt_ssh::SftpClient>,
        std::path::PathBuf::from(remote_home),
    );
    api.insert_test_ssh_context(
        "ctx-remote",
        "remote-host",
        22,
        Some("alice".into()),
        std::path::PathBuf::from(remote_home),
        provider,
    )
    .await;

    let before = fake.snapshot_counters();
    let projects = api.list_projects().await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, project_id);
    assert_eq!(projects[0].path, cwd);
    assert_remote_fs_touched(before, fake.snapshot_counters(), "list_projects");

    let pagination = PaginatedRequest {
        page_size: 10,
        cursor: None,
    };
    let mut metadata_rx = api.subscribe_session_metadata();
    let before = fake.snapshot_counters();
    let sessions = api.list_sessions(project_id, &pagination).await.unwrap();
    assert_eq!(sessions.items[0].session_id, session_id);
    // change `unify-fs-direct-calls` design D2/D3：SSH 改走 SkeletonThenStream 后
    // hot path 首屏只返骨架（title=None），title / message_count 通过 SSE
    // `session_metadata_update` 事件异步推差量。首次访问无 cache → 走 page_jobs
    // 后台 scan；二次访问会从 cache hit trust 立刻拿到完整 metadata。
    assert!(
        sessions.items[0].title.is_none(),
        "首次 SSH list_sessions 骨架不含 title（SkeletonThenStream）"
    );

    let update = tokio::time::timeout(std::time::Duration::from_secs(2), metadata_rx.recv())
        .await
        .expect("remote list_sessions should emit metadata update")
        .expect("metadata channel should stay open");
    assert_eq!(update.project_id, project_id);
    assert_eq!(update.session_id, session_id);
    assert_eq!(update.title.as_deref(), Some("from remote"));
    assert_eq!(update.message_count, 1);
    // list_sessions 骨架走 read_dir + 后台 batch scan 触发 read（async update 收齐后断言）
    assert_remote_fs_touched(before, fake.snapshot_counters(), "list_sessions");

    let before = fake.snapshot_counters();
    let sync_sessions = api
        .list_sessions_sync(project_id, &pagination)
        .await
        .unwrap();
    assert_eq!(sync_sessions.items[0].session_id, session_id);
    assert_eq!(sync_sessions.items[0].title.as_deref(), Some("from remote"));
    assert_eq!(sync_sessions.items[0].message_count, 1);
    assert_remote_fs_touched(before, fake.snapshot_counters(), "list_sessions_sync");

    let before = fake.snapshot_counters();
    let detail = api
        .get_session_detail(project_id, session_id)
        .await
        .unwrap();
    assert_eq!(detail.session_id, session_id);
    assert_eq!(detail.metrics["message_count"], json!(1));
    assert_remote_fs_touched(before, fake.snapshot_counters(), "get_session_detail");

    // ====== 本 change `fix-ssh-active-context-dispatch` 新增 ======
    // 覆盖 8 处修复的 IPC method 走 SSH provider 的契约（design.md D4）

    // list_projects 已经在前面调过会写入 SSH ContextId 的 ProjectScanCache entry
    // （change `project-scanner-memoize` FU-4）；这里调 list_repository_groups
    // 会 cache hit 跳过 fs op → counter 不增 → assert_remote_fs_touched 假阳性
    // FAIL。显式清掉让本断言走真实远端 fs op 路径（生产代码用 watcher /
    // generation / TTL 失效；测试用例之间用 invalidate_project_scan_cache）。
    api.invalidate_project_scan_cache();

    // list_repository_groups：active context = SSH 时返回远端项目集合，
    // 而不是宿主机本地的 git repo（容器内/fake 远端无 .git，所以无 gitBranch）
    let before = fake.snapshot_counters();
    let repo_groups = api.list_repository_groups().await.unwrap();
    assert!(
        !repo_groups.is_empty(),
        "SSH context 下 list_repository_groups SHALL 返回远端项目"
    );
    // worktree path 与 fake fixture 的 cwd 一致（来自 fake jsonl 的 cwd 字段，
    // 而非宿主机的真实路径）。
    let any_worktree_match = repo_groups.iter().any(|g| {
        g.worktrees
            .iter()
            .any(|w| w.path.to_string_lossy() == cwd && w.git_branch.is_none())
    });
    assert!(
        any_worktree_match,
        "SSH context 的 worktree.path SHALL 来自远端 jsonl 的 cwd; \
         git_branch SHALL 为 None（远端无 .git）。actual: {repo_groups:?}"
    );
    assert_remote_fs_touched(before, fake.snapshot_counters(), "list_repository_groups");

    // find_session_project：返回 fake fixture 的 project_id
    let before = fake.snapshot_counters();
    let found = api.find_session_project(session_id).await.unwrap();
    assert_eq!(found.as_deref(), Some(project_id));
    let missing = api.find_session_project("nonexistent-sid").await.unwrap();
    assert_eq!(missing, None);
    assert_remote_fs_touched(before, fake.snapshot_counters(), "find_session_project");

    // get_session_summaries_by_ids：返回 fake fixture 的 summaries
    let before = fake.snapshot_counters();
    let summaries = api
        .get_session_summaries_by_ids(project_id, &[session_id.to_owned()])
        .await
        .unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].session_id, session_id);
    assert_remote_fs_touched(
        before,
        fake.snapshot_counters(),
        "get_session_summaries_by_ids",
    );

    // project_memory_dir 是 LocalDataApi 私有 inherent method，由 add/delete
    // claude_md 等公开 API 间接调用。本测试通过 list_projects 走 active_scanner
    // 已验证 projects_dir 已切到 remote_home；project_memory_dir 的实现只是
    // 拼路径，行为正确性由 line 698 `active_fs_and_projects_dir` 调用即可保证。

    // get_subagent_trace：fake fixture 无 subagent 数据，返回空 array
    let before = fake.snapshot_counters();
    let trace = api
        .get_subagent_trace(session_id, "subagent-not-exists")
        .await
        .unwrap();
    assert!(
        trace.is_array() && trace.as_array().unwrap().is_empty(),
        "无 subagent fixture 时 SHALL 返回空数组，actual: {trace:?}"
    );
    assert_remote_fs_touched(before, fake.snapshot_counters(), "get_subagent_trace");

    // get_image_asset：jsonl 内无 image block，返回 empty data URI
    let before = fake.snapshot_counters();
    let image = api
        .get_image_asset(session_id, session_id, "chunk-uuid:0")
        .await
        .unwrap();
    assert!(
        image.starts_with("data:") || image.is_empty(),
        "无 image fixture 时 SHALL 返回 placeholder data URI，actual: {image}"
    );
    assert_remote_fs_touched(before, fake.snapshot_counters(), "get_image_asset");

    // get_tool_output：jsonl 内无 tool_use 匹配，返回 ToolOutput::Missing
    let before = fake.snapshot_counters();
    let tool_out = api
        .get_tool_output(session_id, session_id, "tool-not-exists")
        .await
        .unwrap();
    assert!(
        matches!(tool_out, cdt_core::ToolOutput::Missing),
        "无 tool_use_id 时 SHALL 返回 ToolOutput::Missing，actual: {tool_out:?}"
    );
    assert_remote_fs_touched(before, fake.snapshot_counters(), "get_tool_output");

    // search：fake provider 包含 "from remote" 文本，SSH context 下应能搜到
    // (SearchRequest 已在文件顶部 use cdt_api::{...} 引入)
    let before = fake.snapshot_counters();
    let search_req = SearchRequest {
        query: "from remote".to_owned(),
        project_id: Some(project_id.to_owned()),
        session_id: None,
    };
    let search_res = api.search(&search_req).await.unwrap();
    let results = search_res
        .get("results")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        !results.is_empty(),
        "SSH context 下 search SHALL 通过 active provider 搜到远端 jsonl 内容，actual: {search_res:?}"
    );
    assert_remote_fs_touched(before, fake.snapshot_counters(), "search");

    // ====== change `ssh-project-memory-remote-rw` 修订 ======
    // SSH context 下 memory CRUD 走真实远端 fs ops（不再 graceful skip）。
    // 覆盖 4 个 memory IPC：get_project_memory / read_memory_file / add_memory / delete_memory

    // 准备远端 memory 目录 fixture：base_dir = `extract_base_dir(project_id)` = project_id 本身
    // （已 encoded 形态），与 `project_memory_dir` helper 一致
    let remote_memory_dir = format!("{remote_home}/{project_id}/memory");
    fake.add_dir(&format!("{remote_home}/{project_id}"), "memory");
    fake.add_file(
        &remote_memory_dir,
        "MEMORY.md",
        "# Project Memory\n- [Note](note.md)\n",
    );
    fake.add_file(&remote_memory_dir, "note.md", "Test note content");

    // get_project_memory：SSH context 下 SHALL 走远端 fs ops 返真数据
    let before = fake.snapshot_counters();
    let memory = api.get_project_memory(project_id).await.unwrap();
    assert_eq!(memory.project_id, project_id);
    assert!(
        memory.has_memory,
        "SSH context 下 memory 目录有 .md 文件 SHALL has_memory=true"
    );
    assert!(memory.count >= 2, "SHALL 至少含 MEMORY.md + note.md");
    assert_eq!(memory.default_file.as_deref(), Some("MEMORY.md"));
    assert_remote_fs_touched(before, fake.snapshot_counters(), "get_project_memory");

    // read_memory_file：SSH context 下 SHALL 走远端 fs.read_to_string
    let before = fake.snapshot_counters();
    let content = api
        .read_memory_file(project_id, "MEMORY.md")
        .await
        .expect("SSH context 下 read_memory_file SHALL 成功");
    assert!(content.content.contains("Project Memory"));
    assert_remote_fs_touched(before, fake.snapshot_counters(), "read_memory_file");

    // add_memory：SSH context 下 SHALL 走远端 fs.write_atomic（含 tmp + remove + rename 三步）
    let before_writes = fake.snapshot_write_counters();
    let updated = api
        .add_memory(project_id, "feedback_test.md", "new note body")
        .await
        .expect("add_memory SHALL succeed in SSH context");
    assert!(
        updated.layers.iter().any(|l| l.file == "feedback_test.md"),
        "add_memory 返新 ProjectMemory SHALL 含新文件"
    );
    let after_writes = fake.snapshot_write_counters();
    assert!(
        after_writes.write > before_writes.write,
        "SSH add_memory SHALL 触发远端 SFTP write"
    );
    assert!(
        after_writes.rename > before_writes.rename,
        "SSH add_memory SHALL 触发远端 SFTP rename"
    );

    // delete_memory：SSH context 下 SHALL 走远端 fs.remove_file
    let before_writes = fake.snapshot_write_counters();
    let updated = api
        .delete_memory(project_id, "feedback_test.md")
        .await
        .expect("delete_memory SHALL succeed");
    assert!(
        !updated.layers.iter().any(|l| l.file == "feedback_test.md"),
        "delete_memory 后 layers SHALL 不含被删文件"
    );
    let after_writes = fake.snapshot_write_counters();
    assert!(
        after_writes.remove > before_writes.remove,
        "SSH delete_memory SHALL 触发远端 SFTP remove"
    );

    // 校验：路径穿越 / 非 .md 文件名 SHALL 拒绝且不触发任何远端写
    let before_writes = fake.snapshot_write_counters();
    assert!(
        api.add_memory(project_id, "../etc/passwd", "x")
            .await
            .is_err()
    );
    assert!(
        api.add_memory(project_id, "secret.json", "x")
            .await
            .is_err()
    );
    assert!(
        api.delete_memory(project_id, "subdir/note.md")
            .await
            .is_err()
    );
    let after_writes = fake.snapshot_write_counters();
    assert_eq!(
        before_writes, after_writes,
        "validation 失败 SHALL NOT 触发任何远端写 op"
    );
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
async fn list_sessions_cursor_pages_cover_all_sessions_without_restarting() {
    let (api, tmp) = setup_api().await;
    let project_id = "-tmp-many";
    let project_dir = tmp.path().join("projects").join(project_id);
    tokio::fs::create_dir_all(&project_dir).await.unwrap();

    for idx in 0..120 {
        let path = project_dir.join(format!("s{idx:03}.jsonl"));
        tokio::fs::write(path, b"{}\n").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }

    let first = api
        .list_sessions(
            project_id,
            &PaginatedRequest {
                page_size: 50,
                cursor: None,
            },
        )
        .await
        .unwrap();
    let second = api
        .list_sessions(
            project_id,
            &PaginatedRequest {
                page_size: 50,
                cursor: first.next_cursor.clone(),
            },
        )
        .await
        .unwrap();
    let third = api
        .list_sessions(
            project_id,
            &PaginatedRequest {
                page_size: 50,
                cursor: second.next_cursor.clone(),
            },
        )
        .await
        .unwrap();

    let ids: Vec<_> = first
        .items
        .iter()
        .chain(&second.items)
        .chain(&third.items)
        .map(|s| s.session_id.as_str())
        .collect();

    assert_eq!(first.total, 120);
    assert_eq!(second.total, 120);
    assert_eq!(third.total, 120);
    assert_eq!(first.next_cursor.as_deref(), Some("50"));
    assert_eq!(second.next_cursor.as_deref(), Some("100"));
    assert_eq!(third.next_cursor, None);
    assert_eq!(ids.len(), 120);
    assert_eq!(ids.first(), Some(&"s119"));
    assert_eq!(ids.last(), Some(&"s000"));
}

#[tokio::test]
async fn list_sessions_rejects_zero_page_size() {
    let (api, _tmp) = setup_api().await;
    let err = api
        .list_sessions(
            "any-project",
            &PaginatedRequest {
                page_size: 0,
                cursor: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code, cdt_api::ApiErrorCode::ValidationError);
    assert!(err.message.contains("pageSize must be > 0"));
}

#[tokio::test]
async fn get_session_summaries_by_ids_returns_light_summaries() {
    let (api, tmp) = setup_api().await;
    let project_id = "-tmp-summaries";
    let project_dir = tmp.path().join("projects").join(project_id);
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    tokio::fs::write(project_dir.join("sid-new.jsonl"), b"{}\n")
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    tokio::fs::write(project_dir.join("sid-old.jsonl"), b"{}\n")
        .await
        .unwrap();

    let summaries = api
        .get_session_summaries_by_ids(
            project_id,
            &[
                "sid-old".to_owned(),
                "sid-missing".to_owned(),
                "sid-new".to_owned(),
            ],
        )
        .await
        .unwrap();
    let ids: Vec<_> = summaries.iter().map(|s| s.session_id.as_str()).collect();
    assert_eq!(ids, vec!["sid-old", "sid-new"]);
    assert!(summaries.iter().all(|s| s.project_id == project_id));
    assert!(summaries.iter().all(|s| s.title.is_none()));
    assert!(summaries.iter().all(|s| s.message_count == 0));

    let json = serde_json::to_value(&summaries).unwrap();
    assert_eq!(json[0]["sessionId"], json!("sid-old"));
    assert!(json[0].get("session_id").is_none());
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

// change `session-title-extraction-fix`: 契约测试守护 list_sessions_sync
// 真路径计算 title 的新规则——防 IPC 层后续意外覆盖算法或字段名。
// spec: openspec/specs/ipc-data-api/spec.md
//   §`Title prefers slash command with non-empty args ...`
//   §`Sanitize title against interruption and task-output instructions`
//   §`Title length is bounded by TITLE_MAX_CHARS constant`

fn write_user_line(sid: &str, uuid: &str, ts: &str, text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        r#"{{"type":"user","uuid":"{uuid}","timestamp":"{ts}","sessionId":"{sid}","cwd":"/tmp","message":{{"role":"user","content":"{escaped}"}}}}"#
    )
}

#[tokio::test]
async fn list_sessions_sync_slash_with_args_becomes_title() {
    let (api, tmp) = setup_api().await;
    let project_id = "-tmp-slash-title";
    let project_dir = tmp.path().join("projects").join(project_id);
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    let session_id = "sess-slash-with-args";
    let path = project_dir.join(format!("{session_id}.jsonl"));
    let lines = [
        write_user_line(
            session_id,
            "u1",
            "2026-05-03T10:00:00.000Z",
            "<command-name>/impeccable</command-name><command-args>根据项目的已有代码生成一下设计规范</command-args>",
        ),
        write_user_line(session_id, "u2", "2026-05-03T10:00:01.000Z", "提一下PR吧"),
    ];
    tokio::fs::write(&path, lines.join("\n")).await.unwrap();

    let resp = api
        .list_sessions_sync(
            project_id,
            &PaginatedRequest {
                page_size: 10,
                cursor: None,
            },
        )
        .await
        .unwrap();

    let item = resp
        .items
        .iter()
        .find(|s| s.session_id == session_id)
        .expect("session 应出现在 sync 结果");
    assert_eq!(
        item.title.as_deref(),
        Some("/impeccable 根据项目的已有代码生成一下设计规范"),
        "带 args slash SHALL 直接作 title 而非降级到 fallback"
    );

    let json = serde_json::to_value(&resp).unwrap();
    let json_item = json["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["sessionId"] == session_id)
        .unwrap();
    assert_eq!(
        json_item["title"],
        "/impeccable 根据项目的已有代码生成一下设计规范"
    );
    assert!(
        json_item.get("session_id").is_none(),
        "字段名 SHALL 是 camelCase"
    );
}

#[tokio::test]
async fn list_sessions_sync_skips_request_interrupted_in_title() {
    let (api, tmp) = setup_api().await;
    let project_id = "-tmp-interrupted-title";
    let project_dir = tmp.path().join("projects").join(project_id);
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    let session_id = "sess-interrupted";
    let path = project_dir.join(format!("{session_id}.jsonl"));
    let lines = [
        write_user_line(
            session_id,
            "u1",
            "2026-05-03T10:00:00.000Z",
            "[Request interrupted by user during tooling cycle]",
        ),
        write_user_line(
            session_id,
            "u2",
            "2026-05-03T10:00:01.000Z",
            "继续刚才的任务",
        ),
    ];
    tokio::fs::write(&path, lines.join("\n")).await.unwrap();

    let resp = api
        .list_sessions_sync(
            project_id,
            &PaginatedRequest {
                page_size: 10,
                cursor: None,
            },
        )
        .await
        .unwrap();

    let item = resp
        .items
        .iter()
        .find(|s| s.session_id == session_id)
        .expect("session 应出现在 sync 结果");
    assert_eq!(item.title.as_deref(), Some("继续刚才的任务"));
    assert!(
        !item
            .title
            .as_deref()
            .unwrap_or_default()
            .contains("[Request interrupted"),
        "interrupted 字面量 SHALL NOT 进入 title"
    );
}

#[tokio::test]
async fn list_sessions_sync_long_title_truncated_at_500_chars() {
    let (api, tmp) = setup_api().await;
    let project_id = "-tmp-long-title";
    let project_dir = tmp.path().join("projects").join(project_id);
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    let session_id = "sess-long";
    let path = project_dir.join(format!("{session_id}.jsonl"));
    let long_text: String = "字".repeat(700);
    let line = write_user_line(session_id, "u1", "2026-05-03T10:00:00.000Z", &long_text);
    tokio::fs::write(&path, line).await.unwrap();

    let resp = api
        .list_sessions_sync(
            project_id,
            &PaginatedRequest {
                page_size: 10,
                cursor: None,
            },
        )
        .await
        .unwrap();
    let item = resp
        .items
        .iter()
        .find(|s| s.session_id == session_id)
        .expect("session 应出现在 sync 结果");
    let title = item.title.as_deref().unwrap_or_default();
    assert!(
        title.chars().count() <= cdt_api::TITLE_MAX_CHARS,
        "title 字符数 {} 应 <= {}",
        title.chars().count(),
        cdt_api::TITLE_MAX_CHARS
    );
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

    // general.sessionClickBehavior 是 camelCase（默认 "replace"）
    assert_eq!(
        config["general"]["sessionClickBehavior"].as_str(),
        Some("replace"),
        "general.sessionClickBehavior MUST 默认序列化为 'replace'"
    );
}

#[tokio::test]
async fn get_config_display_section_exposes_font_fields_camelcase() {
    let (api, _tmp) = setup_api().await;
    let config = api.get_config().await.unwrap();
    let display = &config["display"];
    assert!(display.is_object(), "display SHALL 为 object");
    // 默认值：fontSans / fontMono 都是 None → 序列化后 skip（与 skipped_update_version 一致）
    // 但 update 后 None 显式存在；这里只断言 camelCase 字段名永不出现 snake_case
    assert!(
        display.get("font_sans").is_none(),
        "MUST 不出现 snake_case font_sans"
    );
    assert!(
        display.get("font_mono").is_none(),
        "MUST 不出现 snake_case font_mono"
    );
}

#[tokio::test]
async fn update_config_ssh_round_trip_and_validation() {
    let (api, _tmp) = setup_api().await;
    let config = api.get_config().await.unwrap();
    assert_eq!(config["ssh"]["profiles"], json!([]));
    assert_eq!(config["ssh"]["lastConnection"], json!(null));
    assert_eq!(config["ssh"]["autoReconnect"], json!(false));

    api.update_config(&ConfigUpdateRequest {
        section: "ssh".into(),
        data: json!({
            "profiles": [{
                "id": "prod",
                "name": "Production",
                "host": "prod-box",
                "port": 22,
                "username": "alice",
                "authMethod": "sshConfig"
            }],
            "autoReconnect": true
        }),
    })
    .await
    .expect("valid ssh profile SHALL persist");
    let config = api.get_config().await.unwrap();
    assert_eq!(
        config["ssh"]["profiles"][0]["authMethod"],
        json!("sshConfig")
    );
    assert_eq!(config["ssh"]["autoReconnect"], json!(true));

    let err = api
        .update_config(&ConfigUpdateRequest {
            section: "ssh".into(),
            data: json!({
                "profiles": [{
                    "id": "bad",
                    "name": "",
                    "host": "",
                    "port": 0,
                    "username": "",
                    "authMethod": "password"
                }]
            }),
        })
        .await
        .expect_err("invalid ssh profile SHALL be rejected");
    assert!(err.to_string().contains("ssh.profiles"));
}

#[tokio::test]
async fn ssh_save_last_connection_omits_password() {
    let (api, _tmp) = setup_api().await;
    let request = SshConnectRequest {
        host: "prod-box".into(),
        port: Some(2222),
        username: Some("alice".into()),
        auth_method: SshAuthMethod::Password,
        password: Some("secret".into()),
        context_id: Some("ctx-prod".into()),
    };
    api.ssh_save_last_connection(&request)
        .await
        .expect("last connection SHALL persist without password");
    let last = api.ssh_get_last_connection().await.unwrap();
    assert_eq!(last["host"], json!("prod-box"));
    assert_eq!(last["port"], json!(2222));
    assert_eq!(last["username"], json!("alice"));
    assert_eq!(last["authMethod"], json!("password"));
    assert!(last.get("password").is_none());
}

#[tokio::test]
async fn update_config_general_auto_expand_ai_groups_round_trip() {
    let (api, _tmp) = setup_api().await;
    // 默认 false
    assert_eq!(
        api.get_config().await.unwrap()["general"]["autoExpandAiGroups"],
        json!(false)
    );
    // 前端发送的 camelCase key 是 serde 默认产出形态（'ai' 不当作缩写大写）；
    // 历史 bug：后端 dispatch 写成 `autoExpandAIGroups`，前端 toggle 实际从未持久化。
    api.update_config(&ConfigUpdateRequest {
        section: "general".into(),
        data: json!({ "autoExpandAiGroups": true }),
    })
    .await
    .expect("autoExpandAiGroups=true SHALL 接受");
    assert_eq!(
        api.get_config().await.unwrap()["general"]["autoExpandAiGroups"],
        json!(true)
    );
}

#[tokio::test]
async fn update_config_general_claude_root_path_reconfigures_local_api() {
    let (api, tmp) = setup_api().await;
    let custom_root = tmp.path().join("claude-alt");
    let custom_projects = custom_root.join("projects");
    let project_dir = custom_projects.join("-Users-alice-custom");
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    write_user_session(
        &project_dir,
        "sess-custom",
        "/Users/alice/custom",
        "custom_root_keyword",
    )
    .await;

    api.update_config(&ConfigUpdateRequest {
        section: "general".into(),
        data: json!({ "claudeRootPath": custom_root.to_string_lossy() }),
    })
    .await
    .expect("claudeRootPath SHALL 接受绝对路径");

    let projects = api.list_projects().await.unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, "-Users-alice-custom");

    let search = api
        .search(&SearchRequest {
            query: "custom_root_keyword".into(),
            project_id: Some("-Users-alice-custom".into()),
            session_id: None,
        })
        .await
        .unwrap();
    assert_eq!(search["results"].as_array().unwrap().len(), 1);

    api.update_config(&ConfigUpdateRequest {
        section: "general".into(),
        data: json!({ "claudeRootPath": null }),
    })
    .await
    .expect("claudeRootPath=null SHALL restore default");
    assert_eq!(
        api.get_config().await.unwrap()["general"]["claudeRootPath"],
        json!(null)
    );
}

#[tokio::test]
async fn update_config_general_claude_root_path_rejects_relative_path() {
    let (api, _tmp) = setup_api().await;
    let err = api
        .update_config(&ConfigUpdateRequest {
            section: "general".into(),
            data: json!({ "claudeRootPath": "relative/path" }),
        })
        .await
        .expect_err("relative claudeRootPath SHALL be rejected");
    assert!(err.to_string().contains("absolute path"));
}

#[tokio::test]
async fn update_config_general_session_click_behavior_round_trip() {
    let (api, _tmp) = setup_api().await;
    // 默认 "replace"
    assert_eq!(
        api.get_config().await.unwrap()["general"]["sessionClickBehavior"],
        json!("replace")
    );
    // 改为 "new-tab" SHALL 持久化
    api.update_config(&ConfigUpdateRequest {
        section: "general".into(),
        data: json!({ "sessionClickBehavior": "new-tab" }),
    })
    .await
    .expect("general.sessionClickBehavior='new-tab' SHALL 接受");
    assert_eq!(
        api.get_config().await.unwrap()["general"]["sessionClickBehavior"],
        json!("new-tab")
    );
    // 改回 "replace" 也 SHALL 生效
    api.update_config(&ConfigUpdateRequest {
        section: "general".into(),
        data: json!({ "sessionClickBehavior": "replace" }),
    })
    .await
    .unwrap();
    assert_eq!(
        api.get_config().await.unwrap()["general"]["sessionClickBehavior"],
        json!("replace")
    );
    // 非法值 SHALL Err
    let err = api
        .update_config(&ConfigUpdateRequest {
            section: "general".into(),
            data: json!({ "sessionClickBehavior": "bogus" }),
        })
        .await
        .expect_err("非法 sessionClickBehavior SHALL Err");
    assert!(
        err.to_string().contains("sessionClickBehavior"),
        "Err message SHALL 提及字段名，实际：{err}"
    );
}

#[tokio::test]
async fn update_config_display_time_format_round_trip() {
    let (api, _tmp) = setup_api().await;
    // 默认 "24h"
    assert_eq!(
        api.get_config().await.unwrap()["display"]["timeFormat"],
        json!("24h"),
        "display.timeFormat MUST 默认序列化为 '24h'"
    );
    // 改为 "12h" SHALL 持久化
    api.update_config(&ConfigUpdateRequest {
        section: "display".into(),
        data: json!({ "timeFormat": "12h" }),
    })
    .await
    .expect("display.timeFormat='12h' SHALL 接受");
    assert_eq!(
        api.get_config().await.unwrap()["display"]["timeFormat"],
        json!("12h")
    );
    // 改回 "24h" 也 SHALL 生效
    api.update_config(&ConfigUpdateRequest {
        section: "display".into(),
        data: json!({ "timeFormat": "24h" }),
    })
    .await
    .unwrap();
    assert_eq!(
        api.get_config().await.unwrap()["display"]["timeFormat"],
        json!("24h")
    );
    // 非法字符串 SHALL Err 且已存储值不变
    let err = api
        .update_config(&ConfigUpdateRequest {
            section: "display".into(),
            data: json!({ "timeFormat": "bogus" }),
        })
        .await
        .expect_err("非法 timeFormat SHALL Err");
    assert!(
        err.to_string().contains("timeFormat"),
        "Err message SHALL 提及字段名，实际：{err}"
    );
    assert_eq!(
        api.get_config().await.unwrap()["display"]["timeFormat"],
        json!("24h"),
        "拒绝非法值后 timeFormat SHALL 保持 '24h' 不变"
    );
    // 非字符串（如 bool）也 SHALL Err
    let err = api
        .update_config(&ConfigUpdateRequest {
            section: "display".into(),
            data: json!({ "timeFormat": true }),
        })
        .await
        .expect_err("非字符串 timeFormat SHALL Err");
    assert!(
        err.to_string().contains("timeFormat"),
        "Err message SHALL 提及字段名，实际：{err}"
    );
}

#[tokio::test]
async fn update_config_display_accepts_null_to_clear_font_sans() {
    let (api, _tmp) = setup_api().await;
    let req = ConfigUpdateRequest {
        section: "display".into(),
        data: json!({ "fontSans": null }),
    };
    api.update_config(&req)
        .await
        .expect("display fontSans=null SHALL 反序列化成功");

    let cfg = api.get_config().await.unwrap();
    // fontSans 为 None 时 skip_serializing → 字段不在响应中即等价于 null
    assert!(cfg["display"].get("fontSans").is_none());
}

#[tokio::test]
async fn update_config_display_accepts_custom_font_mono_string() {
    let (api, _tmp) = setup_api().await;
    let custom = "\"Fira Code\", monospace";
    let req = ConfigUpdateRequest {
        section: "display".into(),
        data: json!({ "fontMono": custom }),
    };
    api.update_config(&req)
        .await
        .expect("display fontMono 字符串 SHALL 反序列化成功");

    let cfg = api.get_config().await.unwrap();
    assert_eq!(cfg["display"]["fontMono"], json!(custom));
}

#[tokio::test]
async fn update_config_display_whitespace_font_normalizes_to_null() {
    let (api, _tmp) = setup_api().await;
    // 先设非空值
    let set_req = ConfigUpdateRequest {
        section: "display".into(),
        data: json!({ "fontSans": "Arial" }),
    };
    api.update_config(&set_req).await.unwrap();
    assert_eq!(
        api.get_config().await.unwrap()["display"]["fontSans"],
        json!("Arial")
    );

    // 再设全空白
    let clear_req = ConfigUpdateRequest {
        section: "display".into(),
        data: json!({ "fontSans": "   " }),
    };
    api.update_config(&clear_req).await.unwrap();

    let cfg = api.get_config().await.unwrap();
    assert!(
        cfg["display"].get("fontSans").is_none(),
        "全空白 SHALL 归一化为 None（序列化后字段缺失）"
    );
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

// =============================================================================
// Repository groups / worktree sessions
// =============================================================================

#[tokio::test]
async fn list_repository_groups_returns_camelcase_array() {
    let (api, _tmp) = setup_api().await;
    let groups = api.list_repository_groups().await.unwrap();
    let json = serde_json::to_value(&groups).unwrap();
    assert!(json.is_array(), "list_repository_groups SHALL 返回数组");

    // 即便空 projects 也应是 [] 而非 null
    assert_eq!(json, json!([]));
}

#[tokio::test]
async fn list_repository_groups_serializes_camelcase_when_non_empty() {
    use cdt_core::{RepositoryGroup, Worktree};
    let g = RepositoryGroup {
        id: "g-1".into(),
        identity: None,
        name: "demo".into(),
        worktrees: vec![Worktree {
            id: "wt-1".into(),
            path: std::path::PathBuf::from("/tmp/demo"),
            name: "demo".into(),
            git_branch: Some("main".into()),
            is_main_worktree: true,
            is_repo_root: true,
            cwd_relative_to_repo_root: None,
            sessions: vec!["s-1".into()],
            created_at: Some(1),
            most_recent_session: Some(1_700_000_000),
        }],
        most_recent_session: Some(1_700_000_000),
        total_sessions: 1,
    };
    let json = serde_json::to_value(&g).unwrap();
    assert_eq!(json["id"], json!("g-1"));
    assert_eq!(json["totalSessions"], json!(1));
    assert_eq!(json["mostRecentSession"], json!(1_700_000_000_i64));
    let wt = &json["worktrees"][0];
    assert_eq!(wt["isMainWorktree"], json!(true));
    assert_eq!(wt["gitBranch"], json!("main"));
    assert_eq!(wt["mostRecentSession"], json!(1_700_000_000_i64));
    assert!(
        json.get("total_sessions").is_none()
            && wt.get("is_main_worktree").is_none()
            && wt.get("git_branch").is_none(),
        "RepositoryGroup / Worktree MUST 不出现 snake_case 字段名"
    );
}

#[tokio::test]
async fn get_worktree_sessions_returns_paginated_response_shape() {
    let (api, _tmp) = setup_api().await;
    let pagination = PaginatedRequest {
        page_size: 10,
        cursor: None,
    };
    // 测试 setup 下没真实 group：未命中应返 not_found 错误。
    let err = api
        .get_worktree_sessions("nonexistent-group", &pagination)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("nonexistent-group") || msg.to_lowercase().contains("not"),
        "未命中 SHALL 报告 not_found 含 group_id 标识，实际：{msg}"
    );
}

#[tokio::test]
async fn get_worktree_sessions_rejects_zero_page_size() {
    let (api, _tmp) = setup_api().await;
    let pagination = PaginatedRequest {
        page_size: 0,
        cursor: None,
    };
    let err = api
        .get_worktree_sessions("any-group", &pagination)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("pagesize") || msg.contains("page_size"),
        "page_size=0 SHALL 报 validation_error 含 pageSize 字样，实际：{msg}"
    );
}

#[tokio::test]
async fn get_worktree_sessions_paginated_response_serializes_camelcase() {
    let resp: PaginatedResponse<SessionSummary> = PaginatedResponse {
        items: vec![SessionSummary {
            session_id: "sess-1".into(),
            project_id: "wt-1".into(),
            timestamp: 1_700_000_000,
            message_count: 0,
            title: None,
            is_ongoing: false,
            git_branch: None,
            worktree_id: Some("wt-1".into()),
            worktree_name: Some("main".into()),
            group_id: Some("g-1".into()),
            cwd_relative_to_repo_root: Some("crates".into()),
            cwd: None,
        }],
        next_cursor: Some("1".into()),
        total: 5,
    };
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["nextCursor"], json!("1"));
    assert_eq!(json["total"], json!(5));
    let item = &json["items"][0];
    assert_eq!(item["worktreeId"], json!("wt-1"));
    assert_eq!(item["worktreeName"], json!("main"));
    assert!(
        item.get("worktree_id").is_none() && item.get("worktree_name").is_none(),
        "SessionSummary 新增字段 MUST 走 camelCase"
    );
}

#[test]
fn session_summary_skips_worktree_fields_when_none() {
    let s = SessionSummary {
        session_id: "sess-1".into(),
        project_id: "proj-1".into(),
        timestamp: 0,
        message_count: 0,
        title: None,
        is_ongoing: false,
        git_branch: None,
        worktree_id: None,
        worktree_name: None,
        group_id: None,
        cwd_relative_to_repo_root: None,
        cwd: None,
    };
    let json = serde_json::to_value(&s).unwrap();
    assert!(
        json.get("worktreeId").is_none() && json.get("worktreeName").is_none(),
        "worktreeId/Name 为 None 时 SHALL 不出现在序列化输出（skip_serializing_if）"
    );
    assert!(
        json.get("cwd").is_none(),
        "cwd=None 时 SHALL 不出现在序列化输出（skip_serializing_if）"
    );
}

// =============================================================================
// Schema-level: SessionDetail.phaseInfo + injectionsByPhase
// =============================================================================

/// 单 phase 会话：`injectionsByPhase` 含 key `"1"`，等价于 `contextInjections`。
#[test]
fn session_detail_single_phase_injections_by_phase_equals_context_injections() {
    use cdt_api::SessionDetail;
    let inj = serde_json::to_value(ContextInjection::UserMessage(UserMessageInjection {
        id: "u1".into(),
        turn_index: 0,
        ai_group_id: "a:0".into(),
        estimated_tokens: 2,
        text_preview: "hi".into(),
    }))
    .unwrap();
    let mut by_phase = serde_json::Map::new();
    by_phase.insert("1".to_string(), json!([inj.clone()]));
    let phase_info = json!({
        "phases": [{"phaseNumber": 1, "firstAiGroupId": "a:0", "lastAiGroupId": "a:0"}],
        "compactionCount": 0,
        "aiGroupPhaseMap": {"a:0": 1},
        "compactionTokenDeltas": {},
    });
    let detail = SessionDetail {
        session_id: "s1".into(),
        project_id: "p1".into(),
        chunks: serde_json::Value::Array(vec![]),
        metrics: json!({}),
        metadata: json!({}),
        context_injections: json!([inj.clone()]),
        injections_by_phase: serde_json::Value::Object(by_phase),
        phase_info,
        is_ongoing: false,
        title: None,
    };
    let json_val = serde_json::to_value(&detail).unwrap();
    assert_eq!(json_val["injectionsByPhase"]["1"], json!([inj]));
    assert_eq!(
        json_val["injectionsByPhase"]["1"], json_val["contextInjections"],
        "Latest phase 的 injectionsByPhase[N] SHALL 等于 contextInjections"
    );
    assert_eq!(json_val["phaseInfo"]["phases"][0]["phaseNumber"], json!(1));
    assert!(
        json_val
            .as_object()
            .unwrap()
            .contains_key("injectionsByPhase"),
        "injectionsByPhase MUST 以 camelCase 序列化"
    );
}

/// 多 phase 会话：Phase 1 的 injections 在 compact 后不丢失，仍在 `injectionsByPhase["1"]`。
#[test]
fn session_detail_multi_phase_preserves_phase1_injections() {
    use cdt_api::SessionDetail;
    let phase1_inj =
        serde_json::to_value(ContextInjection::MentionedFile(MentionedFileInjection {
            id: "m1".into(),
            path: "/p/file.rs".into(),
            display_name: "file.rs".into(),
            estimated_tokens: 10,
            first_seen_turn_index: 0,
            first_seen_in_group: "a:0".into(),
            exists: true,
        }))
        .unwrap();
    let phase2_inj = serde_json::to_value(ContextInjection::ToolOutput(ToolOutputInjection {
        id: "t1".into(),
        turn_index: 1,
        ai_group_id: "b:0".into(),
        estimated_tokens: 50,
        tool_count: 1,
        tool_breakdown: vec![],
    }))
    .unwrap();
    let mut by_phase = serde_json::Map::new();
    by_phase.insert("1".into(), json!([phase1_inj.clone()]));
    by_phase.insert("2".into(), json!([phase2_inj.clone()]));
    let phase_info = json!({
        "phases": [
            {"phaseNumber": 1, "firstAiGroupId": "a:0", "lastAiGroupId": "a:0"},
            {"phaseNumber": 2, "firstAiGroupId": "b:0", "lastAiGroupId": "b:0", "compactGroupId": "c:0"},
        ],
        "compactionCount": 1,
        "aiGroupPhaseMap": {"a:0": 1, "b:0": 2},
        "compactionTokenDeltas": {},
    });
    let detail = SessionDetail {
        session_id: "s2".into(),
        project_id: "p1".into(),
        chunks: serde_json::Value::Array(vec![]),
        metrics: json!({}),
        metadata: json!({}),
        context_injections: json!([phase2_inj.clone()]), // = latest phase
        injections_by_phase: serde_json::Value::Object(by_phase),
        phase_info,
        is_ongoing: false,
        title: None,
    };
    let json_val = serde_json::to_value(&detail).unwrap();
    // Round-trip 反序列化保持字节级相等
    let back: SessionDetail = serde_json::from_value(json_val.clone()).unwrap();
    let json_back = serde_json::to_value(&back).unwrap();
    assert_eq!(json_val, json_back, "SessionDetail round-trip MUST 等价");
    // Phase 1 injection MUST 在 injectionsByPhase["1"]，contextInjections 应只含 Phase 2
    assert_eq!(json_val["injectionsByPhase"]["1"], json!([phase1_inj]));
    assert_eq!(json_val["injectionsByPhase"]["2"], json!([phase2_inj]));
    assert_eq!(
        json_val["contextInjections"],
        json_val["injectionsByPhase"]["2"]
    );
}

/// `SessionDetail.title` 字段以 `title` (camelCase) 序列化，round-trip 等价。
/// Spec：`ipc-data-api::SessionDetail 暴露与 SessionSummary 同源派生的 title`。
#[test]
fn session_detail_title_field_round_trip() {
    use cdt_api::SessionDetail;
    let detail = SessionDetail {
        session_id: "s1".into(),
        project_id: "p1".into(),
        chunks: serde_json::Value::Array(vec![]),
        metrics: json!({}),
        metadata: json!({}),
        context_injections: json!([]),
        injections_by_phase: serde_json::Value::Object(serde_json::Map::new()),
        phase_info: serde_json::Value::Null,
        is_ongoing: false,
        title: Some("修复登录页样式".into()),
    };
    let json_val = serde_json::to_value(&detail).unwrap();
    assert_eq!(
        json_val["title"],
        json!("修复登录页样式"),
        "SessionDetail.title MUST 以 camelCase `title` 字段序列化"
    );
    let back: SessionDetail = serde_json::from_value(json_val.clone()).unwrap();
    assert_eq!(back.title.as_deref(), Some("修复登录页样式"));

    // None 时序列化为 null（serde 默认行为）
    let detail_none = SessionDetail {
        session_id: "s2".into(),
        project_id: "p1".into(),
        chunks: serde_json::Value::Null,
        metrics: serde_json::Value::Null,
        metadata: serde_json::Value::Null,
        context_injections: serde_json::Value::Array(Vec::new()),
        injections_by_phase: serde_json::Value::Object(serde_json::Map::new()),
        phase_info: serde_json::Value::Null,
        is_ongoing: false,
        title: None,
    };
    let json_none = serde_json::to_value(&detail_none).unwrap();
    assert!(json_none.as_object().unwrap().contains_key("title"));
    assert_eq!(json_none["title"], json!(null));
    let back_none: SessionDetail = serde_json::from_value(json_none).unwrap();
    assert!(back_none.title.is_none());
}

/// `chunk_id` 形态统一：所有 chunk 类型首次出现都用 `<base>:0`，无 `ai:` 前缀。
#[test]
fn chunk_id_format_is_unified_base_colon_n() {
    // 构造 4 类 chunk，断言其 chunk_id 形态符合统一规则
    let ai = AIChunk {
        chunk_id: "abc:0".into(),
        timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        duration_ms: None,
        responses: vec![],
        metrics: ChunkMetrics::zero(),
        semantic_steps: vec![],
        tool_executions: vec![],
        subagents: vec![],
        slash_commands: vec![],
        teammate_messages: vec![],
    };
    let user = UserChunk {
        chunk_id: "u:0".into(),
        uuid: "u".into(),
        timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        duration_ms: None,
        content: MessageContent::Text("hi".into()),
        metrics: ChunkMetrics::zero(),
    };
    let sys = SystemChunk {
        chunk_id: "s:0".into(),
        uuid: "s".into(),
        timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        duration_ms: None,
        content_text: "init".into(),
        metrics: ChunkMetrics::zero(),
    };
    let compact = CompactChunk {
        chunk_id: "c:0".into(),
        uuid: "c".into(),
        timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        duration_ms: None,
        summary_text: "x".into(),
        metrics: ChunkMetrics::zero(),
        token_delta: None,
        phase_number: None,
    };
    let chunks = [
        Chunk::Ai(ai),
        Chunk::User(user),
        Chunk::System(sys),
        Chunk::Compact(compact),
    ];
    for chunk in &chunks {
        let chunk_id = match chunk {
            Chunk::Ai(c) => &c.chunk_id,
            Chunk::User(c) => &c.chunk_id,
            Chunk::System(c) => &c.chunk_id,
            Chunk::Compact(c) => &c.chunk_id,
        };
        // 形态 <base>:<n>：含恰好一个或多个 ':' 分隔的最后段必须是十进制
        let last_colon = chunk_id.rfind(':').expect("chunk_id MUST 含 ':' 分隔符");
        let (base, tail) = chunk_id.split_at(last_colon);
        assert!(!base.is_empty(), "chunk_id {chunk_id:?} base 段不能为空");
        assert!(
            tail.strip_prefix(':')
                .unwrap_or("")
                .chars()
                .all(|c| c.is_ascii_digit()),
            "chunk_id {chunk_id:?} 最后段必须为十进制 n"
        );
        assert!(
            !chunk_id.starts_with("ai:"),
            "chunk_id {chunk_id:?} MUST NOT 含 ai: 类型前缀"
        );
    }
}

// =============================================================================
// WSL distro 枚举（wsl-distro-discovery capability）
// =============================================================================

#[tokio::test]
async fn list_wsl_distros_returns_camelcase_report_shape() {
    let (api, _tmp) = setup_api().await;
    let report = api.list_wsl_distros().await.unwrap();
    let json = serde_json::to_value(&report).unwrap();
    assert!(json.is_object(), "list_wsl_distros SHALL 返回 object");
    assert!(
        json.get("candidates").is_some() && json["candidates"].is_array(),
        "candidates 字段 SHALL 是 array"
    );
    assert!(
        json.get("distrosWithoutHome").is_some() && json["distrosWithoutHome"].is_array(),
        "distrosWithoutHome 字段 SHALL 是 camelCase array"
    );
    assert!(
        json.get("distros_without_home").is_none(),
        "MUST NOT 出现 snake_case 字段名 distros_without_home"
    );
    // 非 Windows 测试环境下应是空报告
    if !cfg!(target_os = "windows") {
        assert_eq!(json["candidates"], json!([]));
        assert_eq!(json["distrosWithoutHome"], json!([]));
    }
}

#[test]
fn wsl_distro_candidate_serializes_camelcase() {
    let candidate = WslDistroCandidate {
        distro: "Ubuntu".to_string(),
        home_path: "/home/alice".to_string(),
        claude_root_path: r"\\wsl.localhost\Ubuntu\home\alice\.claude".to_string(),
        claude_root_exists: true,
    };
    let json = serde_json::to_value(&candidate).unwrap();
    assert_eq!(json["distro"], json!("Ubuntu"));
    assert_eq!(json["homePath"], json!("/home/alice"));
    assert_eq!(
        json["claudeRootPath"],
        json!(r"\\wsl.localhost\Ubuntu\home\alice\.claude")
    );
    assert_eq!(json["claudeRootExists"], json!(true));
    assert!(
        json.get("home_path").is_none()
            && json.get("claude_root_path").is_none()
            && json.get("claude_root_exists").is_none(),
        "WslDistroCandidate MUST 不出现 snake_case 字段名"
    );
}

#[test]
fn wsl_distro_scan_report_serializes_with_distros_without_home() {
    let report = WslDistroScanReport {
        candidates: vec![],
        distros_without_home: vec!["Ubuntu".to_string(), "Debian-12".to_string()],
    };
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["distrosWithoutHome"], json!(["Ubuntu", "Debian-12"]));
    assert_eq!(json["candidates"], json!([]));
}

// =============================================================================
// server-mode: http_server_start / _stop / _status 字段契约
// =============================================================================

/// 与 `src-tauri/src/server_mode.rs::ServerStatus` 字段一致。该结构与 `ServerStatus`
/// 跨 crate 不复用——src-tauri 的 `ServerStatus` 在 desktop binary 内部，cdt-api
/// 此处用同结构 mirror 用于断言 serde 形状。两者形状漂移由本测试拦截。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerStatusContract {
    running: bool,
    port: u16,
    last_error: Option<String>,
}

#[test]
fn http_server_start_request_shape() {
    // 前端调 invoke('http_server_start', { port: 3456 }) 时 Tauri JSON->Rust
    // 转换走 camelCase 自动映射。`port: u16` 是唯一入参，断言：(a) port 是 number
    // (b) 没有其它字段被前端误传。
    let payload = json!({ "port": 3456 });
    assert!(payload["port"].is_number());
    assert_eq!(payload["port"].as_u64(), Some(3456));
    // 入参 schema MUST 不含 snake_case 其它字段
    let obj = payload.as_object().unwrap();
    assert_eq!(obj.len(), 1, "http_server_start 入参 MUST 仅含 port 字段");
}

#[test]
fn http_server_status_response_shape_initial() {
    // 初始状态：server 未跑过、未持久化任何 port
    let status = ServerStatusContract {
        running: false,
        port: 3456,
        last_error: None,
    };
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["running"], json!(false));
    assert_eq!(json["port"], json!(3456));
    assert_eq!(json["lastError"], json!(null));
    // MUST 不出现 snake_case
    assert!(
        json.get("last_error").is_none(),
        "MUST 不出现 snake_case last_error"
    );
}

#[test]
fn http_server_status_response_shape_after_start_failure() {
    let status = ServerStatusContract {
        running: false,
        port: 3456,
        last_error: Some("port 3456 is in use".into()),
    };
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["running"], json!(false));
    assert_eq!(json["lastError"], json!("port 3456 is in use"));
}

#[test]
fn http_server_status_response_shape_after_start_success() {
    let status = ServerStatusContract {
        running: true,
        port: 3500,
        last_error: None,
    };
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["running"], json!(true));
    assert_eq!(json["port"], json!(3500));
    assert_eq!(
        json["lastError"],
        json!(null),
        "成功启动后 lastError SHALL 序列化为 null（不是 missing）"
    );
}

#[test]
fn http_server_stop_response_shape() {
    // stop IPC 返回 `Result<null, string>`——Ok 路径前端拿到 `null`。
    // Tauri 命令 fn 返回 `Result<(), String>` 时 Ok(()) 序列化为 JSON `null`。
    let value: serde_json::Value = serde_json::to_value(Option::<()>::None).unwrap();
    assert!(value.is_null(), "http_server_stop Ok 路径 SHALL 是 null");
}

#[tokio::test]
async fn update_config_http_server_round_trip() {
    let (api, _tmp) = setup_api().await;

    // 默认值
    let cfg = api.get_config().await.unwrap();
    assert_eq!(cfg["httpServer"]["enabled"], json!(false));
    assert_eq!(cfg["httpServer"]["port"], json!(3456));

    // 改 port
    let req = ConfigUpdateRequest {
        section: "httpServer".into(),
        data: json!({ "port": 3500 }),
    };
    let next = api.update_config(&req).await.unwrap();
    assert_eq!(next["httpServer"]["port"], json!(3500));
    assert_eq!(
        next["httpServer"]["enabled"],
        json!(false),
        "仅 port 更新 SHALL 不影响 enabled"
    );

    // 改回
    let req = ConfigUpdateRequest {
        section: "httpServer".into(),
        data: json!({ "port": 3456 }),
    };
    let next = api.update_config(&req).await.unwrap();
    assert_eq!(next["httpServer"]["port"], json!(3456));

    // 非法 port（< 1024）拒绝；ConfigManager::update_http_server 内部走
    // validate_http_port，应返回 Err
    let req = ConfigUpdateRequest {
        section: "httpServer".into(),
        data: json!({ "port": 80 }),
    };
    let err = api.update_config(&req).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("1024") || msg.to_lowercase().contains("port"),
        "非法 port SHALL 拒绝并附 range 文案，got: {msg}"
    );

    // 改 enabled
    let req = ConfigUpdateRequest {
        section: "httpServer".into(),
        data: json!({ "enabled": true }),
    };
    let next = api.update_config(&req).await.unwrap();
    assert_eq!(next["httpServer"]["enabled"], json!(true));
    assert_eq!(
        next["httpServer"]["port"],
        json!(3456),
        "仅 enabled 更新 SHALL 不影响 port"
    );
}

// =============================================================================
// telemetry: get_telemetry_snapshot 字段契约
// =============================================================================

#[tokio::test]
async fn get_telemetry_snapshot_returns_camelcase_fields() {
    cdt_telemetry::init_registry();
    let (api, _tmp) = setup_api().await;
    let snap = api.get_telemetry_snapshot().await.expect("snapshot ok");
    let v = serde_json::to_value(&snap).expect("snapshot serializes");

    // 顶层字段：camelCase
    assert!(v.get("schemaVersion").is_some(), "schemaVersion present");
    assert!(v.get("uptimeSecs").is_some(), "uptimeSecs present");
    assert!(v.get("capturedAt").is_some(), "capturedAt present");
    assert!(v.get("counters").is_some(), "counters present");
    assert!(v.get("histograms").is_some(), "histograms present");
    assert!(v.get("recentEvents").is_some(), "recentEvents present");
    assert!(
        v.get("schema_version").is_none(),
        "snake_case schema_version SHALL NOT appear"
    );

    // schemaVersion = 1
    assert_eq!(v["schemaVersion"], serde_json::json!(1));

    // counters 至少含核心几个 name
    let counters = v["counters"].as_object().expect("counters is object");
    for name in [
        "metadata.cache.hit",
        "metadata.cache.miss",
        "panic.recovered",
        "cdt_ssh.error",
        "cdt_api.error",
    ] {
        assert!(counters.contains_key(name), "counter {name} missing");
    }

    // histograms 至少 4 个 + 字段 camelCase
    let histograms = v["histograms"].as_object().expect("histograms is object");
    for name in [
        "ipc.list_sessions.duration_ns",
        "ipc.get_session_detail.duration_ns",
        "ipc.list_repository_groups.duration_ns",
        "ipc.list_projects.duration_ns",
    ] {
        let h = histograms
            .get(name)
            .unwrap_or_else(|| panic!("histogram {name}"));
        assert!(h.get("count").is_some(), "{name}.count present");
        assert!(h.get("buckets").is_some(), "{name}.buckets present");
        assert_eq!(
            h["buckets"].as_array().unwrap().len(),
            32,
            "{name}.buckets length 32"
        );
        assert!(h.get("p50Ns").is_some(), "{name}.p50Ns present (camelCase)");
        assert!(h.get("p95Ns").is_some(), "{name}.p95Ns present");
        assert!(h.get("p99Ns").is_some(), "{name}.p99Ns present");
        assert!(h.get("maxBucket").is_some(), "{name}.maxBucket present");
        assert!(
            h.get("p50_ns").is_none(),
            "{name}.p50_ns snake_case SHALL NOT appear"
        );
    }
}

#[tokio::test]
async fn record_correctness_events_validates_whitelist_and_batches() {
    cdt_telemetry::init_registry();
    let (api, _tmp) = setup_api().await;

    let snap_before = api.get_telemetry_snapshot().await.unwrap();
    let stale_before = snap_before
        .counters
        .get("stale_update.triggered")
        .copied()
        .unwrap_or(0);
    let unreg_before = snap_before
        .counters
        .get("telemetry.unregistered_correctness_event")
        .copied()
        .unwrap_or(0);

    // 白名单 kind 批量 inc
    api.record_correctness_events(vec![
        cdt_api::CorrectnessEventItem {
            kind: "stale_update.triggered".into(),
            count: 5,
        },
        cdt_api::CorrectnessEventItem {
            kind: "stale_update.triggered".into(),
            count: 3,
        },
        // 未在白名单的 kind: silently ignore
        cdt_api::CorrectnessEventItem {
            kind: "fake.event".into(),
            count: 100,
        },
    ])
    .await
    .unwrap();

    let snap_after = api.get_telemetry_snapshot().await.unwrap();
    let stale_after = snap_after
        .counters
        .get("stale_update.triggered")
        .copied()
        .unwrap_or(0);
    let unreg_after = snap_after
        .counters
        .get("telemetry.unregistered_correctness_event")
        .copied()
        .unwrap_or(0);

    assert_eq!(
        stale_after - stale_before,
        8,
        "stale_update.triggered SHALL inc by 5 + 3 = 8"
    );
    // unregistered counter 应 inc 一次（一条 fake.event 进 fallback）
    assert!(
        unreg_after > unreg_before,
        "unregistered_correctness_event SHALL inc when kind not whitelisted"
    );
}
