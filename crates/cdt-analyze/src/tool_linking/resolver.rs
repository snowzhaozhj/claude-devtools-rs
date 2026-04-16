//! 三阶段 Task → subagent 回退匹配。
//!
//! Spec：`openspec/specs/tool-execution-linking/spec.md` §"Resolve Task subagents
//! with three-phase fallback matching"。
//!
//! 纯函数，输入为已预过滤的 `SubagentCandidate` 列表（装载逻辑不属本 capability）。

use cdt_core::{Process, SubagentCandidate, ToolCall, ToolExecution, ToolOutput};

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
        let Some(session_id) = extract_session_id(&exec.output) else {
            continue;
        };
        if let Some((c_idx, cand)) = candidates
            .iter()
            .enumerate()
            .find(|(ci, c)| !used[*ci] && c.session_id == session_id)
        {
            used[c_idx] = true;
            results[i].resolution = Resolution::ResultBased {
                process: candidate_to_process(cand, task),
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
            results[i].resolution = Resolution::DescriptionBased {
                process: candidate_to_process(&candidates[ci], task),
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
            results[task_idx].resolution = Resolution::Positional {
                process: candidate_to_process(&candidates[ci], &task_calls[task_idx]),
            };
        }
    }

    results
}

fn extract_session_id(output: &ToolOutput) -> Option<String> {
    match output {
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

fn candidate_to_process(cand: &SubagentCandidate, task: &ToolCall) -> Process {
    Process {
        session_id: cand.session_id.clone(),
        root_task_description: task.task_description.clone(),
        spawn_ts: cand.spawn_ts,
        end_ts: None,
        metrics: cand.metrics.clone(),
        team: None,
    }
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
        }
    }

    fn cand(session: &str, hint: &str, n: i64) -> SubagentCandidate {
        SubagentCandidate {
            session_id: session.into(),
            description_hint: Some(hint.into()),
            spawn_ts: ts(n),
            parent_session_id: Some("parent".into()),
            metrics: ChunkMetrics::zero(),
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
}
