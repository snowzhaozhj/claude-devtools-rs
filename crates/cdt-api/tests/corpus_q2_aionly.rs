//! Turn 计数守卫（change `first-class-turn`）：在本机 `~/.claude/projects/` 真实语料上断言
//! `derive_turns` 产出的 turn 计数收敛于「驱动输入数」，防止续写 / 压缩 `AIChunk` 错误地各占
//! 独立 turn 号（D5 折叠的回归守卫）。
//!
//! 命令：`cargo test -p cdt-api --test corpus_q2_aionly -- --ignored --nocapture`
//! （`#[ignore]` 手动跑；CI 无 `~/.claude/projects` 自动空跑）。
//!
//! 不变量（design D4/D5 / corpus 守卫）：
//! - **非 headless turn 计数 == 驱动输入数**：每条 `UserChunk` 是一个 User 驱动 turn（被打断也算）；
//!   每个未消费 `UserChunk` 却携带 teammate 消息的 `AIChunk` 是一个 Teammate 驱动 turn。
//!   续写 / 压缩切出的无驱动 `AIChunk` 折叠，不增 turn 号。
//! - **headless turn 至多 1 个、且 index == 0**：首个驱动之前的退化前缀。
//!
//! `expected_driver_turns` 用一个独立、更简单的计数器算出「应有的驱动 turn 数」，与
//! `derive_turns` 的内部实现解耦，从而对「续写 `AIChunk` 误开 turn」这类回归有真实 teeth。
#![allow(clippy::cast_precision_loss)]

use std::path::PathBuf;

use cdt_analyze::{TurnDriver, build_chunks, derive_turns};
use cdt_core::Chunk;
use cdt_parse::parse_file;

fn projects_dir() -> PathBuf {
    cdt_discover::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("projects")
}

/// 独立计数器：应有的「驱动 turn」数（不含 headless）。与 `derive_turns` 解耦实现，
/// 故能抓住 `derive_turns` 把续写 `AIChunk` 误当驱动这类回归。
fn expected_driver_turns(chunks: &[Chunk]) -> usize {
    let mut count = 0usize;
    let mut pending_user = false;
    for c in chunks {
        match c {
            Chunk::User(_) => {
                count += 1;
                pending_user = true;
            }
            Chunk::Ai(ai) => {
                if pending_user {
                    pending_user = false; // 对 pending user 的响应，不是新驱动
                } else if !ai.teammate_messages.is_empty() {
                    count += 1; // teammate 驱动
                }
                // 否则：无驱动续写，折叠，不计
            }
            // Compact / System 不开 turn，也不打断 pending user（D9）。
            Chunk::Compact(_) | Chunk::System(_) => {}
        }
    }
    count
}

#[tokio::test]
#[ignore = "本机语料守卫，手动跑；CI 无 ~/.claude/projects 自动空跑"]
async fn corpus_turn_count_converges_to_driver_inputs() {
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
        eprintln!("无本机语料，跳过 corpus_turn_count_converges_to_driver_inputs");
        return;
    }

    let mut sessions = 0usize;
    let mut total_turns = 0usize;
    let mut total_driver_turns = 0usize;
    let mut total_headless = 0usize;
    let mut max_turns_session: (usize, String) = (0, String::new());
    let mut failures: Vec<String> = Vec::new();

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

        let turns = derive_turns(&chunks);
        let headless: Vec<&_> = turns
            .iter()
            .filter(|t| t.driver == TurnDriver::Headless)
            .collect();
        let driver_turns = turns.len() - headless.len();
        let expected = expected_driver_turns(&chunks);

        // 不变量 1：驱动 turn 数 == 独立算出的驱动输入数。
        if driver_turns != expected {
            failures.push(format!(
                "{sid:.8}: driver_turns={driver_turns} != expected={expected}（turns={}）",
                turns.len()
            ));
        }
        // 不变量 2：headless 至多 1 个、index==0。
        if headless.len() > 1 || headless.iter().any(|t| t.index != 0) {
            failures.push(format!(
                "{sid:.8}: headless 异常 count={} indices={:?}",
                headless.len(),
                headless.iter().map(|t| t.index).collect::<Vec<_>>()
            ));
        }
        // 不变量 3：turn 序号连续无空洞、单调（index == 在 vec 中的位置）。
        if let Some((pos, t)) = turns
            .iter()
            .enumerate()
            .find(|(pos, t)| u32::try_from(*pos).unwrap_or(u32::MAX) != t.index)
        {
            failures.push(format!(
                "{sid:.8}: turn 序号非连续 @pos{pos} index={}",
                t.index
            ));
        }

        total_turns += turns.len();
        total_driver_turns += driver_turns;
        total_headless += headless.len();
        if turns.len() > max_turns_session.0 {
            max_turns_session = (turns.len(), sid);
        }
    }

    println!("\n===== Turn 计数守卫 =====");
    println!("session: {sessions}");
    println!("总 turn: {total_turns}（驱动 {total_driver_turns} + headless {total_headless}）");
    println!(
        "最多 turn 的 session: {} turn @ {:.8}",
        max_turns_session.0, max_turns_session.1
    );
    println!("=========================\n");

    assert!(
        failures.is_empty(),
        "turn 计数守卫失败 {} 处:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
