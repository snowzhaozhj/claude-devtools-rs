use cdt_api::{DataApi, LocalDataApi};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{ProjectScanner, local_handle, path_decoder};
use cdt_ssh::SshConnectionManager;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let mut config_mgr = ConfigManager::new(None);
    let _ = config_mgr.load().await;
    let mut notif_mgr = NotificationManager::new(None);
    let _ = notif_mgr.load().await;
    let fs = local_handle();
    let projects_dir = path_decoder::get_projects_base_path();
    let scanner = ProjectScanner::new(fs, projects_dir);
    let ssh_mgr = SshConnectionManager::new();
    let api = Arc::new(LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr));

    let project_id = "-Users-zhaohejie-RustroverProjects-Project-claude-devtools-rs";
    let session_id = "46a25772-b57c-43bb-9ca6-f0292f9ca912";

    // 诊断：直接解析 JSONL 看 task_calls 和 tool outputs
    let jsonl_path = path_decoder::get_projects_base_path()
        .join(project_id)
        .join(format!("{session_id}.jsonl"));
    let messages = cdt_parse::parse_file(&jsonl_path).await.unwrap();
    eprintln!("Messages: {}", messages.len());

    let task_calls: Vec<_> = messages
        .iter()
        .flat_map(|m| m.tool_calls.iter())
        .filter(|tc| tc.is_task)
        .collect();
    eprintln!("Task/Agent tool_calls: {}", task_calls.len());
    for tc in &task_calls {
        eprintln!(
            "  {} id={} desc={:?}",
            tc.name,
            &tc.id[..16.min(tc.id.len())],
            tc.task_description
                .as_deref()
                .map(|s| &s[..40.min(s.len())])
        );
    }

    // 检查 pair_tool_executions 中 Agent tool 的 output 类型
    let linking = cdt_analyze::tool_linking::pair_tool_executions(&messages);
    for exec in &linking.executions {
        if task_calls.iter().any(|tc| tc.id == exec.tool_use_id) {
            let output_kind = match &exec.output {
                cdt_core::ToolOutput::Text { text } => {
                    format!("Text({text:.60})")
                }
                cdt_core::ToolOutput::Structured { value } => {
                    format!("Structured({value:.80})")
                }
                cdt_core::ToolOutput::Missing => "Missing".to_string(),
            };
            eprintln!(
                "  exec {} output: {output_kind}",
                &exec.tool_use_id[..16.min(exec.tool_use_id.len())]
            );
        }
    }

    eprintln!("\nCalling get_session_detail...");
    match api.get_session_detail(project_id, session_id).await {
        Ok(detail) => {
            let detail_json: serde_json::Value = serde_json::to_value(&detail).unwrap();
            let chunks = detail_json["chunks"].as_array().unwrap();
            eprintln!("Total chunks: {}", chunks.len());

            for (i, chunk) in chunks.iter().enumerate() {
                let kind = chunk["kind"].as_str().unwrap_or("?");
                if kind == "ai" {
                    let steps = chunk["semanticSteps"].as_array().unwrap();
                    let execs = chunk["toolExecutions"].as_array().unwrap();
                    let subs = chunk["subagents"].as_array().unwrap();

                    let spawn_count = steps
                        .iter()
                        .filter(|s| s["kind"].as_str() == Some("subagent_spawn"))
                        .count();
                    let text_count = steps
                        .iter()
                        .filter(|s| s["kind"].as_str() == Some("text"))
                        .count();
                    let tool_count = steps
                        .iter()
                        .filter(|s| s["kind"].as_str() == Some("tool_execution"))
                        .count();

                    if !subs.is_empty() || spawn_count > 0 || execs.len() > 2 {
                        eprintln!(
                            "  chunk[{i}]: execs={}, subagents={}, spawn_steps={}, text={}, tool_steps={}",
                            execs.len(),
                            subs.len(),
                            spawn_count,
                            text_count,
                            tool_count,
                        );
                        for sub in subs {
                            let sid = sub["sessionId"].as_str().unwrap_or("?");
                            let desc = sub["rootTaskDescription"]
                                .as_str()
                                .unwrap_or("")
                                .chars()
                                .take(50)
                                .collect::<String>();
                            eprintln!("    sub: {}, desc={desc}", &sid[..16.min(sid.len())]);
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    }
}
