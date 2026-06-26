//! `process_session_context_with_phases` —— 整条 session 的 phase 管理。
//!
//! 对齐 TS `processSessionContextWithPhases`：
//! 1. 初始 `phase = 1`；无 compact 时整条 session 是 phase 1。
//! 2. 碰到 `Chunk::Compact`：
//!    - **先 flush** 仍 pending 的被打断 user turn（见下方 turn 锚定）进累积链，
//!      使其 injection 随接下来的 backfill 落入 compact 前 phase 的 last AI group；
//!    - backfill 上一个 AI group 的 `accumulated_injections`；
//!    - finalize 当前 phase（写入 `phases` vec）；
//!    - 清空 `accumulated_injections` / `previous_paths` / `is_first_ai_group`；
//!    - `phase_number += 1`；
//!    - **保留** `last_ai_group_before_compact`，留给新 phase 的 first group 计算 delta。
//! 3. 新 phase 的第一个 AI group：若存在 `last_ai_group_before_compact`，
//!    算出 `CompactionTokenDelta { pre, post, delta }` 塞进 `compaction_token_deltas`。
//! 4. 循环结束后 flush 末尾仍 pending 的被打断 user turn，再 backfill 最后一个
//!    AI group + finalize 最后一个 phase。
//!
//! **turn 锚定**（issue #540 / #542）：turn 序号取自共享派生 [`derive_turns`]
//! （capability `turn-model`），而非在本循环内独立自增。进入循环前先 `derive_turns(chunks)`
//! 建 `chunk_id -> turn.index` 映射，给每个 `AIChunk` / `UserChunk` 的 injection 标
//! `turnIndex = map[chunk_id]`。`pending_user` 仍持有一条尚未被 AI 响应消费的真实
//! `UserChunk`，用于驱动 injection 累积链的 flush 时机（遇下一条 `Chunk::User`、
//! 遇 `Chunk::Compact`、循环结束）；被打断的 turn 照样产 `user-message` injection
//! （aiGroupId 锚 `UserChunk.chunk_id`）。
//!
//! **turn 与 phase 正交**（design D5/D9）：被 compact / 中断切出、无驱动输入的续写
//! `AIChunk` 折进所属 turn（共享同一 `turnIndex`），不再各占独立 turn 号——compact 边界
//! 由 phase 表达，不由 turn 号编码。injection id 按 chunkId 派生（见 `aggregator`），
//! 折叠后多个 group 共享 turnIndex 仍唯一。被打断 turn 不写 `stats_map`；其 injection
//! 仅靠累积链 surface，故所在 phase 无任何 AI group 承载时会丢失（已知限制——这是 phase
//! 重置导致的承载缺口，与 turn 归属正交，见 `openspec/followups.md`）。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use cdt_core::{
    AIChunk, AssistantResponse, Chunk, CompactionTokenDelta, ContextInjection, ContextPhase,
    ContextPhaseInfo, ContextStats, TokensByCategory, UserChunk,
};

use super::stats::{ComputeStatsParams, compute_context_stats};
use super::types::TokenDictionaries;
use crate::turn::derive_turns;

/// 对外入口的参数包。
pub struct ProcessSessionParams<'a> {
    pub project_root: &'a Path,
    pub token_dictionaries: TokenDictionaries<'a>,
    /// 首 phase 开始时要注入的 CLAUDE.md injection 列表。由
    /// `port-configuration-management` 未来填充；本 port 的测试直接用
    /// fixture 传入。
    pub initial_claude_md_injections: &'a [ContextInjection],
}

/// 整个 session 的 context 计算结果。
pub struct SessionContextResult {
    /// `ai_group_id → ContextStats`。
    pub stats_map: HashMap<String, ContextStats>,
    pub phase_info: ContextPhaseInfo,
}

/// 会话级入口 —— 对齐 TS `processSessionContextWithPhases`。
pub fn process_session_context_with_phases(
    chunks: &[Chunk],
    params: &ProcessSessionParams<'_>,
) -> SessionContextResult {
    // turn 序号的单一权威：先派生 turn 结构，建 chunk_id -> turn.index 映射，循环里查表
    // 标注 injection.turnIndex，不再内联自增（issue #542）。折叠后多个 AIChunk 可共享同一
    // turn 序号（compact 续写 / D9），由 aggregator 的 chunkId-based id 派生保证唯一。
    let turns = derive_turns(chunks);
    let mut turn_of: HashMap<&str, u32> = HashMap::new();
    for turn in &turns {
        for cid in &turn.member_chunk_ids {
            turn_of.insert(cid.as_str(), turn.index);
        }
    }
    let turn_index_of = |chunk_id: &str| -> u32 { turn_of.get(chunk_id).copied().unwrap_or(0) };

    let mut stats_map: HashMap<String, ContextStats> = HashMap::new();
    let mut phases: Vec<ContextPhase> = Vec::new();
    let mut ai_group_phase_map: HashMap<String, u32> = HashMap::new();
    let mut compaction_token_deltas: HashMap<String, CompactionTokenDelta> = HashMap::new();

    let mut accumulated_injections: Vec<ContextInjection> = Vec::new();
    let mut previous_paths: HashSet<String> = HashSet::new();
    let mut is_first_ai_group = true;
    // `pending_user`：一条已产出 UserChunk、但尚未被 AI 响应消费的真实用户消息。
    // 每条真实用户消息开启一个 turn（turn 序号取自 `turn_of`）；若它在下一条用户消息 /
    // compact / 会话结束之前没有产出 AIChunk（被打断），仍占一个 turn 序号并产出
    // user-message injection（spec: context-tracking "Anchor turns on real user messages"）。
    let mut pending_user: Option<&UserChunk> = None;

    let mut current_phase_number: u32 = 1;
    let mut current_phase_first_ai_group_id: Option<String> = None;
    let mut current_phase_last_ai_group_id: Option<String> = None;
    let mut current_phase_compact_group_id: Option<String> = None;
    let mut last_ai_group_before_compact: Option<&AIChunk> = None;

    // 被打断的 turn：无对应 AIChunk，其 user-message injection 的 aiGroupId 锚到
    // UserChunk 自身的 chunk_id（导航跳到用户气泡），并直接推入累积链，使其在后续
    // AI group 的 accumulated_injections 中可见。不产 stats_map 条目（无 AI group）。
    // turn 序号取自共享派生 `turn_of`（被打断 turn 在 derive_turns 里照样占一个 User 驱动
    // 的 turn 号）。
    let emit_interrupted_turn = |user: &UserChunk, accumulated: &mut Vec<ContextInjection>| {
        let ti = turn_index_of(&user.chunk_id);
        if let Some(inj) =
            super::aggregator::create_user_message_injection(user, ti, &user.chunk_id)
        {
            accumulated.push(inj);
        }
    };

    for chunk in chunks {
        match chunk {
            Chunk::User(u) => {
                // 上一条用户消息没等到 AI 响应 = 被打断的 turn，先 flush 它再开新 turn。
                if let Some(prev) = pending_user.take() {
                    emit_interrupted_turn(prev, &mut accumulated_injections);
                }
                pending_user = Some(u);
            }
            Chunk::Compact(compact) => {
                // compact 边界前的被打断 turn 归属当前 phase——先 flush 进累积链，
                // 让其 injection 随下面的 backfill 写回当前 phase 的 last AI group。
                // 注意（D9）：`[User, Compact, AIChunk]` 中 user 在 derive_turns 里**不**被
                // 判为被打断（A0 折回其 turn），但此处仍 flush 其 user-message injection 到
                // 累积链——该 injection 因压缩前 phase 无 AI carrier 而被丢弃（phase 重置承载
                // 缺口，与 turn 归属正交），与折叠前行为一致；turn 序号由 `turn_of` 决定，
                // 故 A0 仍正确取得 user 的 turn 号。
                if let Some(prev) = pending_user.take() {
                    emit_interrupted_turn(prev, &mut accumulated_injections);
                }
                // backfill 上一组 accumulated_injections
                if let Some(last_id) = &current_phase_last_ai_group_id {
                    if let Some(prev_stats) = stats_map.get_mut(last_id) {
                        prev_stats
                            .accumulated_injections
                            .clone_from(&accumulated_injections);
                    }
                }

                // finalize phase
                if let (Some(first), Some(last)) = (
                    current_phase_first_ai_group_id.as_ref(),
                    current_phase_last_ai_group_id.as_ref(),
                ) {
                    phases.push(ContextPhase {
                        phase_number: current_phase_number,
                        first_ai_group_id: first.clone(),
                        last_ai_group_id: last.clone(),
                        compact_group_id: current_phase_compact_group_id.clone(),
                    });
                }

                // reset phase-local state
                accumulated_injections.clear();
                previous_paths.clear();
                is_first_ai_group = true;
                pending_user = None;

                // start new phase
                current_phase_number += 1;
                // 用 chunk_id 而非 uuid——前端 ContextPanel 通过 chunk_id 在 DOM
                // 锚定 compact 节点，delta map key 与之对齐才能反查。
                current_phase_compact_group_id = Some(compact.chunk_id.clone());
                current_phase_first_ai_group_id = None;
                current_phase_last_ai_group_id = None;
                // last_ai_group_before_compact 刻意不 reset
            }
            Chunk::Ai(ai) => {
                let ai_group_id = ai_chunk_id(ai);

                let result = compute_context_stats(&ComputeStatsParams {
                    ai_chunk: ai,
                    ai_group_id: &ai_group_id,
                    user_chunk: pending_user,
                    // turn 序号取自共享派生：被 pending_user 消费时 user 与本 AIChunk 同 turn；
                    // 折叠续写（compact / D9）时本 AIChunk 与所属 user 共享同一 turn 号。
                    turn_index: turn_index_of(&ai_group_id),
                    is_first_group: is_first_ai_group,
                    previous_injections: &accumulated_injections,
                    previous_paths: &previous_paths,
                    initial_claude_md_injections: if is_first_ai_group && current_phase_number == 1
                    {
                        params.initial_claude_md_injections
                    } else {
                        &[]
                    },
                });

                let mut stats = result.stats;
                stats.phase_number = Some(current_phase_number);

                // 若是 phase 的第一个 AI group 且前一个 phase 有 last group，
                // 计算 CompactionTokenDelta。
                if is_first_ai_group {
                    if let (Some(compact_id), Some(prev_ai)) = (
                        current_phase_compact_group_id.as_ref(),
                        last_ai_group_before_compact,
                    ) {
                        if let (Some(pre), Some(post)) = (
                            get_last_assistant_total_tokens(prev_ai),
                            get_first_assistant_total_tokens(ai),
                        ) {
                            let pre_i = i64::try_from(pre).unwrap_or(i64::MAX);
                            let post_i = i64::try_from(post).unwrap_or(i64::MAX);
                            compaction_token_deltas.insert(
                                compact_id.clone(),
                                CompactionTokenDelta {
                                    pre_compaction_tokens: pre,
                                    post_compaction_tokens: post,
                                    delta: post_i - pre_i,
                                },
                            );
                        }
                    }
                }

                // 中间 group 的 accumulated_injections 暂存空，省 O(N²)；
                // 只有每 phase 最后一个 group 会在遇到 compact / 结束时 backfill。
                let accumulated_snapshot = std::mem::take(&mut stats.accumulated_injections);
                stats_map.insert(ai_group_id.clone(), stats);
                accumulated_injections = accumulated_snapshot;

                // phase 边界跟踪
                ai_group_phase_map.insert(ai_group_id.clone(), current_phase_number);
                if current_phase_first_ai_group_id.is_none() {
                    current_phase_first_ai_group_id = Some(ai_group_id.clone());
                }
                current_phase_last_ai_group_id = Some(ai_group_id.clone());
                last_ai_group_before_compact = Some(ai);

                previous_paths = result.next_previous_paths;
                is_first_ai_group = false;
                pending_user = None;
            }
            Chunk::System(_) => {
                // system chunks 不参与 context-tracking
            }
        }
    }

    // 会话结束时仍 pending = 末尾被打断的 turn，flush 进累积链，让其 injection
    // 经下面的 backfill 写回最后一个 AI group。
    if let Some(prev) = pending_user.take() {
        emit_interrupted_turn(prev, &mut accumulated_injections);
    }

    // session 末尾 backfill + finalize
    if let Some(last_id) = &current_phase_last_ai_group_id {
        if let Some(prev_stats) = stats_map.get_mut(last_id) {
            prev_stats
                .accumulated_injections
                .clone_from(&accumulated_injections);
        }
    }
    if let (Some(first), Some(last)) = (
        current_phase_first_ai_group_id.as_ref(),
        current_phase_last_ai_group_id.as_ref(),
    ) {
        phases.push(ContextPhase {
            phase_number: current_phase_number,
            first_ai_group_id: first.clone(),
            last_ai_group_id: last.clone(),
            compact_group_id: current_phase_compact_group_id.clone(),
        });
    }

    SessionContextResult {
        stats_map,
        phase_info: ContextPhaseInfo {
            phases,
            compaction_count: current_phase_number - 1,
            ai_group_phase_map,
            compaction_token_deltas,
        },
    }
}

/// 复用 `AIChunk.chunk_id`（由 `chunk/builder.rs::next_ai_chunk_id` 统一生成，
/// 形如 `ai:<base>:<n>`，已通过全局 `used_chunk_ids: HashSet<String>` 做
/// collision-free 兜底）。这让 `ContextInjection.aiGroupId` 与前端 chunk DOM
/// 锚点 `data-chunk-id` 字节级相等，无需任何映射层。
///
/// spec：`openspec/specs/context-tracking/spec.md` — "Expose context stats to
/// display surfaces" Requirement 的 `aiGroupId equals the corresponding AIChunk
/// chunkId` Scenario。
fn ai_chunk_id(ai: &AIChunk) -> String {
    ai.chunk_id.clone()
}

fn assistant_total_tokens(resp: &AssistantResponse) -> Option<u64> {
    resp.usage.as_ref().map(|u| {
        u.input_tokens + u.output_tokens + u.cache_read_input_tokens + u.cache_creation_input_tokens
    })
}

fn get_last_assistant_total_tokens(ai: &AIChunk) -> Option<u64> {
    for resp in ai.responses.iter().rev() {
        if let Some(total) = assistant_total_tokens(resp) {
            return Some(total);
        }
    }
    None
}

fn get_first_assistant_total_tokens(ai: &AIChunk) -> Option<u64> {
    for resp in &ai.responses {
        if let Some(total) = assistant_total_tokens(resp) {
            return Some(total);
        }
    }
    None
}

// 让 TokensByCategory 的 Default 实现参与测试断言（避免 "unused imports"）。
#[allow(dead_code)]
fn _assert_default_tokens() -> TokensByCategory {
    TokensByCategory::default()
}
