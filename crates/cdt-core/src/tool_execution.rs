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

/// `Teammate spawn` 信息：从 `tool_result.toolUseResult.status == "teammate_spawned"`
/// 中抽出的成员名 + 颜色。当 `ToolExecution` 关联的 tool 是"派生 teammate"行为
/// 时，UI 用这个字段渲染极简单行卡（"member-X 圆点 + Teammate spawned"），替代
/// 普通 tool item。对齐原版 `claude-devtools/src/renderer/components/chat/items/
/// LinkedToolItem.tsx` 的 `isTeammateSpawned` 分支。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeammateSpawnInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// 单次 tool 执行的完整记录，`AIChunk.tool_executions` 的元素类型。
///
/// `output_omitted` 是 IPC payload 优化字段（见 change `session-detail-tool-output-lazy-load`）：
/// `get_session_detail` 返回路径默认把 `output` 内 `text` / `value` 清空（保留 enum
/// variant kind） + 设此 flag 为 true，砍掉首屏 IPC 中 tool 输出。前端
/// `ExecutionTrace` 在用户点击展开时通过 `get_tool_output` IPC 按需懒拉。
///
/// `output_bytes` 与 `output_omitted` 配套（见 change `tool-output-omit-preserve-size`）：
/// IPC OMIT 层在 `trim` 前记录原始字节长度，让前端在懒加载前即可估算 output token
/// （按 4 字符/token 启发式）。解析层不主动填充——保持 `None`。
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
    /// IPC OMIT 时记录的 output 原始字节长度。解析层 `None`；OMIT 层仅
    /// `Text` / `Structured` variant 填值，`Missing` 保持 `None`。
    #[serde(default)]
    pub output_bytes: Option<u64>,
    /// `tool_result.toolUseResult.status == "teammate_spawned"` 时由
    /// `cdt-analyze::tool_linking::pair` 填入的成员名 + 颜色（spec：
    /// `tool-execution-linking` §`Detect teammate-spawned tool results`）。
    /// `None` 表示这条 execution 不是 teammate spawn。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teammate_spawn: Option<TeammateSpawnInfo>,
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
            output_bytes: None,
            teammate_spawn: None,
        };
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<ToolExecution>(&json).unwrap(), value);
    }

    #[test]
    fn tool_execution_teammate_spawn_roundtrip() {
        let value = ToolExecution {
            tool_use_id: "tu-spawn".into(),
            tool_name: "Agent".into(),
            input: serde_json::json!({"name": "member-1"}),
            output: ToolOutput::Structured {
                value: serde_json::json!({"status": "teammate_spawned", "name": "member-1", "color": "blue"}),
            },
            is_error: false,
            start_ts: ts(),
            end_ts: Some(ts()),
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: Some(TeammateSpawnInfo {
                name: "member-1".into(),
                color: Some("blue".into()),
            }),
        };
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.contains("\"teammateSpawn\":{\"name\":\"member-1\",\"color\":\"blue\"}"));
        assert_eq!(serde_json::from_str::<ToolExecution>(&json).unwrap(), value);
    }

    #[test]
    fn tool_execution_default_teammate_spawn_none() {
        let json = r#"{"toolUseId":"tu1","toolName":"Bash","input":{"cmd":"ls"},"output":{"kind":"text","text":"hi"},"isError":false,"startTs":"2026-04-11T00:00:00Z","endTs":null,"sourceAssistantUuid":"a1","resultAgentId":null,"outputOmitted":false,"outputBytes":null}"#;
        let exec: ToolExecution = serde_json::from_str(json).unwrap();
        assert!(exec.teammate_spawn.is_none());
    }

    #[test]
    fn tool_execution_empty_teammate_spawn_omitted() {
        let value = ToolExecution {
            tool_use_id: "tu1".into(),
            tool_name: "Bash".into(),
            input: serde_json::json!({"cmd": "ls"}),
            output: ToolOutput::Text { text: "ok".into() },
            is_error: false,
            start_ts: ts(),
            end_ts: Some(ts()),
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
        };
        let json = serde_json::to_string(&value).unwrap();
        assert!(
            !json.contains("teammateSpawn"),
            "None teammate_spawn SHALL be omitted: {json}"
        );
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
            output_bytes: Some(42),
            teammate_spawn: None,
        };
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<ToolExecution>(&json).unwrap(), value);
    }

    #[test]
    fn tool_execution_output_bytes_defaults_to_none() {
        // 旧 payload 不带 outputBytes 字段：serde default 反序列化为 None。
        let json = r#"{"toolUseId":"tu1","toolName":"Bash","input":{"cmd":"ls"},"output":{"kind":"text","text":"hi"},"isError":false,"startTs":"2026-04-11T00:00:00Z","endTs":null,"sourceAssistantUuid":"a1","resultAgentId":null,"outputOmitted":false}"#;
        let exec: ToolExecution = serde_json::from_str(json).unwrap();
        assert_eq!(exec.output_bytes, None);
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
