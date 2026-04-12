//! Team 元数据从 Task input 提取。
//!
//! Spec：`openspec/specs/team-coordination-metadata/spec.md`
//! §"`Enrich subagent processes with team metadata`"。

use cdt_core::{TeamMeta, ToolCall};

/// 从 Task call 的 input 提取 team 元数据。
///
/// TS 侧 `SubagentResolver.ts` 从 `input.team_name` + 描述匹配中提取。
/// Rust 简化为：如果 input 含 `team_name` 字段，则认为是 team spawn。
pub fn extract_team_meta_from_task(task: &ToolCall) -> Option<TeamMeta> {
    let team_name = task.input.get("team_name")?.as_str()?;
    let member_name = task
        .input
        .get("name")
        .or_else(|| task.input.get("member_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("unnamed");

    Some(TeamMeta {
        team_name: team_name.to_owned(),
        member_name: member_name.to_owned(),
        member_color: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(input: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "t1".into(),
            name: "Task".into(),
            input,
            is_task: true,
            task_description: None,
            task_subagent_type: None,
        }
    }

    #[test]
    fn extract_team_meta_present() {
        let task = make_task(serde_json::json!({
            "team_name": "alpha",
            "name": "scout",
            "prompt": "investigate"
        }));
        let meta = extract_team_meta_from_task(&task).unwrap();
        assert_eq!(meta.team_name, "alpha");
        assert_eq!(meta.member_name, "scout");
        assert!(meta.member_color.is_none());
    }

    #[test]
    fn extract_team_meta_member_name_field() {
        let task = make_task(serde_json::json!({
            "team_name": "beta",
            "member_name": "researcher"
        }));
        let meta = extract_team_meta_from_task(&task).unwrap();
        assert_eq!(meta.member_name, "researcher");
    }

    #[test]
    fn extract_no_team_info() {
        let task = make_task(serde_json::json!({"prompt": "do stuff"}));
        assert!(extract_team_meta_from_task(&task).is_none());
    }

    #[test]
    fn extract_unnamed_member() {
        let task = make_task(serde_json::json!({"team_name": "gamma"}));
        let meta = extract_team_meta_from_task(&task).unwrap();
        assert_eq!(meta.member_name, "unnamed");
    }
}
