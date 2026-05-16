use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub const PERF_PROJECT_ID: &str = "-perf-fixture-project";
pub const PERF_SESSION_IDS: [&str; 3] = ["perf-large-000", "perf-large-001", "perf-large-002"];

pub struct PerfProjectsDir {
    pub path: PathBuf,
    _temp_dir: Option<TempDir>,
}

impl PerfProjectsDir {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn resolve_projects_dir() -> PerfProjectsDir {
    if let Ok(path) = std::env::var("CDT_PERF_PROJECTS_DIR") {
        return PerfProjectsDir {
            path: PathBuf::from(path),
            _temp_dir: None,
        };
    }

    if std::env::var("CDT_PERF_USE_FIXTURE").as_deref() == Ok("1") {
        let temp_dir = TempDir::new().expect("perf fixture tempdir");
        let path = temp_dir.path().join("projects");
        write_fixture_corpus(&path);
        return PerfProjectsDir {
            path,
            _temp_dir: Some(temp_dir),
        };
    }

    PerfProjectsDir {
        path: cdt_discover::get_projects_base_path(),
        _temp_dir: None,
    }
}

fn write_fixture_corpus(projects_dir: &Path) {
    std::fs::create_dir_all(projects_dir).expect("create perf projects dir");

    for project_idx in 0..8 {
        let project_id = format!("-perf-fixture-{project_idx:02}");
        let project_dir = projects_dir.join(project_id);
        std::fs::create_dir_all(&project_dir).expect("create perf project dir");
        for session_idx in 0..12 {
            let sid = format!("scan-{project_idx:02}-{session_idx:03}");
            let cwd = format!("/workspace/perf-fixture/{project_idx:02}");
            write_session(&project_dir, &sid, &cwd, 6, 64, &[]);
        }
    }

    let project_dir = projects_dir.join(PERF_PROJECT_ID);
    std::fs::create_dir_all(&project_dir).expect("create detail perf project dir");
    for (idx, sid) in PERF_SESSION_IDS.iter().enumerate() {
        let subagent_ids: Vec<String> = (0..3)
            .map(|sub_idx| format!("sub-{sid}-{sub_idx}"))
            .collect();
        write_session(
            &project_dir,
            sid,
            "/workspace/perf-fixture/detail",
            220 + idx * 80,
            2048,
            &subagent_ids,
        );
        write_subagent_sessions(&project_dir, sid, &subagent_ids);
    }
}

fn write_session(
    project_dir: &Path,
    sid: &str,
    cwd: &str,
    turns: usize,
    payload_bytes: usize,
    subagent_ids: &[String],
) {
    let mut lines = Vec::with_capacity(turns * 3);
    for i in 0..turns {
        lines.push(
            serde_json::json!({
                "type": "user",
                "uuid": format!("u-{sid}-{i}"),
                "timestamp": timestamp(i * 3),
                "cwd": cwd,
                "message": {"role": "user", "content": format!("请分析第 {i} 个性能样本")}
            })
            .to_string(),
        );
        let subagent_id = subagent_ids.get(i % subagent_ids.len().max(1));
        let tool_name = if subagent_id.is_some() {
            "Task"
        } else {
            "Bash"
        };
        let tool_input = if subagent_id.is_some() {
            serde_json::json!({"description": format!("分析样本 {i}"), "subagent_type": "general-purpose", "prompt": "分析性能样本"})
        } else {
            serde_json::json!({"command": "printf perf"})
        };
        lines.push(
            serde_json::json!({
                "type": "assistant",
                "uuid": format!("a-{sid}-{i}"),
                "timestamp": timestamp(i * 3 + 1),
                "cwd": cwd,
                "message": {
                    "role": "assistant",
                    "model": "claude-opus-4-7",
                    "content": [
                        {"type": "text", "text": format!("样本 {i} 的分析结果")},
                        {"type": "tool_use", "id": format!("tu-{sid}-{i}"), "name": tool_name, "input": tool_input}
                    ],
                    "usage": {"input_tokens": 100 + i, "output_tokens": 20}
                }
            })
            .to_string(),
        );
        let tool_result_content = subagent_id.map_or_else(
            || serde_json::json!("x".repeat(payload_bytes)),
            |id| serde_json::json!({"session_id": id}),
        );
        lines.push(
            serde_json::json!({
                "type": "user",
                "uuid": format!("tr-{sid}-{i}"),
                "timestamp": timestamp(i * 3 + 2),
                "cwd": cwd,
                "message": {
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": format!("tu-{sid}-{i}"),
                        "content": tool_result_content,
                        "is_error": false
                    }]
                }
            })
            .to_string(),
        );
    }

    std::fs::write(
        project_dir.join(format!("{sid}.jsonl")),
        lines.join("\n") + "\n",
    )
    .expect("write perf session fixture");
}

fn write_subagent_sessions(project_dir: &Path, parent_sid: &str, subagent_ids: &[String]) {
    let subagents_dir = project_dir.join(parent_sid).join("subagents");
    std::fs::create_dir_all(&subagents_dir).expect("create subagents dir");
    for sid in subagent_ids {
        write_session(
            &subagents_dir,
            &format!("agent-{sid}"),
            "/workspace/perf-fixture/detail",
            24,
            512,
            &[],
        );
    }
}

fn timestamp(offset_seconds: usize) -> String {
    format!(
        "2026-05-16T10:{:02}:{:02}Z",
        (offset_seconds / 60) % 60,
        offset_seconds % 60
    )
}
