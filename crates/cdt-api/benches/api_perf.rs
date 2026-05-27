//! API 层性能基准——用 divan 统计测量 cold scan 和 session detail 关键路径。
//!
//! 跑法：`cargo bench -p cdt-api`
//!
//! 默认用生成的 fixture 数据（CI 友好）。设置 `CDT_PERF_PROJECTS_DIR` 环境变量
//! 可切换到真实 `~/.claude/projects/` 目录获得更贴近生产的数据。

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use cdt_api::DataApi;
use cdt_api::ipc::LocalDataApi;
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{
    LocalFileSystemProvider, LocalGitIdentityResolver, ProjectScanner, WorktreeGrouper,
};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;

struct FixtureCorpus {
    path: PathBuf,
    _temp_dir: Option<TempDir>,
}

fn corpus() -> &'static FixtureCorpus {
    static CORPUS: OnceLock<FixtureCorpus> = OnceLock::new();
    CORPUS.get_or_init(|| {
        if let Ok(path) = std::env::var("CDT_PERF_PROJECTS_DIR") {
            return FixtureCorpus {
                path: PathBuf::from(path),
                _temp_dir: None,
            };
        }

        let temp_dir = TempDir::new().expect("perf fixture tempdir");
        let path = temp_dir.path().join("projects");
        write_fixture_corpus(&path);
        FixtureCorpus {
            path,
            _temp_dir: Some(temp_dir),
        }
    })
}

fn rt() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn main() {
    divan::main();
}

// --- Cold scan benchmarks ---

#[divan::bench]
fn cold_project_scan(bencher: divan::Bencher<'_, '_>) {
    let dir = corpus().path.clone();
    bencher.bench(|| {
        rt().block_on(async {
            let fs: Arc<dyn cdt_discover::fs_provider::FileSystemProvider> =
                Arc::new(LocalFileSystemProvider::new());
            let mut scanner = ProjectScanner::new(fs, dir.clone());
            let projects = scanner.scan().await.expect("scan ok");
            divan::black_box(projects)
        })
    });
}

#[divan::bench]
fn cold_scan_and_group(bencher: divan::Bencher<'_, '_>) {
    let dir = corpus().path.clone();
    bencher.bench(|| {
        rt().block_on(async {
            let fs: Arc<dyn cdt_discover::fs_provider::FileSystemProvider> =
                Arc::new(LocalFileSystemProvider::new());
            let mut scanner = ProjectScanner::new(fs, dir.clone());
            let projects = scanner.scan().await.expect("scan ok");
            let grouper = WorktreeGrouper::new(LocalGitIdentityResolver::new());
            let groups = grouper.group_by_repository(projects).await;
            divan::black_box(groups)
        })
    });
}

// --- Session detail benchmarks ---

#[divan::bench]
fn get_session_detail(bencher: divan::Bencher<'_, '_>) {
    let dir = corpus().path.clone();
    let api = rt().block_on(async {
        let fs: Arc<dyn cdt_discover::fs_provider::FileSystemProvider> =
            Arc::new(LocalFileSystemProvider::new());
        let scanner = ProjectScanner::new(fs, dir.clone());
        let config_dir = TempDir::new().expect("tempdir");
        let config_mgr = ConfigManager::new(Some(config_dir.path().join("config.json")));
        let notif_mgr = NotificationManager::new(None);
        let ssh_mgr = SshConnectionManager::new();
        let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);
        std::mem::forget(config_dir);
        api
    });

    bencher.bench(|| {
        rt().block_on(async {
            let resp = api
                .get_session_detail("-perf-fixture-project", "perf-large-002", None)
                .await
                .expect("get_session_detail");
            divan::black_box(resp)
        })
    });
}

#[divan::bench]
fn list_repository_groups(bencher: divan::Bencher<'_, '_>) {
    let dir = corpus().path.clone();
    let api = rt().block_on(async {
        let fs: Arc<dyn cdt_discover::fs_provider::FileSystemProvider> =
            Arc::new(LocalFileSystemProvider::new());
        let scanner = ProjectScanner::new(fs, dir.clone());
        let config_dir = TempDir::new().expect("tempdir");
        let config_mgr = ConfigManager::new(Some(config_dir.path().join("config.json")));
        let notif_mgr = NotificationManager::new(None);
        let ssh_mgr = SshConnectionManager::new();
        let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);
        std::mem::forget(config_dir);
        api
    });

    bencher.bench(|| {
        rt().block_on(async {
            let groups = api.list_repository_groups().await.expect("list ok");
            divan::black_box(groups)
        })
    });
}

// --- Fixture generation (same as perf_fixture.rs) ---

fn write_fixture_corpus(projects_dir: &Path) {
    std::fs::create_dir_all(projects_dir).expect("create perf projects dir");

    for project_idx in 0..8 {
        let project_id = format!("-perf-fixture-{project_idx:02}");
        let project_dir = projects_dir.join(&project_id);
        std::fs::create_dir_all(&project_dir).expect("create perf project dir");
        for session_idx in 0..12 {
            let sid = format!("scan-{project_idx:02}-{session_idx:03}");
            let cwd = format!("/workspace/perf-fixture/{project_idx:02}");
            write_session(&project_dir, &sid, &cwd, 6, 64, &[]);
        }
    }

    let project_dir = projects_dir.join("-perf-fixture-project");
    std::fs::create_dir_all(&project_dir).expect("create detail perf project dir");
    let session_ids = ["perf-large-000", "perf-large-001", "perf-large-002"];
    for (idx, sid) in session_ids.iter().enumerate() {
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
