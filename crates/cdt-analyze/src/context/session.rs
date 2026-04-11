//! `process_session_context_with_phases` —— 整条 session 的 phase 管理。
//!
//! 对齐 TS `processSessionContextWithPhases`：
//! 1. 初始 `phase = 1`；无 compact 时整条 session 是 phase 1。
//! 2. 碰到 `Chunk::Compact`：
//!    - backfill 上一个 AI group 的 `accumulated_injections`；
//!    - finalize 当前 phase（写入 `phases` vec）；
//!    - 清空 `accumulated_injections` / `previous_paths` / `is_first_ai_group`；
//!    - `phase_number += 1`；
//!    - **保留** `last_ai_group_before_compact`，留给新 phase 的 first group 计算 delta。
//! 3. 新 phase 的第一个 AI group：若存在 `last_ai_group_before_compact`，
//!    算出 `CompactionTokenDelta { pre, post, delta }` 塞进 `compaction_token_deltas`。
//! 4. 循环结束后 backfill 最后一个 AI group + finalize 最后一个 phase。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use cdt_core::{
    AIChunk, AssistantResponse, Chunk, CompactionTokenDelta, ContextInjection, ContextPhase,
    ContextPhaseInfo, ContextStats, TokensByCategory, UserChunk,
};

use super::stats::{ComputeStatsParams, compute_context_stats};
use super::types::TokenDictionaries;

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
    let mut stats_map: HashMap<String, ContextStats> = HashMap::new();
    let mut phases: Vec<ContextPhase> = Vec::new();
    let mut ai_group_phase_map: HashMap<String, u32> = HashMap::new();
    let mut compaction_token_deltas: HashMap<String, CompactionTokenDelta> = HashMap::new();

    let mut accumulated_injections: Vec<ContextInjection> = Vec::new();
    let mut previous_paths: HashSet<String> = HashSet::new();
    let mut is_first_ai_group = true;
    let mut previous_user_chunk: Option<&UserChunk> = None;

    let mut current_phase_number: u32 = 1;
    let mut current_phase_first_ai_group_id: Option<String> = None;
    let mut current_phase_last_ai_group_id: Option<String> = None;
    let mut current_phase_compact_group_id: Option<String> = None;
    let mut last_ai_group_before_compact: Option<&AIChunk> = None;

    let mut turn_index: u32 = 0;

    for chunk in chunks {
        match chunk {
            Chunk::User(u) => {
                previous_user_chunk = Some(u);
            }
            Chunk::Compact(compact) => {
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
                previous_user_chunk = None;

                // start new phase
                current_phase_number += 1;
                current_phase_compact_group_id = Some(compact.uuid.clone());
                current_phase_first_ai_group_id = None;
                current_phase_last_ai_group_id = None;
                // last_ai_group_before_compact 刻意不 reset
            }
            Chunk::Ai(ai) => {
                let ai_group_id = ai_chunk_id(ai, turn_index);

                let result = compute_context_stats(&ComputeStatsParams {
                    ai_chunk: ai,
                    ai_group_id: &ai_group_id,
                    user_chunk: previous_user_chunk,
                    turn_index,
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
                previous_user_chunk = None;
                turn_index += 1;
            }
            Chunk::System(_) => {
                // system chunks 不参与 context-tracking
            }
        }
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

/// 给 `AIChunk` 分配一个稳定的 id。
///
/// `AIChunk` 在 `cdt-core` 里目前不带 id 字段，这里用第一条 response 的
/// uuid 作为 key；没有 response 时退化为 `ai-<turn_index>`。
fn ai_chunk_id(ai: &AIChunk, turn_index: u32) -> String {
    ai.responses
        .first()
        .map_or_else(|| format!("ai-{turn_index}"), |r| r.uuid.clone())
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
