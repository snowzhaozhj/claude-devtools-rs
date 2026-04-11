//! `AIChunk` 指标聚合。
//!
//! Spec scenario：
//! - `AIChunk` with multiple tool uses —— 统计全部 `tool_use` 块数量。
//! - `UserChunk` without token usage —— 由 `ChunkMetrics::zero()` 直接满足。
//!
//! 注意：`tool_count` 在 `port-tool-execution-linking` 之前统计所有
//! `tool_use`，含 Task 调用；下次 port 会把 Task 过滤后的语义写进来。

use cdt_core::{AssistantResponse, ChunkMetrics};

pub fn aggregate_metrics(responses: &[AssistantResponse]) -> ChunkMetrics {
    let mut m = ChunkMetrics::zero();
    for r in responses {
        if let Some(u) = &r.usage {
            m.input_tokens = m.input_tokens.saturating_add(u.input_tokens);
            m.output_tokens = m.output_tokens.saturating_add(u.output_tokens);
            m.cache_creation_tokens = m
                .cache_creation_tokens
                .saturating_add(u.cache_creation_input_tokens);
            m.cache_read_tokens = m
                .cache_read_tokens
                .saturating_add(u.cache_read_input_tokens);
        }
        m.tool_count = m.tool_count.saturating_add(r.tool_calls.len() as u64);
    }
    m.cost_usd = None;
    m
}
