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
        }));
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
