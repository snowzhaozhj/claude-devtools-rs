//! `tool_use` ↔ `tool_result` 按 id 配对。
//!
//! Spec scenarios：
//! - Immediate / Delayed result
//! - Duplicate result ids（impl-bug fix：warn + 保留首个）
//! - Orphan `tool_use`
//! - Bash 结构化 result、legacy 字符串 result、错误 result

use std::collections::HashMap;

use cdt_core::{
    ContentBlock, MessageCategory, MessageContent, ParsedMessage, ToolExecution, ToolOutput,
};
use chrono::{DateTime, Utc};

use super::ToolLinkingResult;

struct PendingToolUse {
    tool_name: String,
    input: serde_json::Value,
    start_ts: DateTime<Utc>,
    source_assistant_uuid: String,
    linked: bool,
}

pub fn pair_tool_executions(messages: &[ParsedMessage]) -> ToolLinkingResult {
    let mut pending: HashMap<String, PendingToolUse> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut executions: Vec<ToolExecution> = Vec::new();
    let mut duplicates_dropped = 0usize;

    for msg in messages {
        match &msg.category {
            MessageCategory::Assistant => {
                for call in &msg.tool_calls {
                    if pending.contains_key(&call.id) {
                        tracing::warn!(
                            tool_use_id = %call.id,
                            "duplicate tool_use id; keeping first"
                        );
                        duplicates_dropped += 1;
                        continue;
                    }
                    order.push(call.id.clone());
                    pending.insert(
                        call.id.clone(),
                        PendingToolUse {
                            tool_name: call.name.clone(),
                            input: call.input.clone(),
                            start_ts: msg.timestamp,
                            source_assistant_uuid: msg.uuid.clone(),
                            linked: false,
                        },
                    );
                }
            }
            MessageCategory::User => {
                let MessageContent::Blocks(blocks) = &msg.content else {
                    continue;
                };
                for block in blocks {
                    let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } = block
                    else {
                        continue;
                    };
                    let Some(pu) = pending.get_mut(tool_use_id) else {
                        continue;
                    };
                    if pu.linked {
                        tracing::warn!(
                            tool_use_id = %tool_use_id,
                            "duplicate tool_result; keeping first"
                        );
                        duplicates_dropped += 1;
                        continue;
                    }
                    pu.linked = true;
                    let result_agent_id = msg
                        .tool_use_result
                        .as_ref()
                        .and_then(|v| v.get("agentId"))
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);
                    let teammate_spawn = extract_teammate_spawn(msg.tool_use_result.as_ref());
                    executions.push(ToolExecution {
                        tool_use_id: tool_use_id.clone(),
                        tool_name: pu.tool_name.clone(),
                        input: pu.input.clone(),
                        output: classify_output(content),
                        is_error: *is_error,
                        start_ts: pu.start_ts,
                        end_ts: Some(msg.timestamp),
                        source_assistant_uuid: pu.source_assistant_uuid.clone(),
                        result_agent_id,
                        output_omitted: false,
                        output_bytes: None,
                        teammate_spawn,
                    });
                }
            }
            _ => {}
        }
    }

    // orphans：未 linked 的 tool_use，按出现顺序保留
    for id in order {
        if let Some(pu) = pending.remove(&id) {
            if !pu.linked {
                executions.push(ToolExecution {
                    tool_use_id: id,
                    tool_name: pu.tool_name,
                    input: pu.input,
                    output: ToolOutput::Missing,
                    is_error: false,
                    start_ts: pu.start_ts,
                    end_ts: None,
                    source_assistant_uuid: pu.source_assistant_uuid,
                    result_agent_id: None,
                    output_omitted: false,
                    output_bytes: None,
                    teammate_spawn: None,
                });
            }
        }
    }

    ToolLinkingResult {
        executions,
        duplicates_dropped,
    }
}

fn classify_output(content: &serde_json::Value) -> ToolOutput {
    match content {
        serde_json::Value::String(s) => ToolOutput::Text { text: s.clone() },
        serde_json::Value::Null => ToolOutput::Missing,
        other => ToolOutput::Structured {
            value: other.clone(),
        },
    }
}

/// 从 user 消息顶层 `toolUseResult` 抽取 teammate spawn 元数据。
///
/// 当 `toolUseResult.status == "teammate_spawned"` 时，提取 `name` 与 `color`
/// 字段封装为 [`cdt_core::TeammateSpawnInfo`]。其它情况返回 `None`。
///
/// 对齐原版 `claude-devtools/src/main/services/discovery/SubagentResolver.ts`
/// 与 `LinkedToolItem.tsx::isTeammateSpawned` 检测条件。
fn extract_teammate_spawn(
    tool_use_result: Option<&serde_json::Value>,
) -> Option<cdt_core::TeammateSpawnInfo> {
    let v = tool_use_result?;
    let status = v.get("status").and_then(|s| s.as_str())?;
    if status != "teammate_spawned" {
        return None;
    }
    let name = v.get("name").and_then(|s| s.as_str())?.to_owned();
    let color = v.get("color").and_then(|s| s.as_str()).map(str::to_owned);
    Some(cdt_core::TeammateSpawnInfo { name, color })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_core::{MessageType, ToolCall};
    use chrono::Duration;

    fn ts(n: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-04-11T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::seconds(n)
    }

    fn blank(uuid: &str, n: i64) -> ParsedMessage {
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

    fn assistant_with_tool(uuid: &str, n: i64, id: &str, name: &str) -> ParsedMessage {
        let input = serde_json::json!({});
        ParsedMessage {
            message_type: MessageType::Assistant,
            category: MessageCategory::Assistant,
            content: MessageContent::Blocks(vec![ContentBlock::ToolUse {
                id: id.into(),
                name: name.into(),
                input: input.clone(),
            }]),
            tool_calls: vec![ToolCall {
                id: id.into(),
                name: name.into(),
                input,
                is_task: name == "Task",
                task_description: None,
                task_subagent_type: None,
            }],
            ..blank(uuid, n)
        }
    }

    fn user_with_result(
        uuid: &str,
        n: i64,
        id: &str,
        content: serde_json::Value,
        is_error: bool,
    ) -> ParsedMessage {
        ParsedMessage {
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: id.into(),
                content,
                is_error,
            }]),
            ..blank(uuid, n)
        }
    }

    #[test]
    fn immediate_result_links() {
        let msgs = vec![
            assistant_with_tool("a1", 1, "t1", "Bash"),
            user_with_result("u1", 2, "t1", serde_json::json!("done"), false),
        ];
        let r = pair_tool_executions(&msgs);
        assert_eq!(r.executions.len(), 1);
        assert_eq!(r.duplicates_dropped, 0);
        let e = &r.executions[0];
        assert_eq!(e.tool_use_id, "t1");
        assert_eq!(e.end_ts, Some(ts(2)));
        assert!(matches!(e.output, ToolOutput::Text { .. }));
    }

    #[test]
    fn delayed_result_links() {
        let msgs = vec![
            assistant_with_tool("a1", 1, "t1", "Bash"),
            blank("u1", 2),
            assistant_with_tool("a2", 3, "t2", "Read"),
            user_with_result("u2", 4, "t1", serde_json::json!("late"), false),
        ];
        let r = pair_tool_executions(&msgs);
        assert_eq!(r.executions.len(), 2);
        let t1 = r.executions.iter().find(|e| e.tool_use_id == "t1").unwrap();
        assert_eq!(t1.end_ts, Some(ts(4)));
        let t2 = r.executions.iter().find(|e| e.tool_use_id == "t2").unwrap();
        assert_eq!(t2.output, ToolOutput::Missing);
        assert_eq!(t2.end_ts, None);
    }

    #[test]
    fn duplicate_result_id_warns_and_keeps_first() {
        let msgs = vec![
            assistant_with_tool("a1", 1, "t1", "Bash"),
            user_with_result("u1", 2, "t1", serde_json::json!("first"), false),
            user_with_result("u2", 3, "t1", serde_json::json!("second"), false),
        ];
        let r = pair_tool_executions(&msgs);
        assert_eq!(r.executions.len(), 1);
        assert_eq!(r.duplicates_dropped, 1);
        let e = &r.executions[0];
        match &e.output {
            ToolOutput::Text { text } => assert_eq!(text, "first"),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn orphan_tool_use_produces_missing_record() {
        let msgs = vec![assistant_with_tool("a1", 1, "t1", "Bash")];
        let r = pair_tool_executions(&msgs);
        assert_eq!(r.executions.len(), 1);
        let e = &r.executions[0];
        assert_eq!(e.output, ToolOutput::Missing);
        assert_eq!(e.end_ts, None);
        assert!(!e.is_error);
    }

    #[test]
    fn error_result_sets_is_error() {
        let msgs = vec![
            assistant_with_tool("a1", 1, "t1", "Bash"),
            user_with_result("u1", 2, "t1", serde_json::json!("boom"), true),
        ];
        let r = pair_tool_executions(&msgs);
        assert!(r.executions[0].is_error);
    }

    #[test]
    fn bash_structured_result_preserved() {
        let structured = serde_json::json!({"stdout": "ok", "stderr": "warn", "exit_code": 0});
        let msgs = vec![
            assistant_with_tool("a1", 1, "t1", "Bash"),
            user_with_result("u1", 2, "t1", structured.clone(), false),
        ];
        let r = pair_tool_executions(&msgs);
        match &r.executions[0].output {
            ToolOutput::Structured { value } => assert_eq!(value, &structured),
            other => panic!("expected Structured, got {other:?}"),
        }
    }

    #[test]
    fn legacy_string_result_text_variant() {
        let msgs = vec![
            assistant_with_tool("a1", 1, "t1", "Read"),
            user_with_result("u1", 2, "t1", serde_json::json!("file contents"), false),
        ];
        let r = pair_tool_executions(&msgs);
        match &r.executions[0].output {
            ToolOutput::Text { text } => assert_eq!(text, "file contents"),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    // ---- teammate_spawn 检测（与 LinkedToolItem.tsx::isTeammateSpawned 对齐）----

    fn user_with_result_and_top(
        uuid: &str,
        n: i64,
        id: &str,
        content: serde_json::Value,
        tool_use_result: serde_json::Value,
    ) -> ParsedMessage {
        ParsedMessage {
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: id.into(),
                content,
                is_error: false,
            }]),
            tool_use_result: Some(tool_use_result),
            ..blank(uuid, n)
        }
    }

    #[test]
    fn teammate_spawn_status_populates_field() {
        let msgs = vec![
            assistant_with_tool("a1", 1, "tu-spawn", "Agent"),
            user_with_result_and_top(
                "u1",
                2,
                "tu-spawn",
                serde_json::json!("ok"),
                serde_json::json!({
                    "status": "teammate_spawned",
                    "name": "member-1",
                    "color": "blue",
                }),
            ),
        ];
        let r = pair_tool_executions(&msgs);
        let e = r
            .executions
            .iter()
            .find(|e| e.tool_use_id == "tu-spawn")
            .unwrap();
        let spawn = e
            .teammate_spawn
            .as_ref()
            .expect("teammate_spawn should be Some");
        assert_eq!(spawn.name, "member-1");
        assert_eq!(spawn.color.as_deref(), Some("blue"));
    }

    #[test]
    fn teammate_spawn_without_color_is_some() {
        let msgs = vec![
            assistant_with_tool("a1", 1, "tu-spawn", "Agent"),
            user_with_result_and_top(
                "u1",
                2,
                "tu-spawn",
                serde_json::json!("ok"),
                serde_json::json!({"status": "teammate_spawned", "name": "member-2"}),
            ),
        ];
        let r = pair_tool_executions(&msgs);
        let spawn = r.executions[0].teammate_spawn.as_ref().unwrap();
        assert_eq!(spawn.name, "member-2");
        assert!(spawn.color.is_none());
    }

    #[test]
    fn other_status_leaves_teammate_spawn_none() {
        let msgs = vec![
            assistant_with_tool("a1", 1, "t1", "Bash"),
            user_with_result_and_top(
                "u1",
                2,
                "t1",
                serde_json::json!("done"),
                serde_json::json!({"status": "ok"}),
            ),
        ];
        let r = pair_tool_executions(&msgs);
        assert!(r.executions[0].teammate_spawn.is_none());
    }

    #[test]
    fn missing_top_level_tool_use_result_leaves_none() {
        let msgs = vec![
            assistant_with_tool("a1", 1, "t1", "Bash"),
            user_with_result("u1", 2, "t1", serde_json::json!("done"), false),
        ];
        let r = pair_tool_executions(&msgs);
        assert!(r.executions[0].teammate_spawn.is_none());
    }

    #[test]
    fn teammate_spawned_status_without_name_is_none() {
        // status 命中但 name 缺失 → 视为不完整，teammate_spawn = None
        let msgs = vec![
            assistant_with_tool("a1", 1, "t1", "Agent"),
            user_with_result_and_top(
                "u1",
                2,
                "t1",
                serde_json::json!("ok"),
                serde_json::json!({"status": "teammate_spawned"}),
            ),
        ];
        let r = pair_tool_executions(&msgs);
        assert!(r.executions[0].teammate_spawn.is_none());
    }
}
