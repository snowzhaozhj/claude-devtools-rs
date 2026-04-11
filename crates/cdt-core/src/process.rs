//! Subagent / teammate `Process` 元数据。
//!
//! Spec：`openspec/specs/tool-execution-linking/spec.md` 与
//! `openspec/specs/team-coordination-metadata/spec.md`。
//!
//! `SubagentCandidate` 是 resolver 的输入——装载由 `project-discovery` +
//! `team-coordination-metadata` 负责；`Process` 是 resolver 的输出。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::chunk::ChunkMetrics;

/// 团队成员元数据，由 `team-coordination-metadata` capability 填充。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamMeta {
    pub team_name: String,
    pub member_name: String,
    #[serde(default)]
    pub member_color: Option<String>,
}

/// 解析出的 subagent 进程记录。
///
/// `team` 在 `port-tool-execution-linking` 范围内保持 `None`；
/// `port-team-coordination-metadata` 会按 `TeamMeta` 填充。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    #[serde(default)]
    pub parent_session_id: Option<String>,
    pub metrics: ChunkMetrics,
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
    fn process_roundtrip() {
        roundtrip(&Process {
            session_id: "s1".into(),
            root_task_description: Some("investigate logs".into()),
            spawn_ts: ts(),
            end_ts: None,
            metrics: ChunkMetrics::zero(),
            team: None,
        });
    }

    #[test]
    fn subagent_candidate_roundtrip() {
        roundtrip(&SubagentCandidate {
            session_id: "s1".into(),
            description_hint: Some("investigate logs".into()),
            spawn_ts: ts(),
            parent_session_id: Some("parent".into()),
            metrics: ChunkMetrics::zero(),
        });
    }
}
