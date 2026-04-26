//! chunk-building capability 的共享类型。
//!
//! Spec：`openspec/specs/chunk-building/spec.md`。
//!
//! 本模块只定义数据结构，不含构造逻辑——`build_chunks` 由 `cdt_analyze::chunk`
//! 实现。下游 crate（`cdt-api`、未来的 UI）都通过本模块依赖这些类型，因此
//! 它们必须保持纯数据，不引入运行时依赖。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::message::{MessageContent, TokenUsage, ToolCall};
use crate::process::Process;
use crate::tool_execution::ToolExecution;

/// 嵌入到 `AIChunk` 内的队友回信记录。
///
/// 数据来源：被 `cdt_analyze::team::is_teammate_message` 识别出的 user 消息
/// （形如 `<teammate-message teammate_id="..." color="..." summary="...">body</teammate-message>`）。
/// chunk-building 阶段不再为这类消息产 `UserChunk`，而是解析为本结构后注入到下一个 flush
/// 出的 `AIChunk.teammate_messages`，并在 `cdt_analyze::team::reply_link` 中向前扫描配对
/// 触发它的 `SendMessage` `tool_use`（详见 `team-coordination-metadata` spec）。
///
/// `is_noise` / `is_resend` / `token_count` 在数据层预算并随字段透传，前端无须重算。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeammateMessage {
    pub uuid: String,
    pub teammate_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub body: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to_tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u64>,
    #[serde(default)]
    pub is_noise: bool,
    #[serde(default)]
    pub is_resend: bool,
}

/// 从 isMeta 用户消息中提取的 slash 命令信息。
///
/// 格式：`<command-name>/xxx</command-name>` + 可选
/// `<command-message>` 和 `<command-args>`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlashCommand {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    pub message_uuid: String,
    pub timestamp: DateTime<Utc>,
    /// Slash 命令的 follow-up 指令文本（由 `isMeta=true` 且 `parentUuid` 指向
    /// 该 slash 的 user 消息的 text block 提供）。AI group 内 `SlashItem` 展开时
    /// 以 markdown 渲染；为空则不可展开。对齐原版 `extractSlashes` 的 instructions。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// 对单个 chunk 汇总的指标。
///
/// Token 字段语义与 [`TokenUsage`] 对齐；`tool_count` 在
/// `port-tool-execution-linking` 之前统计所有 `tool_use` 块（含 `Task`），
/// 之后会按 Task 过滤语义修正；`cost_usd` 暂为 `None`，等引入价格表后启用。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkMetrics {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub tool_count: u64,
    #[serde(default)]
    pub cost_usd: Option<f64>,
}

impl ChunkMetrics {
    pub fn zero() -> Self {
        Self::default()
    }
}

/// `AIChunk` 内部的一条 assistant 响应。
///
/// 在 chunk-building 阶段，连续 assistant 消息会被合并进同一个 `AIChunk`，
/// 每条原始消息对应一个 `AssistantResponse`。`tool_results` 用于承接
/// tool_result-only 用户消息反向挂载的结果，供后续 `tool-execution-linking`
/// 使用；本 capability 不填充它。
///
/// `content_omitted` 是 IPC payload 优化字段（见 change `session-detail-response-content-omit`）：
/// `get_session_detail` 返回路径默认把 `content` 替换为空 `MessageContent::Text("")` +
/// 设此 flag 为 true，砍掉首屏 IPC 最大单一字段（实测占总 payload 41%）。前端无任何
/// 代码读 `content`（chunk 显示文本走 `semanticSteps`），故无需懒拉接口。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantResponse {
    pub uuid: String,
    pub timestamp: DateTime<Utc>,
    pub content: MessageContent,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub usage: Option<TokenUsage>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub content_omitted: bool,
}

/// `AIChunk` 中按时间顺序提取的语义步骤，用于 UI 可视化。
///
/// `SubagentSpawn` 在 `port-chunk-building` 范围内永远不会被产出，保留变体
/// 是为了让未来的 `port-team-coordination-metadata` 在不修改 `cdt-core`
/// 公共 API 的情况下补上真实来源。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SemanticStep {
    Thinking {
        text: String,
        timestamp: DateTime<Utc>,
    },
    Text {
        text: String,
        timestamp: DateTime<Utc>,
    },
    ToolExecution {
        tool_use_id: String,
        tool_name: String,
        timestamp: DateTime<Utc>,
    },
    SubagentSpawn {
        placeholder_id: String,
        timestamp: DateTime<Utc>,
    },
    /// 用户按 Esc / 拒绝工具触发的 `[Request interrupted by user` 消息。
    ///
    /// chunk-building 将其追加到前一个 `AIChunk.semantic_steps` 末尾，
    /// UI 以红色块渲染 "Session interrupted by user"。
    Interruption {
        text: String,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserChunk {
    pub uuid: String,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: Option<i64>,
    pub content: MessageContent,
    pub metrics: ChunkMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AIChunk {
    pub timestamp: DateTime<Utc>,
    pub duration_ms: Option<i64>,
    pub responses: Vec<AssistantResponse>,
    pub metrics: ChunkMetrics,
    pub semantic_steps: Vec<SemanticStep>,
    #[serde(default)]
    pub tool_executions: Vec<ToolExecution>,
    /// TODO(port-team-coordination-metadata)：由更晚的 port 填充。
    #[serde(default)]
    pub subagents: Vec<Process>,
    /// 前驱 isMeta 消息中提取的 slash 命令（如 `/commit`、`/review-pr`）。
    #[serde(default)]
    pub slash_commands: Vec<SlashCommand>,
    /// 嵌入到该 turn 的队友回信（详见 [`TeammateMessage`] 与 chunk-building spec
    /// 的 `Embed teammate messages into AIChunk` Requirement）。
    /// 默认空 Vec；序列化时 `skip_serializing_if = "Vec::is_empty"`，
    /// 无 teammate 嵌入时 IPC payload 不含 `teammateMessages` 键，老前端兼容。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub teammate_messages: Vec<TeammateMessage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemChunk {
    pub uuid: String,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: Option<i64>,
    pub content_text: String,
    pub metrics: ChunkMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactChunk {
    pub uuid: String,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: Option<i64>,
    pub summary_text: String,
    pub metrics: ChunkMetrics,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Chunk {
    User(UserChunk),
    Ai(AIChunk),
    System(SystemChunk),
    Compact(CompactChunk),
}

impl Chunk {
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Chunk::User(c) => c.timestamp,
            Chunk::Ai(c) => c.timestamp,
            Chunk::System(c) => c.timestamp,
            Chunk::Compact(c) => c.timestamp,
        }
    }

    pub fn metrics(&self) -> &ChunkMetrics {
        match self {
            Chunk::User(c) => &c.metrics,
            Chunk::Ai(c) => &c.metrics,
            Chunk::System(c) => &c.metrics,
            Chunk::Compact(c) => &c.metrics,
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

    fn roundtrip<T>(value: &T)
    where
        T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).unwrap();
        let back: T = serde_json::from_str(&json).unwrap();
        assert_eq!(&back, value);
    }

    #[test]
    fn chunk_metrics_roundtrip() {
        roundtrip(&ChunkMetrics {
            input_tokens: 1,
            output_tokens: 2,
            cache_creation_tokens: 3,
            cache_read_tokens: 4,
            tool_count: 5,
            cost_usd: None,
        });
    }

    #[test]
    fn assistant_response_roundtrip() {
        roundtrip(&AssistantResponse {
            uuid: "u1".into(),
            timestamp: ts(),
            content: MessageContent::Text("hi".into()),
            tool_calls: Vec::new(),
            usage: None,
            model: Some("claude-opus-4-6".into()),
            content_omitted: false,
        });
    }

    #[test]
    fn assistant_response_default_content_omitted_false() {
        let json = r#"{"uuid":"u1","timestamp":"2026-04-19T00:00:00Z","content":"hi","toolCalls":[],"usage":null,"model":null}"#;
        let resp: AssistantResponse = serde_json::from_str(json).unwrap();
        assert!(
            !resp.content_omitted,
            "missing contentOmitted SHALL deserialize to false (legacy compat)"
        );
    }

    #[test]
    fn assistant_response_content_omitted_roundtrip() {
        roundtrip(&AssistantResponse {
            uuid: "u1".into(),
            timestamp: ts(),
            content: MessageContent::Text(String::new()),
            tool_calls: Vec::new(),
            usage: None,
            model: None,
            content_omitted: true,
        });
    }

    #[test]
    fn semantic_step_roundtrip() {
        roundtrip(&SemanticStep::Thinking {
            text: "reason".into(),
            timestamp: ts(),
        });
        roundtrip(&SemanticStep::Text {
            text: "hello".into(),
            timestamp: ts(),
        });
        roundtrip(&SemanticStep::ToolExecution {
            tool_use_id: "tu1".into(),
            tool_name: "Bash".into(),
            timestamp: ts(),
        });
        roundtrip(&SemanticStep::SubagentSpawn {
            placeholder_id: "sa1".into(),
            timestamp: ts(),
        });
        roundtrip(&SemanticStep::Interruption {
            text: "[Request interrupted by user for tool use]".into(),
            timestamp: ts(),
        });
    }

    #[test]
    fn user_chunk_roundtrip() {
        roundtrip(&Chunk::User(UserChunk {
            uuid: "u1".into(),
            timestamp: ts(),
            duration_ms: None,
            content: MessageContent::Text("hello".into()),
            metrics: ChunkMetrics::zero(),
        }));
    }

    #[test]
    fn ai_chunk_roundtrip() {
        roundtrip(&Chunk::Ai(AIChunk {
            timestamp: ts(),
            duration_ms: Some(120),
            responses: Vec::new(),
            metrics: ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        }));
    }

    #[test]
    fn ai_chunk_default_teammate_messages_empty() {
        let json = r#"{"kind":"ai","timestamp":"2026-04-19T00:00:00Z","durationMs":null,"responses":[],"metrics":{},"semanticSteps":[],"toolExecutions":[],"subagents":[],"slashCommands":[]}"#;
        let chunk: Chunk = serde_json::from_str(json).unwrap();
        let Chunk::Ai(ai) = chunk else {
            panic!("expected AI chunk");
        };
        assert!(
            ai.teammate_messages.is_empty(),
            "missing teammateMessages SHALL deserialize to empty Vec (legacy compat)"
        );
    }

    #[test]
    fn ai_chunk_empty_teammate_messages_omitted_in_json() {
        let chunk = AIChunk {
            timestamp: ts(),
            duration_ms: None,
            responses: Vec::new(),
            metrics: ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages: Vec::new(),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(
            !json.contains("teammateMessages"),
            "empty teammate_messages SHALL be omitted from JSON: got {json}"
        );
    }

    #[test]
    fn teammate_message_roundtrip_full() {
        roundtrip(&TeammateMessage {
            uuid: "u1".into(),
            teammate_id: "alice".into(),
            color: Some("blue".into()),
            summary: Some("Hello".into()),
            body: "body text".into(),
            timestamp: ts(),
            reply_to_tool_use_id: Some("toolu_01".into()),
            token_count: Some(120),
            is_noise: false,
            is_resend: false,
        });
    }

    #[test]
    fn teammate_message_roundtrip_minimal() {
        roundtrip(&TeammateMessage {
            uuid: "u2".into(),
            teammate_id: "bob".into(),
            color: None,
            summary: None,
            body: String::new(),
            timestamp: ts(),
            reply_to_tool_use_id: None,
            token_count: None,
            is_noise: true,
            is_resend: false,
        });
    }

    #[test]
    fn teammate_message_serializes_camel_case() {
        let tm = TeammateMessage {
            uuid: "u".into(),
            teammate_id: "alice".into(),
            color: Some("blue".into()),
            summary: Some("s".into()),
            body: "b".into(),
            timestamp: ts(),
            reply_to_tool_use_id: Some("t".into()),
            token_count: Some(10),
            is_noise: false,
            is_resend: true,
        };
        let json = serde_json::to_string(&tm).unwrap();
        assert!(json.contains("\"teammateId\":\"alice\""));
        assert!(json.contains("\"replyToToolUseId\":\"t\""));
        assert!(json.contains("\"tokenCount\":10"));
        assert!(json.contains("\"isNoise\":false"));
        assert!(json.contains("\"isResend\":true"));
    }

    #[test]
    fn system_chunk_roundtrip() {
        roundtrip(&Chunk::System(SystemChunk {
            uuid: "s1".into(),
            timestamp: ts(),
            duration_ms: None,
            content_text: "ls output".into(),
            metrics: ChunkMetrics::zero(),
        }));
    }

    #[test]
    fn compact_chunk_roundtrip() {
        roundtrip(&Chunk::Compact(CompactChunk {
            uuid: "c1".into(),
            timestamp: ts(),
            duration_ms: None,
            summary_text: "summary".into(),
            metrics: ChunkMetrics::zero(),
        }));
    }
}
