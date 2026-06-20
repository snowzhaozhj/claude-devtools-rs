//! 集成测试：`get_subagent_trace` 把嵌套 `Agent` 调用升级为骨架 subagent。
//!
//! 覆盖 spec：
//! - `ipc-data-api` §"Lazy load subagent trace" Scenario
//!   `返回的 trace 把嵌套 Agent 调用暴露为可展开 subagent`
//! - `chunk-building` §"Promote nested Agent calls to skeleton subagents"
//!
//! 端到端验证 `result_agent_id` 提取（cdt-parse）→ `build_chunks` → promote 链路：
//! sub-a 的 transcript 内 spawn sub-b（Agent 调用 + 顶层 `toolUseResult.agentId`），
//! 拉取 sub-a trace 后其 `AIChunk.subagents` SHALL 含 sub-b 的骨架。

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
