//! 集成测试：验证 CLI JSON 输出格式的序列化正确性。

use std::process::Command;

fn cdt_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cdt"));
    cmd.env("RUST_LOG", "off");
    cmd
}

#[test]
fn help_exits_zero() {
    let output = cdt_bin().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("projects"));
    assert!(stdout.contains("sessions"));
    assert!(stdout.contains("serve"));
}

#[test]
fn projects_list_json_outputs_valid_json() {
    let output = cdt_bin()
        .args(["--format", "json", "projects", "list"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("invalid JSON output: {e}\nstdout: {stdout}"));
    assert!(parsed.is_array(), "expected JSON array, got: {parsed}");
}

#[test]
fn projects_list_json_has_camel_case_fields() {
    let output = cdt_bin()
        .args(["--format", "json", "projects", "list"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    if let Some(first) = parsed.first() {
        let obj = first.as_object().unwrap();
        assert!(obj.contains_key("id"), "missing 'id' field");
        assert!(obj.contains_key("name"), "missing 'name' field");
        assert!(obj.contains_key("worktrees"), "missing 'worktrees' field");
        assert!(
            obj.contains_key("totalSessions"),
            "missing 'totalSessions' field (camelCase)"
        );
        assert!(
            obj.contains_key("mostRecentSession"),
            "missing 'mostRecentSession' field (camelCase)"
        );
    }
}

#[test]
fn sessions_list_without_project_fails() {
    let output = cdt_bin()
        .args(["--format", "json", "sessions", "list"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--project"));
}

#[test]
fn projects_list_table_has_header() {
    let output = cdt_bin()
        .args(["--format", "table", "projects", "list"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("NAME"));
    assert!(stdout.contains("PATH"));
    assert!(stdout.contains("SESSIONS"));
    assert!(stdout.contains("LAST ACTIVE"));
}
