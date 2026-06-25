//! turn-model capability —— `derive_turns` 是 turn 边界 + 编号的**单一共享权威**。
//!
//! Spec：`openspec/specs/turn-model/spec.md`。
//!
//! turn = 「一个驱动输入（一条真实用户消息；或无用户消息驱动时一条进入的 teammate
//! 消息）连同其后所有 assistant 响应与工具调用，直到 assistant 停下等待下一个驱动
//! 输入」——与 Claude Code 以 `stop_reason: end_turn` 界定的一轮对话对齐（design D1）。
//! 中途的自动压缩（`Chunk::Compact`）**不**结束 turn，压缩边界由正交的 `phase` 表达。
//!
//! 本模块只产 turn **边界**（每个 turn 含哪些 chunk）；不算 step 内容、不碰 IPC wire。
//! `context-tracking`（`context::session`）消费它标注 `injection.turnIndex`；未来 CLI/MCP
//! `get_turn` 也消费同一派生，从源头消除「桌面 Turn N 与 API turn 序号分叉」。

use cdt_core::Chunk;

/// 一个 turn 的驱动来源（design D3）。
///
/// 每个 `AIChunk` 取一个 driver 归属：消费了前置 `UserChunk` → 折进该 `User` turn；
/// 否则携带进入的 teammate 消息 → 自身为 `Teammate` driver；否则折进前一 turn（无前驱
/// 则归 `Headless` turn 0）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnDriver {
    /// 真实用户消息驱动，携带该 `UserChunk` 的 `chunk_id`。
    User(String),
    /// teammate 消息驱动（无真实用户消息时），携带该次响应批量处理的**全部**
    /// teammate message uuid——N 条 teammate 消息进同一个 `AIChunk` 仍是一个 turn
    /// （一次响应 = 一次交换，design D3 / codex W8）。
    Teammate(Vec<String>),
    /// 无驱动：首个驱动输入之前的退化前缀内容（resumed/fork，实测近乎为空，保留作
    /// 防御性兜底）。
    Headless,
}

/// 一次「提问—响应」对话轮（design D3）。
///
/// `index` 单调递增、连续无空洞。`member_chunk_ids` 按时间顺序列出该 turn 拥有的全部
/// chunk（user / ai / compact / system）；被打断的 turn 只含其 `UserChunk`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub index: u32,
    pub driver: TurnDriver,
    pub member_chunk_ids: Vec<String>,
}

/// 把 chunk 时间线按 driver 切成 turn 序列（design D4/D5）。
///
/// 规则（一条规则统一所有边界）：
/// - 每个驱动输入（`UserChunk`，或没消费 `UserChunk` 却携带 teammate 消息的 `AIChunk`）
///   开启一个新 turn；
/// - 其后所有 chunk（续写 `AIChunk` / `Compact` / `System`）归属最近一个驱动开启的 turn；
/// - 首个驱动之前的 driver-less `AIChunk` 归属无驱动的 `Headless` turn 0；首个驱动之前
///   单独出现的 `Compact` / `System`（无 pre-driver AI）缓冲到首个真实 turn，不另立 turn；
/// - `Compact` / `System` 永不开启 turn；
/// - 压缩**不**打断 pending user：`[User, Compact, AIChunk]` 中 `AIChunk` 折进 user 的 turn，
///   user 不被判为被打断（design D9）。
#[must_use]
pub fn derive_turns(chunks: &[Chunk]) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    // pending_user：当前 turn 由一条 `UserChunk` 开启且尚未被 AI 响应消费。压缩**不**清它
    // （design D9），故压缩后的续写仍能折回该 user 的 turn。
    let mut pending_user = false;
    // 首个 turn 之前单独出现的 `Compact` / `System`，无 turn 可归属，缓冲到首个真实 turn
    // 创建时一并并入（避免给裸 compact 前缀凭空多造一个 headless turn，让其后的首条真实
    // 用户消息错位到 turn 1）。
    let mut leading_buffer: Vec<String> = Vec::new();

    for chunk in chunks {
        match chunk {
            Chunk::User(u) => {
                push_new_turn(
                    &mut turns,
                    &mut leading_buffer,
                    TurnDriver::User(u.chunk_id.clone()),
                    &u.chunk_id,
                );
                pending_user = true;
            }
            Chunk::Ai(ai) => {
                if pending_user {
                    // 对 pending user 的响应 → 折进其 turn。
                    if let Some(last) = turns.last_mut() {
                        last.member_chunk_ids.push(ai.chunk_id.clone());
                    }
                    pending_user = false;
                } else if !ai.teammate_messages.is_empty() {
                    // teammate 驱动 → 新 turn，driver 装该批全部 uuid。
                    let uuids = ai
                        .teammate_messages
                        .iter()
                        .map(|m| m.uuid.clone())
                        .collect();
                    push_new_turn(
                        &mut turns,
                        &mut leading_buffer,
                        TurnDriver::Teammate(uuids),
                        &ai.chunk_id,
                    );
                } else if let Some(last) = turns.last_mut() {
                    // 无驱动续写 → 折进当前 turn。
                    last.member_chunk_ids.push(ai.chunk_id.clone());
                } else {
                    // 首个驱动之前的 driver-less AI → headless turn 0。
                    push_new_turn(
                        &mut turns,
                        &mut leading_buffer,
                        TurnDriver::Headless,
                        &ai.chunk_id,
                    );
                }
            }
            // Compact / System 永不开 turn：有当前 turn 则并入，否则缓冲到首个真实 turn。
            // pending_user 保持不变（design D9：压缩不打断 pending user）。
            Chunk::Compact(c) => append_or_buffer(&mut turns, &mut leading_buffer, &c.chunk_id),
            Chunk::System(s) => append_or_buffer(&mut turns, &mut leading_buffer, &s.chunk_id),
        }
    }

    turns
}

/// 创建一个新 turn：先并入 `leading_buffer`（首个 turn 之前缓冲的 compact/system），
/// 再追加驱动自身的 `chunk_id`，`index` 取当前 turn 数。
fn push_new_turn(
    turns: &mut Vec<Turn>,
    leading_buffer: &mut Vec<String>,
    driver: TurnDriver,
    driver_chunk_id: &str,
) {
    let mut members = std::mem::take(leading_buffer);
    members.push(driver_chunk_id.to_string());
    let index = u32::try_from(turns.len()).unwrap_or(u32::MAX);
    turns.push(Turn {
        index,
        driver,
        member_chunk_ids: members,
    });
}

/// `Compact` / `System` 归属：有当前 turn 则并入其成员，否则缓冲等首个真实 turn。
fn append_or_buffer(turns: &mut [Turn], leading_buffer: &mut Vec<String>, chunk_id: &str) {
    if let Some(last) = turns.last_mut() {
        last.member_chunk_ids.push(chunk_id.to_string());
    } else {
        leading_buffer.push(chunk_id.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{
        AIChunk, ChunkMetrics, CompactChunk, MessageContent, SystemChunk, TeammateMessage,
        UserChunk,
    };
    use chrono::{DateTime, Utc};

    fn ts() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-06-25T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn user(id: &str) -> Chunk {
        Chunk::User(UserChunk {
            chunk_id: id.into(),
            uuid: id.into(),
            timestamp: ts(),
            duration_ms: None,
            content: MessageContent::Text("hi".into()),
            metrics: ChunkMetrics::zero(),
        })
    }

    fn ai(id: &str) -> Chunk {
        ai_with_teammates(id, &[])
    }

    fn ai_with_teammates(id: &str, teammate_uuids: &[&str]) -> Chunk {
        let teammate_messages = teammate_uuids
            .iter()
            .map(|u| TeammateMessage {
                uuid: (*u).into(),
                teammate_id: "alice".into(),
                color: None,
                summary: None,
                body: "body".into(),
                timestamp: ts(),
                reply_to_tool_use_id: None,
                token_count: None,
                is_noise: false,
                is_resend: false,
            })
            .collect();
        Chunk::Ai(AIChunk {
            chunk_id: id.into(),
            timestamp: ts(),
            duration_ms: None,
            responses: Vec::new(),
            metrics: ChunkMetrics::zero(),
            semantic_steps: Vec::new(),
            tool_executions: Vec::new(),
            subagents: Vec::new(),
            slash_commands: Vec::new(),
            teammate_messages,
        })
    }

    fn compact(id: &str) -> Chunk {
        Chunk::Compact(CompactChunk {
            chunk_id: id.into(),
            uuid: id.into(),
            timestamp: ts(),
            duration_ms: None,
            summary_text: "summary".into(),
            metrics: ChunkMetrics::zero(),
            token_delta: None,
            phase_number: None,
        })
    }

    fn system(id: &str) -> Chunk {
        Chunk::System(SystemChunk {
            chunk_id: id.into(),
            uuid: id.into(),
            timestamp: ts(),
            duration_ms: None,
            content_text: "out".into(),
            metrics: ChunkMetrics::zero(),
        })
    }

    /// 抽 `(index, driver, members)` 便于断言。
    fn shape(turns: &[Turn]) -> Vec<(u32, &TurnDriver, Vec<&str>)> {
        turns
            .iter()
            .map(|t| {
                (
                    t.index,
                    &t.driver,
                    t.member_chunk_ids.iter().map(String::as_str).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn empty_input_yields_no_turns() {
        assert!(derive_turns(&[]).is_empty());
    }

    #[test]
    fn one_question_and_full_response_pair_into_one_turn() {
        let turns = derive_turns(&[user("u0"), ai("a0")]);
        assert_eq!(
            shape(&turns),
            vec![(0, &TurnDriver::User("u0".into()), vec!["u0", "a0"])]
        );
    }

    // ===== D5：compact 跨界折叠 =====

    #[test]
    fn compaction_only_continuation_folds_into_its_turn() {
        // [U0, A0, Compact, A1, U2]：A0/A1 同属 U0 的 turn；U2 开下一 turn。
        let turns = derive_turns(&[user("u0"), ai("a0"), compact("c0"), ai("a1"), user("u2")]);
        assert_eq!(
            shape(&turns),
            vec![
                (
                    0,
                    &TurnDriver::User("u0".into()),
                    vec!["u0", "a0", "c0", "a1"]
                ),
                (1, &TurnDriver::User("u2".into()), vec!["u2"]),
            ]
        );
    }

    #[test]
    fn turn_spans_compaction_phase_boundary_two_groups_share_index() {
        // 两个 AI group 折进同一 turn → 同一 index。
        let turns = derive_turns(&[user("u0"), ai("a0"), compact("c0"), ai("a1")]);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].index, 0);
        assert_eq!(turns[0].member_chunk_ids, vec!["u0", "a0", "c0", "a1"]);
    }

    // ===== D9：[User, Compact, AIChunk] 折回 user 的 turn =====

    #[test]
    fn compaction_before_any_ai_group_folds_into_user_turn() {
        // [U1, Compact, A0]：A0 折进 U1 的 turn（turn 跨压缩），U1 不被判为被打断。
        let turns = derive_turns(&[user("u1"), compact("c0"), ai("a0")]);
        assert_eq!(
            shape(&turns),
            vec![(0, &TurnDriver::User("u1".into()), vec!["u1", "c0", "a0"])]
        );
    }

    // ===== headless 前缀 =====

    #[test]
    fn leading_ai_before_first_driver_is_headless_turn_zero() {
        // [A0, U1, A1]：A0 在首驱动前 → headless turn 0；U1 → turn 1。
        let turns = derive_turns(&[ai("a0"), user("u1"), ai("a1")]);
        assert_eq!(
            shape(&turns),
            vec![
                (0, &TurnDriver::Headless, vec!["a0"]),
                (1, &TurnDriver::User("u1".into()), vec!["u1", "a1"]),
            ]
        );
    }

    #[test]
    fn session_starting_with_compact_marker_then_ai_is_headless_turn_zero() {
        // [Compact, A0]：Compact 不开 turn；A0 在首驱动前 → headless turn 0，compact 并入。
        let turns = derive_turns(&[compact("c0"), ai("a0")]);
        assert_eq!(
            shape(&turns),
            vec![(0, &TurnDriver::Headless, vec!["c0", "a0"])]
        );
    }

    #[test]
    fn consecutive_leading_compacts_do_not_each_open_a_turn() {
        // [Compact, Compact, A0]：两个 compact 都不开 turn，A0 归 headless turn 0。
        let turns = derive_turns(&[compact("c0"), compact("c1"), ai("a0")]);
        assert_eq!(
            shape(&turns),
            vec![(0, &TurnDriver::Headless, vec!["c0", "c1", "a0"])]
        );
    }

    #[test]
    fn leading_compact_before_user_does_not_shift_first_user_to_turn_one() {
        // [Compact, U0, A0]：裸 compact 前缀（无 pre-driver AI）不另立 headless turn；
        // U0 仍是 turn 0，compact 并入 U0 的 turn。
        let turns = derive_turns(&[compact("c0"), user("u0"), ai("a0")]);
        assert_eq!(
            shape(&turns),
            vec![(0, &TurnDriver::User("u0".into()), vec!["c0", "u0", "a0"])]
        );
    }

    #[test]
    fn leading_system_before_user_does_not_shift_first_user_to_turn_one() {
        let turns = derive_turns(&[system("s0"), user("u0"), ai("a0")]);
        assert_eq!(
            shape(&turns),
            vec![(0, &TurnDriver::User("u0".into()), vec!["s0", "u0", "a0"])]
        );
    }

    // ===== teammate 会话 =====

    #[test]
    fn teammate_driven_response_opens_a_teammate_turn() {
        // [teammate→A0, U1]：teammate-carrying A0 = driver → turn 0；U1 → turn 1。
        let turns = derive_turns(&[ai_with_teammates("a0", &["tm1"]), user("u1")]);
        assert_eq!(
            shape(&turns),
            vec![
                (0, &TurnDriver::Teammate(vec!["tm1".into()]), vec!["a0"]),
                (1, &TurnDriver::User("u1".into()), vec!["u1"]),
            ]
        );
    }

    #[test]
    fn batched_teammate_messages_in_one_response_count_as_one_turn() {
        // N 条 teammate 消息进同一个 AIChunk = 1 个 turn，driver 装全部 uuid。
        let turns = derive_turns(&[ai_with_teammates("a0", &["tm1", "tm2", "tm3"])]);
        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].driver,
            TurnDriver::Teammate(vec!["tm1".into(), "tm2".into(), "tm3".into()])
        );
    }

    #[test]
    fn user_message_driver_takes_priority_over_teammate_in_same_response() {
        // pending user 存在时，承载 teammate 消息的 AIChunk 仍是对 user 的响应（User driver），
        // 不另开 teammate turn。
        let turns = derive_turns(&[user("u0"), ai_with_teammates("a0", &["tm1"])]);
        assert_eq!(
            shape(&turns),
            vec![(0, &TurnDriver::User("u0".into()), vec!["u0", "a0"])]
        );
    }

    // ===== 被打断 turn =====

    #[test]
    fn interrupted_user_message_still_opens_a_turn() {
        // [U1, U2, A2]：U1 被打断（仅含 UserChunk）；U2 → turn 1，A2 折进 turn 1。
        let turns = derive_turns(&[user("u1"), user("u2"), ai("a2")]);
        assert_eq!(
            shape(&turns),
            vec![
                (0, &TurnDriver::User("u1".into()), vec!["u1"]),
                (1, &TurnDriver::User("u2".into()), vec!["u2", "a2"]),
            ]
        );
    }

    #[test]
    fn trailing_interrupted_user_at_end_of_session_still_a_turn() {
        let turns = derive_turns(&[user("u0"), ai("a0"), user("u1")]);
        assert_eq!(
            shape(&turns),
            vec![
                (0, &TurnDriver::User("u0".into()), vec!["u0", "a0"]),
                (1, &TurnDriver::User("u1".into()), vec!["u1"]),
            ]
        );
    }

    #[test]
    fn turn_indices_are_contiguous_and_monotonic() {
        let turns = derive_turns(&[
            user("u0"),
            ai("a0"),
            user("u1"),
            ai("a1"),
            user("u2"),
            ai("a2"),
        ]);
        let indices: Vec<u32> = turns.iter().map(|t| t.index).collect();
        assert_eq!(indices, vec![0, 1, 2]);
    }
}
