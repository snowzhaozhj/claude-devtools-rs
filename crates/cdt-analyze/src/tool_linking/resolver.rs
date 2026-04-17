//! 三阶段 Task → subagent 回退匹配。
//!
//! Spec：`openspec/specs/tool-execution-linking/spec.md` §"Resolve Task subagents
//! with three-phase fallback matching"。
//!
//! 纯函数，输入为已预过滤的 `SubagentCandidate` 列表（装载逻辑不属本 capability）。

use cdt_core::{
    MainSessionImpact, Process, SubagentCandidate, ToolCall, ToolExecution, ToolOutput,
    estimate_content_tokens, estimate_tokens,
};

use super::{Resolution, ResolvedTask};

/// description-based 匹配的时间窗，取自 TS `SubagentResolver.ts:207-309`。
pub const TIME_WINDOW_SECS: i64 = 60;

pub fn resolve_subagents(
    task_calls: &[ToolCall],
    executions: &[ToolExecution],
    candidates: &[SubagentCandidate],
) -> Vec<ResolvedTask> {
    let mut results: Vec<ResolvedTask> = task_calls
        .iter()
        .map(|t| ResolvedTask {
            task_use_id: t.id.clone(),
            resolution: Resolution::Orphan,
        })
        .collect();

    let mut used: Vec<bool> = vec![false; candidates.len()];

    // Phase 1: result-based
    for (i, task) in task_calls.iter().enumerate() {
        let Some(exec) = executions.iter().find(|e| e.tool_use_id == task.id) else {
            continue;
        };
        let Some(session_id) = extract_session_id(exec) else {
            continue;
        };
        if let Some((c_idx, cand)) = candidates
            .iter()
            .enumerate()
            .find(|(ci, c)| !used[*ci] && c.session_id == session_id)
        {
            used[c_idx] = true;
            results[i].resolution = Resolution::ResultBased {
                process: candidate_to_process(cand, task, Some(exec)),
            };
        }
    }

    // Phase 2: description-based
    // 对未 resolve 的 task，看能否在时间窗 + 唯一 description 匹配下落到某 candidate
    let task_ts_by_id: std::collections::HashMap<&str, chrono::DateTime<chrono::Utc>> = executions
        .iter()
        .map(|e| (e.tool_use_id.as_str(), e.start_ts))
        .collect();

    for (i, task) in task_calls.iter().enumerate() {
        if !results[i].resolution.is_orphan() {
            continue;
        }
        let Some(desc) = task.task_description.as_deref() else {
            continue;
        };
        let Some(task_ts) = task_ts_by_id.get(task.id.as_str()).copied() else {
            continue;
        };
        let mut matches: Vec<usize> = Vec::new();
        for (ci, cand) in candidates.iter().enumerate() {
            if used[ci] {
                continue;
            }
            let Some(hint) = cand.description_hint.as_deref() else {
                continue;
            };
            if !description_matches(desc, hint) {
                continue;
            }
            if !within_window(task_ts, cand.spawn_ts) {
                continue;
            }
            matches.push(ci);
        }
        if matches.len() == 1 {
            let ci = matches[0];
            used[ci] = true;
            let exec = executions.iter().find(|e| e.tool_use_id == task.id);
            results[i].resolution = Resolution::DescriptionBased {
                process: candidate_to_process(&candidates[ci], task, exec),
            };
        }
    }

    // Phase 3: positional
    let unresolved_task_indices: Vec<usize> = results
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.resolution.is_orphan().then_some(i))
        .collect();
    let unresolved_cand_indices: Vec<usize> = used
        .iter()
        .enumerate()
        .filter_map(|(i, u)| (!*u).then_some(i))
        .collect();

    if !unresolved_task_indices.is_empty()
        && unresolved_task_indices.len() == unresolved_cand_indices.len()
    {
        // 按 Task 顺序 ↔ candidate spawn 顺序
        let mut cand_sorted = unresolved_cand_indices.clone();
        cand_sorted.sort_by_key(|&ci| candidates[ci].spawn_ts);

        for (task_pos, &task_idx) in unresolved_task_indices.iter().enumerate() {
            let ci = cand_sorted[task_pos];
            used[ci] = true;
            let task = &task_calls[task_idx];
            let exec = executions.iter().find(|e| e.tool_use_id == task.id);
            results[task_idx].resolution = Resolution::Positional {
                process: candidate_to_process(&candidates[ci], task, exec),
            };
        }
    }

    results
}

fn extract_session_id(exec: &ToolExecution) -> Option<String> {
    // Phase 1 优先：JSONL 顶层 toolUseResult.agentId（由 cdt-parse 保留）
    if let Some(id) = exec.result_agent_id.as_deref() {
        if !id.is_empty() {
            return Some(id.to_owned());
        }
    }
    match &exec.output {
        ToolOutput::Structured { value } => {
            // 直接对象格式：{"session_id": "xxx"}
            if let Some(id) = value.get("session_id").and_then(|v| v.as_str()) {
                return Some(id.to_owned());
            }
            // teammate 格式：{"teammate_spawned": {"session_id": "xxx"}}
            if let Some(id) = value
                .get("teammate_spawned")
                .and_then(|v| v.get("session_id").and_then(|s| s.as_str()))
            {
                return Some(id.to_owned());
            }
            // Agent tool result 格式：[{"text": "...agentId: xxx..."}]
            // 也可能是 [{"type":"text","text":"..."}] 数组
            let text = extract_text_from_structured(value)?;
            extract_agent_id_from_text(&text)
        }
        ToolOutput::Text { text } => extract_agent_id_from_text(text),
        ToolOutput::Missing => None,
    }
}

/// 从结构化 value 中提取文本内容。
/// 支持：数组 `[{"text":"..."}]` / `[{"type":"text","text":"..."}]`
fn extract_text_from_structured(value: &serde_json::Value) -> Option<String> {
    if let Some(arr) = value.as_array() {
        for item in arr {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                return Some(text.to_owned());
            }
        }
    }
    None
}

/// 从文本中提取 `agentId: <id>` 格式的 session ID。
fn extract_agent_id_from_text(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("agentId:") {
            let id = rest.split_whitespace().next()?;
            if !id.is_empty() {
                return Some(id.to_owned());
            }
        }
    }
    None
}

fn description_matches(task_desc: &str, candidate_hint: &str) -> bool {
    let a = normalize(task_desc);
    let b = normalize(candidate_hint);
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || a.starts_with(&b) || b.starts_with(&a)
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

fn within_window(
    task_ts: chrono::DateTime<chrono::Utc>,
    cand_ts: chrono::DateTime<chrono::Utc>,
) -> bool {
    (task_ts - cand_ts).num_seconds().abs() <= TIME_WINDOW_SECS
}

fn candidate_to_process(
    cand: &SubagentCandidate,
    task: &ToolCall,
    exec: Option<&ToolExecution>,
) -> Process {
    Process {
        session_id: cand.session_id.clone(),
        root_task_description: task.task_description.clone(),
        spawn_ts: cand.spawn_ts,
        end_ts: cand.end_ts,
        metrics: cand.metrics.clone(),
        team: None,
        subagent_type: extract_subagent_type_from_task_input(task),
        messages: cand.messages.clone(),
        main_session_impact: aggregate_main_session_impact(exec),
        is_ongoing: compute_is_ongoing(cand),
        duration_ms: compute_duration_ms(cand),
        parent_task_id: Some(task.id.clone()),
        description: task.task_description.clone(),
    }
}

/// 从 Task `tool_call` 中抽取 `subagent_type`，优先走预解析字段。
pub(crate) fn extract_subagent_type_from_task_input(task: &ToolCall) -> Option<String> {
    if let Some(t) = task.task_subagent_type.clone() {
        return Some(t);
    }
    task.input
        .get("subagent_type")
        .and_then(|v| v.as_str())
        .map(std::string::ToString::to_string)
}

/// 估算 subagent 在父 session 中的 token 开销：call tokens + result tokens。
///
/// 对齐原版 `aiGroupHelpers.ts::attachMainSessionImpact`：`callTokens` 来自
/// Task `tool_use.input` 的 token 估算，`resultTokens` 来自 `tool_result.content`
/// 的 token 估算。若 exec 缺失则返回 `None`。
pub(crate) fn aggregate_main_session_impact(
    exec: Option<&ToolExecution>,
) -> Option<MainSessionImpact> {
    let exec = exec?;
    let call_tokens = estimate_content_tokens(&exec.input) as u64;
    let result_tokens = match &exec.output {
        ToolOutput::Text { text } => estimate_tokens(text) as u64,
        ToolOutput::Structured { value } => estimate_content_tokens(value) as u64,
        ToolOutput::Missing => 0,
    };
    let total = call_tokens.saturating_add(result_tokens);
    if total == 0 {
        return None;
    }
    Some(MainSessionImpact {
        total_tokens: total,
    })
}

/// 根据 `spawn_ts` / `end_ts` 计算 subagent 持续时长（毫秒）。
pub(crate) fn compute_duration_ms(cand: &SubagentCandidate) -> Option<u64> {
    let end = cand.end_ts?;
    let ms = (end - cand.spawn_ts).num_milliseconds();
    u64::try_from(ms).ok()
}

/// 是否仍在运行：优先读 candidate 自带的 `is_ongoing`；否则按 `end_ts` 缺失判定。
pub(crate) fn compute_is_ongoing(cand: &SubagentCandidate) -> bool {
    cand.is_ongoing || cand.end_ts.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::ChunkMetrics;
    use chrono::{DateTime, Duration, Utc};

    fn ts(n: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::seconds(n)
    }

    fn task(id: &str, desc: &str) -> ToolCall {
        ToolCall {
            id: id.into(),
            name: "Task".into(),
            input: serde_json::json!({"description": desc}),
            is_task: true,
            task_description: Some(desc.into()),
            task_subagent_type: None,
        }
    }

    fn exec(id: &str, n: i64, output: ToolOutput) -> ToolExecution {
        ToolExecution {
            tool_use_id: id.into(),
            tool_name: "Task".into(),
            input: serde_json::json!({}),
            output,
            is_error: false,
            start_ts: ts(n),
            end_ts: Some(ts(n + 1)),
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
        }
    }

    fn cand(session: &str, hint: &str, n: i64) -> SubagentCandidate {
        SubagentCandidate {
            session_id: session.into(),
            description_hint: Some(hint.into()),
            spawn_ts: ts(n),
            end_ts: None,
            parent_session_id: Some("parent".into()),
            metrics: ChunkMetrics::zero(),
            messages: Vec::new(),
            is_ongoing: false,
        }
    }

    #[test]
    fn phase1_result_based_session_id_links() {
        let tasks = vec![task("t1", "investigate logs")];
        let execs = vec![exec(
            "t1",
            5,
            ToolOutput::Structured {
                value: serde_json::json!({"session_id": "s-99"}),
            },
        )];
        let cands = vec![
            cand("s-11", "other", 4),
            cand("s-99", "investigate logs", 6),
        ];
        let r = resolve_subagents(&tasks, &execs, &cands);
        assert!(matches!(
            r[0].resolution,
            Resolution::ResultBased { ref process } if process.session_id == "s-99"
        ));
    }

    #[test]
    fn phase1_prefers_result_agent_id_over_output() {
        let tasks = vec![task("t1", "anything")];
        let mut exec = exec("t1", 5, ToolOutput::Missing);
        exec.result_agent_id = Some("s-top".into());
        let execs = vec![exec];
        let cands = vec![cand("s-top", "other", 6)];
        let r = resolve_subagents(&tasks, &execs, &cands);
        assert!(matches!(
            r[0].resolution,
            Resolution::ResultBased { ref process } if process.session_id == "s-top"
        ));
    }

    #[test]
    fn phase1_teammate_spawned_nested_session_id_links() {
        let tasks = vec![task("t1", "handle it")];
        let execs = vec![exec(
            "t1",
            5,
            ToolOutput::Structured {
                value: serde_json::json!({"teammate_spawned": {"session_id": "s-77"}}),
            },
        )];
        let cands = vec![cand("s-77", "handle it", 6)];
        let r = resolve_subagents(&tasks, &execs, &cands);
        assert!(matches!(r[0].resolution, Resolution::ResultBased { .. }));
    }

    #[test]
    fn phase2_description_based_unique_match() {
        let tasks = vec![task("t1", "investigate logs")];
        let execs = vec![exec("t1", 5, ToolOutput::Missing)];
        let cands = vec![
            cand("s-1", "investigate logs", 10),
            cand("s-2", "compile metrics", 12),
        ];
        let r = resolve_subagents(&tasks, &execs, &cands);
        assert!(matches!(
            r[0].resolution,
            Resolution::DescriptionBased { ref process } if process.session_id == "s-1"
        ));
    }

    #[test]
    fn phase3_positional_when_counts_match() {
        let tasks = vec![task("t1", "one"), task("t2", "two")];
        let execs = vec![
            exec("t1", 1, ToolOutput::Missing),
            exec("t2", 2, ToolOutput::Missing),
        ];
        // hints 不匹配任何 task desc → phase 2 跳过，落到 phase 3
        let cands = vec![cand("s-a", "aaa", 1), cand("s-b", "bbb", 2)];
        let r = resolve_subagents(&tasks, &execs, &cands);
        assert!(matches!(
            r[0].resolution,
            Resolution::Positional { ref process } if process.session_id == "s-a"
        ));
        assert!(matches!(
            r[1].resolution,
            Resolution::Positional { ref process } if process.session_id == "s-b"
        ));
    }

    #[test]
    fn unrelated_candidate_counts_dont_positional_match() {
        let tasks = vec![task("t1", "one")];
        let execs = vec![exec("t1", 1, ToolOutput::Missing)];
        // 两个 candidate（比如属于别的 parent），数量不等 → 不触发 positional
        let cands = vec![cand("s-a", "aaa", 1), cand("s-b", "bbb", 2)];
        let r = resolve_subagents(&tasks, &execs, &cands);
        assert!(r[0].resolution.is_orphan());
    }

    #[test]
    fn all_phases_fail_returns_orphan() {
        let tasks = vec![task("t1", "orphan task")];
        let execs = vec![exec("t1", 1, ToolOutput::Missing)];
        let cands: Vec<SubagentCandidate> = Vec::new();
        let r = resolve_subagents(&tasks, &execs, &cands);
        assert!(r[0].resolution.is_orphan());
    }

    fn take_process(res: &Resolution) -> &Process {
        match res {
            Resolution::ResultBased { process }
            | Resolution::DescriptionBased { process }
            | Resolution::Positional { process } => process,
            Resolution::Orphan => panic!("orphan"),
        }
    }

    #[test]
    fn subagent_type_populated_from_task_input() {
        let mut t = task("t1", "review");
        t.task_subagent_type = Some("code-reviewer".into());
        let tasks = vec![t];
        let execs = vec![exec(
            "t1",
            5,
            ToolOutput::Structured {
                value: serde_json::json!({"session_id": "s-1"}),
            },
        )];
        let cands = vec![cand("s-1", "review", 6)];
        let r = resolve_subagents(&tasks, &execs, &cands);
        let p = take_process(&r[0].resolution);
        assert_eq!(p.subagent_type.as_deref(), Some("code-reviewer"));
    }

    #[test]
    fn subagent_type_falls_back_to_input_field() {
        let t = ToolCall {
            id: "t1".into(),
            name: "Task".into(),
            input: serde_json::json!({"description": "x", "subagent_type": "deep-explorer"}),
            is_task: true,
            task_description: Some("x".into()),
            task_subagent_type: None,
        };
        let tasks = vec![t];
        let execs = vec![exec(
            "t1",
            5,
            ToolOutput::Structured {
                value: serde_json::json!({"session_id": "s-1"}),
            },
        )];
        let cands = vec![cand("s-1", "x", 6)];
        let r = resolve_subagents(&tasks, &execs, &cands);
        let p = take_process(&r[0].resolution);
        assert_eq!(p.subagent_type.as_deref(), Some("deep-explorer"));
    }

    #[test]
    fn parent_task_id_backfilled_on_match() {
        let tasks = vec![task("t-abc", "desc")];
        let execs = vec![exec(
            "t-abc",
            5,
            ToolOutput::Structured {
                value: serde_json::json!({"session_id": "s-1"}),
            },
        )];
        let cands = vec![cand("s-1", "desc", 6)];
        let r = resolve_subagents(&tasks, &execs, &cands);
        let p = take_process(&r[0].resolution);
        assert_eq!(p.parent_task_id.as_deref(), Some("t-abc"));
    }

    #[test]
    fn duration_ms_computed_from_spawn_and_end() {
        let mut c = cand("s-1", "desc", 10);
        c.end_ts = Some(ts(15)); // +5s
        let tasks = vec![task("t1", "desc")];
        let execs = vec![exec(
            "t1",
            5,
            ToolOutput::Structured {
                value: serde_json::json!({"session_id": "s-1"}),
            },
        )];
        let r = resolve_subagents(&tasks, &execs, &[c]);
        let p = take_process(&r[0].resolution);
        assert_eq!(p.duration_ms, Some(5_000));
        assert!(!p.is_ongoing);
    }

    #[test]
    fn is_ongoing_true_when_no_end_ts() {
        let tasks = vec![task("t1", "desc")];
        let execs = vec![exec(
            "t1",
            5,
            ToolOutput::Structured {
                value: serde_json::json!({"session_id": "s-1"}),
            },
        )];
        let cands = vec![cand("s-1", "desc", 6)]; // end_ts = None
        let r = resolve_subagents(&tasks, &execs, &cands);
        let p = take_process(&r[0].resolution);
        assert!(p.is_ongoing);
        assert_eq!(p.duration_ms, None);
    }

    #[test]
    fn main_session_impact_aggregates_task_result_tokens() {
        // 构造一个带输出文本的 exec，让 estimate 能产出非零 token 数
        let big_text = "x".repeat(200); // 50 tokens 左右
        let tasks = vec![task("t1", "desc")];
        let execs = vec![exec(
            "t1",
            5,
            ToolOutput::Text {
                text: big_text.clone(),
            },
        )];
        let cands = vec![cand("s-1", "desc", 6)];
        // phase1 无 session id → fallback phase2（desc 匹配）
        let r = resolve_subagents(&tasks, &execs, &cands);
        let p = take_process(&r[0].resolution);
        let impact = p
            .main_session_impact
            .expect("main_session_impact should be populated when exec has tokens");
        assert!(impact.total_tokens > 0);
    }

    #[test]
    fn messages_from_candidate_flow_into_process() {
        use cdt_core::{AIChunk, Chunk, ChunkMetrics};
        use chrono::TimeZone;
        let chunk = Chunk::Ai(AIChunk {
            timestamp: chrono::Utc.timestamp_opt(0, 0).unwrap(),
            duration_ms: None,
            responses: vec![],
            metrics: ChunkMetrics::zero(),
            semantic_steps: vec![],
            tool_executions: vec![],
            subagents: vec![],
            slash_commands: vec![],
        });
        let mut c = cand("s-1", "desc", 6);
        c.messages = vec![chunk.clone()];
        let tasks = vec![task("t1", "desc")];
        let execs = vec![exec(
            "t1",
            5,
            ToolOutput::Structured {
                value: serde_json::json!({"session_id": "s-1"}),
            },
        )];
        let r = resolve_subagents(&tasks, &execs, &[c]);
        let p = take_process(&r[0].resolution);
        assert_eq!(p.messages.len(), 1);
    }
}
