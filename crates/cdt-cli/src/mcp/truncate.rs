use cdt_query::TokenEstimator;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TruncatedResult<T: Serialize> {
    pub data: T,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_chunks: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_range: Option<String>,
}

pub fn truncate_chunks_to_budget<T: Serialize + Clone>(
    chunks: &[T],
    estimator: &dyn TokenEstimator,
    budget: usize,
) -> TruncatedResult<Vec<T>> {
    if budget == 0 {
        return TruncatedResult {
            data: chunks.to_vec(),
            truncated: false,
            total_chunks: None,
            next_range: None,
        };
    }

    let mut included = Vec::new();
    let mut used_tokens = 0;

    for (i, chunk) in chunks.iter().enumerate() {
        let serialized = serde_json::to_string(chunk).unwrap_or_default();
        let chunk_tokens = estimator.estimate(&serialized);

        if used_tokens + chunk_tokens > budget {
            if included.is_empty() {
                // First chunk already exceeds budget — include it but mark truncated
                included.push(chunk.clone());
                let next = if chunks.len() > 1 {
                    Some("1:".to_string())
                } else {
                    None
                };
                return TruncatedResult {
                    data: included,
                    truncated: true,
                    total_chunks: Some(chunks.len()),
                    next_range: next,
                };
            }
            return TruncatedResult {
                data: included,
                truncated: true,
                total_chunks: Some(chunks.len()),
                next_range: Some(format!("{i}:")),
            };
        }

        used_tokens += chunk_tokens;
        included.push(chunk.clone());
    }

    TruncatedResult {
        data: included,
        truncated: false,
        total_chunks: None,
        next_range: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_query::CharRatioEstimator;

    #[test]
    fn no_budget_returns_all() {
        let chunks = vec!["chunk1".to_string(), "chunk2".to_string()];
        let est = CharRatioEstimator::default();
        let result = truncate_chunks_to_budget(&chunks, &est, 0);
        assert!(!result.truncated);
        assert_eq!(result.data.len(), 2);
    }

    #[test]
    fn budget_triggers_truncation() {
        let chunks: Vec<String> = (0..100)
            .map(|i| format!("chunk data number {i} with some content"))
            .collect();
        let est = CharRatioEstimator::default();
        let result = truncate_chunks_to_budget(&chunks, &est, 50);
        assert!(result.truncated);
        assert!(result.data.len() < 100);
        assert_eq!(result.total_chunks, Some(100));
        assert!(result.next_range.is_some());
    }

    #[test]
    fn small_chunks_within_budget() {
        let chunks = vec!["a".to_string(), "b".to_string()];
        let est = CharRatioEstimator::default();
        let result = truncate_chunks_to_budget(&chunks, &est, 1000);
        assert!(!result.truncated);
        assert_eq!(result.data.len(), 2);
    }
}
