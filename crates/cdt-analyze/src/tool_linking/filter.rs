//! Task filter：按 resolver 结果从 `ToolExecution` 列表里剔除已匹配 Task。
//!
//! Spec：`openspec/specs/chunk-building/spec.md` §"Filter Task tool uses when
//! subagent data is available"。
//!
//! chunk-building 的默认 `build_chunks` 当前并不调用它——端到端接入留给
//! `port-team-coordination-metadata`。本文件提供可独立使用的纯函数。

use std::collections::HashSet;

use cdt_core::ToolExecution;

use super::ResolvedTask;

pub fn filter_resolved_tasks(executions: &mut Vec<ToolExecution>, resolutions: &[ResolvedTask]) {
    let resolved: HashSet<&str> = resolutions
        .iter()
        .filter(|r| !r.resolution.is_orphan())
        .map(|r| r.task_use_id.as_str())
        .collect();
    executions.retain(|e| !resolved.contains(e.tool_use_id.as_str()));
}

#[cfg(test)]
mod tests {
    use super::super::Resolution;
    use super::*;
    use cdt_core::{ChunkMetrics, Process, ToolOutput};
    use chrono::Utc;

    fn exec(id: &str) -> ToolExecution {
        ToolExecution {
            tool_use_id: id.into(),
            tool_name: "Task".into(),
            input: serde_json::json!({}),
            output: ToolOutput::Missing,
            is_error: false,
            start_ts: Utc::now(),
            end_ts: None,
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
        }
    }

    fn process(sid: &str) -> Process {
        Process {
            session_id: sid.into(),
            root_task_description: None,
            spawn_ts: Utc::now(),
            end_ts: None,
            metrics: ChunkMetrics::zero(),
            team: None,
        }
    }

    #[test]
    fn filters_resolved_tasks_only() {
        let mut execs = vec![
            exec("t1"),
            exec("t2"),
            ToolExecution {
                tool_name: "Bash".into(),
                ..exec("b1")
            },
            exec("t3"),
        ];
        let resolutions = vec![
            ResolvedTask {
                task_use_id: "t1".into(),
                resolution: Resolution::ResultBased {
                    process: process("s1"),
                },
            },
            ResolvedTask {
                task_use_id: "t2".into(),
                resolution: Resolution::DescriptionBased {
                    process: process("s2"),
                },
            },
            ResolvedTask {
                task_use_id: "t3".into(),
                resolution: Resolution::Orphan,
            },
        ];
        filter_resolved_tasks(&mut execs, &resolutions);
        assert_eq!(execs.len(), 2);
        assert!(execs.iter().any(|e| e.tool_use_id == "b1"));
        assert!(execs.iter().any(|e| e.tool_use_id == "t3"));
    }

    #[test]
    fn empty_resolutions_keep_all() {
        let mut execs = vec![exec("t1"), exec("t2")];
        filter_resolved_tasks(&mut execs, &[]);
        assert_eq!(execs.len(), 2);
    }
}
