use cdt_api::SessionSummary;

/// Cross-session filter applied to session lists.
#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    /// Only sessions with timestamp >= since (epoch ms).
    pub since: Option<i64>,

    /// Only sessions with timestamp <= until (epoch ms).
    pub until: Option<i64>,

    /// Only sessions whose title matches this substring (case-insensitive).
    pub grep: Option<String>,

    /// Only sessions that contain errors.
    pub errors_only: bool,

    /// Only sessions that contain tool calls.
    pub tools_only: bool,

    /// Only sessions with at least this many messages.
    pub min_messages: Option<usize>,

    /// Maximum results to return.
    pub limit: Option<usize>,
}

impl QueryFilter {
    pub fn apply(&self, sessions: Vec<SessionSummary>) -> Vec<SessionSummary> {
        let mut result = sessions;

        if let Some(since) = self.since {
            result.retain(|s| s.timestamp >= since);
        }

        if let Some(until) = self.until {
            result.retain(|s| s.timestamp <= until);
        }

        if let Some(ref pattern) = self.grep {
            let lower = pattern.to_lowercase();
            result.retain(|s| {
                s.title
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().contains(&lower))
            });
        }

        if let Some(min) = self.min_messages {
            result.retain(|s| s.message_count >= min);
        }

        if let Some(limit) = self.limit {
            result.truncate(limit);
        }

        result
    }
}
