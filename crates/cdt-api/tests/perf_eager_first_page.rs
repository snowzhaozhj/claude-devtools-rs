//! 性能基准（ignored）：测 change `eager-first-page-metadata` 引入的 eager
//! cursor=None 路径在 30 project × 50 session corpus 下的耗时四维：wall /
//! user / sys / RSS。
//!
//! 跑法：
//! ```sh
//! cargo test -p cdt-api --release --test perf_eager_first_page -- --ignored --nocapture
//! ```
//!
//! 自带固定 corpus（不依赖本机 `~/.claude/projects/`）让本 bench 在 CI fixture
//! 模式下也能跑——基线由 `scripts/run-perf-bench.sh` gate。
//!
//! 度量目标（与 design D5b/D7/D4b 对齐）：
//! - `page_size=20` cursor=None：p50 < 300ms / p95 < 500ms / worst < 1500ms
//! - `page_size=50` cursor=None：eager 前 20 条 + remainder 30 条骨架，wall < 300ms
//! - 复合场景：projectA 翻页扫描进行中 + projectB cursor=None eager
//!   验证 D4b abort 让 projectB 不被 projectA 拖慢
use std::sync::Arc;
use std::time::Instant;

use cdt_api::PaginatedRequest;
use cdt_api::ipc::{DataApi, LocalDataApi};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;

/// 30 project × 50 session 固定 corpus。每个 session 1 user + 1 assistant
/// `tool_use`（fixture 与 `session_metadata_stream.rs` 对齐：`tool_use` sans
/// `tool_result` + 新 mtime → `isOngoing=true`）。
const PROJECT_COUNT: usize = 30;
const SESSIONS_PER_PROJECT: usize = 50;

async fn write_session_jsonl(
    dir: &std::path::Path,
    session_id: &str,
    title: &str,
) -> std::io::Result<()> {
    let path = dir.join(format!("{session_id}.jsonl"));
    let user = serde_json::json!({
        "type": "user",
        "uuid": format!("u-{session_id}"),
        "timestamp": "2026-05-20T10:00:00Z",
        "cwd": "/tmp/proj",
        "message": {"role": "user", "content": title}
    });
    let assistant = serde_json::json!({
        "type": "assistant",
        "uuid": format!("a-{session_id}"),
        "timestamp": "2026-05-20T10:00:01Z",
        "cwd": "/tmp/proj",
        "message": {
            "role": "assistant",
            "model": "claude-sonnet",
            "content": [{
                "type": "tool_use",
                "id": format!("tu-{session_id}"),
                "name": "Bash",
                "input": {"command": "ls"}
            }]
        }
    });
    let body = format!("{user}\n{assistant}\n");
    tokio::fs::write(&path, body).await
}

async fn build_corpus() -> (TempDir, Vec<String>) {
    let tmp = TempDir::new().expect("tempdir");
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).expect("mkdir projects");

    let mut project_ids = Vec::with_capacity(PROJECT_COUNT);
    for p_idx in 0..PROJECT_COUNT {
        let pid = format!("-perf-eager-proj-{p_idx:03}");
        let pdir = projects_base.join(&pid);
        std::fs::create_dir_all(&pdir).expect("mkdir project");
        for s_idx in 0..SESSIONS_PER_PROJECT {
            let sid = format!("sess-{p_idx:03}-{s_idx:03}");
            write_session_jsonl(&pdir, &sid, &format!("title {p_idx}-{s_idx}"))
                .await
                .expect("write session");
        }
        project_ids.push(pid);
    }
    (tmp, project_ids)
}

async fn build_api(
    projects_base: &std::path::Path,
    tmp_root: &std::path::Path,
) -> Arc<LocalDataApi> {
    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base.to_path_buf());
    let mut config_mgr = ConfigManager::new(Some(tmp_root.join("config.json")));
    config_mgr.load().await.expect("config load");
    let mut notif_mgr = NotificationManager::new(Some(tmp_root.join("notifications.json")));
    notif_mgr.load().await.expect("notif load");
    let ssh_mgr = SshConnectionManager::new();
    Arc::new(LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr))
}

fn pct(values: &mut [u128], q: f64) -> u128 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let idx = ((values.len() as f64 - 1.0) * q).round() as usize;
    values[idx.min(values.len() - 1)]
}

/// 三个子场景串行跑（`run-perf-bench.sh` 用 `--exact` 匹配单一 test，所以需
/// 要一个总入口 ignored test 名 `bench_eager_first_page` 把 page_size=20 /
/// page_size=50 / D4b 复合三个场景串起来，让进程级 `/usr/bin/time -lp` wall/
/// user/sys/RSS 四维覆盖完整链路。
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "perf bench, builds 30×50 corpus in tempdir"]
async fn bench_eager_first_page() {
    scenario_page_size_20().await;
    scenario_page_size_50().await;
    scenario_d4b_cross_project_abort().await;
}

/// pageSize=20 cursor=None：eager 路径前 20 条 inline 真值，wall 应在数十～数百 ms。
/// 量分布：p50 < 300ms / p95 < 500ms / worst < 1500ms。
async fn scenario_page_size_20() {
    let (tmp, project_ids) = build_corpus().await;
    let api = build_api(&tmp.path().join("projects"), tmp.path()).await;

    // 跑前给 ProjectScanner 一次 warm scan，避免首调命中冷启动 list_projects 路径
    let _ = api
        .list_sessions(
            &project_ids[0],
            &PaginatedRequest {
                page_size: 1,
                cursor: Some("0".to_owned()),
            },
        )
        .await;

    let mut walls: Vec<u128> = Vec::with_capacity(project_ids.len());
    for pid in &project_ids {
        let t = Instant::now();
        let resp = api
            .list_sessions(
                pid,
                &PaginatedRequest {
                    page_size: 20,
                    cursor: None,
                },
            )
            .await
            .expect("list_sessions");
        walls.push(t.elapsed().as_millis());
        assert_eq!(resp.items.len(), 20, "page_size=20 SHALL return 20 items");
    }

    let p50 = pct(&mut walls.clone(), 0.50);
    let p95 = pct(&mut walls.clone(), 0.95);
    let worst = *walls.iter().max().unwrap_or(&0);
    let total: u128 = walls.iter().sum();
    let avg = total / (walls.len() as u128).max(1);
    println!(
        "[perf] eager page_size=20 projects={} p50={p50}ms p95={p95}ms worst={worst}ms avg={avg}ms",
        project_ids.len()
    );
    // 软断言（避免噪声 flake）：worst < 1500ms（D7 timeout 上界 500ms × 3 retry 余量）
    assert!(
        worst < 1500,
        "page_size=20 cursor=None worst wall SHALL < 1500ms, got {worst}ms"
    );
}

/// `pageSize=50` cursor=None：eager 前 20 条 + 后 30 条骨架（remainder spawn）。
/// wall 应与 `page_size=20` 相近——remainder scan 不阻塞响应。
async fn scenario_page_size_50() {
    let (tmp, project_ids) = build_corpus().await;
    let api = build_api(&tmp.path().join("projects"), tmp.path()).await;
    let _ = api
        .list_sessions(
            &project_ids[0],
            &PaginatedRequest {
                page_size: 1,
                cursor: Some("0".to_owned()),
            },
        )
        .await;

    let mut walls: Vec<u128> = Vec::with_capacity(project_ids.len());
    for pid in &project_ids {
        let t = Instant::now();
        let resp = api
            .list_sessions(
                pid,
                &PaginatedRequest {
                    page_size: 50,
                    cursor: None,
                },
            )
            .await
            .expect("list_sessions");
        walls.push(t.elapsed().as_millis());
        assert_eq!(resp.items.len(), 50, "page_size=50 SHALL return 50 items");
    }

    let p50 = pct(&mut walls.clone(), 0.50);
    let p95 = pct(&mut walls.clone(), 0.95);
    let worst = *walls.iter().max().unwrap_or(&0);
    let avg: u128 = walls.iter().sum::<u128>() / (walls.len() as u128).max(1);
    println!(
        "[perf] eager page_size=50 projects={} p50={p50}ms p95={p95}ms worst={worst}ms avg={avg}ms (eager 20 + remainder 30)",
        project_ids.len()
    );
    assert!(
        worst < 1500,
        "page_size=50 cursor=None worst wall SHALL < 1500ms, got {worst}ms"
    );
}

/// D4b 复合场景：projectA 翻页扫描进行中 + projectB cursor=None eager 启动。
/// projectB 的 wall 应未被 projectA 占满 permit 拖慢——D4b abort 让 permit 立即可用。
async fn scenario_d4b_cross_project_abort() {
    let (tmp, project_ids) = build_corpus().await;
    let api = build_api(&tmp.path().join("projects"), tmp.path()).await;

    let project_a = &project_ids[0];
    let project_b = &project_ids[1];

    // 用 projectA 多次翻页扫描占用 active_scans + permits
    for cursor in ["0", "10", "20", "30"] {
        let _ = api
            .list_sessions(
                project_a,
                &PaginatedRequest {
                    page_size: 10,
                    cursor: Some(cursor.to_owned()),
                },
            )
            .await
            .expect("projectA paged");
    }

    // 立即切到 projectB cursor=None；D4b 应 abort projectA 所有 entry
    let t = Instant::now();
    let resp_b = api
        .list_sessions(
            project_b,
            &PaginatedRequest {
                page_size: 20,
                cursor: None,
            },
        )
        .await
        .expect("projectB eager");
    let wall_b = t.elapsed().as_millis();

    println!(
        "[perf] eager d4b cross-project: projectB wall={wall_b}ms (after projectA × 4 paged scans)"
    );
    assert_eq!(resp_b.items.len(), 20);
    // 软断言：D4b 后 permit 立即可用，projectB wall < 1500ms
    assert!(
        wall_b < 1500,
        "D4b cross-project switch SHALL keep projectB wall < 1500ms, got {wall_b}ms"
    );
}
