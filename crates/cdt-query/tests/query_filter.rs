//! Unit tests for `QueryFilter` and `SessionQueryOptions`.

use cdt_api::SessionListFilter;
use cdt_query::{ChunkKindFilter, QueryFilter, SessionQueryOptions};

#[test]
fn query_filter_to_session_list_filter() {
    let qf = QueryFilter {
        since: Some(1000),
        until: Some(5000),
        grep: Some("auth".to_owned()),
        branch: Some("feat/x".to_owned()),
        limit: Some(50),
    };
    let f: SessionListFilter = qf.to_session_list_filter();
    assert_eq!(f.since, Some(1000));
    assert_eq!(f.until, Some(5000));
    assert_eq!(f.grep.as_deref(), Some("auth"));
    assert_eq!(f.branch.as_deref(), Some("feat/x"));
    assert_eq!(f.limit, Some(50));
}

#[test]
fn query_filter_default_produces_empty_filter() {
    let qf = QueryFilter::default();
    let f = qf.to_session_list_filter();
    assert!(f.since.is_none());
    assert!(f.until.is_none());
    assert!(f.grep.is_none());
    assert!(f.branch.is_none());
    assert!(f.limit.is_none());
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
            workflow_run_id: None,
            workflow_script_path: None,
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
            workflow_run_id: None,
            workflow_script_path: None,
        }],
        subagents: Vec::new(),
        slash_commands: Vec::new(),
        teammate_messages: Vec::new(),
    });

    vec![user, ai_no_error, ai_with_error]
}
