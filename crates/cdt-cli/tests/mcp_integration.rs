//! MCP server integration tests.
//!
//! Uses tokio duplex transport to exercise the MCP server without spawning a process.

use std::sync::Arc;

use rmcp::{ClientHandler, ServiceExt, model::*};

// Minimal client handler for test
#[derive(Default, Clone)]
struct TestClient;
impl ClientHandler for TestClient {}

/// Helper: create a running MCP server + client pair connected via duplex.
async fn setup_pair() -> rmcp::service::RunningService<rmcp::RoleClient, TestClient> {
    use cdt_api::LocalDataApi;
    use cdt_config::{ConfigManager, NotificationManager};
    use cdt_discover::{ProjectScanner, local_handle, path_decoder};
    use cdt_query::QueryEngine;
    use cdt_ssh::SshConnectionManager;
    use tokio::sync::Semaphore;

    let mut config_mgr = ConfigManager::new(None);
    config_mgr.load().await.unwrap();
    let mut notif_mgr = NotificationManager::new(None);
    notif_mgr.load().await.unwrap();

    let fs = local_handle();
    let projects_dir = path_decoder::projects_base_path_for(
        config_mgr
            .get_config()
            .general
            .claude_root_path
            .as_deref()
            .map(std::path::PathBuf::from)
            .as_deref(),
    );
    let scanner_semaphore = Arc::new(Semaphore::new(64));
    let scanner = ProjectScanner::new_with_semaphore(fs, projects_dir, scanner_semaphore);
    let ssh_mgr = SshConnectionManager::new();
    let api = Arc::new(LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr));
    let engine = Arc::new(QueryEngine::new(api));

    let server = cdt_cli::mcp::CdtMcpServer::new(engine, false);

    let (server_transport, client_transport) = tokio::io::duplex(65536);

    tokio::spawn(async move {
        let service = server.serve(server_transport).await.unwrap();
        service.waiting().await.unwrap();
    });

    let client = TestClient;
    client.serve(client_transport).await.unwrap()
}

#[tokio::test]
async fn mcp_server_responds_to_list_tools() {
    let client = setup_pair().await;

    let tools_result = client
        .send_request(rmcp::model::ClientRequest::ListToolsRequest(
            RequestOptionalParam::default(),
        ))
        .await
        .unwrap();

    let rmcp::model::ServerResult::ListToolsResult(list) = tools_result else {
        panic!("expected ListToolsResult");
    };

    let tool_names: Vec<&str> = list.tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(tool_names.contains(&"list_projects"));
    assert!(tool_names.contains(&"get_session_summary"));
    assert!(tool_names.contains(&"get_session_detail"));
    assert!(tool_names.contains(&"search_sessions"));
    assert!(tool_names.contains(&"get_session_errors"));
    assert!(tool_names.contains(&"get_session_cost"));
    assert!(tool_names.contains(&"list_sessions"));
    assert!(tool_names.contains(&"get_stats"));
    assert_eq!(tool_names.len(), 8);

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_list_projects_returns_compact_json() {
    let client = setup_pair().await;

    let result = client
        .send_request(rmcp::model::ClientRequest::CallToolRequest(
            rmcp::model::Request::new(CallToolRequestParams::new("list_projects")),
        ))
        .await
        .unwrap();

    let rmcp::model::ServerResult::CallToolResult(call_result) = result else {
        panic!("expected CallToolResult");
    };

    assert!(!call_result.content.is_empty());
    let text_content = call_result.content[0]
        .raw
        .as_text()
        .expect("expected text content");
    let text = &text_content.text;
    // Should be valid JSON (array) and compact (no leading whitespace after opening bracket)
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert!(parsed.is_array());
    // Compact JSON should not have newlines
    assert!(
        !text.contains('\n'),
        "expected compact JSON without newlines"
    );

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_list_sessions_returns_error_for_unknown_project() {
    let client = setup_pair().await;

    let params = serde_json::json!({
        "project": "nonexistent-project-for-test"
    });

    let result = client
        .send_request(rmcp::model::ClientRequest::CallToolRequest(
            rmcp::model::Request::new(
                CallToolRequestParams::new("list_sessions")
                    .with_arguments(serde_json::from_value(params).unwrap()),
            ),
        ))
        .await;

    // MCP returns JSON-RPC error for invalid params
    assert!(result.is_err(), "expected error for nonexistent project");

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_get_session_detail_rejects_conflicting_window_params() {
    let client = setup_pair().await;

    let params = serde_json::json!({
        "session": "test-session-id",
        "range": "0:10",
        "tail": 5
    });

    let result = client
        .send_request(rmcp::model::ClientRequest::CallToolRequest(
            rmcp::model::Request::new(
                CallToolRequestParams::new("get_session_detail")
                    .with_arguments(serde_json::from_value(params).unwrap()),
            ),
        ))
        .await;

    // Mutually exclusive params → JSON-RPC error
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    assert!(
        err_str.contains("mutually exclusive"),
        "error should mention mutual exclusivity, got: {err_str}"
    );

    client.cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_get_session_detail_rejects_invalid_content_mode() {
    let client = setup_pair().await;

    let params = serde_json::json!({
        "session": "test-session-id",
        "content_mode": "invalid"
    });

    let result = client
        .send_request(rmcp::model::ClientRequest::CallToolRequest(
            rmcp::model::Request::new(
                CallToolRequestParams::new("get_session_detail")
                    .with_arguments(serde_json::from_value(params).unwrap()),
            ),
        ))
        .await;

    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    assert!(
        err_str.contains("Invalid content_mode"),
        "error should mention invalid content_mode, got: {err_str}"
    );

    client.cancel().await.unwrap();
}
