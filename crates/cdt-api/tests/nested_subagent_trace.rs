//! 集成测试：两条路径都把嵌套 `Agent` 调用升级为骨架 subagent。
//!
//! 覆盖 spec：
//! - `ipc-data-api` §"Lazy load subagent trace" 两个 Scenario：
//!   `返回的 trace 把嵌套 Agent 调用暴露为可展开 subagent`（懒拉路径）+
//!   `get_session_detail 内联 subagent messages 升级嵌套 Agent`（内联路径）
//! - `chunk-building` §"Promote nested Agent calls to skeleton subagents"
//!
//! 端到端验证 `result_agent_id` 提取（cdt-parse）→ `build_chunks` → promote 链路：
//! sub-a 的 transcript 内 spawn sub-b（Agent 调用 + 顶层 `toolUseResult.agentId`）。
//! 路径一：拉取 sub-a trace（`get_subagent_trace`）后其 `AIChunk.subagents` SHALL
//! 含 sub-b 骨架。路径二：root session 经 `get_session_detail` 把 sub-a 装为内联
//! `Process`（`messagesOmitted=false`），其内联 messages SHALL 同样含 sub-b 骨架
//! （HTTP / MCP / CLI 完整 payload 路径，bug 复现根因）。

use std::sync::Arc;

use cdt_api::{DataApi, LocalDataApi};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;

async fn build_api(tmp: &TempDir) -> Arc<LocalDataApi> {
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();
    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base);
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.unwrap();
    let mut notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    notif_mgr.load().await.unwrap();
    let ssh_mgr = SshConnectionManager::new();
    Arc::new(LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr))
}

/// sub-a 的 transcript：prompt → assistant(spawn sub-b via Agent) → `tool_result`
/// 携带顶层 `toolUseResult.agentId = "sub-b"`。
fn sub_a_jsonl() -> String {
    let user = serde_json::json!({
        "type": "user",
        "uuid": "u1",
        "timestamp": "2026-06-20T10:00:00Z",
        "cwd": "/tmp/proj",
        "message": {"role": "user", "content": "do nested work"}
    });
    let assistant = serde_json::json!({
        "type": "assistant",
        "uuid": "a1",
        "parentUuid": "u1",
        "timestamp": "2026-06-20T10:00:01Z",
        "cwd": "/tmp/proj",
        "message": {
            "role": "assistant",
            "model": "claude-sonnet",
            "content": [
                {"type": "text", "text": "spawning child"},
                {"type": "tool_use", "id": "toolu_1", "name": "Agent",
                 "input": {"subagent_type": "Explore", "description": "scan submodule"}}
            ]
        }
    });
    let tool_result = serde_json::json!({
        "type": "user",
        "uuid": "u2",
        "parentUuid": "a1",
        "timestamp": "2026-06-20T10:05:00Z",
        "cwd": "/tmp/proj",
        "message": {
            "role": "user",
            "content": [
                {"type": "tool_result", "tool_use_id": "toolu_1", "content": "child done"}
            ]
        },
        "toolUseResult": {"status": "completed", "agentId": "sub-b", "agentType": "Explore"}
    });
    format!("{user}\n{assistant}\n{tool_result}\n")
}

#[tokio::test]
async fn get_subagent_trace_promotes_nested_agent_to_skeleton_subagent() {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    // 新嵌套结构：<projects>/<proj>/<root>/subagents/agent-<sub-a>.jsonl
    let subagents_dir = projects_base
        .join("-proj-nested")
        .join("root-uuid")
        .join("subagents");
    std::fs::create_dir_all(&subagents_dir).unwrap();
    std::fs::write(subagents_dir.join("agent-sub-a.jsonl"), sub_a_jsonl()).unwrap();

    let api = build_api(&tmp).await;

    let trace = api
        .get_subagent_trace("root-uuid", "sub-a")
        .await
        .expect("get_subagent_trace SHALL Ok");
    assert!(!trace.is_empty(), "sub-a trace SHALL 非空");

    // 序列化为 IPC JSON，验证骨架 subagent 形态 + camelCase 字段名。
    let json = serde_json::to_value(&trace).unwrap();
    let mut skeleton = None;
    for chunk in json.as_array().unwrap() {
        if let Some(subs) = chunk.get("subagents").and_then(|v| v.as_array()) {
            for s in subs {
                if s.get("sessionId").and_then(|v| v.as_str()) == Some("sub-b") {
                    skeleton = Some(s.clone());
                }
            }
        }
    }
    let sk = skeleton.expect("sub-a trace SHALL 含 sub-b 的骨架 subagent");
    assert_eq!(
        sk.get("messagesOmitted")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "骨架 SHALL messagesOmitted=true（懒拉）"
    );
    assert_eq!(
        sk.get("parentTaskId").and_then(|v| v.as_str()),
        Some("toolu_1"),
        "骨架 parentTaskId SHALL = 触发 Agent 调用的 tool_use_id（前端据此去重）"
    );
    assert_eq!(
        sk.get("isOngoing").and_then(serde_json::Value::as_bool),
        Some(false),
        "骨架 isOngoing SHALL false（design D4 降级）"
    );
    assert!(
        sk.get("messagesTotalCount").is_some(),
        "骨架 SHALL 含 messagesTotalCount 字段（camelCase round-trip）"
    );
    assert_eq!(
        sk.get("subagentType").and_then(|v| v.as_str()),
        Some("Explore"),
    );
}

/// root session：user → assistant(spawn sub-a via Agent, id=`toolu_root`) →
/// `tool_result` 携带 `toolUseResult.agentId="sub-a"`（result-based 关联）。
/// 让 `get_session_detail` 把 sub-a 装进 `AIChunk.subagents` 作内联 `Process`。
fn root_jsonl() -> String {
    let user = serde_json::json!({
        "type": "user",
        "uuid": "ru1",
        "timestamp": "2026-06-20T09:00:00Z",
        "cwd": "/tmp/proj",
        "message": {"role": "user", "content": "do top-level work"}
    });
    let assistant = serde_json::json!({
        "type": "assistant",
        "uuid": "ra1",
        "parentUuid": "ru1",
        "timestamp": "2026-06-20T09:00:01Z",
        "cwd": "/tmp/proj",
        "message": {
            "role": "assistant",
            "model": "claude-sonnet",
            "content": [
                {"type": "text", "text": "spawning sub-a"},
                {"type": "tool_use", "id": "toolu_root", "name": "Agent",
                 "input": {"subagent_type": "Explore", "description": "review angle"}}
            ]
        }
    });
    let tool_result = serde_json::json!({
        "type": "user",
        "uuid": "ru2",
        "parentUuid": "ra1",
        "timestamp": "2026-06-20T09:30:00Z",
        "cwd": "/tmp/proj",
        "message": {
            "role": "user",
            "content": [
                {"type": "tool_result", "tool_use_id": "toolu_root", "content": "sub-a done"}
            ]
        },
        "toolUseResult": {"status": "completed", "agentId": "sub-a", "agentType": "Explore"}
    });
    format!("{user}\n{assistant}\n{tool_result}\n")
}

/// 递归在 JSON 树里找 `sessionId == target` 的 subagent，返回其 JSON。
fn find_subagent<'a>(value: &'a serde_json::Value, target: &str) -> Option<&'a serde_json::Value> {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(subs) = map.get("subagents").and_then(|v| v.as_array()) {
                for s in subs {
                    if s.get("sessionId").and_then(|v| v.as_str()) == Some(target) {
                        return Some(s);
                    }
                }
            }
            for v in map.values() {
                if let Some(hit) = find_subagent(v, target) {
                    return Some(hit);
                }
            }
            None
        }
        serde_json::Value::Array(arr) => arr.iter().find_map(|v| find_subagent(v, target)),
        _ => None,
    }
}

/// `get_session_detail` 内联路径（candidate.messages → 内联 `Process.messages`，
/// `messagesOmitted=false`，HTTP / MCP / CLI 完整 payload）同样把内联 messages
/// 内的嵌套 `Agent` 调用升级为骨架——覆盖 ipc-data-api §"Lazy load subagent
/// trace" 的 `Scenario: get_session_detail 内联 subagent messages 升级嵌套 Agent`
/// （bug：HTTP 页面嵌套层曾显示为普通工具，根因为内联路径未 promote）。
#[tokio::test]
async fn get_session_detail_promotes_nested_agent_in_inline_subagent_messages() {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    let proj_dir = projects_base.join("-proj-nested");
    let subagents_dir = proj_dir.join("root-uuid").join("subagents");
    std::fs::create_dir_all(&subagents_dir).unwrap();
    // root session 文件 + sub-a transcript（sub-a 内 spawn sub-b）
    std::fs::write(proj_dir.join("root-uuid.jsonl"), root_jsonl()).unwrap();
    std::fs::write(subagents_dir.join("agent-sub-a.jsonl"), sub_a_jsonl()).unwrap();

    let api = build_api(&tmp).await;

    let resp = api
        .get_session_detail("-proj-nested", "root-uuid", None)
        .await
        .expect("get_session_detail SHALL Ok");
    let json = serde_json::to_value(&resp).unwrap();

    // sub-a 应作为内联 Process 装入（messagesOmitted=false，LocalDataApi 不裁剪）
    let sub_a = find_subagent(&json, "sub-a").expect("session detail SHALL 含 sub-a 内联 subagent");
    assert_eq!(
        sub_a
            .get("messagesOmitted")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "LocalDataApi::get_session_detail SHALL 返回未裁剪 sub-a messages（messagesOmitted=false）"
    );

    // 关键断言：sub-a 的内联 messages 内含 sub-b 的骨架（内联路径 promote 生效）
    let sub_b = find_subagent(sub_a, "sub-b")
        .expect("sub-a 内联 messages SHALL 含 sub-b 骨架（parse_subagent_candidate promote 生效）");
    assert_eq!(
        sub_b
            .get("messagesOmitted")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "嵌套骨架 SHALL messagesOmitted=true（懒拉）"
    );
    assert_eq!(
        sub_b.get("parentTaskId").and_then(|v| v.as_str()),
        Some("toolu_1"),
        "嵌套骨架 parentTaskId SHALL = sub-a 内触发 Agent 的 tool_use_id（前端据此去重）"
    );
}
