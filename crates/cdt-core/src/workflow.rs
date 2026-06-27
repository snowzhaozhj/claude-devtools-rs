//! Workflow manifest 解析产出的类型。
//!
//! Spec：`openspec/specs/chunk-building/spec.md` §`Workflow tool_use 识别`
//! + `openspec/specs/ipc-data-api/spec.md` §`WorkflowItem 字段`。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowPhase {
    pub index: u32,
    pub title: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowAgentState {
    #[default]
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowAgent {
    pub label: String,
    pub phase_index: u32,
    pub state: WorkflowAgentState,
    #[serde(default)]
    pub tokens: u64,
    #[serde(default)]
    pub tool_calls: u64,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queued_at: Option<String>,
    #[serde(default)]
    pub failed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    #[default]
    Pending,
    Running,
    Completed,
    PartialFailure,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowItem {
    pub run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub status: WorkflowStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<WorkflowPhase>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agents: Vec<WorkflowAgent>,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// workflow 实际执行的编排脚本预览（inline `{script}` 取 `tool_use.input.script`；
    /// `scriptPath` 形态读脚本文件）。截断到上限并在尾部追加可见 marker。供前端
    /// "View script" disclosure 审计渲染。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_preview: Option<String>,
}

impl WorkflowItem {
    #[must_use]
    pub fn pending(run_id: String) -> Self {
        Self {
            run_id,
            name: None,
            status: WorkflowStatus::Pending,
            phases: Vec::new(),
            agents: Vec::new(),
            total_tokens: 0,
            duration_ms: 0,
            error: None,
            script_preview: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_item_pending_roundtrip() {
        let item = WorkflowItem::pending("wf_abc123".into());
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"runId\":\"wf_abc123\""));
        assert!(json.contains("\"status\":\"pending\""));
        assert!(!json.contains("\"phases\""));
        assert!(!json.contains("\"agents\""));
        let deser: WorkflowItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deser, item);
    }

    #[test]
    fn workflow_item_full_roundtrip() {
        let item = WorkflowItem {
            run_id: "wf_797e9bdf-994".into(),
            name: Some("Code Review".into()),
            status: WorkflowStatus::Completed,
            phases: vec![WorkflowPhase {
                index: 1,
                title: "Analysis".into(),
            }],
            agents: vec![WorkflowAgent {
                label: "reviewer-1".into(),
                phase_index: 1,
                state: WorkflowAgentState::Completed,
                tokens: 5000,
                tool_calls: 12,
                duration_ms: 30000,
                result_preview: Some("LGTM".into()),
                queued_at: Some("2026-05-29T10:00:00Z".into()),
                failed: false,
                session_id: Some("a1b2c3d4e5".into()),
            }],
            total_tokens: 5000,
            duration_ms: 30000,
            error: None,
            script_preview: Some("export const meta = {}".into()),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"totalTokens\":5000"));
        assert!(json.contains("\"phaseIndex\":1"));
        assert!(json.contains("\"scriptPreview\":\"export const meta = {}\""));
        let deser: WorkflowItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deser, item);
    }

    #[test]
    fn workflow_agent_state_serde() {
        let json = serde_json::to_string(&WorkflowAgentState::Failed).unwrap();
        assert_eq!(json, "\"failed\"");
        let deser: WorkflowAgentState = serde_json::from_str("\"completed\"").unwrap();
        assert_eq!(deser, WorkflowAgentState::Completed);
    }

    #[test]
    fn workflow_status_serde() {
        let json = serde_json::to_string(&WorkflowStatus::PartialFailure).unwrap();
        assert_eq!(json, "\"partial_failure\"");
        let deser: WorkflowStatus = serde_json::from_str("\"completed\"").unwrap();
        assert_eq!(deser, WorkflowStatus::Completed);
    }

    #[test]
    fn workflow_item_none_fields_omitted() {
        let item = WorkflowItem::pending("wf_x".into());
        let json = serde_json::to_string(&item).unwrap();
        assert!(!json.contains("\"name\""), "None name SHALL be omitted");
        assert!(!json.contains("\"error\""), "None error SHALL be omitted");
        assert!(
            !json.contains("\"scriptPreview\""),
            "None script_preview SHALL be omitted"
        );
    }
}
