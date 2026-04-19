//! 性能基准（ignored）：测大 session 的解析→建 chunk→context tracking 各阶段耗时。
//!
//! 用真实 `~/.claude/projects/...` 的 JSONL 跑——不是 fixture——目的是为
//! `openspec/followups.md` "性能 / 首次打开大会话卡顿" 提供量化依据。
//!
//! 跑法：
//! ```sh
//! cargo test -p cdt-api --test perf_get_session_detail -- --ignored --nocapture
//! ```
//!
//! 直接调用底层公共 API 各阶段并 wall-clock 计时（避免引入 tracing-subscriber
//! dev-dep；`LocalDataApi` 内部的 tracing event 测试里抓不到）。
//!
//! 不进 CI、不算回归——只是定位工具。
use std::path::{Path, PathBuf};
use std::time::Instant;

use cdt_analyze::{build_chunks_with_subagents, check_messages_ongoing};
use cdt_api::ipc::{DataApi, LocalDataApi};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{ProjectScanner, local_handle};
use cdt_parse::parse_file;
use cdt_ssh::SshConnectionManager;

fn projects_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join(".claude/projects")
}

/// 复刻 `LocalDataApi::scan_subagent_candidates` 的扫描逻辑，但不调
/// `parse_subagent_candidate`——只数文件 + 累加每个 subagent jsonl 的
/// `parse_file` 耗时。这是怀疑点之一（每 subagent 完整 parse + `build_chunks`）。
async fn scan_and_parse_subagents(
    project_dir: &Path,
    session_id: &str,
) -> (Vec<cdt_core::SubagentCandidate>, u128, u128) {
    let mut candidates: Vec<cdt_core::SubagentCandidate> = Vec::new();
    let mut parse_total: u128 = 0;
    let mut chunk_total: u128 = 0;

    let new_dir = project_dir.join(session_id).join("subagents");
    if let Ok(mut entries) = tokio::fs::read_dir(&new_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with("agent-")
                || !name_str.ends_with(".jsonl")
                || name_str.starts_with("agent-acompact")
            {
                continue;
            }
            let path = entry.path();
            let session_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.strip_prefix("agent-").unwrap_or(s).to_owned())
                .unwrap_or_default();

            let t = Instant::now();
            let Ok(mut msgs) = parse_file(&path).await else {
                continue;
            };
            parse_total += t.elapsed().as_millis();

            for m in &mut msgs {
                m.is_sidechain = false;
            }
            let t2 = Instant::now();
            let chunks = cdt_analyze::build_chunks(&msgs);
            chunk_total += t2.elapsed().as_millis();

            candidates.push(cdt_core::SubagentCandidate {
                session_id,
                description_hint: None,
                spawn_ts: chrono::Utc::now(),
                end_ts: None,
                parent_session_id: None,
                metrics: cdt_core::ChunkMetrics::zero(),
                messages: chunks,
                is_ongoing: false,
            });
        }
    }

    (candidates, parse_total, chunk_total)
}

#[tokio::test]
#[ignore = "perf bench, requires real ~/.claude/projects/ data"]
async fn bench_get_session_detail_large_sessions() {
    let projects = projects_dir();
    if !projects.exists() {
        eprintln!("跳过：{} 不存在", projects.display());
        return;
    }

    let project_id = "-Users-zhaohejie-RustroverProjects-Project-claude-devtools-rs";
    let project_dir = projects.join(project_id);
    if !project_dir.exists() {
        eprintln!("跳过：{} 不存在", project_dir.display());
        return;
    }

    let samples = [
        "4cdfdf06-400d-417d-84c6-4b9fefae06a8",
        "7826d1b8-99d9-48ef-8c64-49fcb65b40da",
        "46a25772-b57c-43bb-9ca6-f0292f9ca912",
    ];

    // 同时构造一个 LocalDataApi，验证 get_session_detail 实际经过 IPC
    // 裁剪后的 payload 大小（与底层裸跑数据对比，看 OMIT_SUBAGENT_MESSAGES
    // 减肥效果）。tempdir 仅用于 ConfigManager；scanner 指向真实 projects 目录。
    let config_dir = tempfile::tempdir().expect("tempdir");
    let mut config_mgr = ConfigManager::new(Some(config_dir.path().join("config.json")));
    config_mgr.load().await.expect("config load");
    let api = LocalDataApi::new(
        ProjectScanner::new(local_handle(), projects.clone()),
        config_mgr,
        NotificationManager::new(None),
        SshConnectionManager::new(),
    );
    std::mem::forget(config_dir);

    eprintln!(
        "=== Session detail timings (parse / scan_subagents / build / serialize / total) ==="
    );

    for sid in samples {
        let jsonl = project_dir.join(format!("{sid}.jsonl"));
        if !jsonl.exists() {
            eprintln!("[skip] {sid}: jsonl 不存在");
            continue;
        }

        let total = Instant::now();

        let t_parse = Instant::now();
        let messages = match parse_file(&jsonl).await {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[err] {sid}: parse failed: {e}");
                continue;
            }
        };
        let parse_ms = t_parse.elapsed().as_millis();

        let t_scan = Instant::now();
        let (candidates, sub_parse_ms, sub_chunk_ms) =
            scan_and_parse_subagents(&project_dir, sid).await;
        let scan_ms = t_scan.elapsed().as_millis();

        let t_build = Instant::now();
        let _ongoing = check_messages_ongoing(&messages);
        let chunks = build_chunks_with_subagents(&messages, &candidates);
        let build_ms = t_build.elapsed().as_millis();

        let t_serde = Instant::now();
        let payload = serde_json::to_vec(&chunks).unwrap_or_default();
        let serde_ms = t_serde.elapsed().as_millis();

        let total_ms = total.elapsed().as_millis();

        eprintln!(
            "{sid}: msgs={} chunks={} subs={} | parse {parse_ms}ms | scan_subs {scan_ms}ms (parse {sub_parse_ms}ms + chunk {sub_chunk_ms}ms) | build {build_ms}ms | serde {serde_ms}ms ({} KB) | TOTAL {total_ms}ms",
            messages.len(),
            chunks.len(),
            candidates.len(),
            payload.len() / 1024,
        );

        // Payload 字段占比分解：定位减肥目标。
        let breakdown = analyze_payload(&chunks);
        eprintln!(
            "  payload breakdown: tool_output={} KB, tool_input={} KB, subagent_msgs={} KB, response_content={} KB, semantic_steps={} KB, other≈{} KB",
            breakdown.tool_output / 1024,
            breakdown.tool_input / 1024,
            breakdown.subagent_messages / 1024,
            breakdown.response_content / 1024,
            breakdown.semantic_steps / 1024,
            (payload.len().saturating_sub(
                breakdown.tool_output
                    + breakdown.tool_input
                    + breakdown.subagent_messages
                    + breakdown.response_content
                    + breakdown.semantic_steps,
            )) / 1024,
        );

        // 经过 LocalDataApi::get_session_detail 的真实 IPC 路径——含
        // OMIT_SUBAGENT_MESSAGES 裁剪。对比 raw payload 看减肥效果。
        let project_id = "-Users-zhaohejie-RustroverProjects-Project-claude-devtools-rs";
        let t_ipc = Instant::now();
        let detail = api
            .get_session_detail(project_id, sid)
            .await
            .expect("get_session_detail");
        let ipc_ms = t_ipc.elapsed().as_millis();
        let ipc_payload = serde_json::to_vec(&detail).map_or(0, |v| v.len());
        eprintln!(
            "  ★ get_session_detail (with OMIT): payload={} KB, ipc {} ms",
            ipc_payload / 1024,
            ipc_ms,
        );
    }
}

#[derive(Default)]
struct PayloadBreakdown {
    tool_output: usize,
    tool_input: usize,
    subagent_messages: usize,
    response_content: usize,
    semantic_steps: usize,
}

/// 把 chunks 反序列化为 `serde_json::Value` 后按已知 key 累计字节数（按
/// 各 sub-tree 重新序列化）。粗略口径——目的是判断哪条 sub-tree 是大头。
fn analyze_payload(chunks: &[cdt_core::Chunk]) -> PayloadBreakdown {
    let mut b = PayloadBreakdown::default();
    for chunk in chunks {
        let cdt_core::Chunk::Ai(ai) = chunk else {
            continue;
        };
        for exec in &ai.tool_executions {
            b.tool_input += serde_json::to_vec(&exec.input).map_or(0, |v| v.len());
            b.tool_output += serde_json::to_vec(&exec.output).map_or(0, |v| v.len());
        }
        for sub in &ai.subagents {
            b.subagent_messages += serde_json::to_vec(&sub.messages).map_or(0, |v| v.len());
        }
        for resp in &ai.responses {
            b.response_content += serde_json::to_vec(&resp.content).map_or(0, |v| v.len());
        }
        b.semantic_steps += serde_json::to_vec(&ai.semantic_steps).map_or(0, |v| v.len());
    }
    b
}
