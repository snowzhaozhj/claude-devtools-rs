//! Team 工具摘要格式化。
//!
//! Spec：`openspec/specs/team-coordination-metadata/spec.md`
//! §"`Recognize team coordination tools`"。

/// 是否是 team coordination 工具。
pub fn is_team_tool(name: &str) -> bool {
    matches!(
        name,
        "TeamCreate"
            | "TaskCreate"
            | "TaskUpdate"
            | "TaskList"
            | "TaskGet"
            | "SendMessage"
            | "TeamDelete"
    )
}

/// 格式化 team 工具摘要。
pub fn format_team_tool_summary(name: &str, input: &serde_json::Value) -> String {
    match name {
        "TeamCreate" => {
            let team_name = input
                .get("team_name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let desc = input.get("description").and_then(|v| v.as_str());
            if let Some(d) = desc {
                let truncated = truncate(d, 50);
                format!("{team_name} - {truncated}")
            } else {
                team_name.to_owned()
            }
        }
        "TaskCreate" => {
            let subject = input
                .get("subject")
                .and_then(|v| v.as_str())
                .unwrap_or("untitled");
            truncate(subject, 50)
        }
        "TaskUpdate" => {
            let task_id = input.get("taskId").and_then(|v| v.as_str()).unwrap_or("?");
            let status = input.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let owner = input.get("owner").and_then(|v| v.as_str());
            let mut parts = vec![format!("#{task_id}")];
            if !status.is_empty() {
                parts.push(status.to_owned());
            }
            if let Some(o) = owner {
                parts.push(format!("-> {o}"));
            }
            parts.join(" ")
        }
        "TaskList" => "List tasks".into(),
        "TaskGet" => {
            let task_id = input.get("taskId").and_then(|v| v.as_str()).unwrap_or("?");
            format!("Get task #{task_id}")
        }
        "SendMessage" => format_send_message(input),
        "TeamDelete" => "Delete team".into(),
        _ => name.to_owned(),
    }
}

/// 格式化 `SendMessage` 摘要。
fn format_send_message(input: &serde_json::Value) -> String {
    let msg_type = input
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("message");
    let to = input.get("to").and_then(|v| v.as_str());

    match msg_type {
        "shutdown_response" => {
            let approve = input
                .get("approve")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if approve {
                "Shutdown approved".into()
            } else {
                "Shutdown denied".into()
            }
        }
        "broadcast" => {
            let msg = input.get("message").and_then(|v| v.as_str()).unwrap_or("");
            format!("Broadcast: {}", truncate(msg, 50))
        }
        _ => {
            if let Some(recipient) = to {
                let msg = input.get("message").and_then(|v| v.as_str()).unwrap_or("");
                format!("To {recipient}: {}", truncate(msg, 50))
            } else {
                truncate(msg_type, 50)
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        format!("{}...", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_create_with_desc() {
        let input = serde_json::json!({"team_name": "alpha", "description": "investigate bugs"});
        assert_eq!(
            format_team_tool_summary("TeamCreate", &input),
            "alpha - investigate bugs"
        );
    }

    #[test]
    fn team_create_no_desc() {
        let input = serde_json::json!({"team_name": "beta"});
        assert_eq!(format_team_tool_summary("TeamCreate", &input), "beta");
    }

    #[test]
    fn task_create_summary() {
        let input = serde_json::json!({"subject": "Fix the login bug"});
        assert_eq!(
            format_team_tool_summary("TaskCreate", &input),
            "Fix the login bug"
        );
    }

    #[test]
    fn task_update_summary() {
        let input = serde_json::json!({"taskId": "42", "status": "completed", "owner": "alice"});
        assert_eq!(
            format_team_tool_summary("TaskUpdate", &input),
            "#42 completed -> alice"
        );
    }

    #[test]
    fn task_list_summary() {
        assert_eq!(
            format_team_tool_summary("TaskList", &serde_json::json!({})),
            "List tasks"
        );
    }

    #[test]
    fn task_get_summary() {
        let input = serde_json::json!({"taskId": "7"});
        assert_eq!(format_team_tool_summary("TaskGet", &input), "Get task #7");
    }

    #[test]
    fn send_message_shutdown_approved() {
        let input = serde_json::json!({"type": "shutdown_response", "approve": true});
        assert_eq!(
            format_team_tool_summary("SendMessage", &input),
            "Shutdown approved"
        );
    }

    #[test]
    fn send_message_broadcast() {
        let input = serde_json::json!({"type": "broadcast", "message": "all done"});
        assert_eq!(
            format_team_tool_summary("SendMessage", &input),
            "Broadcast: all done"
        );
    }

    #[test]
    fn send_message_to_recipient() {
        let input = serde_json::json!({"type": "message", "to": "bob", "message": "check this"});
        assert_eq!(
            format_team_tool_summary("SendMessage", &input),
            "To bob: check this"
        );
    }

    #[test]
    fn team_delete_summary() {
        assert_eq!(
            format_team_tool_summary("TeamDelete", &serde_json::json!({})),
            "Delete team"
        );
    }

    #[test]
    fn is_team_tool_positive() {
        assert!(is_team_tool("TeamCreate"));
        assert!(is_team_tool("TaskCreate"));
        assert!(is_team_tool("SendMessage"));
    }

    #[test]
    fn is_team_tool_negative() {
        assert!(!is_team_tool("Bash"));
        assert!(!is_team_tool("Read"));
    }
}
