use serde::{Deserialize, Serialize};

/// Controls which portion of a session's chunks to return.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionQueryOptions {
    /// Return chunks in `[start..end)` (0-based indices).
    pub range: Option<(usize, usize)>,

    /// Return only the last N chunks.
    pub tail: Option<usize>,

    /// Filter by chunk kind.
    pub kind_filter: Option<ChunkKindFilter>,

    /// Only return chunks containing errors.
    pub errors_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkKindFilter {
    ErrorsOnly,
    ToolCalls,
}

impl SessionQueryOptions {
    pub fn full() -> Self {
        Self::default()
    }

    pub fn last_n(n: usize) -> Self {
        Self {
            tail: Some(n),
            ..Default::default()
        }
    }

    pub fn with_range(start: usize, end: usize) -> Self {
        Self {
            range: Some((start, end)),
            ..Default::default()
        }
    }

    /// Apply options to a chunk slice, returning a filtered subset.
    pub fn apply(&self, chunks: Vec<cdt_core::Chunk>) -> Vec<cdt_core::Chunk> {
        let mut result = chunks;

        if self.errors_only {
            result.retain(chunk_has_error);
        }

        if let Some(ChunkKindFilter::ErrorsOnly) = self.kind_filter {
            result.retain(chunk_has_error);
        } else if let Some(ChunkKindFilter::ToolCalls) = self.kind_filter {
            result.retain(chunk_has_tool_use);
        }

        if let Some((start, end)) = self.range {
            let end = end.min(result.len());
            let start = start.min(end);
            result = result[start..end].to_vec();
        }

        if let Some(tail) = self.tail {
            let len = result.len();
            if tail < len {
                result = result[len - tail..].to_vec();
            }
        }

        result
    }
}

fn chunk_has_error(chunk: &cdt_core::Chunk) -> bool {
    match chunk {
        cdt_core::Chunk::Ai(ai) => ai.tool_executions.iter().any(|te| te.is_error),
        _ => false,
    }
}

fn chunk_has_tool_use(chunk: &cdt_core::Chunk) -> bool {
    match chunk {
        cdt_core::Chunk::Ai(ai) => !ai.tool_executions.is_empty(),
        _ => false,
    }
}
