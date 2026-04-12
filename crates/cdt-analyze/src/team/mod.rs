//! team-coordination-metadata capability。
//!
//! Spec：`openspec/specs/team-coordination-metadata/spec.md`。
//!
//! 检测 teammate 消息、格式化 team 工具摘要、从 Task input 提取 `TeamMeta`。

pub mod detection;
pub mod enrichment;
pub mod summary;

pub use detection::{TeammateAttrs, is_teammate_message, parse_teammate_attrs};
pub use enrichment::extract_team_meta_from_task;
pub use summary::{format_team_tool_summary, is_team_tool};
