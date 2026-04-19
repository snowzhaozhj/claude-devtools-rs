//! tool-execution-linking capability 的 `ToolExecution` 类型。
//!
//! Spec：`openspec/specs/tool-execution-linking/spec.md`。
//!
//! 每个 `tool_use` 块在 pair 后都会产出一条 `ToolExecution` 记录；未匹配到
//! `tool_result` 的条目以 orphan 形式保留（`output = Missing`、`end_ts = None`）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// `ToolExecution` 的输出载荷。
///
/// - `Text`：legacy 字符串形态的 `tool_result.content`。
/// - `Structured`：新版 JSON 对象，例如 Bash 工具的 `{stdout, stderr, exit_code}`；
///   原样保留，不在本层拆分——UI 层按需解析。
/// - `Missing`：orphan `tool_use`，没有匹配的 `tool_result`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ToolOutput {
    Text { text: String },
    Structured { value: serde_json::Value },
    Missing,
}

/// 单次 tool 执行的完整记录，`AIChunk.tool_executions` 的元素类型。
///
/// `output_omitted` 是 IPC payload 优化字段（见 change `session-detail-tool-output-lazy-load`）：
/// `get_session_detail` 返回路径默认把 `output` 内 `text` / `value` 清空（保留 enum
/// variant kind） + 设此 flag 为 true，砍掉首屏 IPC 中 tool 输出。前端
/// `ExecutionTrace` 在用户点击展开时通过 `get_tool_output` IPC 按需懒拉。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecution {
    pub tool_use_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub output: ToolOutput,
    #[serde(default)]
    pub is_error: bool,
    pub start_ts: DateTime<Utc>,
    #[serde(default)]
    pub end_ts: Option<DateTime<Utc>>,
    /// 产生该 `tool_use` 的 assistant 消息 uuid，用于 `build_chunks` 把 execution
    /// 分发回正确的 `AIChunk`。
    pub source_assistant_uuid: String,
    /// 从 JSONL 顶层 `toolUseResult.agentId` 提取的 subagent session id。
    /// Subagent 匹配 Phase 1 优先读取此字段（比 content block 文本抽取更可靠）。
    #[serde(default)]
    pub result_agent_id: Option<String>,
    #[serde(default)]
    pub output_omitted: bool,
}

impl ToolOutput {
    /// 把 `output` inner `text` / `value` 字段清空（保留 enum variant kind）。
    /// `Missing` variant 保持不变。用于 IPC 路径的 `OMIT_TOOL_OUTPUT` 裁剪
    /// （见 change `session-detail-tool-output-lazy-load`）。
    pub fn trim(&mut self) {
        match self {
            ToolOutput::Text { text } => text.clear(),
            ToolOutput::Structured { value } => *value = serde_json::Value::Null,
            ToolOutput::Missing => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn tool_output_text_roundtrip() {
        let value = ToolOutput::Text {
            text: "hello".into(),
        };
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<ToolOutput>(&json).unwrap(), value);
    }

    #[test]
    fn tool_output_structured_roundtrip() {
        let value = ToolOutput::Structured {
            value: serde_json::json!({"stdout": "ok", "stderr": ""}),
        };
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<ToolOutput>(&json).unwrap(), value);
    }

    #[test]
    fn tool_output_missing_roundtrip() {
        let value = ToolOutput::Missing;
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<ToolOutput>(&json).unwrap(), value);
    }

    #[test]
    fn tool_execution_roundtrip() {
        let value = ToolExecution {
            tool_use_id: "tu1".into(),
            tool_name: "Bash".into(),
            input: serde_json::json!({"cmd": "ls"}),
            output: ToolOutput::Text {
                text: "a\nb".into(),
            },
            is_error: false,
            start_ts: ts(),
            end_ts: Some(ts()),
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
            output_omitted: false,
        };
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<ToolExecution>(&json).unwrap(), value);
    }

    #[test]
    fn tool_execution_default_output_omitted_false() {
        let json = r#"{"toolUseId":"tu1","toolName":"Bash","input":{"cmd":"ls"},"output":{"kind":"text","text":"hi"},"isError":false,"startTs":"2026-04-11T00:00:00Z","endTs":null,"sourceAssistantUuid":"a1","resultAgentId":null}"#;
        let exec: ToolExecution = serde_json::from_str(json).unwrap();
        assert!(
            !exec.output_omitted,
            "missing outputOmitted SHALL deserialize to false (legacy compat)"
        );
    }

    #[test]
    fn tool_execution_output_omitted_roundtrip() {
        let value = ToolExecution {
            tool_use_id: "tu1".into(),
            tool_name: "Bash".into(),
            input: serde_json::json!({"cmd": "ls"}),
            output: ToolOutput::Text {
                text: String::new(),
            },
            is_error: false,
            start_ts: ts(),
            end_ts: Some(ts()),
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
            output_omitted: true,
        };
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<ToolExecution>(&json).unwrap(), value);
    }

    #[test]
    fn tool_output_trim_text_clears_string() {
        let mut o = ToolOutput::Text {
            text: "hello".into(),
        };
        o.trim();
        assert_eq!(
            o,
            ToolOutput::Text {
                text: String::new()
            }
        );
    }

    #[test]
    fn tool_output_trim_structured_clears_value() {
        let mut o = ToolOutput::Structured {
            value: serde_json::json!({"stdout": "ok"}),
        };
        o.trim();
        assert_eq!(
            o,
            ToolOutput::Structured {
                value: serde_json::Value::Null
            }
        );
    }

    #[test]
    fn tool_output_trim_missing_is_noop() {
        let mut o = ToolOutput::Missing;
        o.trim();
        assert_eq!(o, ToolOutput::Missing);
    }
}
