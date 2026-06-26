use cdt_api::SessionListFilter;

/// Cross-session filter applied to session lists.
#[derive(Debug, Clone, Default)]
pub struct QueryFilter {
    /// Only sessions with mtime >= since (epoch ms).
    pub since: Option<i64>,

    /// Only sessions with created <= until (epoch ms). Interval intersection
    /// semantics: session `[created, mtime]` overlaps query `[since, until]`.
    pub until: Option<i64>,

    /// Only sessions whose title matches this substring (case-insensitive).
    /// Empty string is treated as no-op.
    pub grep: Option<String>,

    /// Maximum results to return (applied last, after all other filters).
    pub limit: Option<usize>,
}

impl QueryFilter {
    /// Convert to `SessionListFilter` for `LocalDataApi::list_sessions_filtered`.
    pub fn to_session_list_filter(&self) -> SessionListFilter {
        SessionListFilter {
            since: self.since,
            until: self.until,
            grep: self.grep.clone(),
            branch: None,
            limit: self.limit,
        }
    }
}
