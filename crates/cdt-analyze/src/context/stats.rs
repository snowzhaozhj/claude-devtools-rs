//! `compute_context_stats` —— 对单个 AI group 聚合 6 类 injection，产出
//! per-turn `ContextStats`。
//!
//! 与 TS `computeContextStats` 行为对齐：
//! - `previous_paths` 线程式去重：CLAUDE.md / mentioned-file injection 若之前
//!   已记录过同 path，则不计入 `new_injections`（但仍计入 `accumulated_*`）。
//! - 聚合 4 类 per-turn injection（tool-output / task-coordination /
//!   thinking-text / user-message）。
//! - CLAUDE.md / mentioned-file 本 port 暂时依赖外部调用方提供首轮 injection
//!   列表（`initial_claude_md_injections`）—— 真实文件 resolve 留给
//!   `port-configuration-management`。

use std::collections::HashSet;

use cdt_core::{
    AIChunk, ContextInjection, ContextStats, CountsByCategory, TokensByCategory, UserChunk,
};

use super::aggregator::{
    aggregate_task_coordination, aggregate_thinking_text, aggregate_tool_outputs,
    create_user_message_injection,
};

/// `compute_context_stats` 的输入参数。
pub struct ComputeStatsParams<'a> {
    pub ai_chunk: &'a AIChunk,
    pub ai_group_id: &'a str,
    pub user_chunk: Option<&'a UserChunk>,
    pub turn_index: u32,
    pub is_first_group: bool,
    pub previous_injections: &'a [ContextInjection],
    pub previous_paths: &'a HashSet<String>,
    /// 仅在 `is_first_group` 为 true 时会被"首次注入"的 CLAUDE.md injection
    /// 集合。后续若要落地真实扫描，由 `port-configuration-management` 填充。
    pub initial_claude_md_injections: &'a [ContextInjection],
}

/// `compute_context_stats` 的返回值。
pub struct ComputeStatsResult {
    pub stats: ContextStats,
    pub next_previous_paths: HashSet<String>,
}

/// 对单个 AI group 计算 context stats。
///
/// 空 AI group（无 tool / 无 semantic / 无 user message 且非 first group）
/// 返回零 stats —— 对齐 spec MODIFIED scenario"Empty AI group still produces
/// a zeroed stats record"。
#[must_use]
pub fn compute_context_stats(params: &ComputeStatsParams<'_>) -> ComputeStatsResult {
    let mut new_injections: Vec<ContextInjection> = Vec::new();
    let mut next_previous_paths = params.previous_paths.clone();

    // ----- CLAUDE.md：仅在 first group 时注入一次 -----
    if params.is_first_group {
        for inj in params.initial_claude_md_injections {
            if let Some(key) = inj.path_dedup_key() {
                if !next_previous_paths.insert(key.to_string()) {
                    continue;
                }
            }
            new_injections.push(inj.clone());
        }
    }

    // ----- mentioned-file：本 port 暂无 response 扫描；留空 -----

    // ----- tool-output -----
    if let Some(inj) =
        aggregate_tool_outputs(params.ai_chunk, params.turn_index, params.ai_group_id)
    {
        new_injections.push(inj);
    }

    // ----- task-coordination -----
    if let Some(inj) =
        aggregate_task_coordination(params.ai_chunk, params.turn_index, params.ai_group_id)
    {
        new_injections.push(inj);
    }

    // ----- thinking-text -----
    if let Some(inj) =
        aggregate_thinking_text(params.ai_chunk, params.turn_index, params.ai_group_id)
    {
        new_injections.push(inj);
    }

    // ----- user-message -----
    if let Some(user) = params.user_chunk {
        if let Some(inj) =
            create_user_message_injection(user, params.turn_index, params.ai_group_id)
        {
            new_injections.push(inj);
        }
    }

    // ----- 累计 -----
    let mut accumulated: Vec<ContextInjection> = params.previous_injections.to_vec();
    accumulated.extend(new_injections.iter().cloned());

    let new_counts = count_by_category(&new_injections);
    let accumulated_counts = count_by_category(&accumulated);
    let tokens_by_category = sum_tokens_by_category(&accumulated);
    let total_estimated_tokens = tokens_by_category.total();

    ComputeStatsResult {
        stats: ContextStats {
            new_injections,
            accumulated_injections: accumulated,
            total_estimated_tokens,
            tokens_by_category,
            new_counts,
            accumulated_counts,
            phase_number: None,
        },
        next_previous_paths,
    }
}

fn count_by_category(list: &[ContextInjection]) -> CountsByCategory {
    let mut c = CountsByCategory::default();
    for inj in list {
        match inj {
            ContextInjection::ClaudeMd(_) => c.claude_md += 1,
            ContextInjection::MentionedFile(_) => c.mentioned_file += 1,
            ContextInjection::ToolOutput(_) => c.tool_output += 1,
            ContextInjection::ThinkingText(_) => c.thinking_text += 1,
            ContextInjection::TaskCoordination(_) => c.task_coordination += 1,
            ContextInjection::UserMessage(_) => c.user_messages += 1,
        }
    }
    c
}

fn sum_tokens_by_category(list: &[ContextInjection]) -> TokensByCategory {
    let mut t = TokensByCategory::default();
    for inj in list {
        let tokens = inj.estimated_tokens();
        match inj {
            ContextInjection::ClaudeMd(_) => t.claude_md += tokens,
            ContextInjection::MentionedFile(_) => t.mentioned_file += tokens,
            ContextInjection::ToolOutput(_) => t.tool_output += tokens,
            ContextInjection::ThinkingText(_) => t.thinking_text += tokens,
            ContextInjection::TaskCoordination(_) => t.task_coordination += tokens,
            ContextInjection::UserMessage(_) => t.user_messages += tokens,
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{
        AssistantResponse, ChunkMetrics, ClaudeMdContextInjection, ClaudeMdScope, MessageContent,
        ToolExecution, ToolOutput,
    };
    use chrono::{DateTime, Utc};

    fn ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn empty_ai() -> AIChunk {
        AIChunk {
            timestamp: ts(),
            duration_ms: None,
            responses: Vec::<AssistantResponse>::new(),
            metrics: ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            slash_commands: Vec::new(),
        }
    }

    #[test]
    fn empty_ai_group_produces_zeroed_stats() {
        let ai = empty_ai();
        let prev_paths = HashSet::new();
        let result = compute_context_stats(&ComputeStatsParams {
            ai_chunk: &ai,
            ai_group_id: "ai-0",
            user_chunk: None,
            turn_index: 0,
            is_first_group: false,
            previous_injections: &[],
            previous_paths: &prev_paths,
            initial_claude_md_injections: &[],
        });
        assert_eq!(result.stats.total_estimated_tokens, 0);
        assert!(result.stats.new_injections.is_empty());
        assert_eq!(result.stats.tokens_by_category, TokensByCategory::default());
    }

    #[test]
    fn single_tool_and_user_message_populate_stats() {
        let mut ai = empty_ai();
        ai.tool_executions.push(ToolExecution {
            tool_use_id: "tu1".into(),
            tool_name: "Bash".into(),
            input: serde_json::json!({"cmd":"echo hello"}),
            output: ToolOutput::Text {
                text: "hello".into(),
            },
            is_error: false,
            start_ts: ts(),
            end_ts: Some(ts()),
            source_assistant_uuid: "a1".into(),
        });
        let user = UserChunk {
            uuid: "u1".into(),
            timestamp: ts(),
            duration_ms: None,
            content: MessageContent::Text("please run echo".into()),
            metrics: ChunkMetrics::zero(),
        };

        let prev = HashSet::new();
        let result = compute_context_stats(&ComputeStatsParams {
            ai_chunk: &ai,
            ai_group_id: "ai-0",
            user_chunk: Some(&user),
            turn_index: 0,
            is_first_group: true,
            previous_injections: &[],
            previous_paths: &prev,
            initial_claude_md_injections: &[],
        });

        assert_eq!(result.stats.new_injections.len(), 2);
        assert!(result.stats.total_estimated_tokens > 0);
        assert_eq!(
            result.stats.total_estimated_tokens,
            result.stats.tokens_by_category.tool_output
                + result.stats.tokens_by_category.user_messages
        );
    }

    #[test]
    fn claude_md_dedup_via_previous_paths() {
        let ai = empty_ai();
        let cm = ContextInjection::ClaudeMd(ClaudeMdContextInjection {
            id: "cm-1".into(),
            path: "/repo/CLAUDE.md".into(),
            display_name: "CLAUDE.md".into(),
            scope: ClaudeMdScope::Project,
            estimated_tokens: 50,
            first_seen_turn_index: 0,
        });

        let prev_empty = HashSet::new();
        let first = compute_context_stats(&ComputeStatsParams {
            ai_chunk: &ai,
            ai_group_id: "ai-0",
            user_chunk: None,
            turn_index: 0,
            is_first_group: true,
            previous_injections: &[],
            previous_paths: &prev_empty,
            initial_claude_md_injections: std::slice::from_ref(&cm),
        });
        assert_eq!(first.stats.new_injections.len(), 1);

        // second group: 同一路径必须通过 previous_paths 去重
        let second = compute_context_stats(&ComputeStatsParams {
            ai_chunk: &ai,
            ai_group_id: "ai-1",
            user_chunk: None,
            turn_index: 1,
            is_first_group: true,
            previous_injections: &first.stats.accumulated_injections,
            previous_paths: &first.next_previous_paths,
            initial_claude_md_injections: std::slice::from_ref(&cm),
        });
        assert!(
            second.stats.new_injections.is_empty(),
            "重复路径不应再次产生 new injection"
        );
    }
}
