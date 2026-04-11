//! context-tracking capability 的共享类型。
//!
//! Spec：`openspec/specs/context-tracking/spec.md`。
//!
//! 这些类型由 `cdt-analyze::context` 产出，由 `cdt-api` 暴露给 UI。
//! 字段名通过 `#[serde(rename_all = "camelCase")]` 对齐 TS `ContextInjection`
//! / `ContextStats` 等 JSON shape，后续 API port 可以直接透传。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// =============================================================================
// Token-by-category buckets
// =============================================================================

/// 6 类 context 来源的 token 聚合桶。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokensByCategory {
    pub claude_md: u64,
    pub mentioned_file: u64,
    pub tool_output: u64,
    pub thinking_text: u64,
    pub task_coordination: u64,
    pub user_messages: u64,
}

impl TokensByCategory {
    #[must_use]
    pub fn total(&self) -> u64 {
        self.claude_md
            + self.mentioned_file
            + self.tool_output
            + self.thinking_text
            + self.task_coordination
            + self.user_messages
    }
}

/// 6 类 context 来源的 injection 个数聚合桶。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountsByCategory {
    pub claude_md: usize,
    pub mentioned_file: usize,
    pub tool_output: usize,
    pub thinking_text: usize,
    pub task_coordination: usize,
    pub user_messages: usize,
}

// =============================================================================
// Breakdown helpers
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolTokenBreakdown {
    pub tool_name: String,
    pub token_count: u64,
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskCoordinationKind {
    SendMessage,
    TaskTool,
    TeammateMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCoordinationBreakdown {
    #[serde(rename = "type")]
    pub kind: TaskCoordinationKind,
    pub token_count: u64,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThinkingTextKind {
    Thinking,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingTextBreakdown {
    #[serde(rename = "type")]
    pub kind: ThinkingTextKind,
    pub token_count: u64,
}

// =============================================================================
// Injected token data (filled by port-configuration-management later)
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeMdFileInfo {
    pub path: String,
    pub estimated_tokens: u64,
    pub scope: ClaudeMdScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClaudeMdScope {
    Enterprise,
    User,
    Project,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MentionedFileInfo {
    pub path: String,
    pub display_name: String,
    pub estimated_tokens: u64,
    #[serde(default)]
    pub exists: bool,
}

// =============================================================================
// ContextInjection — the 6 categories
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "kebab-case")]
pub enum ContextInjection {
    ClaudeMd(ClaudeMdContextInjection),
    MentionedFile(MentionedFileInjection),
    ToolOutput(ToolOutputInjection),
    ThinkingText(ThinkingTextInjection),
    TaskCoordination(TaskCoordinationInjection),
    UserMessage(UserMessageInjection),
}

impl ContextInjection {
    #[must_use]
    pub fn estimated_tokens(&self) -> u64 {
        match self {
            Self::ClaudeMd(x) => x.estimated_tokens,
            Self::MentionedFile(x) => x.estimated_tokens,
            Self::ToolOutput(x) => x.estimated_tokens,
            Self::ThinkingText(x) => x.estimated_tokens,
            Self::TaskCoordination(x) => x.estimated_tokens,
            Self::UserMessage(x) => x.estimated_tokens,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::ClaudeMd(x) => &x.id,
            Self::MentionedFile(x) => &x.id,
            Self::ToolOutput(x) => &x.id,
            Self::ThinkingText(x) => &x.id,
            Self::TaskCoordination(x) => &x.id,
            Self::UserMessage(x) => &x.id,
        }
    }

    /// 该 injection 的去重 key（用于 `previous_paths` 集合）。返回 `None`
    /// 的 variant 不参与路径去重（例如 `ToolOutput` / `UserMessage` 每轮都
    /// 会新产生）。
    #[must_use]
    pub fn path_dedup_key(&self) -> Option<&str> {
        match self {
            Self::ClaudeMd(x) => Some(&x.path),
            Self::MentionedFile(x) => Some(&x.path),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeMdContextInjection {
    pub id: String,
    pub path: String,
    pub display_name: String,
    pub scope: ClaudeMdScope,
    pub estimated_tokens: u64,
    pub first_seen_turn_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MentionedFileInjection {
    pub id: String,
    pub path: String,
    pub display_name: String,
    pub estimated_tokens: u64,
    pub first_seen_turn_index: u32,
    pub first_seen_in_group: String,
    #[serde(default)]
    pub exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolOutputInjection {
    pub id: String,
    pub turn_index: u32,
    pub ai_group_id: String,
    pub estimated_tokens: u64,
    pub tool_count: usize,
    pub tool_breakdown: Vec<ToolTokenBreakdown>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingTextInjection {
    pub id: String,
    pub turn_index: u32,
    pub ai_group_id: String,
    pub estimated_tokens: u64,
    pub breakdown: Vec<ThinkingTextBreakdown>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCoordinationInjection {
    pub id: String,
    pub turn_index: u32,
    pub ai_group_id: String,
    pub estimated_tokens: u64,
    pub breakdown: Vec<TaskCoordinationBreakdown>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserMessageInjection {
    pub id: String,
    pub turn_index: u32,
    pub ai_group_id: String,
    pub estimated_tokens: u64,
    pub text_preview: String,
}

// =============================================================================
// Per-AI-group stats
// =============================================================================

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextStats {
    pub new_injections: Vec<ContextInjection>,
    pub accumulated_injections: Vec<ContextInjection>,
    pub total_estimated_tokens: u64,
    pub tokens_by_category: TokensByCategory,
    pub new_counts: CountsByCategory,
    pub accumulated_counts: CountsByCategory,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_number: Option<u32>,
}

// =============================================================================
// Phase info
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPhase {
    pub phase_number: u32,
    pub first_ai_group_id: String,
    pub last_ai_group_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_group_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionTokenDelta {
    pub pre_compaction_tokens: u64,
    pub post_compaction_tokens: u64,
    pub delta: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPhaseInfo {
    pub phases: Vec<ContextPhase>,
    pub compaction_count: u32,
    pub ai_group_phase_map: HashMap<String, u32>,
    pub compaction_token_deltas: HashMap<String, CompactionTokenDelta>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_by_category_total_sums_all_fields() {
        let t = TokensByCategory {
            claude_md: 10,
            mentioned_file: 20,
            tool_output: 30,
            thinking_text: 40,
            task_coordination: 50,
            user_messages: 60,
        };
        assert_eq!(t.total(), 210);
    }

    #[test]
    fn context_stats_serializes_with_camel_case() {
        let stats = ContextStats::default();
        let v = serde_json::to_value(&stats).unwrap();
        let obj = v.as_object().unwrap();
        assert!(obj.contains_key("tokensByCategory"));
        assert!(obj.contains_key("totalEstimatedTokens"));
        assert!(obj.contains_key("newCounts"));
        assert!(obj.contains_key("accumulatedCounts"));
    }

    #[test]
    fn claude_md_injection_serializes_with_kebab_category() {
        let inj = ContextInjection::ClaudeMd(ClaudeMdContextInjection {
            id: "cm-1".into(),
            path: "/CLAUDE.md".into(),
            display_name: "CLAUDE.md".into(),
            scope: ClaudeMdScope::Project,
            estimated_tokens: 123,
            first_seen_turn_index: 0,
        });
        let v = serde_json::to_value(&inj).unwrap();
        assert_eq!(v["category"], "claude-md");
        assert_eq!(v["scope"], "project");
        assert_eq!(v["estimatedTokens"], 123);
    }

    #[test]
    fn tool_output_injection_roundtrips() {
        let inj = ContextInjection::ToolOutput(ToolOutputInjection {
            id: "to-0".into(),
            turn_index: 0,
            ai_group_id: "ai-0".into(),
            estimated_tokens: 50,
            tool_count: 2,
            tool_breakdown: vec![ToolTokenBreakdown {
                tool_name: "Bash".into(),
                token_count: 50,
                is_error: false,
                tool_use_id: Some("tu1".into()),
            }],
        });
        let json = serde_json::to_string(&inj).unwrap();
        let back: ContextInjection = serde_json::from_str(&json).unwrap();
        assert_eq!(inj, back);
    }

    #[test]
    fn compaction_token_delta_is_camel_case() {
        let d = CompactionTokenDelta {
            pre_compaction_tokens: 1000,
            post_compaction_tokens: 600,
            delta: -400,
        };
        let v = serde_json::to_value(d).unwrap();
        assert_eq!(v["preCompactionTokens"], 1000);
        assert_eq!(v["postCompactionTokens"], 600);
        assert_eq!(v["delta"], -400);
    }
}
