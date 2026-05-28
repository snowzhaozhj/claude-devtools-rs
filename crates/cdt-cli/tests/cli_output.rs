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
    assert!(stdout.contains("search"));
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

#[test]
fn projects_list_jsonl_outputs_ndjson() {
    let output = cdt_bin()
        .args(["--format", "jsonl", "projects", "list"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let _: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("invalid NDJSON line: {e}\nline: {line}"));
    }
}

#[test]
fn sessions_show_without_valid_id_fails() {
    let output = cdt_bin()
        .args([
            "--format",
            "json",
            "sessions",
            "show",
            "nonexistent-session-id-xyz",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn sessions_detail_without_valid_id_fails() {
    let output = cdt_bin()
        .args([
            "--format",
            "json",
            "sessions",
            "detail",
            "nonexistent-session-id-xyz",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn sessions_errors_without_valid_id_fails() {
    let output = cdt_bin()
        .args([
            "--format",
            "json",
            "sessions",
            "errors",
            "nonexistent-session-id-xyz",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn search_without_results_exits_2() {
    let output = cdt_bin()
        .args([
            "--format",
            "json",
            "--project",
            "nonexistent-project-xyz",
            "search",
            "zzz_no_match_zzz",
        ])
        .output()
        .unwrap();
    // Either error (project not found) or exit 2 (no results)
    assert!(!output.status.success());
}

#[test]
fn sessions_list_with_filter_flags_accepted() {
    // Verify the new flags are accepted by the parser (even if no results)
    let output = cdt_bin()
        .args([
            "--format",
            "json",
            "--project",
            "nonexistent-project-xyz",
            "sessions",
            "list",
            "--grep",
            "test",
            "--min-messages",
            "5",
            "--since",
            "7d",
        ])
        .output()
        .unwrap();
    // Project not found → error, but flags parsed OK (no clap error)
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument"),
        "flags not recognized: {stderr}"
    );
}

#[test]
fn sessions_detail_with_range_flag_accepted() {
    let output = cdt_bin()
        .args([
            "--format", "json", "sessions", "detail", "fake-id", "--range", "0:10", "--tail", "5",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument"),
        "flags not recognized: {stderr}"
    );
}

#[test]
fn sessions_detail_with_filter_flag_accepted() {
    let output = cdt_bin()
        .args([
            "--format",
            "json",
            "sessions",
            "detail",
            "fake-id",
            "--filter",
            "errors_only",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument"),
        "flags not recognized: {stderr}"
    );
}

#[test]
fn sessions_help_shows_summary_and_cost() {
    let output = cdt_bin().args(["sessions", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("summary"));
    assert!(stdout.contains("cost"));
    assert!(stdout.contains("list"));
}

#[test]
fn stats_help_works() {
    let output = cdt_bin().args(["stats", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("PERIOD"));
}

#[test]
fn sessions_summary_without_session_id_fails() {
    let output = cdt_bin().args(["sessions", "summary"]).output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn sessions_cost_without_session_id_fails() {
    let output = cdt_bin().args(["sessions", "cost"]).output().unwrap();
    assert!(!output.status.success());
}
