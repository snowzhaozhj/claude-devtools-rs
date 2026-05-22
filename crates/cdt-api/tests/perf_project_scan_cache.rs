//! `LocalDataApi.scan_projects_cached()` cache hit perf 基准。
//!
//! 验证 FU-4 `ProjectScanner` memoize 效果：第二次 `list_repository_groups`
//! 走 cache 路径时跳过全量 `ProjectScanner::scan()`（~14K fs op），仅剩
//! grouper join 与 `worktree_meta_cache` 刷新。
//!
//! 跑法：
//! ```sh
//! cargo test -p cdt-api --release --test perf_project_scan_cache -- --ignored --nocapture
//! ```
//!
//! 不进 CI、不算回归——纯定位工具。CI runner 无 `~/.claude/projects/`
//! corpus，本 bench 也会 short-circuit return。
use std::sync::Arc;
use std::time::Instant;

use cdt_api::DataApi;
use cdt_api::ipc::LocalDataApi;
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;

mod perf_fixture;

/// Hit rate 百分比展示用——纯 bench 输出，精度损失可接受（clippy
/// `cast_precision_loss` 关闭范围限缩到本辅助）。
fn hit_rate_pct(hits: u64, lookups: u64) -> f64 {
    if lookups == 0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let pct = (hits as f64 / lookups as f64) * 100.0;
    pct
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "依赖本机 ~/.claude/projects 真实数据，作为 cache hit 性能定位工具"]
async fn measure_list_repository_groups_cache_hit() {
    let projects_dir = perf_fixture::resolve_projects_dir();
    let dir = projects_dir.path();
    if !dir.exists() {
        eprintln!("[perf] skip: {} not exists", dir.display());
        return;
    }

    // LocalDataApi 构造：用 tmp config / notification 路径避免污染真实
    // 用户配置。projects_dir 直接指向真实 `~/.claude/projects/`。
    let tmp = TempDir::new().expect("tmp dir");
    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, dir.to_path_buf());
    let config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    let notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    let ssh_mgr = SshConnectionManager::new();
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);

    // 第一次：cache miss → 全量 scan + grouper
    let t0 = Instant::now();
    let groups1 = api
        .list_repository_groups()
        .await
        .expect("list_repository_groups 1");
    let cold_ms = t0.elapsed().as_millis();
    let group_count = groups1.len();
    let total_worktrees: usize = groups1.iter().map(|g| g.worktrees.len()).sum();
    let stats_after_cold = api.project_scan_cache_stats();
    eprintln!(
        "[perf] list_repository_groups cold={cold_ms}ms groups={group_count} \
         worktrees={total_worktrees} cache_hits={} lookups={}",
        stats_after_cold.hits, stats_after_cold.lookups
    );

    // 第二次：cache hit → 跳过 scan，仅 grouper + meta 刷新
    let t1 = Instant::now();
    let groups2 = api
        .list_repository_groups()
        .await
        .expect("list_repository_groups 2");
    let warm_ms = t1.elapsed().as_millis();
    let stats_after_warm = api.project_scan_cache_stats();
    eprintln!(
        "[perf] list_repository_groups warm={warm_ms}ms groups={} \
         cache_hits={} lookups={} hit_rate={:.0}%",
        groups2.len(),
        stats_after_warm.hits,
        stats_after_warm.lookups,
        hit_rate_pct(stats_after_warm.hits, stats_after_warm.lookups)
    );

    // 第三次：复用第二次写入的 cache（确认稳定命中）
    let t2 = Instant::now();
    let _ = api
        .list_repository_groups()
        .await
        .expect("list_repository_groups 3");
    let warm2_ms = t2.elapsed().as_millis();
    let stats_after_warm2 = api.project_scan_cache_stats();
    eprintln!(
        "[perf] list_repository_groups warm-3={warm2_ms}ms \
         cache_hits={} lookups={} hit_rate={:.0}%",
        stats_after_warm2.hits,
        stats_after_warm2.lookups,
        hit_rate_pct(stats_after_warm2.hits, stats_after_warm2.lookups)
    );

    assert_eq!(
        groups1.len(),
        groups2.len(),
        "cache hit 必须返回与 cold scan 同样数量的 groups"
    );
    assert!(
        stats_after_warm.hits >= 1,
        "第二次调用 SHALL 命中 cache，当前 hits={}",
        stats_after_warm.hits
    );
    // warm 走 cache，单次耗时 SHALL 显著低于 cold；保守 80% 阈值留余量给
    // grouper 自身（git resolve 已 cache，~ms 级）+ worktree_meta 刷新。
    assert!(
        warm_ms <= cold_ms.saturating_mul(80) / 100 || warm_ms <= 30,
        "warm={warm_ms}ms SHALL NOT 接近 cold={cold_ms}ms（提示：cache 失效或未命中）"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "依赖本机 ~/.claude/projects 真实数据，作为 cache hit 性能定位工具"]
async fn measure_list_projects_cache_hit() {
    let projects_dir = perf_fixture::resolve_projects_dir();
    let dir = projects_dir.path();
    if !dir.exists() {
        eprintln!("[perf] skip: {} not exists", dir.display());
        return;
    }

    let tmp = TempDir::new().expect("tmp dir");
    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, dir.to_path_buf());
    let config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    let notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    let ssh_mgr = SshConnectionManager::new();
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);

    let t0 = Instant::now();
    let cold = api.list_projects().await.expect("list_projects 1");
    let cold_ms = t0.elapsed().as_millis();
    let project_count = cold.len();
    eprintln!("[perf] list_projects cold={cold_ms}ms projects={project_count}");

    let t1 = Instant::now();
    let warm = api.list_projects().await.expect("list_projects 2");
    let warm_ms = t1.elapsed().as_millis();
    let stats = api.project_scan_cache_stats();
    eprintln!(
        "[perf] list_projects warm={warm_ms}ms projects={} cache_hits={} lookups={}",
        warm.len(),
        stats.hits,
        stats.lookups
    );

    assert_eq!(cold.len(), warm.len(), "cache hit 返回项目数 SHALL 一致");
    assert!(stats.hits >= 1, "第二次调用 SHALL 命中 cache");
}
