//! 会话结构化诊断摘要。
//!
//! 纯算法——时间分段、error density、tool usage、idle gaps 等。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use cdt_core::Chunk;

use crate::cost;

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummaryOutput {
    pub session_id: String,
    pub total_duration_ms: i64,
    pub message_count: usize,
    pub phases: Vec<Phase>,
    pub tool_usage: Vec<ToolUsageStat>,
    pub top_files: Vec<FileTouch>,
    pub error_count: usize,
    pub compaction_count: usize,
    pub idle_gaps: Vec<IdleGap>,
    pub cost: cost::SessionCost,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Phase {
    pub index: usize,
    pub start_ts: DateTime<Utc>,
    pub end_ts: DateTime<Utc>,
    pub duration_ms: i64,
    pub chunk_count: usize,
    pub tool_count: u64,
    pub error_count: usize,
    pub top_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsageStat {
    pub name: String,
    pub count: u64,
    pub error_count: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTouch {
    pub path: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdleGap {
    pub after_ts: DateTime<Utc>,
    pub before_ts: DateTime<Utc>,
    pub gap_ms: i64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Algorithm
// ─────────────────────────────────────────────────────────────────────────────

const PHASE_GAP_MS: i64 = 300_000; // 5 min gap => new phase
const IDLE_GAP_THRESHOLD_MS: i64 = 120_000; // 2 min => idle gap

pub fn build_summary(detail: &cdt_api::SessionDetail) -> SessionSummaryOutput {
    let chunks = &detail.chunks;
    let message_count = detail.metrics.message_count;

    let (total_duration_ms, phases) = segment_phases(chunks);
    let tool_usage = compute_tool_usage(chunks);
    let top_files = compute_top_files(chunks);
    let error_count = count_errors(chunks);
    let compaction_count = chunks
        .iter()
        .filter(|c| matches!(c, Chunk::Compact(_)))
        .count();
    let idle_gaps = find_idle_gaps(chunks);
    let session_cost = cost::compute_session_cost(detail);

    SessionSummaryOutput {
        session_id: detail.session_id.clone(),
        total_duration_ms,
        message_count,
        phases,
        tool_usage,
        top_files,
        error_count,
        compaction_count,
        idle_gaps,
        cost: session_cost,
    }
}

fn segment_phases(chunks: &[Chunk]) -> (i64, Vec<Phase>) {
    if chunks.is_empty() {
        return (0, Vec::new());
    }

    let first_ts = chunks[0].timestamp();
    let last_ts = chunks.last().map_or(first_ts, Chunk::timestamp);
    let total_duration_ms = (last_ts - first_ts).num_milliseconds();

    let mut phases: Vec<Phase> = Vec::new();
    let mut phase_start_idx = 0;
    let mut prev_ts = first_ts;

    for (i, chunk) in chunks.iter().enumerate() {
        let ts = chunk.timestamp();
        let gap = (ts - prev_ts).num_milliseconds();

        if gap > PHASE_GAP_MS && i > phase_start_idx {
            phases.push(build_phase(phases.len(), &chunks[phase_start_idx..i]));
            phase_start_idx = i;
        }
        prev_ts = ts;
    }

    if phase_start_idx < chunks.len() {
        phases.push(build_phase(phases.len(), &chunks[phase_start_idx..]));
    }

    (total_duration_ms, phases)
}

fn build_phase(index: usize, chunks: &[Chunk]) -> Phase {
    let start_ts = chunks[0].timestamp();
    let end_ts = chunks.last().map_or(start_ts, Chunk::timestamp);
    let duration_ms = (end_ts - start_ts).num_milliseconds();

    let mut tool_count: u64 = 0;
    let mut error_count: usize = 0;
    let mut tool_names: HashMap<&str, u64> = HashMap::new();

    for chunk in chunks {
        tool_count += chunk.metrics().tool_count;
        if let Chunk::Ai(ai) = chunk {
            for exec in &ai.tool_executions {
                *tool_names.entry(&exec.tool_name).or_default() += 1;
                if exec.is_error {
                    error_count += 1;
                }
            }
        }
    }

    let mut top_tools: Vec<(&str, u64)> = tool_names.into_iter().collect();
    top_tools.sort_by_key(|t| std::cmp::Reverse(t.1));
    let top_tools: Vec<String> = top_tools
        .into_iter()
        .take(5)
        .map(|(n, _)| n.to_string())
        .collect();

    Phase {
        index,
        start_ts,
        end_ts,
        duration_ms,
        chunk_count: chunks.len(),
        tool_count,
        error_count,
        top_tools,
    }
}

fn compute_tool_usage(chunks: &[Chunk]) -> Vec<ToolUsageStat> {
    let mut map: HashMap<String, (u64, u64)> = HashMap::new();

    for chunk in chunks {
        if let Chunk::Ai(ai) = chunk {
            for exec in &ai.tool_executions {
                let entry = map.entry(exec.tool_name.clone()).or_default();
                entry.0 += 1;
                if exec.is_error {
                    entry.1 += 1;
                }
            }
        }
    }

    let mut stats: Vec<ToolUsageStat> = map
        .into_iter()
        .map(|(name, (count, error_count))| {
            #[allow(clippy::cast_precision_loss)]
            let success_rate = if count > 0 {
                (count - error_count) as f64 / count as f64
            } else {
                1.0
            };
            ToolUsageStat {
                name,
                count,
                error_count,
                success_rate,
            }
        })
        .collect();

    stats.sort_by_key(|s| std::cmp::Reverse(s.count));
    stats
}

fn compute_top_files(chunks: &[Chunk]) -> Vec<FileTouch> {
    let mut map: HashMap<String, u64> = HashMap::new();

    for chunk in chunks {
        if let Chunk::Ai(ai) = chunk {
            for exec in &ai.tool_executions {
                if let Some(path) = extract_file_path(&exec.tool_name, &exec.input) {
                    *map.entry(path).or_default() += 1;
                }
            }
        }
    }

    let mut files: Vec<FileTouch> = map
        .into_iter()
        .map(|(path, count)| FileTouch { path, count })
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.count));
    files.truncate(10);
    files
}

fn extract_file_path(tool_name: &str, input: &serde_json::Value) -> Option<String> {
    match tool_name {
        "Read" | "Write" | "Edit" => input
            .get("file_path")
            .or_else(|| input.get("path"))
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string),
        _ => None,
    }
}

fn count_errors(chunks: &[Chunk]) -> usize {
    let mut count = 0;
    for chunk in chunks {
        if let Chunk::Ai(ai) = chunk {
            for exec in &ai.tool_executions {
                if exec.is_error {
                    count += 1;
                }
            }
        }
    }
    count
}

fn find_idle_gaps(chunks: &[Chunk]) -> Vec<IdleGap> {
    let mut gaps = Vec::new();
    let mut prev_ts: Option<DateTime<Utc>> = None;

    for chunk in chunks {
        let ts = chunk.timestamp();
        if let Some(prev) = prev_ts {
            let gap_ms = (ts - prev).num_milliseconds();
            if gap_ms >= IDLE_GAP_THRESHOLD_MS {
                gaps.push(IdleGap {
                    after_ts: prev,
                    before_ts: ts,
                    gap_ms,
                });
            }
        }
        prev_ts = Some(ts);
    }

    gaps
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::message::MessageContent;
    use cdt_core::{AIChunk, ChunkMetrics, UserChunk, chunk::AssistantResponse};
    use chrono::TimeZone;

    fn ts(minutes: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::minutes(minutes)
    }

    fn make_ai_chunk(minute: i64, tool_count: u64) -> Chunk {
        Chunk::Ai(AIChunk {
            chunk_id: format!("ai-{minute}"),
            timestamp: ts(minute),
            duration_ms: Some(1000),
            responses: vec![AssistantResponse {
                uuid: format!("resp-{minute}"),
                timestamp: ts(minute),
                content: MessageContent::Text("test".into()),
                tool_calls: vec![],
                usage: None,
                model: Some("claude-sonnet-4-6".into()),
                content_omitted: false,
            }],
            metrics: ChunkMetrics {
                tool_count,
                ..Default::default()
            },
            semantic_steps: vec![],
            tool_executions: vec![],
            subagents: vec![],
            slash_commands: vec![],
            teammate_messages: vec![],
        })
    }

    fn make_user_chunk(minute: i64) -> Chunk {
        Chunk::User(UserChunk {
            chunk_id: format!("user-{minute}"),
            uuid: format!("u-{minute}"),
            timestamp: ts(minute),
            duration_ms: None,
            content: MessageContent::Text("hello".into()),
            metrics: ChunkMetrics::default(),
        })
    }

    #[test]
    fn empty_chunks_no_phases() {
        let (dur, phases) = segment_phases(&[]);
        assert_eq!(dur, 0);
        assert!(phases.is_empty());
    }

    #[test]
    fn continuous_work_single_phase() {
        let chunks = vec![
            make_user_chunk(0),
            make_ai_chunk(1, 2),
            make_user_chunk(3),
            make_ai_chunk(4, 1),
        ];
        let (_, phases) = segment_phases(&chunks);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0].chunk_count, 4);
    }

    #[test]
    fn large_gap_creates_new_phase() {
        let chunks = vec![
            make_user_chunk(0),
            make_ai_chunk(1, 2),
            make_user_chunk(10), // 9 min gap > 5 min threshold
            make_ai_chunk(11, 1),
        ];
        let (_, phases) = segment_phases(&chunks);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].chunk_count, 2);
        assert_eq!(phases[1].chunk_count, 2);
    }

    #[test]
    fn idle_gaps_detected() {
        let chunks = vec![
            make_user_chunk(0),
            make_ai_chunk(1, 0),
            make_ai_chunk(5, 0), // 4 min gap > 2 min threshold
        ];
        let gaps = find_idle_gaps(&chunks);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].gap_ms, 4 * 60_000);
    }
}
