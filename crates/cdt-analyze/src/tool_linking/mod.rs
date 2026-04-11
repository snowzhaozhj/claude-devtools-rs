//! tool-execution-linking capability。
//!
//! Spec：`openspec/specs/tool-execution-linking/spec.md`。
//!
//! 三个纯同步 API：
//! - [`pair_tool_executions`]：把 `tool_use` 与 `tool_result` 按 id 配对。
//! - [`resolve_subagents`]：三阶段 Task → subagent 回退匹配。
//! - [`filter_resolved_tasks`]：按解析结果从 `ToolExecution` 列表里剔除已匹配 Task。
//!
//! 候选（`SubagentCandidate`）的装载不属本 capability——
//! 由 `port-project-discovery` + `port-team-coordination-metadata` 提供。

mod filter;
mod pair;
mod resolver;

use cdt_core::{Process, ToolExecution};
use serde::{Deserialize, Serialize};

pub use filter::filter_resolved_tasks;
pub use pair::pair_tool_executions;
pub use resolver::{TIME_WINDOW_SECS, resolve_subagents};

/// [`pair_tool_executions`] 的返回结构。
#[derive(Debug, Clone, PartialEq)]
pub struct ToolLinkingResult {
    pub executions: Vec<ToolExecution>,
    /// 观测到的重复 `tool_use_id`（impl-bug fix：TS 版静默合并，这里告警并计数）。
    pub duplicates_dropped: usize,
}

/// [`resolve_subagents`] 的返回项。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedTask {
    pub task_use_id: String,
    pub resolution: Resolution,
}

/// 三阶段回退的解析结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Resolution {
    ResultBased { process: Process },
    DescriptionBased { process: Process },
    Positional { process: Process },
    Orphan,
}

impl Resolution {
    pub fn is_orphan(&self) -> bool {
        matches!(self, Resolution::Orphan)
    }
}
