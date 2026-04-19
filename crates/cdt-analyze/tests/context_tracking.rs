//! context-tracking capability 的端到端集成测试。
//!
//! 对齐 `openspec/specs/context-tracking/spec.md` 与
//! `openspec/changes/port-context-tracking/specs/context-tracking/spec.md`
//! 中的全部 Requirement × Scenario。

use std::collections::HashMap;
use std::path::Path;

use cdt_analyze::context::TokenDictionaries;
use cdt_analyze::{ProcessSessionParams, process_session_context_with_phases};
use cdt_core::{
    AIChunk, AssistantResponse, Chunk, ChunkMetrics, ClaudeMdContextInjection, ClaudeMdFileInfo,
    ClaudeMdScope, CompactChunk, ContextInjection, MentionedFileInfo, MessageContent, TokenUsage,
    ToolExecution, ToolOutput, UserChunk,
};
use chrono::{DateTime, Duration, Utc};

fn ts(offset_seconds: i64) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
        + Duration::seconds(offset_seconds)
}

fn user_chunk(uuid: &str, text: &str, offset: i64) -> Chunk {
    Chunk::User(UserChunk {
        uuid: uuid.into(),
        timestamp: ts(offset),
        duration_ms: None,
        content: MessageContent::Text(text.into()),
        metrics: ChunkMetrics::zero(),
    })
}

fn ai_chunk(
    uuid: &str,
    offset: i64,
    usage: Option<TokenUsage>,
    tools: Vec<ToolExecution>,
) -> Chunk {
    Chunk::Ai(AIChunk {
        timestamp: ts(offset),
        duration_ms: None,
        responses: vec![AssistantResponse {
            uuid: uuid.into(),
            timestamp: ts(offset),
            content: MessageContent::Text("ack".into()),
            tool_calls: Vec::new(),
            usage,
            model: Some("claude-opus-4-6".into()),
            content_omitted: false,
        }],
        metrics: ChunkMetrics::zero(),
        semantic_steps: Vec::new(),
        tool_executions: tools,
        subagents: Vec::new(),
        slash_commands: Vec::new(),
    })
}

fn compact_chunk(uuid: &str, offset: i64) -> Chunk {
    Chunk::Compact(CompactChunk {
        uuid: uuid.into(),
        timestamp: ts(offset),
        duration_ms: None,
        summary_text: "summary of phase".into(),
        metrics: ChunkMetrics::zero(),
    })
}

fn bash_tool(id: &str, cmd: &str, output: &str) -> ToolExecution {
    ToolExecution {
        tool_use_id: id.into(),
        tool_name: "Bash".into(),
        input: serde_json::json!({"cmd": cmd}),
        output: ToolOutput::Text {
            text: output.into(),
        },
        is_error: false,
        start_ts: ts(0),
        end_ts: Some(ts(0)),
        source_assistant_uuid: "a0".into(),
        result_agent_id: None,
    }
}

fn default_params<'a>(
    claude_md: &'a HashMap<String, ClaudeMdFileInfo>,
    directory: &'a HashMap<String, ClaudeMdFileInfo>,
    mentioned: &'a HashMap<String, MentionedFileInfo>,
    initial: &'a [ContextInjection],
) -> ProcessSessionParams<'a> {
    ProcessSessionParams {
        project_root: Path::new("/repo"),
        token_dictionaries: TokenDictionaries::new(
            Path::new("/repo"),
            claude_md,
            directory,
            mentioned,
        ),
        initial_claude_md_injections: initial,
    }
}

#[test]
fn empty_chunk_slice_yields_empty_result() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();
    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&[], &params);
    assert!(result.stats_map.is_empty());
    assert!(result.phase_info.phases.is_empty());
    assert_eq!(result.phase_info.compaction_count, 0);
}

#[test]
fn single_ai_group_with_tool_and_user_message_populates_stats() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();

    let chunks = vec![
        user_chunk("u0", "please run echo hello", 0),
        ai_chunk(
            "a0",
            1,
            None,
            vec![bash_tool("tu1", "echo hello", "hello world")],
        ),
    ];

    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);

    assert_eq!(result.stats_map.len(), 1);
    let stats = result.stats_map.get("a0").unwrap();
    assert_eq!(stats.new_injections.len(), 2);
    assert!(stats.total_estimated_tokens > 0);
    assert_eq!(
        stats.total_estimated_tokens,
        stats.tokens_by_category.tool_output + stats.tokens_by_category.user_messages
    );
    assert_eq!(stats.phase_number, Some(1));
    // last group in phase → accumulated 非空
    assert!(!stats.accumulated_injections.is_empty());

    assert_eq!(result.phase_info.phases.len(), 1);
    assert!(result.phase_info.compaction_token_deltas.is_empty());
}

#[test]
fn mid_compaction_produces_two_phases_and_token_delta() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();

    let usage_pre = TokenUsage {
        input_tokens: 600,
        output_tokens: 200,
        cache_read_input_tokens: 150,
        cache_creation_input_tokens: 50,
    }; // total = 1000

    let usage_post = TokenUsage {
        input_tokens: 400,
        output_tokens: 100,
        cache_read_input_tokens: 60,
        cache_creation_input_tokens: 40,
    }; // total = 600

    let chunks = vec![
        ai_chunk("a0", 0, Some(usage_pre), vec![]),
        compact_chunk("c0", 1),
        ai_chunk("a1", 2, Some(usage_post), vec![]),
    ];

    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);

    assert_eq!(result.phase_info.phases.len(), 2);
    assert_eq!(result.phase_info.compaction_count, 1);
    let delta = result
        .phase_info
        .compaction_token_deltas
        .get("c0")
        .expect("delta exists for c0");
    assert_eq!(delta.pre_compaction_tokens, 1000);
    assert_eq!(delta.post_compaction_tokens, 600);
    assert_eq!(delta.delta, -400);

    let phase1 = &result.phase_info.phases[0];
    assert_eq!(phase1.phase_number, 1);
    assert_eq!(phase1.first_ai_group_id, "a0");
    let phase2 = &result.phase_info.phases[1];
    assert_eq!(phase2.phase_number, 2);
    assert_eq!(phase2.first_ai_group_id, "a1");
    assert_eq!(phase2.compact_group_id.as_deref(), Some("c0"));
}

#[test]
fn trailing_compaction_finalizes_last_phase_without_delta() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();

    let chunks = vec![ai_chunk("a0", 0, None, vec![]), compact_chunk("c0", 1)];

    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);

    assert_eq!(result.phase_info.phases.len(), 1);
    assert!(result.phase_info.compaction_token_deltas.is_empty());
    assert_eq!(result.phase_info.phases[0].first_ai_group_id, "a0");
    assert_eq!(result.phase_info.phases[0].compact_group_id, None);
}

#[test]
fn claude_md_path_is_deduped_across_groups() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();

    let initial = vec![ContextInjection::ClaudeMd(ClaudeMdContextInjection {
        id: "cm-1".into(),
        path: "/repo/CLAUDE.md".into(),
        display_name: "CLAUDE.md".into(),
        scope: ClaudeMdScope::Project,
        estimated_tokens: 100,
        first_seen_turn_index: 0,
    })];

    let chunks = vec![
        ai_chunk("a0", 0, None, vec![]),
        ai_chunk("a1", 1, None, vec![]),
    ];

    let params = default_params(&cm, &dir, &mf, &initial);
    let result = process_session_context_with_phases(&chunks, &params);

    // 第一个 group 拿到 CLAUDE.md injection，第二个 group 不再拿到新 injection。
    let a0_new = result
        .stats_map
        .get("a0")
        .unwrap()
        .new_injections
        .iter()
        .filter(|inj| matches!(inj, ContextInjection::ClaudeMd(_)))
        .count();
    let a1_new = result
        .stats_map
        .get("a1")
        .unwrap()
        .new_injections
        .iter()
        .filter(|inj| matches!(inj, ContextInjection::ClaudeMd(_)))
        .count();
    assert_eq!(a0_new, 1);
    assert_eq!(a1_new, 0);
}

#[test]
fn missing_token_data_does_not_panic() {
    // 即便外部没传任何 CLAUDE.md 字典，process 函数也能跑完并产出零 token injection。
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();

    let chunks = vec![ai_chunk("a0", 0, None, vec![])];
    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);
    assert_eq!(result.stats_map.len(), 1);
    assert_eq!(
        result.stats_map.get("a0").unwrap().total_estimated_tokens,
        0
    );
}

#[test]
fn context_stats_serializes_with_camel_case_fields() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();
    let chunks = vec![
        user_chunk("u0", "hi", 0),
        ai_chunk("a0", 1, None, vec![bash_tool("tu1", "ls", "file")]),
    ];
    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);
    let stats = result.stats_map.get("a0").unwrap();
    let v = serde_json::to_value(stats).unwrap();
    let obj = v.as_object().unwrap();
    assert!(obj.contains_key("tokensByCategory"));
    assert!(obj.contains_key("totalEstimatedTokens"));
    assert!(obj.contains_key("newCounts"));
    assert!(obj.contains_key("accumulatedCounts"));
}
