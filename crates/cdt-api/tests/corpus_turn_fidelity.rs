//! Turn 保真度诊断：用本机 `~/.claude/projects/` 真实语料量化 turn 模型的偏差，
//! 并作为 issue #540（turn 锚定）修复的回归守卫。
//! 与 `perf_get_session_detail.rs` 同类——`#[ignore]` 手动跑，CI 无语料自动跳过。
//!
//! 命令：`cargo test -p cdt-api --test corpus_turn_fidelity -- --ignored --nocapture`
//!
//! 三个指标：
//! - **A（chunk 层）：一个 user-turn 内出现 >1 个 `AIChunk`**（"一句话 → 多 chunk"）。
//!   衡量 turn 与 `AIChunk` 是否 1:1。
//! - **B（chunk 层）：真实对话用户消息后没有任何 `AIChunk`**。根因是响应被打断时
//!   Claude Code 写 `model:"<synthetic>"` 占位消息、被 `cdt-parse` 当
//!   `HardNoise(SyntheticAssistant)` 过滤，这一轮不产 `AIChunk`。
//! - **C（context turn 层）：锚到 `UserChunk`（而非 `AIChunk`）的 user-message
//!   injection 数**。这就是被 turn-anchoring 修复"救回"的被打断 turn。
//!
//! 基线（2026-06-25 · 842 session / 9112 `UserChunk` / 7922 `AIChunk`）：
//! - A：13 次（其中 10 次由 `CompactChunk` 切分），≈0.14% —— turn 与 `AIChunk` 基本 1:1
//! - B：597 条真实对话消息在 chunk 流里无后继 `AIChunk`（≈6.5%）；修复前后**不变**
//! - C：turn-anchoring 修复后 1193（修复前 0）—— 被救回的被打断 turn injection
//!   （含真实对话 597 + slash/bash + trailing 中有后继 AI group 承载的部分）
//!
//! 关键：turn-anchoring 修复**不改 chunk 流**（被打断响应本就没有 `AIChunk`），改的是
//! context turn 层——让这些被打断的用户消息照样占一个 turn 并产 user-message injection。
//! 因此 **B（chunk 层）修复后不变**；真正归零的是"被打断消息丢 injection"——由 C 反向
//! 度量：修复前 C==0，修复后 C 应 ≈ B 中"非末尾、有后继 AI group 可承载"的部分。

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use cdt_analyze::context::TokenDictionaries;
use cdt_analyze::{ProcessSessionParams, build_chunks, process_session_context_with_phases};
use cdt_core::{
    Chunk, ClaudeMdFileInfo, ContentBlock, ContextInjection, MentionedFileInfo, MessageContent,
};
use cdt_parse::parse_file;

fn projects_dir() -> PathBuf {
    cdt_discover::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("projects")
}

fn user_text(c: &Chunk) -> String {
    let Chunk::User(u) = c else {
        return String::new();
    };
    match &u.content {
        MessageContent::Text(t) => t.clone(),
        MessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// `<...>` / `/...` 开头的内容是 slash 命令、bash 模式输入、command 元信息等非对话载荷。
fn is_system_ish(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with('<') || t.starts_with('/')
}

#[tokio::test]
#[ignore = "本机语料诊断，手动跑；CI 无 ~/.claude/projects 自动空跑"]
async fn corpus_turn_fidelity() {
    let base = projects_dir();
    let mut session_files = Vec::new();
    if let Ok(projects) = std::fs::read_dir(&base) {
        for p in projects.flatten() {
            if let Ok(sessions) = std::fs::read_dir(p.path()) {
                for s in sessions.flatten() {
                    let path = s.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        session_files.push(path);
                    }
                }
            }
        }
    }
    if session_files.is_empty() {
        eprintln!("无本机语料，跳过 corpus_turn_fidelity");
        return;
    }

    let claude_md: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let directory: HashMap<String, ClaudeMdFileInfo> = HashMap::new();
    let mentioned: HashMap<String, MentionedFileInfo> = HashMap::new();

    let mut sessions = 0usize;
    let mut user_chunks = 0usize;
    let mut ai_chunks = 0usize;

    // A
    let mut multi_ai = 0usize;
    let mut multi_ai_compact = 0usize;
    // B（chunk 层）
    let mut b_trailing = 0usize; // 之后再无 UserChunk（末尾刚发 / ongoing）
    let mut b_system_ish = 0usize; // slash / bash 模式输入
    let mut b_real_dropped = 0usize; // 真实对话消息在 chunk 流无后继 AIChunk
    // C（context turn 层）：锚到 UserChunk 的 user-message injection（被救回的被打断 turn）
    let mut c_interrupted_injections = 0usize;
    let mut examples: Vec<String> = Vec::new();

    for path in &session_files {
        let Ok(msgs) = parse_file(path).await else {
            continue;
        };
        if msgs.is_empty() {
            continue;
        }
        let chunks = build_chunks(&msgs);
        if chunks.is_empty() {
            continue;
        }
        sessions += 1;
        let sid = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();

        // ----- chunk 层 A / B -----
        let mut i = 0;
        while i < chunks.len() {
            if !matches!(chunks[i], Chunk::User(_)) {
                if matches!(chunks[i], Chunk::Ai(_)) {
                    ai_chunks += 1;
                }
                i += 1;
                continue;
            }
            user_chunks += 1;

            let mut ai_count = 0;
            let mut saw_compact = false;
            let mut next_user = false;
            let mut j = i + 1;
            while j < chunks.len() {
                match &chunks[j] {
                    Chunk::User(_) => {
                        next_user = true;
                        break;
                    }
                    Chunk::Ai(_) => {
                        ai_count += 1;
                        ai_chunks += 1;
                    }
                    Chunk::Compact(_) if ai_count >= 1 => saw_compact = true,
                    _ => {}
                }
                j += 1;
            }

            if ai_count == 0 {
                let text = user_text(&chunks[i]);
                if !next_user {
                    b_trailing += 1;
                } else if is_system_ish(&text) {
                    b_system_ish += 1;
                } else {
                    b_real_dropped += 1;
                    if examples.len() < 15 {
                        let preview: String = text.chars().take(40).collect();
                        examples.push(format!("{sid:.8} | {preview}"));
                    }
                }
            } else if ai_count > 1 {
                multi_ai += 1;
                if saw_compact {
                    multi_ai_compact += 1;
                }
            }
            i = j;
        }

        // ----- context turn 层 C -----
        let ai_ids: HashSet<&str> = chunks
            .iter()
            .filter_map(|c| match c {
                Chunk::Ai(ai) => Some(ai.chunk_id.as_str()),
                _ => None,
            })
            .collect();

        let params = ProcessSessionParams {
            project_root: &base,
            token_dictionaries: TokenDictionaries::new(&base, &claude_md, &directory, &mentioned),
            initial_claude_md_injections: &[],
        };
        let result = process_session_context_with_phases(&chunks, &params);

        // 去重收集所有 user-message injection（按 id），统计 aiGroupId ∉ AIChunk 集合的。
        let mut seen: HashSet<String> = HashSet::new();
        for stats in result.stats_map.values() {
            for inj in &stats.accumulated_injections {
                if let ContextInjection::UserMessage(x) = inj {
                    if seen.insert(x.id.clone()) && !ai_ids.contains(x.ai_group_id.as_str()) {
                        c_interrupted_injections += 1;
                    }
                }
            }
        }
    }

    println!("\n===== TURN 保真度诊断 =====");
    println!("session: {sessions}  UserChunk: {user_chunks}  AIChunk: {ai_chunks}");
    println!("[A] 一句话→多 AIChunk: {multi_ai}（Compact 切分 {multi_ai_compact}）");
    println!("[B] chunk 层 UserChunk 后无 AIChunk:");
    println!("    trailing(benign): {b_trailing}");
    println!("    slash/bash(benign): {b_system_ish}");
    println!("    真实对话(被打断): {b_real_dropped}");
    println!(
        "[C] context turn 层 救回的被打断 injection（锚 UserChunk）: {c_interrupted_injections}"
    );
    println!("    修复前 C==0；修复后 C 应覆盖 B 中有后继 AI group 承载的被打断 turn");
    for e in &examples {
        println!("      {e}");
    }
    println!("===========================\n");
}
