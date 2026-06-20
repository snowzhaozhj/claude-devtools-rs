//! `promote_result_agent_tasks`：把携带 `result_agent_id` 的嵌套 `Agent` /
//! `Task` `ToolExecution` 就地升级为骨架 subagent `Process`。
//!
//! 用于 `get_subagent_trace` 路径——subagent 自身 transcript 经 `build_chunks`
//! （非 `build_chunks_with_subagents`）构建后，其内部 spawn 子 agent 的 `Agent`
//! 调用只剩一个 `ToolExecution`，前端按普通工具渲染。本后处理就地把这些调用
//! 暴露为可展开的骨架 subagent，**不读子文件、不额外 parse**——仅复用已解析的
//! `result_agent_id`，让嵌套层可逐级懒拉展开。
//!
//! Spec：`openspec/specs/chunk-building/spec.md` §"Promote nested Agent calls
//! to skeleton subagents"。

use std::collections::HashSet;

use cdt_core::{Chunk, ChunkMetrics, Process, SemanticStep, ToolExecution};

/// 骨架 `description` 截断上限（字符）——对齐 `SubagentCandidate.description_hint`
/// 的 `chars().take(200)` 口径，防一层 fan-out 大量子节点时 payload 膨胀。
const DESCRIPTION_MAX_CHARS: usize = 200;

/// 可升级为 subagent 的工具名集合——MUST 与 `cdt-parse::is_task` 及前端
/// `displayItemBuilder` 跳过判定一致（`{ "Task", "Agent" }`），否则前端无法据
/// `parent_task_id` 跳过原始工具，会同时渲染骨架卡片与普通工具。
fn is_task_tool(name: &str) -> bool {
    name == "Task" || name == "Agent"
}

/// 在一段已构建的 chunks 上，把带 `result_agent_id` 的嵌套 `Agent` / `Task`
/// `ToolExecution` 升级为骨架 subagent。纯同步、无 IO。
pub fn promote_result_agent_tasks(chunks: &mut [Chunk]) {
    for chunk in chunks.iter_mut() {
        let Chunk::Ai(ai) = chunk else { continue };

        // 第一遍：在不可变借用下决定要升级哪些 exec，生成骨架与 spawn step。
        let mut promoted_ids: HashSet<String> = HashSet::new();
        let mut new_subagents: Vec<Process> = Vec::new();
        let mut spawns: Vec<(String, SemanticStep)> = Vec::new();
        for exec in &ai.tool_executions {
            if !is_task_tool(&exec.tool_name) {
                continue;
            }
            // 空字符串 agentId 视同缺失——否则会产 session_id="" 的骨架且删掉原工具。
            let Some(agent_id) = exec.result_agent_id.as_deref().filter(|s| !s.is_empty()) else {
                continue;
            };
            // 完整 resolve 路径已为该 task 产出 subagent → 跳过，避免重复。
            if ai
                .subagents
                .iter()
                .any(|s| s.parent_task_id.as_deref() == Some(exec.tool_use_id.as_str()))
            {
                continue;
            }
            let process = skeleton_process(exec, agent_id);
            let spawn = SemanticStep::SubagentSpawn {
                placeholder_id: process.session_id.clone(),
                timestamp: exec.start_ts,
            };
            new_subagents.push(process);
            spawns.push((exec.tool_use_id.clone(), spawn));
            promoted_ids.insert(exec.tool_use_id.clone());
        }

        if promoted_ids.is_empty() {
            continue;
        }

        ai.subagents.extend(new_subagents);

        // SubagentSpawn 紧随对应 ToolExecution step 插入（相邻）；每次重新定位
        // 以吸收前一次 insert 造成的位移。找不到则 append + warn。
        for (tool_use_id, spawn) in spawns {
            let pos = ai.semantic_steps.iter().position(|s| {
                matches!(s, SemanticStep::ToolExecution { tool_use_id: t, .. } if t == &tool_use_id)
            });
            if let Some(p) = pos {
                ai.semantic_steps.insert(p + 1, spawn);
            } else {
                tracing::warn!(
                    tool_use_id = %tool_use_id,
                    "promote_result_agent_tasks: matching ToolExecution step not found, appending SubagentSpawn"
                );
                ai.semantic_steps.push(spawn);
            }
        }

        // payload 瘦身：移除被升级的 Agent/Task ToolExecution——其 output 是子 agent
        // 完整输出文本。前端靠 `parent_task_id` 跳过工具、靠 SubagentSpawn 渲染
        // subagent，移除 exec 后 `tool_execution` 分支 `if (!exec) break` 自然跳过。
        ai.tool_executions
            .retain(|e| !promoted_ids.contains(&e.tool_use_id));
    }
}

/// 由 `ToolExecution` 合成骨架 `Process`——只填 IPC / 前端渲染必需的最小字段，
/// `messages` 留空且 `messages_omitted=true`，由消费方首次展开懒拉。
fn skeleton_process(exec: &ToolExecution, agent_id: &str) -> Process {
    let subagent_type = exec
        .input
        .get("subagent_type")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let description = exec
        .input
        .get("description")
        .and_then(|v| v.as_str())
        .map(|d| d.chars().take(DESCRIPTION_MAX_CHARS).collect::<String>());
    Process {
        session_id: agent_id.to_owned(),
        root_task_description: None,
        spawn_ts: exec.start_ts,
        end_ts: None,
        metrics: ChunkMetrics::zero(),
        team: None,
        subagent_type,
        messages: Vec::new(),
        main_session_impact: None,
        // 已知降级（design D4）：骨架不读子文件，状态以首次展开懒拉为准。
        is_ongoing: false,
        duration_ms: None,
        parent_task_id: Some(exec.tool_use_id.clone()),
        description,
        header_model: None,
        last_isolated_tokens: 0,
        is_shutdown_only: false,
        messages_omitted: true,
        messages_total_count: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{AIChunk, ToolOutput};
    use chrono::{TimeZone, Utc};

    fn exec(
        tool_name: &str,
        id: &str,
        agent_id: Option<&str>,
        input: serde_json::Value,
    ) -> ToolExecution {
        ToolExecution {
            tool_use_id: id.into(),
            tool_name: tool_name.into(),
            input,
            output: ToolOutput::Text {
                text: "child agent full output".into(),
            },
            is_error: false,
            start_ts: Utc.with_ymd_and_hms(2026, 6, 20, 0, 0, 0).unwrap(),
            end_ts: None,
            source_assistant_uuid: "a1".into(),
            result_agent_id: agent_id.map(str::to_owned),
            error_message: None,
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
            workflow_run_id: None,
            workflow_script_path: None,
        }
    }

    fn tool_step(id: &str) -> SemanticStep {
        SemanticStep::ToolExecution {
            tool_use_id: id.into(),
            tool_name: "Agent".into(),
            timestamp: Utc.with_ymd_and_hms(2026, 6, 20, 0, 0, 0).unwrap(),
        }
    }

    fn ai_chunk(execs: Vec<ToolExecution>, steps: Vec<SemanticStep>) -> Chunk {
        Chunk::Ai(AIChunk {
            chunk_id: "c0".into(),
            timestamp: Utc.with_ymd_and_hms(2026, 6, 20, 0, 0, 0).unwrap(),
            duration_ms: None,
            responses: Vec::new(),
            metrics: ChunkMetrics::zero(),
            semantic_steps: steps,
            tool_executions: execs,
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        })
    }

    #[test]
    fn agent_call_with_result_agent_id_promoted_to_skeleton_subagent() {
        let e = exec(
            "Agent",
            "toolu_1",
            Some("sub-x"),
            serde_json::json!({ "subagent_type": "Explore", "description": "scan code" }),
        );
        let mut chunks = vec![ai_chunk(vec![e], vec![tool_step("toolu_1")])];
        promote_result_agent_tasks(&mut chunks);

        let Chunk::Ai(ai) = &chunks[0] else { panic!() };
        assert_eq!(ai.subagents.len(), 1);
        let p = &ai.subagents[0];
        assert_eq!(p.session_id, "sub-x");
        assert_eq!(p.parent_task_id.as_deref(), Some("toolu_1"));
        assert_eq!(p.subagent_type.as_deref(), Some("Explore"));
        assert_eq!(p.description.as_deref(), Some("scan code"));
        assert!(p.messages_omitted);
        assert!(!p.is_ongoing);
        assert_eq!(p.messages_total_count, 0);
        // 升级后该 Agent ToolExecution 被移除（payload 瘦身）。
        assert!(ai.tool_executions.is_empty());
    }

    #[test]
    fn subagent_spawn_inserted_right_after_matching_tool_execution_step() {
        let e = exec("Agent", "toolu_1", Some("sub-x"), serde_json::json!({}));
        let steps = vec![
            SemanticStep::Text {
                text: "before".into(),
                timestamp: Utc.with_ymd_and_hms(2026, 6, 20, 0, 0, 0).unwrap(),
            },
            tool_step("toolu_1"),
            SemanticStep::Text {
                text: "after".into(),
                timestamp: Utc.with_ymd_and_hms(2026, 6, 20, 0, 0, 0).unwrap(),
            },
        ];
        let mut chunks = vec![ai_chunk(vec![e], steps)];
        promote_result_agent_tasks(&mut chunks);

        let Chunk::Ai(ai) = &chunks[0] else { panic!() };
        // 顺序：Text(before) → ToolExecution → SubagentSpawn → Text(after)
        assert!(matches!(
            ai.semantic_steps[1],
            SemanticStep::ToolExecution { .. }
        ));
        match &ai.semantic_steps[2] {
            SemanticStep::SubagentSpawn { placeholder_id, .. } => {
                assert_eq!(placeholder_id, "sub-x");
            }
            other => panic!("expected SubagentSpawn, got {other:?}"),
        }
        assert!(matches!(ai.semantic_steps[3], SemanticStep::Text { .. }));
    }

    #[test]
    fn tool_without_result_agent_id_not_promoted() {
        let e = exec(
            "Bash",
            "toolu_b",
            None,
            serde_json::json!({ "command": "ls" }),
        );
        let mut chunks = vec![ai_chunk(vec![e], vec![tool_step("toolu_b")])];
        promote_result_agent_tasks(&mut chunks);

        let Chunk::Ai(ai) = &chunks[0] else { panic!() };
        assert!(ai.subagents.is_empty());
        assert_eq!(ai.tool_executions.len(), 1);
    }

    #[test]
    fn agent_call_without_result_agent_id_not_promoted() {
        // 未完成 / 中断的嵌套 subagent：Agent 调用存在但 tool_result 未回填
        // agentId（result_agent_id=None）→ 无法关联子 transcript，SHALL 保持工具
        // 显示，不升级（design D4 边界：零 IO 方案只覆盖已回填 agentId 的嵌套）。
        let e = exec("Agent", "toolu_pending", None, serde_json::json!({}));
        let mut chunks = vec![ai_chunk(vec![e], vec![tool_step("toolu_pending")])];
        promote_result_agent_tasks(&mut chunks);

        let Chunk::Ai(ai) = &chunks[0] else { panic!() };
        assert!(
            ai.subagents.is_empty(),
            "无 agentId 的 Agent 调用 SHALL NOT 升级"
        );
        assert_eq!(ai.tool_executions.len(), 1, "原始 Agent 工具 SHALL 保留");
    }

    #[test]
    fn empty_result_agent_id_not_promoted() {
        // 空字符串 agentId 视同缺失：不升级、不产 session_id="" 的空骨架、不删原工具。
        let e = exec("Agent", "toolu_empty", Some(""), serde_json::json!({}));
        let mut chunks = vec![ai_chunk(vec![e], vec![tool_step("toolu_empty")])];
        promote_result_agent_tasks(&mut chunks);

        let Chunk::Ai(ai) = &chunks[0] else { panic!() };
        assert!(ai.subagents.is_empty(), "空 agentId SHALL NOT 升级");
        assert_eq!(
            ai.tool_executions.len(),
            1,
            "空 agentId 时原始工具 SHALL 保留"
        );
    }

    #[test]
    fn mixed_agents_only_promote_those_with_agent_id() {
        // 同一 chunk 多调用混合：Agent(有 id) / Bash / Agent(无 id) / Task(有 id)
        // → 只升级 a / b，顺序各自紧贴对应 ToolExecution step，未升级项保留。
        let ea = exec("Agent", "tu_a", Some("sub-a"), serde_json::json!({}));
        let eb = exec(
            "Bash",
            "tu_bash",
            None,
            serde_json::json!({ "command": "ls" }),
        );
        let ec = exec("Agent", "tu_pending", None, serde_json::json!({}));
        let ed = exec("Task", "tu_b", Some("sub-b"), serde_json::json!({}));
        let steps = vec![
            tool_step("tu_a"),
            tool_step("tu_bash"),
            tool_step("tu_pending"),
            tool_step("tu_b"),
        ];
        let mut chunks = vec![ai_chunk(vec![ea, eb, ec, ed], steps)];
        promote_result_agent_tasks(&mut chunks);

        let Chunk::Ai(ai) = &chunks[0] else { panic!() };
        // 只升级 sub-a / sub-b 两个骨架。
        let ids: Vec<&str> = ai.subagents.iter().map(|p| p.session_id.as_str()).collect();
        assert_eq!(ids, vec!["sub-a", "sub-b"]);
        // 未升级的 Bash / pending Agent 的 ToolExecution 保留；a / b 被移除。
        let kept: Vec<&str> = ai
            .tool_executions
            .iter()
            .map(|e| e.tool_use_id.as_str())
            .collect();
        assert_eq!(kept, vec!["tu_bash", "tu_pending"]);
        // 每个 SubagentSpawn 紧贴其对应 ToolExecution step。
        let pos_a = ai
            .semantic_steps
            .iter()
            .position(|s| matches!(s, SemanticStep::ToolExecution { tool_use_id, .. } if tool_use_id == "tu_a"))
            .unwrap();
        assert!(
            matches!(&ai.semantic_steps[pos_a + 1], SemanticStep::SubagentSpawn { placeholder_id, .. } if placeholder_id == "sub-a")
        );
        let pos_b = ai
            .semantic_steps
            .iter()
            .position(|s| matches!(s, SemanticStep::ToolExecution { tool_use_id, .. } if tool_use_id == "tu_b"))
            .unwrap();
        assert!(
            matches!(&ai.semantic_steps[pos_b + 1], SemanticStep::SubagentSpawn { placeholder_id, .. } if placeholder_id == "sub-b")
        );
    }

    #[test]
    fn already_resolved_task_not_duplicated() {
        let e = exec("Agent", "toolu_1", Some("sub-x"), serde_json::json!({}));
        let Chunk::Ai(mut ai) = ai_chunk(vec![e], vec![tool_step("toolu_1")]) else {
            unreachable!()
        };
        // 模拟完整 resolve 路径已产出的 subagent（同 parent_task_id）。
        let existing = skeleton_process(&ai.tool_executions[0].clone(), "sub-x");
        ai.subagents.push(existing);
        let mut chunks = vec![Chunk::Ai(ai)];
        promote_result_agent_tasks(&mut chunks);

        let Chunk::Ai(ai) = &chunks[0] else { panic!() };
        assert_eq!(
            ai.subagents.len(),
            1,
            "已有同 parent_task_id 的 subagent 不应重复升级"
        );
    }

    #[test]
    fn long_description_truncated_to_char_limit() {
        let long = "x".repeat(500);
        let e = exec(
            "Agent",
            "toolu_1",
            Some("sub-x"),
            serde_json::json!({ "description": long }),
        );
        let mut chunks = vec![ai_chunk(vec![e], vec![tool_step("toolu_1")])];
        promote_result_agent_tasks(&mut chunks);

        let Chunk::Ai(ai) = &chunks[0] else { panic!() };
        assert_eq!(
            ai.subagents[0]
                .description
                .as_ref()
                .unwrap()
                .chars()
                .count(),
            DESCRIPTION_MAX_CHARS
        );
    }
}
