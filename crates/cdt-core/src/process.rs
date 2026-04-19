//! Subagent / teammate `Process` 元数据。
//!
//! Spec：`openspec/specs/tool-execution-linking/spec.md` 与
//! `openspec/specs/team-coordination-metadata/spec.md`。
//!
//! `SubagentCandidate` 是 resolver 的输入——装载由 `project-discovery` +
//! `team-coordination-metadata` 负责；`Process` 是 resolver 的输出。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::chunk::{Chunk, ChunkMetrics};

/// 团队成员元数据，由 `team-coordination-metadata` capability 填充。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamMeta {
    pub team_name: String,
    pub member_name: String,
    #[serde(default)]
    pub member_color: Option<String>,
}

/// Subagent 对父 session 的 token 贡献。
///
/// 来源：parent session 中 Task `tool_result` 携带的 `usage` 聚合。
/// 字段目前仅 `total_tokens`；`breakdown` 细项留待后续扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainSessionImpact {
    pub total_tokens: u64,
}

/// 解析出的 subagent 进程记录。
///
/// `team` 在 `port-tool-execution-linking` 范围内保持 `None`；
/// `port-team-coordination-metadata` 会按 `TeamMeta` 填充。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    pub session_id: String,
    #[serde(default)]
    pub root_task_description: Option<String>,
    pub spawn_ts: DateTime<Utc>,
    #[serde(default)]
    pub end_ts: Option<DateTime<Utc>>,
    pub metrics: ChunkMetrics,
    /// TODO(port-team-coordination-metadata)：由下一轮 port 填充。
    #[serde(default)]
    pub team: Option<TeamMeta>,
    /// Task `tool_use` `input.subagent_type`，例如 `"code-reviewer"`。
    #[serde(default)]
    pub subagent_type: Option<String>,
    /// 由 `build_chunks` 从 subagent session 的 `ParsedMessage` 流构建；用于
    /// 前端内联渲染 `ExecutionTrace`。
    #[serde(default)]
    pub messages: Vec<Chunk>,
    /// 此 subagent 对父 session 的 token 贡献（来自 Task `tool_result.usage` 聚合）。
    #[serde(default)]
    pub main_session_impact: Option<MainSessionImpact>,
    /// subagent session 是否仍在运行（最后一条 assistant 消息尚无配对 `tool_result` 且无 `end_ts`）。
    #[serde(default)]
    pub is_ongoing: bool,
    /// `end_ts - spawn_ts` 的毫秒差；未结束时为 `None`。
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// 触发此 subagent 的 Task/Agent `tool_use` 的 id；由 resolver 匹配成功时回填。
    #[serde(default)]
    pub parent_task_id: Option<String>,
    /// Task `tool_use` 的 `input.description`，独立于 `root_task_description`
    /// （后者来自 subagent session root prompt）。
    #[serde(default)]
    pub description: Option<String>,
    /// Subagent header 显示用的 model 名（已跑过 `parse_model_string` 简化，如
    /// `"claude-haiku-4-5-20251001"` → `"haiku4.5"`）。`None` 表示无法识别。
    /// 由 `candidate_to_process` 在 resolver 阶段从 `messages` 派生填充，让
    /// `SubagentCard` header 在 IPC 裁剪 `messages` 后仍可独立渲染。
    #[serde(default)]
    pub header_model: Option<String>,
    /// Subagent 最后一条 assistant `usage` 的 `input + output + cache_read +
    /// cache_creation` 之和；`messages` 缺失时仍可显示 Context Window 槽位。
    /// 0 表示无 usage 数据。
    #[serde(default)]
    pub last_isolated_tokens: u64,
    /// Team-only 特例：subagent 仅含 1 条 assistant + 单一 `SendMessage`
    /// `shutdown_response` 调用时为 true。让 `SubagentCard` 不依赖 `messages`
    /// 即可走 shutdown-only 极简渲染分支。
    #[serde(default)]
    pub is_shutdown_only: bool,
    /// IPC 优化标志：`get_session_detail` 默认裁剪 `messages` 为空 `Vec`、
    /// 把本字段设为 true，调用方需通过 `get_subagent_trace` 按需拉取完整
    /// trace；`messagesOmitted=false` 时 `messages` 应已含完整 chunks。
    #[serde(default)]
    pub messages_omitted: bool,
}

/// Resolver 的输入候选：一个已预装载的 subagent session 的轻量摘要。
///
/// 装载路径不属本 capability；此类型仅作为纯函数 `resolve_subagents` 的输入契约。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubagentCandidate {
    pub session_id: String,
    /// 从 subagent 会话 root 消息里抽取的 prompt / 描述，用于 description-based 匹配。
    #[serde(default)]
    pub description_hint: Option<String>,
    pub spawn_ts: DateTime<Utc>,
    /// Subagent 会话结束时间（JSONL 最后一条消息的 timestamp）；正在运行时为 `None`。
    #[serde(default)]
    pub end_ts: Option<DateTime<Utc>>,
    #[serde(default)]
    pub parent_session_id: Option<String>,
    pub metrics: ChunkMetrics,
    /// 预构建的 subagent session chunk 流；用于 resolver 把 subagent 执行链
    /// 透传给 UI（`Process.messages`）。装载方调用 `build_chunks` 生成。
    #[serde(default)]
    pub messages: Vec<Chunk>,
    /// 是否还在运行——由装载方根据 JSONL 尾部状态判定。
    #[serde(default)]
    pub is_ongoing: bool,
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
        assert_eq!(&serde_json::from_str::<T>(&json).unwrap(), value);
    }

    #[test]
    fn team_meta_roundtrip() {
        roundtrip(&TeamMeta {
            team_name: "alpha".into(),
            member_name: "scout".into(),
            member_color: Some("#ff0000".into()),
        });
    }

    #[test]
    fn process_roundtrip_defaults() {
        roundtrip(&Process {
            session_id: "s1".into(),
            root_task_description: Some("investigate logs".into()),
            spawn_ts: ts(),
            end_ts: None,
            metrics: ChunkMetrics::zero(),
            team: None,
            subagent_type: None,
            messages: Vec::new(),
            main_session_impact: None,
            is_ongoing: false,
            duration_ms: None,
            parent_task_id: None,
            description: None,
            header_model: None,
            last_isolated_tokens: 0,
            is_shutdown_only: false,
            messages_omitted: false,
        });
    }

    #[test]
    fn process_roundtrip_full() {
        roundtrip(&Process {
            session_id: "s1".into(),
            root_task_description: Some("investigate logs".into()),
            spawn_ts: ts(),
            end_ts: Some(ts()),
            metrics: ChunkMetrics::zero(),
            team: Some(TeamMeta {
                team_name: "alpha".into(),
                member_name: "scout".into(),
                member_color: Some("#ff0000".into()),
            }),
            subagent_type: Some("code-reviewer".into()),
            messages: Vec::new(),
            main_session_impact: Some(MainSessionImpact { total_tokens: 1234 }),
            is_ongoing: false,
            duration_ms: Some(5678),
            parent_task_id: Some("toolu_abc".into()),
            description: Some("review the PR".into()),
            header_model: Some("haiku4.5".into()),
            last_isolated_tokens: 4242,
            is_shutdown_only: false,
            messages_omitted: true,
        });
    }

    #[test]
    fn process_messages_omitted_serializes_camel_case() {
        let p = Process {
            session_id: "s1".into(),
            root_task_description: None,
            spawn_ts: ts(),
            end_ts: None,
            metrics: ChunkMetrics::zero(),
            team: None,
            subagent_type: None,
            messages: Vec::new(),
            main_session_impact: None,
            is_ongoing: false,
            duration_ms: None,
            parent_task_id: None,
            description: None,
            header_model: Some("opus4.7".into()),
            last_isolated_tokens: 100,
            is_shutdown_only: true,
            messages_omitted: true,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["headerModel"], serde_json::json!("opus4.7"));
        assert_eq!(v["lastIsolatedTokens"], serde_json::json!(100));
        assert_eq!(v["isShutdownOnly"], serde_json::json!(true));
        assert_eq!(v["messagesOmitted"], serde_json::json!(true));
    }

    #[test]
    fn main_session_impact_serializes_camel_case() {
        let v = serde_json::to_value(MainSessionImpact { total_tokens: 42 }).unwrap();
        assert_eq!(v, serde_json::json!({ "totalTokens": 42 }));
    }

    #[test]
    fn subagent_candidate_roundtrip() {
        roundtrip(&SubagentCandidate {
            session_id: "s1".into(),
            description_hint: Some("investigate logs".into()),
            spawn_ts: ts(),
            end_ts: None,
            parent_session_id: Some("parent".into()),
            metrics: ChunkMetrics::zero(),
            messages: Vec::new(),
            is_ongoing: false,
        });
    }
}
