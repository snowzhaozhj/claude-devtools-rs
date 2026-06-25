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
        chunk_id: format!("{uuid}:0"),
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
        chunk_id: format!("{uuid}:0"),
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
        teammate_messages: Vec::new(),
    })
}

fn compact_chunk(uuid: &str, offset: i64) -> Chunk {
    Chunk::Compact(CompactChunk {
        chunk_id: format!("{uuid}:0"),
        uuid: uuid.into(),
        timestamp: ts(offset),
        duration_ms: None,
        summary_text: "summary of phase".into(),
        metrics: ChunkMetrics::zero(),
        token_delta: None,
        phase_number: None,
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
        error_message: None,
        output_omitted: false,
        output_bytes: None,
        teammate_spawn: None,
        workflow_run_id: None,
        workflow_script_path: None,
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
    let stats = result.stats_map.get("a0:0").unwrap();
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
        .get("c0:0")
        .expect("delta exists for c0:0");
    assert_eq!(delta.pre_compaction_tokens, 1000);
    assert_eq!(delta.post_compaction_tokens, 600);
    assert_eq!(delta.delta, -400);

    let phase1 = &result.phase_info.phases[0];
    assert_eq!(phase1.phase_number, 1);
    assert_eq!(phase1.first_ai_group_id, "a0:0");
    let phase2 = &result.phase_info.phases[1];
    assert_eq!(phase2.phase_number, 2);
    assert_eq!(phase2.first_ai_group_id, "a1:0");
    assert_eq!(phase2.compact_group_id.as_deref(), Some("c0:0"));
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
    assert_eq!(result.phase_info.phases[0].first_ai_group_id, "a0:0");
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
        .get("a0:0")
        .unwrap()
        .new_injections
        .iter()
        .filter(|inj| matches!(inj, ContextInjection::ClaudeMd(_)))
        .count();
    let a1_new = result
        .stats_map
        .get("a1:0")
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
        result.stats_map.get("a0:0").unwrap().total_estimated_tokens,
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
    let stats = result.stats_map.get("a0:0").unwrap();
    let v = serde_json::to_value(stats).unwrap();
    let obj = v.as_object().unwrap();
    assert!(obj.contains_key("tokensByCategory"));
    assert!(obj.contains_key("totalEstimatedTokens"));
    assert!(obj.contains_key("newCounts"));
    assert!(obj.contains_key("accumulatedCounts"));
}

// ===== turn 锚点：被打断的 turn（issue #540 / change turn-anchoring）=====

/// 从一组累积 injection 里抽出所有 user-message injection 的 `(turn_index, ai_group_id)`，
/// 按 `turn_index` 升序——用于断言 turn 锚点与序号。
fn user_msg_anchors(injs: &[ContextInjection]) -> Vec<(u32, String)> {
    let mut v: Vec<(u32, String)> = injs
        .iter()
        .filter_map(|inj| match inj {
            ContextInjection::UserMessage(x) => Some((x.turn_index, x.ai_group_id.clone())),
            _ => None,
        })
        .collect();
    v.sort_by_key(|(ti, _)| *ti);
    v
}

#[test]
fn completed_turn_anchors_on_its_user_message() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();
    let chunks = vec![
        user_chunk("u0", "do a thing", 0),
        ai_chunk("a0", 1, None, vec![]),
    ];
    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);

    let stats = result.stats_map.get("a0:0").unwrap();
    let anchors = user_msg_anchors(&stats.accumulated_injections);
    // 完整 turn：user-message injection 占 turn 0，锚到 AIChunk chunkId。
    assert_eq!(anchors, vec![(0, "a0:0".to_string())]);
}

#[test]
fn interrupted_user_message_still_opens_a_turn() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();
    // U1 被打断（U1 与 U2 之间没有 AI group），U2 → A2。
    let chunks = vec![
        user_chunk("u1", "first message that gets interrupted", 0),
        user_chunk("u2", "second message", 1),
        ai_chunk("a2", 2, None, vec![]),
    ];
    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);

    // 被打断的 U1 不进 stats_map；只有完整 turn A2 有 stats 条目。
    assert_eq!(result.stats_map.len(), 1);
    assert!(result.stats_map.contains_key("a2:0"));
    assert!(!result.stats_map.contains_key("u1:0"));

    // A2 的累积链里同时含 U1（被打断，turn 0，锚 UserChunk）与 U2（完整，turn 1，锚 AIChunk）。
    let stats = result.stats_map.get("a2:0").unwrap();
    let anchors = user_msg_anchors(&stats.accumulated_injections);
    assert_eq!(
        anchors,
        vec![(0, "u1:0".to_string()), (1, "a2:0".to_string())]
    );
}

#[test]
fn interrupted_turn_at_end_of_session() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();
    // U0 → A0（完整），U1 在会话结束前没有 AI group（末尾被打断）。
    let chunks = vec![
        user_chunk("u0", "answered message", 0),
        ai_chunk("a0", 1, None, vec![]),
        user_chunk("u1", "trailing message with no response", 2),
    ];
    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);

    assert_eq!(result.stats_map.len(), 1);
    assert!(!result.stats_map.contains_key("u1:0"));
    // 末尾被打断的 U1 经 backfill 出现在最后一个 AI group A0 的累积链。
    let stats = result.stats_map.get("a0:0").unwrap();
    let anchors = user_msg_anchors(&stats.accumulated_injections);
    assert_eq!(
        anchors,
        vec![(0, "a0:0".to_string()), (1, "u1:0".to_string())]
    );
}

#[test]
fn interrupted_turn_anchor_is_userchunk_not_any_aichunk() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();
    let chunks = vec![
        user_chunk("u1", "interrupted", 0),
        user_chunk("u2", "next", 1),
        ai_chunk("a2", 2, None, vec![]),
    ];
    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);

    let ai_chunk_ids: std::collections::HashSet<String> =
        result.stats_map.keys().cloned().collect();
    let stats = result.stats_map.get("a2:0").unwrap();
    let interrupted_anchor = user_msg_anchors(&stats.accumulated_injections)
        .into_iter()
        .find(|(ti, _)| *ti == 0)
        .map(|(_, id)| id)
        .unwrap();
    // 被打断 turn 的锚 = UserChunk chunkId，且不属于任何 AI group（stats_map key）集合。
    assert_eq!(interrupted_anchor, "u1:0");
    assert!(!ai_chunk_ids.contains(&interrupted_anchor));
}

#[test]
fn consecutive_interruptions_each_open_a_turn() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();
    // 连续三条被打断后才有一个 AI group——每条都该占一个 turn，不丢不吞。
    let chunks = vec![
        user_chunk("u1", "继续", 0),
        user_chunk("u2", "继续", 1),
        user_chunk("u3", "继续", 2),
        ai_chunk("a3", 3, None, vec![]),
    ];
    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);

    let stats = result.stats_map.get("a3:0").unwrap();
    let anchors = user_msg_anchors(&stats.accumulated_injections);
    assert_eq!(
        anchors,
        vec![
            (0, "u1:0".to_string()),
            (1, "u2:0".to_string()),
            (2, "a3:0".to_string()),
        ]
    );
}

#[test]
fn interrupted_turn_before_compaction_lands_in_pre_compact_phase() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();
    // U1 在 compact 前被打断（A0 与 compact 之间无 AI group 承载 U1）。
    let chunks = vec![
        user_chunk("u0", "first answered", 0),
        ai_chunk("a0", 1, None, vec![]),
        user_chunk("u1", "interrupted before compact", 2),
        compact_chunk("c0", 3),
        user_chunk("u2", "after compact", 4),
        ai_chunk("a2", 5, None, vec![]),
    ];
    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);

    // U1 的 injection 经 compact 分支 flush + backfill 落在 compact 前 phase 的 A0。
    let a0 = result.stats_map.get("a0:0").unwrap();
    assert_eq!(
        user_msg_anchors(&a0.accumulated_injections),
        vec![(0, "a0:0".to_string()), (1, "u1:0".to_string())]
    );
    // compact 后 phase 的 A2 锚定 U2（turn 2）。
    let a2 = result.stats_map.get("a2:0").unwrap();
    assert_eq!(
        user_msg_anchors(&a2.accumulated_injections),
        vec![(2, "a2:0".to_string())]
    );
}

#[test]
fn interrupted_turn_with_no_ai_carrier_phase_is_dropped() {
    let cm: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let dir: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mf: HashMap<String, MentionedFileInfo> = HashMap::new();
    // 退化情形：compact 前 phase 只有被打断的 U1、无任何 AI group 承载累积链。
    // 文档化的已知限制：不 panic，U1 injection 无承载点而丢失。
    let chunks = vec![
        user_chunk("u1", "interrupted with no carrier", 0),
        compact_chunk("c0", 1),
        ai_chunk("a0", 2, None, vec![]),
    ];
    let params = default_params(&cm, &dir, &mf, &[]);
    let result = process_session_context_with_phases(&chunks, &params);

    // 良定义结果：只有 compact 后 phase 的 A0 进 stats_map；U1 不在任何地方 surface。
    assert_eq!(result.stats_map.len(), 1);
    assert!(result.stats_map.contains_key("a0:0"));
    assert!(!result.stats_map.contains_key("u1:0"));
    let a0 = result.stats_map.get("a0:0").unwrap();
    assert!(
        user_msg_anchors(&a0.accumulated_injections).is_empty(),
        "无 AI carrier 的被打断 injection 不应 surface（已知限制）"
    );
}
