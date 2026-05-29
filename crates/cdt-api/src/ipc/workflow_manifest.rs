//! Workflow manifest 读取 + `FileSignature` 缓存。
//!
//! manifest `<session_dir>/workflows/wf_<runId>.json` immutable——
//! 写入后内容不变。首次 stat+read 后按 `FileSignature` 缓存，
//! 后续只 stat 比对。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cdt_core::workflow::{
    WorkflowAgent, WorkflowAgentState, WorkflowItem, WorkflowPhase, WorkflowStatus,
};
use cdt_discover::FileSystemProvider;

use crate::cache_signature::FileSignature;

struct CacheEntry {
    sig: FileSignature,
    item: WorkflowItem,
}

#[derive(Default)]
pub struct WorkflowManifestCache {
    entries: HashMap<PathBuf, CacheEntry>,
}

impl WorkflowManifestCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, path: &Path, sig: &FileSignature) -> Option<WorkflowItem> {
        self.entries
            .get(path)
            .filter(|e| &e.sig == sig)
            .map(|e| e.item.clone())
    }

    fn insert(&mut self, path: PathBuf, sig: FileSignature, item: WorkflowItem) {
        self.entries.insert(path, CacheEntry { sig, item });
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawManifest {
    #[serde(default)]
    workflow_progress: Vec<serde_json::Value>,
    #[serde(default)]
    logs: Vec<String>,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    duration_ms: u64,
    #[serde(default)]
    workflow_name: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

pub fn parse_manifest(run_id: &str, content: &str) -> Result<WorkflowItem, String> {
    let raw: RawManifest =
        serde_json::from_str(content).map_err(|e| format!("manifest JSON parse: {e}"))?;

    let failed_indices = extract_failed_indices(&raw.logs);

    let mut phases: Vec<WorkflowPhase> = Vec::new();
    let mut agents: Vec<WorkflowAgent> = Vec::new();

    for entry_val in &raw.workflow_progress {
        let Some(type_str) = entry_val.get("type").and_then(serde_json::Value::as_str) else {
            continue;
        };
        match type_str {
            "workflow_phase" => {
                if let (Some(index), Some(title)) = (
                    entry_val.get("index").and_then(serde_json::Value::as_u64),
                    entry_val.get("title").and_then(serde_json::Value::as_str),
                ) {
                    #[allow(clippy::cast_possible_truncation)]
                    phases.push(WorkflowPhase {
                        index: index as u32,
                        title: title.to_owned(),
                    });
                }
            }
            "workflow_agent" => {
                let label = entry_val
                    .get("label")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                #[allow(clippy::cast_possible_truncation)]
                let phase_index = entry_val
                    .get("phaseIndex")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as u32;
                let state_str = entry_val
                    .get("state")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("pending");
                let tokens = entry_val
                    .get("tokens")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let tool_calls = entry_val
                    .get("toolCalls")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let duration_ms = entry_val
                    .get("durationMs")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let result_preview = entry_val
                    .get("resultPreview")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
                let queued_at = entry_val
                    .get("queuedAt")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);

                let agent_index = agents.len();
                let failed_by_log = failed_indices.contains(&(agent_index + 1));
                let failed_by_heuristic =
                    tokens == 0 && tool_calls == 0 && result_preview.is_none();
                let failed = failed_by_log || failed_by_heuristic;

                let state = if failed {
                    WorkflowAgentState::Failed
                } else {
                    match state_str {
                        "completed" | "done" => WorkflowAgentState::Completed,
                        "running" => WorkflowAgentState::Running,
                        _ => WorkflowAgentState::Pending,
                    }
                };

                agents.push(WorkflowAgent {
                    label,
                    phase_index,
                    state,
                    tokens,
                    tool_calls,
                    duration_ms,
                    result_preview,
                    queued_at,
                    failed,
                });
            }
            _ => {}
        }
    }

    let any_failed = agents.iter().any(|a| a.failed);
    let all_done = !agents.is_empty()
        && agents.iter().all(|a| {
            matches!(
                a.state,
                WorkflowAgentState::Completed | WorkflowAgentState::Failed
            )
        });
    let top_level_completed = raw.status.as_deref() == Some("completed");

    let status = if agents.is_empty() {
        match raw.status.as_deref() {
            Some("completed") => WorkflowStatus::Completed,
            Some("failed") => WorkflowStatus::Failed,
            _ => WorkflowStatus::Pending,
        }
    } else if any_failed && all_done {
        WorkflowStatus::PartialFailure
    } else if all_done || top_level_completed {
        WorkflowStatus::Completed
    } else {
        WorkflowStatus::Running
    };

    let error = if any_failed {
        raw.logs.iter().find(|l| l.contains("failed")).cloned()
    } else {
        None
    };

    Ok(WorkflowItem {
        run_id: run_id.to_owned(),
        name: raw.workflow_name,
        status,
        phases,
        agents,
        total_tokens: raw.total_tokens,
        duration_ms: raw.duration_ms,
        error,
    })
}

fn extract_failed_indices(logs: &[String]) -> Vec<usize> {
    logs.iter()
        .filter_map(|l| extract_index_from_log(l))
        .collect()
}

fn extract_index_from_log(log: &str) -> Option<usize> {
    for prefix in &["parallel[", "pipeline["] {
        if let Some(start) = log.find(prefix) {
            let after = &log[start + prefix.len()..];
            if let Some(end) = after.find(']') {
                if let Ok(n) = after[..end].parse::<usize>() {
                    if log[start..].contains("failed") {
                        return Some(n);
                    }
                }
            }
        }
    }
    None
}

pub fn collect_workflow_run_ids(chunks: &[cdt_core::Chunk]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut ids = Vec::new();
    for chunk in chunks {
        if let cdt_core::Chunk::Ai(ai) = chunk {
            for exec in &ai.tool_executions {
                if let Some(ref run_id) = exec.workflow_run_id {
                    if seen.insert(run_id.clone()) {
                        ids.push(run_id.clone());
                    }
                }
            }
        }
    }
    ids
}

pub async fn resolve_workflow_items(
    chunks: &[cdt_core::Chunk],
    session_dir: &Path,
    fs: &dyn FileSystemProvider,
    cache: &std::sync::Mutex<WorkflowManifestCache>,
) -> Vec<WorkflowItem> {
    let run_ids = collect_workflow_run_ids(chunks);
    if run_ids.is_empty() {
        return Vec::new();
    }

    let workflows_dir = session_dir.join("workflows");
    let mut items = Vec::with_capacity(run_ids.len());

    for run_id in &run_ids {
        let manifest_path = workflows_dir.join(format!("{run_id}.json"));
        let item = resolve_single(run_id, &manifest_path, fs, cache).await;
        items.push(item);
    }

    items
}

async fn resolve_single(
    run_id: &str,
    manifest_path: &Path,
    fs: &dyn FileSystemProvider,
    cache: &std::sync::Mutex<WorkflowManifestCache>,
) -> WorkflowItem {
    let Ok(fs_meta) = fs.stat(manifest_path).await else {
        tracing::debug!(
            run_id,
            path = %manifest_path.display(),
            "workflow manifest not found, using pending placeholder"
        );
        return WorkflowItem::pending(run_id.to_owned());
    };

    let sig = FileSignature::from_fs_metadata(&fs_meta);

    {
        let Ok(guard) = cache.lock() else {
            return WorkflowItem::pending(run_id.to_owned());
        };
        if let Some(cached) = guard.get(manifest_path, &sig) {
            return cached;
        }
    }

    let content = match fs.read_to_string(manifest_path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                run_id,
                path = %manifest_path.display(),
                error = %e,
                "workflow manifest read failed, using pending placeholder"
            );
            return WorkflowItem::pending(run_id.to_owned());
        }
    };

    let item = match parse_manifest(run_id, &content) {
        Ok(item) => item,
        Err(e) => {
            tracing::warn!(
                run_id,
                path = %manifest_path.display(),
                error = %e,
                "workflow manifest parse failed, using pending placeholder"
            );
            WorkflowItem::pending(run_id.to_owned())
        }
    };

    if let Ok(mut guard) = cache.lock() {
        guard.insert(manifest_path.to_owned(), sig, item.clone());
    }

    item
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest_full_success() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_phase", "index": 1, "title": "Build"},
                {"type": "workflow_agent", "label": "agent-1", "phaseIndex": 1, "state": "completed", "tokens": 3000, "toolCalls": 8, "durationMs": 20000, "resultPreview": "Done"}
            ],
            "status": "completed",
            "logs": [],
            "result": {"output": "success"},
            "agentCount": 1,
            "totalTokens": 3000,
            "durationMs": 20000,
            "workflowName": "Code Review"
        }"#;
        let item = parse_manifest("wf_test1", json).unwrap();

        assert_eq!(item.run_id, "wf_test1");
        assert_eq!(item.name.as_deref(), Some("Code Review"));
        assert_eq!(item.status, WorkflowStatus::Completed);
        assert_eq!(item.phases.len(), 1);
        assert_eq!(item.phases[0].title, "Build");
        assert_eq!(item.agents.len(), 1);
        assert_eq!(item.agents[0].label, "agent-1");
        assert!(!item.agents[0].failed);
        assert_eq!(item.total_tokens, 3000);
    }

    #[test]
    fn parse_manifest_agent_failed_by_heuristic() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_agent", "label": "agent-dead", "phaseIndex": 1, "state": "completed", "tokens": 0, "toolCalls": 0}
            ],
            "status": "completed",
            "logs": [],
            "totalTokens": 0,
            "durationMs": 5000
        }"#;
        let item = parse_manifest("wf_fail1", json).unwrap();

        assert_eq!(item.status, WorkflowStatus::PartialFailure);
        assert!(item.agents[0].failed);
        assert_eq!(item.agents[0].state, WorkflowAgentState::Failed);
    }

    #[test]
    fn parse_manifest_agent_failed_by_log() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_agent", "label": "agent-1", "phaseIndex": 1, "state": "completed", "tokens": 5000, "toolCalls": 10, "durationMs": 30000, "resultPreview": "OK"}
            ],
            "status": "completed",
            "logs": ["parallel[1] failed: timeout"],
            "totalTokens": 5000,
            "durationMs": 30000
        }"#;
        let item = parse_manifest("wf_fail2", json).unwrap();

        assert_eq!(item.status, WorkflowStatus::PartialFailure);
        assert!(item.agents[0].failed);
        assert_eq!(item.error.as_deref(), Some("parallel[1] failed: timeout"));
    }

    #[test]
    fn parse_manifest_invalid_json() {
        let result = parse_manifest("wf_bad", "not json {{{");
        assert!(result.is_err());
    }

    #[test]
    fn extract_failed_indices_various_patterns() {
        let logs = vec![
            "parallel[1] failed: error".to_owned(),
            "pipeline[3] failed: timeout".to_owned(),
            "some other log".to_owned(),
        ];
        let indices = extract_failed_indices(&logs);
        assert_eq!(indices, vec![1, 3]);
    }

    #[test]
    fn cache_hit_and_miss() {
        use crate::cache_signature::FileIdentity;
        use std::time::SystemTime;

        let sig1 = FileSignature {
            mtime: SystemTime::UNIX_EPOCH,
            size: 100,
            identity: FileIdentity::None,
        };
        let sig2 = FileSignature {
            mtime: SystemTime::UNIX_EPOCH,
            size: 200,
            identity: FileIdentity::None,
        };

        let mut cache = WorkflowManifestCache::new();
        let path = PathBuf::from("/tmp/wf_test.json");
        let item = WorkflowItem::pending("wf_test".into());

        cache.insert(path.clone(), sig1, item.clone());

        assert_eq!(cache.get(&path, &sig1), Some(item));
        assert_eq!(cache.get(&path, &sig2), None);
    }

    #[test]
    fn collect_workflow_run_ids_deduplicates() {
        use cdt_core::chunk::{AIChunk, Chunk, ChunkMetrics};
        use cdt_core::tool_execution::{ToolExecution, ToolOutput};
        use chrono::{TimeZone, Utc};

        let ts = Utc.with_ymd_and_hms(2026, 5, 29, 0, 0, 0).unwrap();
        let exec = |run_id: &str| ToolExecution {
            tool_use_id: format!("tu_{run_id}"),
            tool_name: "Workflow".into(),
            input: serde_json::json!({}),
            output: ToolOutput::Missing,
            is_error: false,
            start_ts: ts,
            end_ts: None,
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
            error_message: None,
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
            workflow_run_id: Some(run_id.to_owned()),
        };

        let chunks = vec![Chunk::Ai(AIChunk {
            chunk_id: "c1".into(),
            timestamp: ts,
            duration_ms: None,
            responses: vec![],
            metrics: ChunkMetrics::default(),
            semantic_steps: vec![],
            tool_executions: vec![exec("wf_a"), exec("wf_b"), exec("wf_a")],
            subagents: vec![],
            slash_commands: vec![],
            teammate_messages: vec![],
        })];

        let ids = collect_workflow_run_ids(&chunks);
        assert_eq!(ids, vec!["wf_a", "wf_b"]);
    }

    #[test]
    fn empty_chunks_returns_empty() {
        let ids = collect_workflow_run_ids(&[]);
        assert!(ids.is_empty());
    }

    #[test]
    fn parse_manifest_agent_state_done_recognized_as_completed() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_phase", "index": 1, "title": "Assess"},
                {"type": "workflow_agent", "label": "assess:bugs", "phaseIndex": 1, "state": "done", "tokens": 80000, "toolCalls": 5, "durationMs": 90000, "resultPreview": "Found 3 bugs"},
                {"type": "workflow_agent", "label": "assess:perf", "phaseIndex": 1, "state": "done", "tokens": 75000, "toolCalls": 3, "durationMs": 85000, "resultPreview": "No issues"}
            ],
            "status": "completed",
            "logs": [],
            "totalTokens": 155000,
            "durationMs": 95000,
            "workflowName": "bug-hunt"
        }"#;
        let item = parse_manifest("wf_done_test", json).unwrap();

        assert_eq!(item.status, WorkflowStatus::Completed);
        assert_eq!(item.agents.len(), 2);
        assert_eq!(item.agents[0].state, WorkflowAgentState::Completed);
        assert_eq!(item.agents[1].state, WorkflowAgentState::Completed);
        assert!(!item.agents[0].failed);
        assert!(!item.agents[1].failed);
    }

    #[test]
    fn parse_manifest_top_level_status_fallback() {
        let json = r#"{
            "workflowProgress": [],
            "status": "completed",
            "logs": [],
            "totalTokens": 50000,
            "durationMs": 10000,
            "workflowName": "empty-progress"
        }"#;
        let item = parse_manifest("wf_status_fb", json).unwrap();

        assert_eq!(item.status, WorkflowStatus::Completed);
        assert_eq!(item.agents.len(), 0);
    }
}
