//! 跨会话聚合统计。
//!
//! `cdt stats [today|week|7d] [--project X]`

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use cdt_core::Chunk;

use crate::cost::{self, SessionCost};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedStats {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub session_count: usize,
    pub total_messages: usize,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub tool_frequency: Vec<ToolFrequency>,
    pub error_count: usize,
    pub error_rate: f64,
    pub model_usage: Vec<ModelUsage>,
    pub active_hours: Vec<HourBucket>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolFrequency {
    pub name: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    pub session_count: usize,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HourBucket {
    pub hour: u32,
    pub session_count: usize,
    pub message_count: usize,
}

pub struct SessionData {
    pub timestamp: i64,
    pub message_count: usize,
    pub chunks: Vec<Chunk>,
    pub cost: SessionCost,
    pub model: String,
}

pub fn aggregate(sessions: &[SessionData], since: DateTime<Utc>) -> AggregatedStats {
    let now = Utc::now();
    let mut total_messages: usize = 0;
    let mut total_tokens: u64 = 0;
    let mut total_cost: f64 = 0.0;
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut cache_creation_tokens: u64 = 0;
    let mut error_count: usize = 0;
    let mut tool_total: u64 = 0;
    let mut tool_map: HashMap<String, u64> = HashMap::new();
    let mut model_map: HashMap<String, (usize, f64)> = HashMap::new();
    let mut hour_map: HashMap<u32, (usize, usize)> = HashMap::new();

    for s in sessions {
        total_messages = total_messages.saturating_add(s.message_count);
        total_tokens = total_tokens.saturating_add(s.cost.total_tokens);
        total_cost += s.cost.total_cost;
        input_tokens = input_tokens.saturating_add(s.cost.input_tokens);
        output_tokens = output_tokens.saturating_add(s.cost.output_tokens);
        cache_read_tokens = cache_read_tokens.saturating_add(s.cost.cache_read_tokens);
        cache_creation_tokens = cache_creation_tokens.saturating_add(s.cost.cache_creation_tokens);

        let model_entry = model_map.entry(s.model.clone()).or_default();
        model_entry.0 += 1;
        model_entry.1 += s.cost.total_cost;

        let hour = session_hour(s.timestamp);
        let h_entry = hour_map.entry(hour).or_default();
        h_entry.0 += 1;
        h_entry.1 += s.message_count;

        for chunk in &s.chunks {
            if let Chunk::Ai(ai) = chunk {
                for exec in &ai.tool_executions {
                    *tool_map.entry(exec.tool_name.clone()).or_default() += 1;
                    tool_total += 1;
                    if exec.is_error {
                        error_count += 1;
                    }
                }
            }
        }
    }

    #[allow(clippy::cast_precision_loss)]
    let error_rate = if tool_total > 0 {
        error_count as f64 / tool_total as f64
    } else {
        0.0
    };

    let mut tool_frequency: Vec<ToolFrequency> = tool_map
        .into_iter()
        .map(|(name, count)| ToolFrequency { name, count })
        .collect();
    tool_frequency.sort_by_key(|t| std::cmp::Reverse(t.count));

    let mut model_usage: Vec<ModelUsage> = model_map
        .into_iter()
        .map(|(model, (session_count, total_cost))| ModelUsage {
            model,
            session_count,
            total_cost,
        })
        .collect();
    model_usage.sort_by_key(|m| std::cmp::Reverse(m.session_count));

    let mut active_hours: Vec<HourBucket> = hour_map
        .into_iter()
        .map(|(hour, (session_count, message_count))| HourBucket {
            hour,
            session_count,
            message_count,
        })
        .collect();
    active_hours.sort_by_key(|h| h.hour);

    AggregatedStats {
        period_start: since,
        period_end: now,
        session_count: sessions.len(),
        total_messages,
        total_tokens,
        total_cost,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        tool_frequency,
        error_count,
        error_rate,
        model_usage,
        active_hours,
    }
}

pub fn build_session_data(detail: &cdt_api::SessionDetail) -> SessionData {
    let session_cost = cost::compute_session_cost(detail);
    let model = session_cost.model.clone();
    SessionData {
        timestamp: detail
            .chunks
            .first()
            .map_or(0, |c| c.timestamp().timestamp_millis()),
        message_count: detail.metrics.message_count,
        chunks: detail.chunks.clone(),
        cost: session_cost,
        model,
    }
}

fn session_hour(timestamp_ms: i64) -> u32 {
    let secs = timestamp_ms / 1000;
    let dt = DateTime::from_timestamp(secs, 0).unwrap_or_default();
    dt.format("%H").to_string().parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn aggregate_empty_sessions() {
        let since = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let result = aggregate(&[], since);
        assert_eq!(result.session_count, 0);
        assert_eq!(result.total_tokens, 0);
        assert!((result.total_cost).abs() < f64::EPSILON);
    }

    #[test]
    fn session_hour_extracts_correctly() {
        let ts = Utc
            .with_ymd_and_hms(2026, 1, 1, 14, 30, 0)
            .unwrap()
            .timestamp_millis();
        assert_eq!(session_hour(ts), 14);
    }
}
