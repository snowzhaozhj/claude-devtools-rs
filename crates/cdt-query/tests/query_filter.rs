//! Unit tests for `QueryFilter` and `SessionQueryOptions`.

use cdt_api::SessionSummary;
use cdt_query::{ChunkKindFilter, QueryFilter, SessionQueryOptions};

fn make_session(id: &str, title: Option<&str>, ts: i64, msg_count: usize) -> SessionSummary {
    SessionSummary {
        session_id: id.to_owned(),
        project_id: "proj-1".to_owned(),
        timestamp: ts,
        message_count: msg_count,
        title: title.map(ToOwned::to_owned),
        is_ongoing: false,
        git_branch: None,
        worktree_id: None,
        worktree_name: None,
        group_id: None,
        cwd_relative_to_repo_root: None,
        cwd: None,
    }
}

#[test]
fn filter_since() {
    let sessions = vec![
        make_session("a", Some("old"), 1000, 5),
        make_session("b", Some("new"), 3000, 10),
    ];
    let filter = QueryFilter {
        since: Some(2000),
        ..Default::default()
    };
    let result = filter.apply(sessions);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].session_id, "b");
}

#[test]
fn filter_until() {
    let sessions = vec![
        make_session("a", Some("old"), 1000, 5),
        make_session("b", Some("new"), 3000, 10),
    ];
    let filter = QueryFilter {
        until: Some(2000),
        ..Default::default()
    };
    let result = filter.apply(sessions);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].session_id, "a");
}

#[test]
fn filter_grep_case_insensitive() {
    let sessions = vec![
        make_session("a", Some("Fix Authentication Bug"), 1000, 5),
        make_session("b", Some("Add new feature"), 2000, 10),
        make_session("c", None, 3000, 3),
    ];
    let filter = QueryFilter {
        grep: Some("auth".to_owned()),
        ..Default::default()
    };
    let result = filter.apply(sessions);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].session_id, "a");
}

#[test]
fn filter_min_messages() {
    let sessions = vec![
        make_session("a", Some("small"), 1000, 3),
        make_session("b", Some("large"), 2000, 50),
    ];
    let filter = QueryFilter {
        min_messages: Some(10),
        ..Default::default()
    };
    let result = filter.apply(sessions);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].session_id, "b");
}

#[test]
fn filter_limit() {
    let sessions = vec![
        make_session("a", Some("s1"), 1000, 5),
        make_session("b", Some("s2"), 2000, 5),
        make_session("c", Some("s3"), 3000, 5),
    ];
    let filter = QueryFilter {
        limit: Some(2),
        ..Default::default()
    };
    let result = filter.apply(sessions);
    assert_eq!(result.len(), 2);
}

#[test]
fn filter_combined() {
    let sessions = vec![
        make_session("a", Some("fix bug"), 500, 3),
        make_session("b", Some("fix auth"), 2000, 20),
        make_session("c", Some("fix login"), 3000, 50),
    ];
    let filter = QueryFilter {
        since: Some(1000),
        grep: Some("fix".to_owned()),
        min_messages: Some(10),
        limit: Some(1),
        ..Default::default()
    };
    let result = filter.apply(sessions);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].session_id, "b");
}

#[test]
fn options_tail() {
    let chunks = make_dummy_chunks(10);
    let opts = SessionQueryOptions::last_n(3);
    let result = opts.apply(chunks);
    assert_eq!(result.len(), 3);
}

#[test]
fn options_range() {
    let chunks = make_dummy_chunks(10);
    let opts = SessionQueryOptions::with_range(2, 5);
    let result = opts.apply(chunks);
    assert_eq!(result.len(), 3);
}

#[test]
fn options_full() {
    let chunks = make_dummy_chunks(10);
    let opts = SessionQueryOptions::full();
    let result = opts.apply(chunks);
    assert_eq!(result.len(), 10);
}

#[test]
fn options_errors_only_filter() {
    let chunks = make_mixed_chunks();
    let opts = SessionQueryOptions {
        kind_filter: Some(ChunkKindFilter::ErrorsOnly),
        ..Default::default()
    };
    let result = opts.apply(chunks);
    assert_eq!(result.len(), 1);
}

#[test]
fn options_tool_calls_filter() {
    let chunks = make_mixed_chunks();
    let opts = SessionQueryOptions {
        kind_filter: Some(ChunkKindFilter::ToolCalls),
        ..Default::default()
    };
    let result = opts.apply(chunks);
    assert_eq!(result.len(), 2);
}

fn make_dummy_chunks(n: usize) -> Vec<cdt_core::Chunk> {
    use chrono::Utc;
    (0..n)
        .map(|i| {
            cdt_core::Chunk::User(cdt_core::UserChunk {
                chunk_id: format!("chunk-{i}"),
                uuid: format!("uuid-{i}"),
                timestamp: Utc::now(),
                duration_ms: None,
                content: cdt_core::MessageContent::Text(format!("message {i}")),
                metrics: cdt_core::ChunkMetrics::default(),
            })
        })
        .collect()
}

fn make_mixed_chunks() -> Vec<cdt_core::Chunk> {
    use chrono::Utc;

    let user = cdt_core::Chunk::User(cdt_core::UserChunk {
        chunk_id: "u1".into(),
        uuid: "u1".into(),
        timestamp: Utc::now(),
        duration_ms: None,
        content: cdt_core::MessageContent::Text("hello".into()),
        metrics: cdt_core::ChunkMetrics::default(),
    });

    let ai_no_error = cdt_core::Chunk::Ai(cdt_core::AIChunk {
        chunk_id: "ai1".into(),
        timestamp: Utc::now(),
        duration_ms: None,
        responses: Vec::new(),
        metrics: cdt_core::ChunkMetrics::default(),
        semantic_steps: Vec::new(),
        tool_executions: vec![cdt_core::ToolExecution {
            tool_use_id: "t1".into(),
            tool_name: "Bash".into(),
            input: serde_json::json!({}),
            output: cdt_core::ToolOutput::Missing,
            is_error: false,
            start_ts: Utc::now(),
            end_ts: None,
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
            error_message: None,
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
        }],
        subagents: Vec::new(),
        slash_commands: Vec::new(),
        teammate_messages: Vec::new(),
    });

    let ai_with_error = cdt_core::Chunk::Ai(cdt_core::AIChunk {
        chunk_id: "ai2".into(),
        timestamp: Utc::now(),
        duration_ms: None,
        responses: Vec::new(),
        metrics: cdt_core::ChunkMetrics::default(),
        semantic_steps: Vec::new(),
        tool_executions: vec![cdt_core::ToolExecution {
            tool_use_id: "t2".into(),
            tool_name: "Read".into(),
            input: serde_json::json!({}),
            output: cdt_core::ToolOutput::Missing,
            is_error: true,
            start_ts: Utc::now(),
            end_ts: None,
            source_assistant_uuid: "a2".into(),
            result_agent_id: None,
            error_message: Some("file not found".into()),
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
        }],
        subagents: Vec::new(),
        slash_commands: Vec::new(),
        teammate_messages: Vec::new(),
    });

    vec![user, ai_no_error, ai_with_error]
}
