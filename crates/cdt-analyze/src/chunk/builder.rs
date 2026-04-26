//! `build_chunks`：从已解析消息流构造 chunk 序列。
//!
//! Spec：`openspec/specs/chunk-building/spec.md`。
//!
//! 状态机：
//! 1. 过滤 `is_sidechain` 与 `HardNoise`；
//! 2. 顺序扫描剩余消息：
//!    - 遇 `MessageCategory::Assistant` → 累进 assistant buffer；
//!    - 遇 `MessageCategory::Compact` → 先 flush buffer，再产出 `CompactChunk`；
//!    - 遇 `MessageCategory::User` →
//!         - 若内容精确被 `<local-command-stdout>…</local-command-stdout>`
//!           包裹且非空 → flush buffer，产出 `SystemChunk`；
//!         - 若是"只含 `tool_result`"的回传 → 附加到 buffer 最后一条 assistant
//!           响应的 `tool_results`；buffer 为空则降级为普通 `UserChunk`；
//!         - 否则 → flush buffer，产出 `UserChunk`；
//! 3. 末尾 flush。

use std::collections::{HashMap, HashSet};

use cdt_core::{
    AIChunk, AssistantResponse, Chunk, ChunkMetrics, CompactChunk, ContentBlock, MessageCategory,
    MessageContent, ParsedMessage, SemanticStep, SlashCommand, SystemChunk, TeammateMessage,
    ToolExecution, UserChunk,
};

use cdt_core::SubagentCandidate;

use super::metrics::aggregate_metrics;
use super::semantic::extract_semantic_steps;
use crate::team::{
    detect_noise, detect_resend, is_teammate_message, link_teammate_to_send_message,
    parse_all_teammate_attrs,
};
use crate::tool_linking::{
    Resolution, ResolvedTask, filter_resolved_tasks, pair_tool_executions, resolve_subagents,
};

const STDOUT_OPEN: &str = "<local-command-stdout>";
const STDOUT_CLOSE: &str = "</local-command-stdout>";

/// Teammate 消息嵌入 `AIChunk` 的回滚开关。
///
/// `true`（默认）：teammate user 消息不产 `UserChunk`，转化为 [`TeammateMessage`]
/// 注入到下一个 flush 出的 `AIChunk.teammate_messages`，并向后扫描 `SendMessage`
/// 配对 `reply_to_tool_use_id`。
///
/// `false`：退回旧行为——teammate user 消息直接 `continue` 丢弃，
/// `AIChunk.teammate_messages` 永远为空（`skip_serializing_if` 让 IPC payload 兼容）。
///
/// Spec：`openspec/specs/chunk-building/spec.md` §`Embed teammate messages into AIChunk`。
const EMBED_TEAMMATES: bool = true;

/// `TeammateMessage.token_count` 缺失 `usage` 时的字符 → token 启发式除数。
///
/// 与既有 `cdt_core::estimate_tokens` 的 4 字符/token 估算口径一致。
const TEAMMATE_BODY_CHARS_PER_TOKEN: u64 = 4;

pub fn build_chunks(messages: &[ParsedMessage]) -> Vec<Chunk> {
    let linking = pair_tool_executions(messages);
    let mut executions_by_assistant: HashMap<String, Vec<ToolExecution>> = HashMap::new();
    for exec in linking.executions {
        executions_by_assistant
            .entry(exec.source_assistant_uuid.clone())
            .or_default()
            .push(exec);
    }

    let follow_ups = build_slash_follow_up_map(messages);
    let mut out: Vec<Chunk> = Vec::new();
    let mut buffer: Vec<AssistantResponse> = Vec::new();
    let mut pending_slashes: Vec<SlashCommand> = Vec::new();
    let mut pending_teammates: Vec<TeammateMessage> = Vec::new();
    let mut used_send_message_ids: HashSet<String> = HashSet::new();

    chunk_loop(
        messages,
        &mut buffer,
        &mut out,
        &mut executions_by_assistant,
        &mut pending_slashes,
        &mut pending_teammates,
        &mut used_send_message_ids,
        &follow_ups,
    );

    flush_buffer(
        &mut buffer,
        &mut out,
        &mut executions_by_assistant,
        &mut pending_slashes,
        &mut pending_teammates,
        &mut used_send_message_ids,
    );
    drain_trailing_teammates(&mut out, &mut pending_teammates, &mut used_send_message_ids);
    out
}

/// 预扫 messages 建立 `parent_uuid → instructions_text` 映射。
///
/// Slash 命令的 follow-up 指令文本是 `is_meta=true` 且 `parent_uuid` 指向 slash
/// 消息 uuid 的 user 消息，其 content 的第一个 text block。在 chunk-building
/// 前一次性建 map，slash 分支按 `msg.uuid` 查表注入 instructions。
fn build_slash_follow_up_map(messages: &[ParsedMessage]) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for msg in messages {
        if !msg.is_meta {
            continue;
        }
        let Some(parent) = msg.parent_uuid.as_ref() else {
            continue;
        };
        if msg.source_tool_use_id.is_some() {
            continue;
        }
        let MessageContent::Blocks(blocks) = &msg.content else {
            continue;
        };
        for b in blocks {
            if let ContentBlock::Text { text } = b {
                if !text.is_empty() {
                    map.entry(parent.clone()).or_insert_with(|| text.clone());
                    break;
                }
            }
        }
    }
    map
}

/// 主循环：遍历消息序列产出 chunk，被 `build_chunks` 和
/// `build_chunks_with_subagents` 共用。
#[allow(clippy::too_many_arguments)]
fn chunk_loop(
    messages: &[ParsedMessage],
    buffer: &mut Vec<AssistantResponse>,
    out: &mut Vec<Chunk>,
    executions_by_assistant: &mut HashMap<String, Vec<ToolExecution>>,
    pending_slashes: &mut Vec<SlashCommand>,
    pending_teammates: &mut Vec<TeammateMessage>,
    used_send_message_ids: &mut HashSet<String>,
    follow_ups: &HashMap<String, String>,
) {
    for msg in messages {
        if msg.is_sidechain || msg.category.is_hard_noise() {
            continue;
        }

        match &msg.category {
            MessageCategory::Assistant => {
                buffer.push(AssistantResponse {
                    uuid: msg.uuid.clone(),
                    timestamp: msg.timestamp,
                    content: msg.content.clone(),
                    tool_calls: msg.tool_calls.clone(),
                    usage: msg.usage.clone(),
                    model: msg.model.clone(),
                    content_omitted: false,
                });
            }
            MessageCategory::Compact => {
                flush_buffer(
                    buffer,
                    out,
                    executions_by_assistant,
                    pending_slashes,
                    pending_teammates,
                    used_send_message_ids,
                );
                out.push(Chunk::Compact(CompactChunk {
                    uuid: msg.uuid.clone(),
                    timestamp: msg.timestamp,
                    duration_ms: None,
                    summary_text: extract_plain_text(&msg.content),
                    metrics: ChunkMetrics::zero(),
                }));
            }
            MessageCategory::User => {
                // Teammate 消息不产出 `UserChunk`（spec: team-coordination-metadata）。
                // EMBED_TEAMMATES=true 时 push 到 pending_teammates 等下一次 flush 注入；
                // EMBED_TEAMMATES=false 时退回旧"丢弃"行为（直接 continue）。
                if is_teammate_message(msg) {
                    if EMBED_TEAMMATES {
                        // 一条 user 消息可能含 N 个 <teammate-message> 块，每块各产
                        // 一条 TeammateMessage（多 block 修复，对齐原版）。
                        pending_teammates.extend(build_pending_teammates(msg));
                    }
                    continue;
                }
                // `is_meta` 消息是 skill prompt / system-reminder 注入，
                // 不是真正用户输入——跳过但仍需处理 tool_result 合并
                if msg.is_meta {
                    if is_tool_result_only(&msg.content) {
                        if let Some(last) = buffer.last_mut() {
                            append_tool_results(last, &msg.content);
                        }
                    }
                    continue;
                }
                // Slash 命令消息（<command-name>/xxx</command-name>）：
                // 对齐原版——既产出 UserChunk（UI 侧 cleanDisplayText 会把 XML
                // 清洗为 `/name args` 气泡），也把 slash 信息留给下一个 AIChunk 的
                // `slash_commands`（供 AI group 内 SlashItem 展示 instructions）。
                if let Some(mut slash) = extract_slash_info(&msg.content, &msg.uuid, msg.timestamp)
                {
                    if let Some(instructions) = follow_ups.get(&msg.uuid) {
                        slash.instructions = Some(instructions.clone());
                    }
                    flush_buffer(
                        buffer,
                        out,
                        executions_by_assistant,
                        pending_slashes,
                        pending_teammates,
                        used_send_message_ids,
                    );
                    out.push(Chunk::User(UserChunk {
                        uuid: msg.uuid.clone(),
                        timestamp: msg.timestamp,
                        duration_ms: None,
                        content: msg.content.clone(),
                        metrics: ChunkMetrics::zero(),
                    }));
                    pending_slashes.push(slash);
                    continue;
                }
                if let Some(stdout) = extract_local_command_stdout(&msg.content) {
                    flush_buffer(
                        buffer,
                        out,
                        executions_by_assistant,
                        pending_slashes,
                        pending_teammates,
                        used_send_message_ids,
                    );
                    out.push(Chunk::System(SystemChunk {
                        uuid: msg.uuid.clone(),
                        timestamp: msg.timestamp,
                        duration_ms: None,
                        content_text: stdout,
                        metrics: ChunkMetrics::zero(),
                    }));
                } else if is_tool_result_only(&msg.content) {
                    // tool_result only 的用户消息合并到前一个 assistant buffer；
                    // buffer 为空时丢弃——这些不是真正的用户输入
                    if let Some(last) = buffer.last_mut() {
                        append_tool_results(last, &msg.content);
                    }
                } else {
                    flush_buffer(
                        buffer,
                        out,
                        executions_by_assistant,
                        pending_slashes,
                        pending_teammates,
                        used_send_message_ids,
                    );
                    // 普通用户输入会"打断" slash → AIChunk 的紧邻关系：
                    // 对齐原版 extractPrecedingSlashInfo 只看紧邻前一个 UserGroup 的语义，
                    // 未被 AIChunk 消费的 slash 在此抛弃，不会跨过这条 user 挂到后续 AI。
                    pending_slashes.clear();
                    out.push(Chunk::User(UserChunk {
                        uuid: msg.uuid.clone(),
                        timestamp: msg.timestamp,
                        duration_ms: None,
                        content: msg.content.clone(),
                        metrics: ChunkMetrics::zero(),
                    }));
                }
            }
            MessageCategory::Interruption => {
                // 先 flush 已有 assistant buffer 产出 AIChunk；再把
                // Interruption 追加到最后一个 AIChunk 的 semantic_steps。
                // 没有前驱 AIChunk 时丢弃（对齐原版：孤立中断不产出新 chunk）。
                flush_buffer(
                    buffer,
                    out,
                    executions_by_assistant,
                    pending_slashes,
                    pending_teammates,
                    used_send_message_ids,
                );
                append_interruption_to_last_ai(out, msg);
            }
            // `System` 这个 variant 在 parser 端被 hard-noise 前置拦截，
            // 实际不会走到这里；保留分支只是为了避免漏 match 告警。
            MessageCategory::System | MessageCategory::HardNoise(_) => {}
        }
    }
}

/// 解析 teammate user 消息为 N 条 [`TeammateMessage`]——一条 user 消息可能含
/// 多个 `<teammate-message>` 块（典型场景：team 启动阶段把多个 `idle_notification`
/// 拼到同一条消息），需要各自产出独立卡片。
///
/// 多 block 时 uuid 加 `-N` 后缀去重，避免下游 `{#each}` key 冲突；
/// timestamp 共享原 user msg 的时间戳（chunk-building 不强求微秒区分，UI
/// timestamp 排序时同 ts 按出现顺序稳定排列即可）。
///
/// 属性解析失败（无效 teammate 标签）返回空 Vec，主循环直接丢弃。
fn build_pending_teammates(msg: &ParsedMessage) -> Vec<TeammateMessage> {
    let all = parse_all_teammate_attrs(msg);
    if all.is_empty() {
        return Vec::new();
    }
    let multiple = all.len() > 1;
    all.into_iter()
        .enumerate()
        .map(|(idx, attrs)| {
            let body = attrs.body.trim().to_owned();
            let is_noise = detect_noise(&body, &attrs.teammate_id);
            let is_resend = detect_resend(attrs.summary.as_deref(), &body);
            // 多 block 时 token 估算用 body 字符数（无法切分原 usage）；单 block 走原逻辑。
            let token_count = if multiple {
                let chars = body.chars().count() as u64;
                if chars == 0 {
                    None
                } else {
                    Some(chars.div_ceil(TEAMMATE_BODY_CHARS_PER_TOKEN))
                }
            } else {
                estimate_teammate_tokens(msg, &body)
            };
            let uuid = if multiple {
                format!("{}-{idx}", msg.uuid)
            } else {
                msg.uuid.clone()
            };
            TeammateMessage {
                uuid,
                teammate_id: attrs.teammate_id,
                color: attrs.color,
                summary: attrs.summary,
                body,
                timestamp: msg.timestamp,
                reply_to_tool_use_id: None,
                token_count,
                is_noise,
                is_resend,
            }
        })
        .collect()
}

/// teammate body 灌入主 session 的 token 估算：
/// 优先取 `msg.usage.input_tokens` 真实值；缺失时退化到 body 字符数 ÷ 4 启发式。
fn estimate_teammate_tokens(msg: &ParsedMessage, body: &str) -> Option<u64> {
    if let Some(usage) = msg.usage.as_ref() {
        if usage.input_tokens > 0 {
            return Some(usage.input_tokens);
        }
    }
    let chars = body.chars().count() as u64;
    if chars == 0 {
        return None;
    }
    Some(chars.div_ceil(TEAMMATE_BODY_CHARS_PER_TOKEN))
}

/// 主循环结束时把仍在 `pending_teammates` 中的条目追加到最后一个 `AIChunk`。
///
/// `out` 末尾不是 `AIChunk`（或全为空、或没有任何 `AIChunk`）时静默丢弃，
/// 与 chunk-building spec 的 orphan teammate 边界规则一致。
fn drain_trailing_teammates(
    out: &mut [Chunk],
    pending_teammates: &mut Vec<TeammateMessage>,
    used_send_message_ids: &mut HashSet<String>,
) {
    if pending_teammates.is_empty() {
        return;
    }
    let last_ai_idx = out.iter().rposition(|c| matches!(c, Chunk::Ai(_)));
    let Some(idx) = last_ai_idx else {
        // 没有任何 AIChunk → 丢弃 trailing teammates，避免 panic。
        pending_teammates.clear();
        return;
    };
    // 给 link 用的 candidate chain：last AI + 此前最近 N-1 个（最近优先）。
    let mut linked: Vec<TeammateMessage> = Vec::with_capacity(pending_teammates.len());
    for mut tm in pending_teammates.drain(..) {
        let link_target =
            link_against_chunks(&tm.teammate_id, out, idx, used_send_message_ids, None);
        tm.reply_to_tool_use_id = link_target;
        linked.push(tm);
    }
    if let Chunk::Ai(ai) = &mut out[idx] {
        ai.teammate_messages.extend(linked);
    }
}

/// 收集 `out[..=until_idx]` 中"最近优先"的最多 `LOOKBACK_LIMIT` 个 `AIChunk` 引用，
/// 可选叠加一个尚未 push 的 `pending_chunk`（放在最前），调 [`link_teammate_to_send_message`]。
fn link_against_chunks<'a>(
    teammate_id: &str,
    emitted: &'a [Chunk],
    until_idx: usize,
    used: &mut HashSet<String>,
    pending_chunk: Option<&'a AIChunk>,
) -> Option<String> {
    let mut chain: Vec<&AIChunk> = Vec::new();
    if let Some(p) = pending_chunk {
        chain.push(p);
    }
    for chunk in emitted[..=until_idx].iter().rev() {
        if chain.len() >= crate::team::reply_link::LOOKBACK_LIMIT {
            break;
        }
        if let Chunk::Ai(ai) = chunk {
            chain.push(ai);
        }
    }
    link_teammate_to_send_message(teammate_id, &chain, used)
}

/// 把 `Interruption` 消息追加为最后一个 `AIChunk` 的 `SemanticStep::Interruption`。
///
/// 若 `out` 末尾不是 `AIChunk`（或完全为空），静默丢弃——这与原版
/// `SemanticStep` 序列中孤立中断不占位的行为一致。
fn append_interruption_to_last_ai(out: &mut [Chunk], msg: &ParsedMessage) {
    if let Some(Chunk::Ai(ai)) = out.iter_mut().rev().find(|c| matches!(c, Chunk::Ai(_))) {
        ai.semantic_steps.push(SemanticStep::Interruption {
            text: extract_plain_text(&msg.content),
            timestamp: msg.timestamp,
        });
    }
}

/// 带 subagent 候选的 chunk 构建。
///
/// 在 `build_chunks` 基础上额外：
/// 1. 调用 `resolve_subagents` 匹配 Task → subagent session
/// 2. 调用 `filter_resolved_tasks` 从 execution 列表过滤已 resolve 的 Task
///
/// 调用方负责装载 `SubagentCandidate` 列表（从磁盘扫描 subagent session）。
pub fn build_chunks_with_subagents(
    messages: &[ParsedMessage],
    candidates: &[SubagentCandidate],
) -> Vec<Chunk> {
    let linking = pair_tool_executions(messages);

    let task_calls: Vec<_> = messages
        .iter()
        .flat_map(|m| m.tool_calls.iter())
        .filter(|tc| tc.is_task)
        .cloned()
        .collect();

    let resolved = resolve_subagents(&task_calls, &linking.executions, candidates);

    // 构建 task_use_id → source_assistant_uuid 映射
    let task_to_assistant: HashMap<String, String> = linking
        .executions
        .iter()
        .filter(|e| task_calls.iter().any(|t| t.id == e.tool_use_id))
        .map(|e| (e.tool_use_id.clone(), e.source_assistant_uuid.clone()))
        .collect();

    let mut executions = linking.executions;
    filter_resolved_tasks(&mut executions, &resolved);

    let mut executions_by_assistant: HashMap<String, Vec<ToolExecution>> = HashMap::new();
    for exec in executions {
        executions_by_assistant
            .entry(exec.source_assistant_uuid.clone())
            .or_default()
            .push(exec);
    }

    let follow_ups = build_slash_follow_up_map(messages);
    let mut out: Vec<Chunk> = Vec::new();
    let mut buffer: Vec<AssistantResponse> = Vec::new();
    let mut pending_slashes: Vec<SlashCommand> = Vec::new();
    let mut pending_teammates: Vec<TeammateMessage> = Vec::new();
    let mut used_send_message_ids: HashSet<String> = HashSet::new();

    chunk_loop(
        messages,
        &mut buffer,
        &mut out,
        &mut executions_by_assistant,
        &mut pending_slashes,
        &mut pending_teammates,
        &mut used_send_message_ids,
        &follow_ups,
    );

    flush_buffer(
        &mut buffer,
        &mut out,
        &mut executions_by_assistant,
        &mut pending_slashes,
        &mut pending_teammates,
        &mut used_send_message_ids,
    );
    drain_trailing_teammates(&mut out, &mut pending_teammates, &mut used_send_message_ids);

    // 把 resolved subagent Process 分配到对应 AIChunk
    attach_subagents_to_chunks(&mut out, &resolved, &task_to_assistant);

    out
}

/// 把 resolved subagent `Process` 分配到拥有对应 Task `tool_use` 的 `AIChunk`。
fn attach_subagents_to_chunks(
    chunks: &mut [Chunk],
    resolved: &[ResolvedTask],
    task_to_assistant: &HashMap<String, String>,
) {
    // 构建 assistant_uuid → chunk_index 映射（owned keys 避免借用冲突）
    let mut assistant_to_chunk: HashMap<String, usize> = HashMap::new();
    for (i, chunk) in chunks.iter().enumerate() {
        if let Chunk::Ai(ai) = chunk {
            for r in &ai.responses {
                assistant_to_chunk.insert(r.uuid.clone(), i);
            }
        }
    }

    for rt in resolved {
        let process = match &rt.resolution {
            Resolution::ResultBased { process }
            | Resolution::DescriptionBased { process }
            | Resolution::Positional { process } => process,
            Resolution::Orphan => continue,
        };
        if let Some(assistant_uuid) = task_to_assistant.get(&rt.task_use_id) {
            if let Some(&chunk_idx) = assistant_to_chunk.get(assistant_uuid) {
                if let Chunk::Ai(ai) = &mut chunks[chunk_idx] {
                    ai.subagents.push(process.clone());
                    let spawn_step = SemanticStep::SubagentSpawn {
                        placeholder_id: process.session_id.clone(),
                        timestamp: process.spawn_ts,
                    };
                    // SubagentSpawn 必须紧随其对应 Task 的 ToolExecution step；
                    // 找不到时退化 append 并 warn（见 chunk-building spec 对应
                    // Scenario "SubagentSpawn step inserted after the matching
                    // Task ToolExecution"）。
                    let task_pos = ai.semantic_steps.iter().position(
                        |s| matches!(s, SemanticStep::ToolExecution { tool_use_id, .. } if tool_use_id == &rt.task_use_id),
                    );
                    if let Some(pos) = task_pos {
                        ai.semantic_steps.insert(pos + 1, spawn_step);
                    } else {
                        tracing::warn!(
                            task_use_id = %rt.task_use_id,
                            subagent_session = %process.session_id,
                            "attach_subagents: Task ToolExecution step not found, appending SubagentSpawn to tail"
                        );
                        ai.semantic_steps.push(spawn_step);
                    }
                }
            }
        }
    }
}

fn flush_buffer(
    buffer: &mut Vec<AssistantResponse>,
    out: &mut Vec<Chunk>,
    executions_by_assistant: &mut HashMap<String, Vec<ToolExecution>>,
    pending_slashes: &mut Vec<SlashCommand>,
    pending_teammates: &mut Vec<TeammateMessage>,
    used_send_message_ids: &mut HashSet<String>,
) {
    if buffer.is_empty() {
        // buffer 空但 pending teammate 非空（极少见）：保留 pending 给下一轮 flush
        // 配对，drain_trailing_teammates 兜底。
        return;
    }
    let responses = std::mem::take(buffer);
    let metrics = aggregate_metrics(&responses);
    let semantic_steps = extract_semantic_steps(&responses);
    let timestamp = responses.first().map(|r| r.timestamp).unwrap_or_default();
    let duration_ms = match (responses.first(), responses.last()) {
        (Some(a), Some(b)) if responses.len() > 1 => {
            Some((b.timestamp - a.timestamp).num_milliseconds())
        }
        _ => None,
    };
    let mut tool_executions: Vec<ToolExecution> = Vec::new();
    for r in &responses {
        if let Some(mut execs) = executions_by_assistant.remove(&r.uuid) {
            tool_executions.append(&mut execs);
        }
    }
    let slash_commands = std::mem::take(pending_slashes);
    let mut new_chunk = AIChunk {
        timestamp,
        duration_ms,
        responses,
        metrics,
        semantic_steps,
        tool_executions,
        subagents: Vec::new(),
        slash_commands,
        teammate_messages: Vec::new(),
    };

    // 把 pending teammate 注入新构造的 AIChunk：先按"自身 + 历史 N-1 个"配对
    // reply_to_tool_use_id，再 move 到 new_chunk.teammate_messages。
    if !pending_teammates.is_empty() {
        let drained: Vec<TeammateMessage> = std::mem::take(pending_teammates);
        let last_emitted_idx = out.iter().rposition(|c| matches!(c, Chunk::Ai(_)));
        let mut linked: Vec<TeammateMessage> = Vec::with_capacity(drained.len());
        for mut tm in drained {
            let link_target = if let Some(idx) = last_emitted_idx {
                link_against_chunks(
                    &tm.teammate_id,
                    out,
                    idx,
                    used_send_message_ids,
                    Some(&new_chunk),
                )
            } else {
                // out 中无 AIChunk：仅在 new_chunk 自身扫描
                let chain: Vec<&AIChunk> = vec![&new_chunk];
                link_teammate_to_send_message(&tm.teammate_id, &chain, used_send_message_ids)
            };
            tm.reply_to_tool_use_id = link_target;
            linked.push(tm);
        }
        new_chunk.teammate_messages = linked;
    }

    out.push(Chunk::Ai(new_chunk));
}

/// 从 isMeta 消息内容中提取 slash 命令信息。
///
/// 格式：`<command-name>/xxx</command-name>`，可选
/// `<command-message>` 和 `<command-args>`。
fn extract_slash_info(
    content: &MessageContent,
    uuid: &str,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> Option<SlashCommand> {
    let text = match content {
        MessageContent::Text(s) => s.as_str(),
        MessageContent::Blocks(blocks) => {
            // 取第一个 text block
            blocks.iter().find_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })?
        }
    };
    // <command-name>/xxx</command-name>
    let name_start = text.find("<command-name>/")?;
    let after_prefix = &text[name_start + "<command-name>/".len()..];
    let name_end = after_prefix.find("</command-name>")?;
    let name = after_prefix[..name_end].trim().to_owned();
    if name.is_empty() {
        return None;
    }

    let message = extract_xml_tag(text, "command-message");
    let args = extract_xml_tag(text, "command-args");

    Some(SlashCommand {
        name,
        message,
        args,
        message_uuid: uuid.to_owned(),
        timestamp,
        instructions: None,
    })
}

fn extract_xml_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    let val = text[start..end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_owned())
    }
}

fn extract_plain_text(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut acc = String::new();
            for b in blocks {
                if let ContentBlock::Text { text } = b {
                    if !acc.is_empty() {
                        acc.push('\n');
                    }
                    acc.push_str(text);
                }
            }
            acc
        }
    }
}

fn extract_local_command_stdout(content: &MessageContent) -> Option<String> {
    let text = match content {
        MessageContent::Text(s) => s.clone(),
        MessageContent::Blocks(blocks) => {
            let mut acc = String::new();
            let mut saw_non_text = false;
            for b in blocks {
                if let ContentBlock::Text { text } = b {
                    if !acc.is_empty() {
                        acc.push('\n');
                    }
                    acc.push_str(text);
                } else {
                    saw_non_text = true;
                    break;
                }
            }
            if saw_non_text {
                return None;
            }
            acc
        }
    };
    let trimmed = text.trim();
    if !trimmed.starts_with(STDOUT_OPEN) || !trimmed.ends_with(STDOUT_CLOSE) {
        return None;
    }
    let inner = &trimmed[STDOUT_OPEN.len()..trimmed.len() - STDOUT_CLOSE.len()];
    if inner.is_empty() {
        return None;
    }
    Some(inner.to_owned())
}

fn is_tool_result_only(content: &MessageContent) -> bool {
    let MessageContent::Blocks(blocks) = content else {
        return false;
    };
    if blocks.is_empty() {
        return false;
    }
    blocks
        .iter()
        .all(|b| matches!(b, ContentBlock::ToolResult { .. }))
}

fn append_tool_results(target: &mut AssistantResponse, incoming: &MessageContent) {
    let MessageContent::Blocks(blocks) = incoming else {
        return;
    };
    let MessageContent::Blocks(existing) = &mut target.content else {
        let mut merged = Vec::new();
        merged.extend(blocks.iter().cloned());
        target.content = MessageContent::Blocks(merged);
        return;
    };
    for b in blocks {
        if matches!(b, ContentBlock::ToolResult { .. }) {
            existing.push(b.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{
        HardNoiseReason, MessageContent, MessageType, SemanticStep, TokenUsage, ToolCall,
        ToolResult,
    };
    use chrono::{DateTime, Duration, Utc};

    fn ts(n: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::seconds(n)
    }

    fn blank_message(uuid: &str, n: i64) -> ParsedMessage {
        ParsedMessage {
            uuid: uuid.into(),
            parent_uuid: None,
            message_type: MessageType::User,
            category: MessageCategory::User,
            timestamp: ts(n),
            role: None,
            content: MessageContent::Text(String::new()),
            usage: None,
            model: None,
            cwd: None,
            git_branch: None,
            agent_id: None,
            is_sidechain: false,
            is_meta: false,
            user_type: None,
            tool_calls: Vec::new(),
            tool_results: Vec::new(),
            source_tool_use_id: None,
            source_tool_assistant_uuid: None,
            is_compact_summary: false,
            request_id: None,
            tool_use_result: None,
        }
    }

    fn user(uuid: &str, n: i64, text: &str) -> ParsedMessage {
        ParsedMessage {
            content: MessageContent::Text(text.into()),
            ..blank_message(uuid, n)
        }
    }

    fn assistant(uuid: &str, n: i64, blocks: &[ContentBlock]) -> ParsedMessage {
        ParsedMessage {
            message_type: MessageType::Assistant,
            category: MessageCategory::Assistant,
            content: MessageContent::Blocks(blocks.to_vec()),
            tool_calls: blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                        is_task: name == "Task",
                        task_description: None,
                        task_subagent_type: None,
                    }),
                    _ => None,
                })
                .collect(),
            tool_results: Vec::new(),
            ..blank_message(uuid, n)
        }
    }

    #[test]
    fn user_question_then_ai_response_emits_two_chunks() {
        let msgs = vec![
            user("u1", 0, "hi"),
            assistant(
                "a1",
                1,
                &[ContentBlock::Text {
                    text: "hello".into(),
                }],
            ),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 2);
        assert!(matches!(chunks[0], Chunk::User(_)));
        assert!(matches!(chunks[1], Chunk::Ai(_)));
    }

    #[test]
    fn multiple_assistant_turns_coalesce_into_one_ai_chunk() {
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "1".into() }]),
            assistant("a2", 2, &[ContentBlock::Text { text: "2".into() }]),
            assistant("a3", 3, &[ContentBlock::Text { text: "3".into() }]),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 1);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.responses.len(), 3);
    }

    #[test]
    fn assistant_buffer_flushes_before_new_user() {
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "1".into() }]),
            assistant("a2", 2, &[ContentBlock::Text { text: "2".into() }]),
            user("u1", 3, "next?"),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 2);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.responses.len(), 2);
        assert!(matches!(chunks[1], Chunk::User(_)));
    }

    #[test]
    fn local_command_stdout_becomes_system_chunk() {
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "1".into() }]),
            user(
                "u1",
                2,
                "<local-command-stdout>ls output</local-command-stdout>",
            ),
            assistant("a2", 3, &[ContentBlock::Text { text: "2".into() }]),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 3);
        assert!(matches!(chunks[0], Chunk::Ai(_)));
        let Chunk::System(sys) = &chunks[1] else {
            panic!("expected SystemChunk");
        };
        assert_eq!(sys.content_text, "ls output");
        assert!(matches!(chunks[2], Chunk::Ai(_)));
    }

    #[test]
    fn sidechain_messages_are_dropped() {
        let mut side = assistant("a1", 1, &[ContentBlock::Text { text: "x".into() }]);
        side.is_sidechain = true;
        let msgs = vec![
            side,
            assistant("a2", 2, &[ContentBlock::Text { text: "y".into() }]),
        ];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.responses.len(), 1);
        assert_eq!(ai.responses[0].uuid, "a2");
    }

    #[test]
    fn hard_noise_messages_are_dropped() {
        let mut synthetic = assistant("a1", 1, &[ContentBlock::Text { text: "x".into() }]);
        synthetic.category = MessageCategory::HardNoise(HardNoiseReason::SyntheticAssistant);
        let msgs = vec![
            synthetic,
            assistant("a2", 2, &[ContentBlock::Text { text: "y".into() }]),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 1);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.responses.len(), 1);
    }

    #[test]
    fn ai_chunk_metrics_sum_tool_calls() {
        let msgs = vec![assistant(
            "a1",
            1,
            &[
                ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                },
                ContentBlock::ToolUse {
                    id: "t2".into(),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                },
                ContentBlock::ToolUse {
                    id: "t3".into(),
                    name: "Grep".into(),
                    input: serde_json::json!({}),
                },
            ],
        )];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.metrics.tool_count, 3);
    }

    #[test]
    fn user_chunk_metrics_all_zero_and_duration_none() {
        let msgs = vec![user("u1", 0, "hi")];
        let chunks = build_chunks(&msgs);
        let Chunk::User(u) = &chunks[0] else {
            panic!("expected UserChunk");
        };
        assert_eq!(u.metrics, ChunkMetrics::zero());
        assert_eq!(u.duration_ms, None);
    }

    #[test]
    fn compact_summary_emits_compact_chunk_and_flushes_buffer() {
        let mut compact = user("c1", 3, "summary text");
        compact.category = MessageCategory::Compact;
        compact.is_compact_summary = true;
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "1".into() }]),
            assistant("a2", 2, &[ContentBlock::Text { text: "2".into() }]),
            compact,
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 2);
        assert!(matches!(chunks[0], Chunk::Ai(_)));
        let Chunk::Compact(c) = &chunks[1] else {
            panic!("expected CompactChunk");
        };
        assert_eq!(c.summary_text, "summary text");
    }

    #[test]
    fn semantic_steps_follow_block_order() {
        let msgs = vec![assistant(
            "a1",
            1,
            &[
                ContentBlock::Thinking {
                    thinking: "reason".into(),
                    signature: String::new(),
                },
                ContentBlock::Text {
                    text: "hello".into(),
                },
                ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                },
            ],
        )];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.semantic_steps.len(), 3);
        assert!(matches!(
            ai.semantic_steps[0],
            SemanticStep::Thinking { .. }
        ));
        assert!(matches!(ai.semantic_steps[1], SemanticStep::Text { .. }));
        assert!(matches!(
            ai.semantic_steps[2],
            SemanticStep::ToolExecution { .. }
        ));
    }

    #[test]
    fn subagent_spawn_variant_not_emitted_yet() {
        let msgs = vec![assistant(
            "a1",
            1,
            &[ContentBlock::ToolUse {
                id: "t1".into(),
                name: "Task".into(),
                input: serde_json::json!({"description": "find things"}),
            }],
        )];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert!(
            !ai.semantic_steps
                .iter()
                .any(|s| matches!(s, SemanticStep::SubagentSpawn { .. }))
        );
    }

    fn make_candidate(session_id: &str, n: i64, desc: Option<&str>) -> SubagentCandidate {
        SubagentCandidate {
            session_id: session_id.into(),
            description_hint: desc.map(str::to_owned),
            spawn_ts: ts(n),
            end_ts: Some(ts(n + 10)),
            parent_session_id: None,
            metrics: ChunkMetrics::zero(),
            messages: Vec::new(),
            is_ongoing: false,
        }
    }

    /// Result-based 匹配需要 Task `ToolExecution` 的 `output.toolUseResult` 中
    /// 含 `subagentSessionId` 字段——构造一个满足的 parsed `tool_result`。
    fn tool_result_with_subagent_session(tool_use_id: &str, session_id: &str) -> ContentBlock {
        ContentBlock::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: serde_json::json!([
                {"type": "text", "text": format!("spawned {session_id}")}
            ]),
            is_error: false,
        }
    }

    fn assistant_with_task(
        uuid: &str,
        n: i64,
        pre_tools: &[(&str, &str)],
        task_id: &str,
        task_desc: &str,
        post_tools: &[(&str, &str)],
    ) -> ParsedMessage {
        let mut blocks: Vec<ContentBlock> = Vec::new();
        for (id, name) in pre_tools {
            blocks.push(ContentBlock::ToolUse {
                id: (*id).into(),
                name: (*name).into(),
                input: serde_json::json!({}),
            });
        }
        blocks.push(ContentBlock::ToolUse {
            id: task_id.into(),
            name: "Task".into(),
            input: serde_json::json!({"description": task_desc}),
        });
        for (id, name) in post_tools {
            blocks.push(ContentBlock::ToolUse {
                id: (*id).into(),
                name: (*name).into(),
                input: serde_json::json!({}),
            });
        }
        assistant(uuid, n, &blocks)
    }

    fn result_user(uuid: &str, n: i64, pairs: &[(&str, Option<&str>)]) -> ParsedMessage {
        // 对每个 tool_use 产一个 tool_result；description 用于生成 subagent_session_id 提示
        let blocks: Vec<ContentBlock> = pairs
            .iter()
            .map(|(tid, sid_hint)| {
                let content = if let Some(sid) = sid_hint {
                    serde_json::json!([{"type": "text", "text": format!("session:{sid}")}])
                } else {
                    serde_json::json!("ok")
                };
                ContentBlock::ToolResult {
                    tool_use_id: (*tid).into(),
                    content,
                    is_error: false,
                }
            })
            .collect();
        let mut m = blank_message(uuid, n);
        m.content = MessageContent::Blocks(blocks);
        m
    }

    #[test]
    fn subagent_spawn_inserted_after_matching_task_step() {
        // 前置 Read + Task + 后置 Grep，Task 匹配 subagent cand-1
        let msgs = vec![
            assistant_with_task(
                "a1",
                1,
                &[("t_read", "Read")],
                "t_task",
                "inspect logs",
                &[("t_grep", "Grep")],
            ),
            result_user(
                "u1",
                2,
                &[
                    ("t_read", None),
                    ("t_task", Some("cand-1")),
                    ("t_grep", None),
                ],
            ),
        ];
        let cands = vec![make_candidate("cand-1", 1, Some("inspect logs"))];
        let chunks = build_chunks_with_subagents(&msgs, &cands);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        let kinds: Vec<&str> = ai
            .semantic_steps
            .iter()
            .map(|s| match s {
                SemanticStep::ToolExecution { tool_name, .. } => tool_name.as_str(),
                SemanticStep::SubagentSpawn { .. } => "SubagentSpawn",
                SemanticStep::Thinking { .. } => "Thinking",
                SemanticStep::Text { .. } => "Text",
                SemanticStep::Interruption { .. } => "Interruption",
            })
            .collect();
        // Task 步骤仍在（前端层做去重），SubagentSpawn 紧随其后
        assert_eq!(kinds, vec!["Read", "Task", "SubagentSpawn", "Grep"]);
    }

    #[test]
    fn multiple_tasks_each_get_spawn_inserted_after_own_task() {
        let msgs = vec![
            assistant_with_task("a1", 1, &[], "t_task1", "first", &[]),
            assistant_with_task("a2", 2, &[], "t_task2", "second", &[]),
            result_user(
                "u1",
                3,
                &[("t_task1", Some("cand-A")), ("t_task2", Some("cand-B"))],
            ),
        ];
        let cands = vec![
            make_candidate("cand-A", 1, Some("first")),
            make_candidate("cand-B", 2, Some("second")),
        ];
        let chunks = build_chunks_with_subagents(&msgs, &cands);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        // 顺序：Task t_task1 → SubagentSpawn(A) → Task t_task2 → SubagentSpawn(B)
        let trail: Vec<String> = ai
            .semantic_steps
            .iter()
            .map(|s| match s {
                SemanticStep::ToolExecution { tool_use_id, .. } => format!("t:{tool_use_id}"),
                SemanticStep::SubagentSpawn { placeholder_id, .. } => format!("s:{placeholder_id}"),
                _ => "other".into(),
            })
            .collect();
        assert_eq!(
            trail,
            vec![
                "t:t_task1".to_string(),
                "s:cand-A".into(),
                "t:t_task2".into(),
                "s:cand-B".into(),
            ]
        );
    }

    #[test]
    fn orphan_task_emits_no_subagent_spawn() {
        let msgs = vec![assistant_with_task(
            "a1",
            1,
            &[],
            "t_task",
            "unmatched",
            &[],
        )];
        // 没有 candidate 匹配
        let chunks = build_chunks_with_subagents(&msgs, &[]);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert!(
            !ai.semantic_steps
                .iter()
                .any(|s| matches!(s, SemanticStep::SubagentSpawn { .. })),
            "orphan Task should not emit SubagentSpawn"
        );
        assert!(
            ai.semantic_steps
                .iter()
                .any(|s| matches!(s, SemanticStep::ToolExecution { tool_name, .. } if tool_name == "Task")),
            "orphan Task ToolExecution should remain"
        );
        // 允许使用以避免未使用警告（mock tool_result 工具函数在其它测试里也用）
        let _ = tool_result_with_subagent_session("x", "y");
    }

    #[test]
    fn tool_executions_populated_for_tool_use() {
        // 孤立 tool_use：应产出 1 条 orphan ToolExecution
        let msgs = vec![assistant(
            "a1",
            1,
            &[ContentBlock::ToolUse {
                id: "t1".into(),
                name: "Bash".into(),
                input: serde_json::json!({}),
            }],
        )];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.tool_executions.len(), 1);
        assert_eq!(ai.tool_executions[0].tool_use_id, "t1");
        assert_eq!(ai.tool_executions[0].end_ts, None);
        assert_eq!(ai.tool_executions[0].output, cdt_core::ToolOutput::Missing);
        assert!(ai.subagents.is_empty());
    }

    #[test]
    fn tool_executions_pair_assistant_and_user_result() {
        let mut result_user = blank_message("u1", 2);
        result_user.content = MessageContent::Blocks(vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: serde_json::json!("done"),
            is_error: false,
        }]);
        let msgs = vec![
            assistant(
                "a1",
                1,
                &[ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({"cmd": "ls"}),
                }],
            ),
            result_user,
        ];
        let chunks = build_chunks(&msgs);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.tool_executions.len(), 1);
        let exec = &ai.tool_executions[0];
        assert_eq!(exec.source_assistant_uuid, "a1");
        assert!(exec.end_ts.is_some());
        assert!(matches!(exec.output, cdt_core::ToolOutput::Text { .. }));
    }

    #[test]
    fn tool_executions_distributed_across_chunks() {
        let mut u1 = blank_message("uu1", 2);
        u1.content = MessageContent::Blocks(vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: serde_json::json!("first"),
            is_error: false,
        }]);
        let mut u2 = blank_message("uu2", 4);
        u2.content = MessageContent::Text("real user msg".into());
        let mut u3 = blank_message("uu3", 6);
        u3.content = MessageContent::Blocks(vec![ContentBlock::ToolResult {
            tool_use_id: "t2".into(),
            content: serde_json::json!("second"),
            is_error: false,
        }]);
        let msgs = vec![
            assistant(
                "a1",
                1,
                &[ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            u1,
            u2, // flush AIChunk #1
            assistant(
                "a2",
                5,
                &[ContentBlock::ToolUse {
                    id: "t2".into(),
                    name: "Read".into(),
                    input: serde_json::json!({}),
                }],
            ),
            u3,
        ];
        let chunks = build_chunks(&msgs);
        let ai_chunks: Vec<&AIChunk> = chunks
            .iter()
            .filter_map(|c| if let Chunk::Ai(a) = c { Some(a) } else { None })
            .collect();
        assert_eq!(ai_chunks.len(), 2);
        assert_eq!(ai_chunks[0].tool_executions.len(), 1);
        assert_eq!(ai_chunks[0].tool_executions[0].tool_use_id, "t1");
        assert_eq!(ai_chunks[1].tool_executions.len(), 1);
        assert_eq!(ai_chunks[1].tool_executions[0].tool_use_id, "t2");
    }

    #[test]
    fn tool_result_only_user_message_attaches_to_last_assistant() {
        let tool_result_block = ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: serde_json::json!("ok"),
            is_error: false,
        };
        let mut tool_result_user = blank_message("u2", 2);
        tool_result_user.content = MessageContent::Blocks(vec![tool_result_block]);
        let msgs = vec![
            assistant(
                "a1",
                1,
                &[ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            tool_result_user,
            assistant(
                "a2",
                3,
                &[ContentBlock::Text {
                    text: "done".into(),
                }],
            ),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 1);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.responses.len(), 2);
    }

    #[test]
    fn metrics_sum_token_usage_across_responses() {
        let mut a1 = assistant("a1", 1, &[ContentBlock::Text { text: "1".into() }]);
        a1.usage = Some(TokenUsage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_input_tokens: 2,
            cache_creation_input_tokens: 1,
        });
        let mut a2 = assistant("a2", 2, &[ContentBlock::Text { text: "2".into() }]);
        a2.usage = Some(TokenUsage {
            input_tokens: 3,
            output_tokens: 4,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        });
        let chunks = build_chunks(&[a1, a2]);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        assert_eq!(ai.metrics.input_tokens, 13);
        assert_eq!(ai.metrics.output_tokens, 9);
        assert_eq!(ai.metrics.cache_read_tokens, 2);
        assert_eq!(ai.metrics.cache_creation_tokens, 1);
        assert_eq!(ai.metrics.cost_usd, None);
    }

    #[test]
    fn unused_tool_result_import_sanity() {
        let _ = ToolResult {
            tool_use_id: "x".into(),
            content: serde_json::json!(null),
            is_error: false,
        };
    }

    #[test]
    fn meta_messages_are_skipped() {
        let mut meta = user("m1", 2, "Propose a new change - skill prompt...");
        meta.is_meta = true;
        let msgs = vec![
            user("u1", 0, "hi"),
            assistant(
                "a1",
                1,
                &[ContentBlock::Text {
                    text: "hello".into(),
                }],
            ),
            meta,
            assistant(
                "a2",
                3,
                &[ContentBlock::Text {
                    text: "done".into(),
                }],
            ),
        ];
        let chunks = build_chunks(&msgs);
        // meta 消息不产出 UserChunk，a1 和 a2 合并为一个 AIChunk
        assert_eq!(chunks.len(), 2);
        assert!(matches!(chunks[0], Chunk::User(_)));
        assert!(matches!(chunks[1], Chunk::Ai(_)));
    }

    #[test]
    fn slash_adjacent_to_ai_emits_user_chunk_and_populates_slash_commands() {
        // slash 紧邻 AIChunk（中间没有其他 user message）：
        // 既产出 UserChunk（UI 气泡），也挂到 AIChunk.slash_commands（AI group 内 SlashItem）。
        let slash = user(
            "s1",
            0,
            "<command-name>/claude-md-management:claude-md-improver</command-name><command-message>claude-md-management:claude-md-improver</command-message>",
        );
        let msgs = vec![
            slash,
            assistant(
                "a1",
                1,
                &[ContentBlock::Text {
                    text: "开始改 CLAUDE.md".into(),
                }],
            ),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 2);
        let Chunk::User(slash_user) = &chunks[0] else {
            panic!("expected slash UserChunk at index 0");
        };
        assert_eq!(slash_user.uuid, "s1");
        let Chunk::Ai(ai) = &chunks[1] else {
            panic!("expected AIChunk at index 1");
        };
        assert_eq!(ai.slash_commands.len(), 1);
        assert_eq!(
            ai.slash_commands[0].name,
            "claude-md-management:claude-md-improver"
        );
        assert_eq!(ai.slash_commands[0].message_uuid, "s1");
    }

    #[test]
    fn normal_user_message_between_slash_and_ai_drops_pending_slash() {
        // slash → 普通 user → AI 响应：原版 precedingSlash 只看紧邻 user group，
        // 中间夹了普通 user 后 AIChunk 不应再挂 slash。
        let slash = user(
            "s1",
            0,
            "<command-name>/clear</command-name><command-message>clear</command-message>",
        );
        let msgs = vec![
            slash,
            user("u1", 1, "真实提问"),
            assistant(
                "a1",
                2,
                &[ContentBlock::Text {
                    text: "回复".into(),
                }],
            ),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 3);
        let Chunk::Ai(ai) = &chunks[2] else {
            panic!("expected AIChunk at index 2");
        };
        assert!(
            ai.slash_commands.is_empty(),
            "slash 应被普通 user 打断，不挂到后续 AIChunk"
        );
    }

    #[test]
    fn meta_tool_result_still_merges_into_buffer() {
        let mut meta_result = blank_message("m1", 2);
        meta_result.is_meta = true;
        meta_result.content = MessageContent::Blocks(vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: serde_json::json!("ok"),
            is_error: false,
        }]);
        let msgs = vec![
            assistant(
                "a1",
                1,
                &[ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                }],
            ),
            meta_result,
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 1);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        // tool_result 仍应被合并，execution 应有结果
        assert_eq!(ai.tool_executions.len(), 1);
        assert!(ai.tool_executions[0].end_ts.is_some());
    }

    fn interruption(uuid: &str, n: i64, text: &str) -> ParsedMessage {
        ParsedMessage {
            category: MessageCategory::Interruption,
            content: MessageContent::Text(text.into()),
            ..blank_message(uuid, n)
        }
    }

    #[test]
    fn interrupt_marker_appended_as_semantic_step_to_last_ai_chunk() {
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "hi".into() }]),
            interruption("u1", 2, "[Request interrupted by user for tool use]"),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 1);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        let Some(SemanticStep::Interruption { text, .. }) = ai.semantic_steps.last() else {
            panic!(
                "expected trailing Interruption step, got {:?}",
                ai.semantic_steps
            );
        };
        assert_eq!(text, "[Request interrupted by user for tool use]");
    }

    #[test]
    fn interrupt_marker_appended_after_flushed_ai_chunk() {
        // assistant 之后先遇 user 消息 flush，再出现 interrupt：
        // interrupt 应追加到已 flush 的最后一个 AIChunk。
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "hi".into() }]),
            user("u1", 2, "next?"),
            interruption("u2", 3, "[Request interrupted by user]"),
        ];
        let chunks = build_chunks(&msgs);
        // AIChunk + UserChunk，interrupt 追加到 AIChunk
        assert_eq!(chunks.len(), 2);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk first");
        };
        assert!(matches!(
            ai.semantic_steps.last(),
            Some(SemanticStep::Interruption { .. })
        ));
    }

    #[test]
    fn interrupt_marker_without_prior_ai_is_dropped() {
        // 文件开头就 interrupt：没有前驱 AIChunk，丢弃，不产 chunk。
        let msgs = vec![interruption("u1", 0, "[Request interrupted by user]")];
        let chunks = build_chunks(&msgs);
        assert!(chunks.is_empty());
    }

    #[test]
    fn multiple_interruptions_preserve_order_in_same_ai_chunk() {
        let msgs = vec![
            assistant("a1", 1, &[ContentBlock::Text { text: "hi".into() }]),
            interruption("u1", 2, "[Request interrupted by user A]"),
            interruption("u2", 3, "[Request interrupted by user B]"),
        ];
        let chunks = build_chunks(&msgs);
        assert_eq!(chunks.len(), 1);
        let Chunk::Ai(ai) = &chunks[0] else {
            panic!("expected AIChunk");
        };
        let interrupts: Vec<&str> = ai
            .semantic_steps
            .iter()
            .filter_map(|s| match s {
                SemanticStep::Interruption { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            interrupts,
            vec![
                "[Request interrupted by user A]",
                "[Request interrupted by user B]"
            ]
        );
    }

    // ---- teammate-message-rendering ----
    //
    // Spec：`openspec/specs/chunk-building/spec.md`
    // §`Embed teammate messages into AIChunk` 全部 5 个 Scenario。
    //
    // 注意：`EMBED_TEAMMATES = false` 的回退路径走 const 早绑定，runtime 测试
    // 不可达；切换常量后跑这些测试即可验证回退（断言会反向：teammate 不再嵌入）。

    fn teammate_user(uuid: &str, n: i64, body: &str) -> ParsedMessage {
        let xml = format!(
            r#"<teammate-message teammate_id="alice" color="blue" summary="hi">{body}</teammate-message>"#
        );
        ParsedMessage {
            content: MessageContent::Text(xml),
            ..blank_message(uuid, n)
        }
    }

    fn teammate_user_to(uuid: &str, n: i64, recipient: &str, body: &str) -> ParsedMessage {
        let xml = format!(
            r#"<teammate-message teammate_id="{recipient}" color="green" summary="ok">{body}</teammate-message>"#
        );
        ParsedMessage {
            content: MessageContent::Text(xml),
            ..blank_message(uuid, n)
        }
    }

    fn send_message_assistant(uuid: &str, n: i64, recipient: &str) -> ParsedMessage {
        assistant(
            uuid,
            n,
            &[ContentBlock::ToolUse {
                id: format!("tu-{uuid}"),
                name: "SendMessage".into(),
                input: serde_json::json!({ "recipient": recipient, "content": "do work" }),
            }],
        )
    }

    #[test]
    fn teammate_message_does_not_produce_user_chunk() {
        // 流：user → assistant → teammate-message → assistant
        // 期望：UserChunk(u1) + AIChunk(a1) + AIChunk(a2 with teammate)
        let msgs = vec![
            user("u1", 0, "real user"),
            assistant("a1", 1, &[ContentBlock::Text { text: "ack".into() }]),
            teammate_user("tm1", 2, "queen reply"),
            assistant("a2", 3, &[ContentBlock::Text { text: "got".into() }]),
        ];
        let chunks = build_chunks(&msgs);
        // 没有任何 UserChunk 来自 teammate；只有 u1 那条
        let user_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| matches!(c, Chunk::User(_)))
            .collect();
        assert_eq!(user_chunks.len(), 1, "teammate 不应产 UserChunk");
        let Chunk::User(only_user) = user_chunks[0] else {
            panic!()
        };
        assert_eq!(only_user.uuid, "u1");
    }

    #[test]
    fn teammate_message_embedded_into_ai_chunk_with_reply_to() {
        // 流：assistant(SendMessage→alice) → teammate(alice) → assistant("got it")
        // teammate 不打断 assistant buffer 合并，a1 + a2 合并到同一 AIChunk；
        // teammate 嵌入合并 AIChunk.teammate_messages，按 reply_to_tool_use_id 配对
        // 到 a1 的 SendMessage tool_use_id。前端 displayItemBuilder 按 reply_to 把
        // teammate 卡片紧贴对应 SendMessage 渲染，与原版 TS displayItem 维度一致。
        let msgs = vec![
            send_message_assistant("a1", 1, "alice"),
            teammate_user("tm1", 2, "alice reply body"),
            assistant("a2", 3, &[ContentBlock::Text { text: "got".into() }]),
        ];
        let chunks = build_chunks(&msgs);
        let ai_chunks: Vec<_> = chunks
            .iter()
            .filter_map(|c| {
                if let Chunk::Ai(ai) = c {
                    Some(ai)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            ai_chunks.len(),
            1,
            "teammate 不打断连续 assistant 合并，a1+a2 应合并到同一 AIChunk"
        );
        let ai = ai_chunks[0];
        assert_eq!(ai.responses.len(), 2);
        assert_eq!(ai.teammate_messages.len(), 1);
        let tm = &ai.teammate_messages[0];
        assert_eq!(tm.teammate_id, "alice");
        assert_eq!(tm.summary.as_deref(), Some("hi"));
        assert_eq!(tm.body, "alice reply body");
        assert_eq!(tm.reply_to_tool_use_id.as_deref(), Some("tu-a1"));
        assert!(!tm.is_noise);
        assert!(!tm.is_resend);
    }

    #[test]
    fn trailing_teammate_attaches_to_last_ai_chunk() {
        // 流：assistant(SendMessage→alice) → teammate(alice)（无后续 assistant）
        let msgs = vec![
            send_message_assistant("a1", 1, "alice"),
            teammate_user("tm1", 2, "trailing reply"),
        ];
        let chunks = build_chunks(&msgs);
        // 仅一个 AIChunk（a1），teammate 追加到它身上
        let ai_chunks: Vec<_> = chunks
            .iter()
            .filter_map(|c| {
                if let Chunk::Ai(ai) = c {
                    Some(ai)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(ai_chunks.len(), 1);
        assert_eq!(ai_chunks[0].teammate_messages.len(), 1);
        let tm = &ai_chunks[0].teammate_messages[0];
        // 自身 SendMessage 在同 AIChunk 内 → 配对成功
        assert_eq!(tm.reply_to_tool_use_id.as_deref(), Some("tu-a1"));
    }

    #[test]
    fn orphan_teammate_with_no_ai_chunk_is_silently_dropped() {
        // 全 teammate 无任何 assistant
        let msgs = vec![
            teammate_user("tm1", 0, "lone reply 1"),
            teammate_user_to("tm2", 1, "bob", "lone reply 2"),
        ];
        let chunks = build_chunks(&msgs);
        assert!(
            chunks.is_empty(),
            "全 teammate 无 AIChunk 时 chunks 应为空（teammate 不再产 UserChunk，无去处即丢弃）"
        );
    }

    #[test]
    fn multiple_teammates_grouped_under_their_send_message() {
        // 流：a1(SendMessage→alice) → a2(SendMessage→bob) → tm(alice) → tm(bob) → a3
        let msgs = vec![
            send_message_assistant("a1", 1, "alice"),
            send_message_assistant("a2", 2, "bob"),
            teammate_user_to("tm-alice", 3, "alice", "alice body"),
            teammate_user_to("tm-bob", 4, "bob", "bob body"),
            assistant(
                "a3",
                5,
                &[ContentBlock::Text {
                    text: "done".into(),
                }],
            ),
        ];
        let chunks = build_chunks(&msgs);
        let ai_chunks: Vec<_> = chunks
            .iter()
            .filter_map(|c| {
                if let Chunk::Ai(ai) = c {
                    Some(ai)
                } else {
                    None
                }
            })
            .collect();
        // 全部 a1 / a2 / a3 是连续 assistant（teammate 不打断）→ 合并到 1 个 AIChunk
        assert_eq!(ai_chunks.len(), 1);
        // 两条 teammate 都嵌入这个合并 AIChunk
        assert_eq!(ai_chunks[0].teammate_messages.len(), 2);
        let alice = &ai_chunks[0].teammate_messages[0];
        let bob = &ai_chunks[0].teammate_messages[1];
        assert_eq!(alice.teammate_id, "alice");
        assert_eq!(bob.teammate_id, "bob");
        assert_eq!(alice.reply_to_tool_use_id.as_deref(), Some("tu-a1"));
        assert_eq!(bob.reply_to_tool_use_id.as_deref(), Some("tu-a2"));
    }
}
