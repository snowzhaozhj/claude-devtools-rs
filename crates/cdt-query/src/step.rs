//! Turn-level step building — ports the desktop `buildDisplayItems` logic.
//!
//! Consumes `cdt_core::Chunk` + `cdt_analyze::Turn` and produces a flat
//! `Vec<Step>` per turn. This is the CLI/MCP exclusive layer; the desktop
//! frontend continues using its own TS renderer.

use std::collections::HashMap;

use cdt_core::{
    AIChunk, Chunk, CompactChunk, Process, SemanticStep, SlashCommand, SystemChunk,
    TeammateMessage, ToolExecution,
};
use chrono::{DateTime, Utc};

/// A single step within a turn — 12 types covering the full `DisplayItem`
/// taxonomy plus `compaction` and `system`.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Thinking {
        text: String,
        timestamp: DateTime<Utc>,
    },
    Text {
        text: String,
        timestamp: DateTime<Utc>,
    },
    Tool {
        tool_use_id: String,
        name: String,
        input: serde_json::Value,
        output: cdt_core::ToolOutput,
        is_error: bool,
        error_message: Option<String>,
        timestamp: DateTime<Utc>,
    },
    Subagent {
        name: String,
        description: Option<String>,
        subagent_session_id: Option<String>,
        steps_count: usize,
        timestamp: DateTime<Utc>,
    },
    TeammateSpawn {
        name: String,
        color: Option<String>,
        tool_use_id: String,
        timestamp: DateTime<Utc>,
    },
    Workflow {
        tool_use_id: String,
        name: String,
        run_id: String,
        timestamp: DateTime<Utc>,
    },
    Interruption {
        text: String,
        timestamp: DateTime<Utc>,
    },
    UserMessage {
        text: String,
        timestamp: DateTime<Utc>,
    },
    Slash {
        name: String,
        message: Option<String>,
        args: Option<String>,
        timestamp: DateTime<Utc>,
    },
    TeammateMsg {
        teammate_id: String,
        body: String,
        color: Option<String>,
        timestamp: DateTime<Utc>,
    },
    Compaction {
        summary: String,
        timestamp: DateTime<Utc>,
    },
    System {
        content: String,
        timestamp: DateTime<Utc>,
    },
}

impl Step {
    #[must_use]
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Thinking { timestamp, .. }
            | Self::Text { timestamp, .. }
            | Self::Tool { timestamp, .. }
            | Self::Subagent { timestamp, .. }
            | Self::TeammateSpawn { timestamp, .. }
            | Self::Workflow { timestamp, .. }
            | Self::Interruption { timestamp, .. }
            | Self::UserMessage { timestamp, .. }
            | Self::Slash { timestamp, .. }
            | Self::TeammateMsg { timestamp, .. }
            | Self::Compaction { timestamp, .. }
            | Self::System { timestamp, .. } => *timestamp,
        }
    }

    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Thinking { .. } => "thinking",
            Self::Text { .. } => "text",
            Self::Tool { .. } => "tool",
            Self::Subagent { .. } => "subagent",
            Self::TeammateSpawn { .. } => "teammate_spawn",
            Self::Workflow { .. } => "workflow",
            Self::Interruption { .. } => "interruption",
            Self::UserMessage { .. } => "user_message",
            Self::Slash { .. } => "slash",
            Self::TeammateMsg { .. } => "teammate_message",
            Self::Compaction { .. } => "compaction",
            Self::System { .. } => "system",
        }
    }
}

/// Build steps from a single `AIChunk`.
///
/// Iterates `semantic_steps` (the time-ordered backbone), links each
/// `ToolExecution` step to its full record, resolves subagent / teammate-spawn /
/// workflow upgrades, then appends slash commands and teammate messages sorted
/// by timestamp.
#[must_use]
pub fn build_steps_from_ai_chunk(chunk: &AIChunk) -> Vec<Step> {
    let tool_exec_map: HashMap<&str, &ToolExecution> = chunk
        .tool_executions
        .iter()
        .map(|te| (te.tool_use_id.as_str(), te))
        .collect();

    let subagent_by_task: HashMap<&str, &Process> = chunk
        .subagents
        .iter()
        .filter_map(|p| p.parent_task_id.as_deref().map(|id| (id, p)))
        .collect();

    let mut steps: Vec<Step> = Vec::new();

    for sem in &chunk.semantic_steps {
        match sem {
            SemanticStep::Thinking { text, timestamp } => {
                if !text.is_empty() {
                    steps.push(Step::Thinking {
                        text: text.clone(),
                        timestamp: *timestamp,
                    });
                }
            }
            SemanticStep::Text { text, timestamp } => {
                if !text.is_empty() {
                    steps.push(Step::Text {
                        text: text.clone(),
                        timestamp: *timestamp,
                    });
                }
            }
            SemanticStep::ToolExecution {
                tool_use_id,
                tool_name,
                timestamp,
            } => {
                if let Some(te) = tool_exec_map.get(tool_use_id.as_str()) {
                    steps.push(classify_tool_step(te, &subagent_by_task, *timestamp));
                } else {
                    steps.push(Step::Tool {
                        tool_use_id: tool_use_id.clone(),
                        name: tool_name.clone(),
                        input: serde_json::Value::Null,
                        output: cdt_core::ToolOutput::Missing,
                        is_error: false,
                        error_message: None,
                        timestamp: *timestamp,
                    });
                }
            }
            SemanticStep::SubagentSpawn {
                placeholder_id,
                timestamp,
            } => {
                steps.push(resolve_subagent_spawn(
                    placeholder_id,
                    &chunk.subagents,
                    *timestamp,
                ));
            }
            SemanticStep::Interruption { text, timestamp } => {
                steps.push(Step::Interruption {
                    text: text.clone(),
                    timestamp: *timestamp,
                });
            }
            SemanticStep::UserMessage {
                text, timestamp, ..
            } => {
                steps.push(Step::UserMessage {
                    text: text.clone(),
                    timestamp: *timestamp,
                });
            }
        }
    }

    append_slash_commands(&mut steps, &chunk.slash_commands);
    append_teammate_messages(&mut steps, &chunk.teammate_messages);

    steps.sort_by_key(Step::timestamp);
    steps
}

/// Build a `Step::Compaction` from a `CompactChunk`.
#[must_use]
pub fn build_compaction_step(chunk: &CompactChunk) -> Step {
    Step::Compaction {
        summary: chunk.summary_text.clone(),
        timestamp: chunk.timestamp,
    }
}

/// Build a `Step::System` from a `SystemChunk`.
#[must_use]
pub fn build_system_step(chunk: &SystemChunk) -> Step {
    Step::System {
        content: chunk.content_text.clone(),
        timestamp: chunk.timestamp,
    }
}

/// Build steps for a turn given its `member_chunk_ids` and a chunk lookup map.
///
/// Chunks are processed in order; within each `AIChunk`, steps are sorted by
/// timestamp. System and Compact chunks produce single steps.
#[must_use]
pub fn build_steps_for_turn<S: ::std::hash::BuildHasher>(
    member_chunk_ids: &[String],
    chunk_map: &HashMap<&str, &Chunk, S>,
) -> Vec<Step> {
    let mut steps = Vec::new();

    for chunk_id in member_chunk_ids {
        let Some(chunk) = chunk_map.get(chunk_id.as_str()) else {
            continue;
        };
        match chunk {
            Chunk::Ai(ai) => {
                steps.extend(build_steps_from_ai_chunk(ai));
            }
            Chunk::Compact(c) => {
                steps.push(build_compaction_step(c));
            }
            Chunk::System(s) => {
                steps.push(build_system_step(s));
            }
            Chunk::User(_) => {}
        }
    }

    steps
}

/// Detect the answer text from a step list.
///
/// Returns `None` if the turn was interrupted or contains no text output.
#[must_use]
pub fn detect_answer(steps: &[Step]) -> Option<String> {
    if steps.iter().any(|s| matches!(s, Step::Interruption { .. })) {
        return None;
    }
    for step in steps.iter().rev() {
        if let Step::Text { text, .. } = step {
            if !text.is_empty() {
                return Some(text.clone());
            }
        }
    }
    None
}

/// Aggregate tool usage by name for the compact overview.
#[must_use]
pub fn aggregate_tools(steps: &[Step]) -> Vec<ToolAggregation> {
    let mut map: HashMap<&str, (usize, usize)> = HashMap::new();
    for step in steps {
        if let Step::Tool { name, is_error, .. } = step {
            let entry = map.entry(name.as_str()).or_default();
            entry.0 += 1;
            if *is_error {
                entry.1 += 1;
            }
        }
    }
    let mut result: Vec<ToolAggregation> = map
        .into_iter()
        .map(|(name, (count, error_count))| ToolAggregation {
            name: name.to_string(),
            count,
            error_count,
        })
        .collect();
    result.sort_by_key(|a| std::cmp::Reverse(a.count));
    result
}

/// Aggregated tool usage for the compact turn overview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolAggregation {
    pub name: String,
    pub count: usize,
    pub error_count: usize,
}

/// Build a `chunk_id → &Chunk` lookup map for efficient turn assembly.
#[must_use]
pub fn build_chunk_map(chunks: &[Chunk]) -> HashMap<&str, &Chunk> {
    chunks
        .iter()
        .map(|c| {
            let id = match c {
                Chunk::User(u) => u.chunk_id.as_str(),
                Chunk::Ai(ai) => ai.chunk_id.as_str(),
                Chunk::System(s) => s.chunk_id.as_str(),
                Chunk::Compact(c) => c.chunk_id.as_str(),
            };
            (id, c)
        })
        .collect()
}

// --- internal helpers ---

fn classify_tool_step(
    te: &ToolExecution,
    subagent_by_task: &HashMap<&str, &Process>,
    timestamp: DateTime<Utc>,
) -> Step {
    if let Some(spawn) = &te.teammate_spawn {
        return Step::TeammateSpawn {
            name: spawn.name.clone(),
            color: spawn.color.clone(),
            tool_use_id: te.tool_use_id.clone(),
            timestamp,
        };
    }

    if let Some(run_id) = &te.workflow_run_id {
        return Step::Workflow {
            tool_use_id: te.tool_use_id.clone(),
            name: te.tool_name.clone(),
            run_id: run_id.clone(),
            timestamp,
        };
    }

    if let Some(subagent) = subagent_by_task.get(te.tool_use_id.as_str()) {
        return Step::Subagent {
            name: subagent
                .subagent_type
                .clone()
                .unwrap_or_else(|| te.tool_name.clone()),
            description: subagent
                .description
                .clone()
                .or_else(|| subagent.root_task_description.clone()),
            subagent_session_id: Some(subagent.session_id.clone()),
            steps_count: count_subagent_steps(subagent),
            timestamp,
        };
    }

    Step::Tool {
        tool_use_id: te.tool_use_id.clone(),
        name: te.tool_name.clone(),
        input: te.input.clone(),
        output: te.output.clone(),
        is_error: te.is_error,
        error_message: te.error_message.clone(),
        timestamp,
    }
}

fn resolve_subagent_spawn(
    placeholder_id: &str,
    subagents: &[Process],
    timestamp: DateTime<Utc>,
) -> Step {
    let found = subagents.iter().find(|p| {
        p.session_id == placeholder_id || p.parent_task_id.as_deref() == Some(placeholder_id)
    });

    if let Some(sub) = found {
        Step::Subagent {
            name: sub
                .subagent_type
                .clone()
                .unwrap_or_else(|| "subagent".to_string()),
            description: sub
                .description
                .clone()
                .or_else(|| sub.root_task_description.clone()),
            subagent_session_id: Some(sub.session_id.clone()),
            steps_count: count_subagent_steps(sub),
            timestamp,
        }
    } else {
        Step::Subagent {
            name: "subagent".to_string(),
            description: None,
            subagent_session_id: None,
            steps_count: 0,
            timestamp,
        }
    }
}

fn count_subagent_steps(process: &Process) -> usize {
    process
        .messages
        .iter()
        .map(|chunk| match chunk {
            Chunk::Ai(ai) => ai.semantic_steps.len(),
            _ => 0,
        })
        .sum()
}

fn append_slash_commands(steps: &mut Vec<Step>, slashes: &[SlashCommand]) {
    for slash in slashes {
        steps.push(Step::Slash {
            name: slash.name.clone(),
            message: slash.message.clone(),
            args: slash.args.clone(),
            timestamp: slash.timestamp,
        });
    }
}

fn append_teammate_messages(steps: &mut Vec<Step>, messages: &[TeammateMessage]) {
    for tm in messages {
        steps.push(Step::TeammateMsg {
            teammate_id: tm.teammate_id.clone(),
            body: tm.body.clone(),
            color: tm.color.clone(),
            timestamp: tm.timestamp,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::chunk::ChunkMetrics;
    use cdt_core::tool_execution::{TeammateSpawnInfo, ToolOutput};

    fn ts(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).unwrap()
    }

    fn make_ai_chunk(
        chunk_id: &str,
        semantic_steps: Vec<SemanticStep>,
        tool_executions: Vec<ToolExecution>,
    ) -> AIChunk {
        AIChunk {
            chunk_id: chunk_id.to_string(),
            timestamp: ts(1000),
            duration_ms: None,
            responses: vec![],
            metrics: ChunkMetrics::default(),
            semantic_steps,
            tool_executions,
            subagents: vec![],
            slash_commands: vec![],
            teammate_messages: vec![],
        }
    }

    fn make_tool_exec(id: &str, name: &str, output: ToolOutput, is_error: bool) -> ToolExecution {
        ToolExecution {
            tool_use_id: id.to_string(),
            tool_name: name.to_string(),
            input: serde_json::json!({"path": "/foo"}),
            output,
            is_error,
            start_ts: ts(1000),
            end_ts: Some(ts(1001)),
            source_assistant_uuid: "a1".to_string(),
            result_agent_id: None,
            error_message: if is_error {
                Some("failed".to_string())
            } else {
                None
            },
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
            workflow_run_id: None,
            workflow_script_path: None,
        }
    }

    #[test]
    fn thinking_step_from_semantic_step() {
        let chunk = make_ai_chunk(
            "c1",
            vec![SemanticStep::Thinking {
                text: "Let me think...".into(),
                timestamp: ts(100),
            }],
            vec![],
        );
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].type_name(), "thinking");
        assert!(matches!(&steps[0], Step::Thinking { text, .. } if text == "Let me think..."));
    }

    #[test]
    fn empty_thinking_skipped() {
        let chunk = make_ai_chunk(
            "c1",
            vec![SemanticStep::Thinking {
                text: String::new(),
                timestamp: ts(100),
            }],
            vec![],
        );
        let steps = build_steps_from_ai_chunk(&chunk);
        assert!(steps.is_empty());
    }

    #[test]
    fn tool_step_with_text_output() {
        let chunk = make_ai_chunk(
            "c1",
            vec![SemanticStep::ToolExecution {
                tool_use_id: "tu1".into(),
                tool_name: "Read".into(),
                timestamp: ts(100),
            }],
            vec![make_tool_exec(
                "tu1",
                "Read",
                ToolOutput::Text {
                    text: "file content".into(),
                },
                false,
            )],
        );
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].type_name(), "tool");
        if let Step::Tool {
            name,
            output,
            is_error,
            ..
        } = &steps[0]
        {
            assert_eq!(name, "Read");
            assert!(matches!(output, ToolOutput::Text { text } if text == "file content"));
            assert!(!is_error);
        } else {
            panic!("expected Tool step");
        }
    }

    #[test]
    fn tool_step_structured_output() {
        let chunk = make_ai_chunk(
            "c1",
            vec![SemanticStep::ToolExecution {
                tool_use_id: "tu1".into(),
                tool_name: "Bash".into(),
                timestamp: ts(100),
            }],
            vec![make_tool_exec(
                "tu1",
                "Bash",
                ToolOutput::Structured {
                    value: serde_json::json!({"stdout": "ok", "stderr": "", "exitCode": 0}),
                },
                false,
            )],
        );
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        if let Step::Tool { output, .. } = &steps[0] {
            assert!(matches!(output, ToolOutput::Structured { .. }));
        } else {
            panic!("expected Tool step");
        }
    }

    #[test]
    fn tool_step_missing_output() {
        let chunk = make_ai_chunk(
            "c1",
            vec![SemanticStep::ToolExecution {
                tool_use_id: "tu-orphan".into(),
                tool_name: "Read".into(),
                timestamp: ts(100),
            }],
            vec![],
        );
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        if let Step::Tool { output, .. } = &steps[0] {
            assert!(matches!(output, ToolOutput::Missing));
        } else {
            panic!("expected Tool step");
        }
    }

    #[test]
    fn teammate_spawn_upgrade() {
        let mut te = make_tool_exec(
            "tu-spawn",
            "Agent",
            ToolOutput::Structured {
                value: serde_json::json!({"status": "teammate_spawned"}),
            },
            false,
        );
        te.teammate_spawn = Some(TeammateSpawnInfo {
            name: "designer".into(),
            color: Some("blue".into()),
        });

        let chunk = make_ai_chunk(
            "c1",
            vec![SemanticStep::ToolExecution {
                tool_use_id: "tu-spawn".into(),
                tool_name: "Agent".into(),
                timestamp: ts(100),
            }],
            vec![te],
        );
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].type_name(), "teammate_spawn");
        if let Step::TeammateSpawn { name, color, .. } = &steps[0] {
            assert_eq!(name, "designer");
            assert_eq!(color.as_deref(), Some("blue"));
        } else {
            panic!("expected TeammateSpawn step");
        }
    }

    #[test]
    fn workflow_upgrade() {
        let mut te = make_tool_exec(
            "tu-wf",
            "Workflow",
            ToolOutput::Text {
                text: "done".into(),
            },
            false,
        );
        te.workflow_run_id = Some("wf_abc123".into());

        let chunk = make_ai_chunk(
            "c1",
            vec![SemanticStep::ToolExecution {
                tool_use_id: "tu-wf".into(),
                tool_name: "Workflow".into(),
                timestamp: ts(100),
            }],
            vec![te],
        );
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].type_name(), "workflow");
        if let Step::Workflow { run_id, .. } = &steps[0] {
            assert_eq!(run_id, "wf_abc123");
        } else {
            panic!("expected Workflow step");
        }
    }

    #[test]
    fn interruption_step() {
        let chunk = make_ai_chunk(
            "c1",
            vec![SemanticStep::Interruption {
                text: "Request interrupted by user".into(),
                timestamp: ts(100),
            }],
            vec![],
        );
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].type_name(), "interruption");
    }

    #[test]
    fn user_message_step() {
        let chunk = make_ai_chunk(
            "c1",
            vec![SemanticStep::UserMessage {
                uuid: "u1".into(),
                text: "queued input".into(),
                timestamp: ts(100),
            }],
            vec![],
        );
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].type_name(), "user_message");
    }

    #[test]
    fn slash_commands_appended() {
        let mut chunk = make_ai_chunk("c1", vec![], vec![]);
        chunk.slash_commands.push(SlashCommand {
            name: "review".into(),
            message: Some("PR #42".into()),
            args: None,
            message_uuid: "u-slash".into(),
            timestamp: ts(200),
            instructions: None,
        });
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].type_name(), "slash");
    }

    #[test]
    fn teammate_messages_appended() {
        let mut chunk = make_ai_chunk("c1", vec![], vec![]);
        chunk.teammate_messages.push(TeammateMessage {
            uuid: "tm1".into(),
            teammate_id: "member-1".into(),
            color: Some("green".into()),
            summary: None,
            body: "Hello from teammate".into(),
            timestamp: ts(200),
            reply_to_tool_use_id: None,
            token_count: None,
            is_noise: false,
            is_resend: false,
        });
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].type_name(), "teammate_message");
    }

    #[test]
    fn steps_sorted_by_timestamp() {
        let chunk = make_ai_chunk(
            "c1",
            vec![
                SemanticStep::Thinking {
                    text: "think".into(),
                    timestamp: ts(300),
                },
                SemanticStep::Text {
                    text: "answer".into(),
                    timestamp: ts(100),
                },
            ],
            vec![],
        );
        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 2);
        assert!(steps[0].timestamp() <= steps[1].timestamp());
    }

    #[test]
    fn detect_answer_last_text() {
        let steps = vec![
            Step::Thinking {
                text: "hmm".into(),
                timestamp: ts(100),
            },
            Step::Text {
                text: "The answer is 42".into(),
                timestamp: ts(200),
            },
        ];
        assert_eq!(detect_answer(&steps), Some("The answer is 42".into()));
    }

    #[test]
    fn detect_answer_interrupted_is_none() {
        let steps = vec![
            Step::Text {
                text: "partial".into(),
                timestamp: ts(100),
            },
            Step::Interruption {
                text: "interrupted".into(),
                timestamp: ts(200),
            },
        ];
        assert_eq!(detect_answer(&steps), None);
    }

    #[test]
    fn detect_answer_no_text_is_none() {
        let steps = vec![Step::Thinking {
            text: "hmm".into(),
            timestamp: ts(100),
        }];
        assert_eq!(detect_answer(&steps), None);
    }

    #[test]
    fn aggregate_tools_counts() {
        let steps = vec![
            Step::Tool {
                tool_use_id: "t1".into(),
                name: "Read".into(),
                input: serde_json::Value::Null,
                output: cdt_core::ToolOutput::Missing,
                is_error: false,
                error_message: None,
                timestamp: ts(100),
            },
            Step::Tool {
                tool_use_id: "t2".into(),
                name: "Read".into(),
                input: serde_json::Value::Null,
                output: cdt_core::ToolOutput::Missing,
                is_error: true,
                error_message: Some("err".into()),
                timestamp: ts(200),
            },
            Step::Tool {
                tool_use_id: "t3".into(),
                name: "Bash".into(),
                input: serde_json::Value::Null,
                output: cdt_core::ToolOutput::Missing,
                is_error: false,
                error_message: None,
                timestamp: ts(300),
            },
        ];
        let agg = aggregate_tools(&steps);
        assert_eq!(agg.len(), 2);
        let read = agg.iter().find(|a| a.name == "Read").unwrap();
        assert_eq!(read.count, 2);
        assert_eq!(read.error_count, 1);
        let bash = agg.iter().find(|a| a.name == "Bash").unwrap();
        assert_eq!(bash.count, 1);
        assert_eq!(bash.error_count, 0);
    }

    #[test]
    fn compaction_step_from_compact_chunk() {
        let chunk = CompactChunk {
            chunk_id: "compact-1".into(),
            uuid: "u1".into(),
            timestamp: ts(500),
            duration_ms: None,
            summary_text: "Conversation was compacted".into(),
            metrics: ChunkMetrics::default(),
            token_delta: None,
            phase_number: Some(2),
        };
        let step = build_compaction_step(&chunk);
        assert_eq!(step.type_name(), "compaction");
        if let Step::Compaction { summary, .. } = &step {
            assert_eq!(summary, "Conversation was compacted");
        }
    }

    #[test]
    fn system_step_from_system_chunk() {
        let chunk = SystemChunk {
            chunk_id: "sys-1".into(),
            uuid: "u1".into(),
            timestamp: ts(500),
            duration_ms: None,
            content_text: "System prompt here".into(),
            metrics: ChunkMetrics::default(),
        };
        let step = build_system_step(&chunk);
        assert_eq!(step.type_name(), "system");
        if let Step::System { content, .. } = &step {
            assert_eq!(content, "System prompt here");
        }
    }

    #[test]
    fn build_steps_for_turn_mixes_chunk_types() {
        let chunks = vec![
            Chunk::System(SystemChunk {
                chunk_id: "sys-1".into(),
                uuid: "u1".into(),
                timestamp: ts(10),
                duration_ms: None,
                content_text: "system".into(),
                metrics: ChunkMetrics::default(),
            }),
            Chunk::Ai(make_ai_chunk(
                "ai-1",
                vec![SemanticStep::Text {
                    text: "hello".into(),
                    timestamp: ts(20),
                }],
                vec![],
            )),
        ];
        let chunk_map = build_chunk_map(&chunks);
        let member_ids = vec!["sys-1".to_string(), "ai-1".to_string()];
        let steps = build_steps_for_turn(&member_ids, &chunk_map);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].type_name(), "system");
        assert_eq!(steps[1].type_name(), "text");
    }

    #[test]
    fn subagent_upgrade_via_task_id() {
        let mut chunk = make_ai_chunk(
            "c1",
            vec![SemanticStep::ToolExecution {
                tool_use_id: "tu-task".into(),
                tool_name: "Agent".into(),
                timestamp: ts(100),
            }],
            vec![make_tool_exec(
                "tu-task",
                "Agent",
                ToolOutput::Text {
                    text: "result".into(),
                },
                false,
            )],
        );
        chunk.subagents.push(Process {
            session_id: "sub-sess-1".into(),
            root_task_description: Some("Do something".into()),
            spawn_ts: ts(100),
            end_ts: Some(ts(200)),
            metrics: ChunkMetrics::default(),
            team: None,
            subagent_type: Some("code-reviewer".into()),
            messages: vec![],
            main_session_impact: None,
            is_ongoing: false,
            duration_ms: Some(100_000),
            parent_task_id: Some("tu-task".into()),
            description: Some("Review code".into()),
            header_model: None,
            last_isolated_tokens: 0,
            is_shutdown_only: false,
            messages_omitted: false,
            messages_total_count: 0,
        });

        let steps = build_steps_from_ai_chunk(&chunk);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].type_name(), "subagent");
        if let Step::Subagent {
            name,
            description,
            subagent_session_id,
            ..
        } = &steps[0]
        {
            assert_eq!(name, "code-reviewer");
            assert_eq!(description.as_deref(), Some("Review code"));
            assert_eq!(subagent_session_id.as_deref(), Some("sub-sess-1"));
        }
    }
}
