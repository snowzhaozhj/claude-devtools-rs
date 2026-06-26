//! CLI help 输出的快照测试。
//!
//! 锁定 `cdt --help` / 子命令 `--help` 的完整输出，
//! 防止意外修改用户可见的命令行界面。版本号会被过滤，
//! clap 的 help 换行会被 normalize（本地终端宽时不换行，
//! CI 无 TTY 时 fallback 到 100 列换行）。

use std::process::{Command, Stdio};

fn cdt_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cdt"));
    cmd.env("RUST_LOG", "off");
    // clap 的 help 换行宽度来自 terminal_size，它会依次探测 stdout/stderr/stdin 的 tty。
    // `.output()` 已管道 stdout/stderr，但 stdin 默认继承——本地交互式 shell 下 stdin 是
    // 宽 tty 会让 clap 不换行，CI 无 tty 则回退默认列宽换行，导致快照跨环境 flaky。
    // 显式 null 掉三条 std 流，让 clap 永远走无 tty 默认宽度，快照确定性。
    cmd.stdin(Stdio::null());
    cmd
}

fn normalize_help_wrapping(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let leading = line.len() - trimmed.len();
        // clap 续行缩进 ≥ 25 个空格（option 描述列对齐位置）
        if leading >= 25 && !trimmed.is_empty() {
            if let Some(prev) = lines.last_mut() {
                prev.push(' ');
                prev.push_str(trimmed);
                continue;
            }
        }
        lines.push(line.to_string());
    }
    let mut result = lines.join("\n");
    result.push('\n');
    result
}

fn help_output(args: &[&str]) -> String {
    let output = cdt_bin().args(args).output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    normalize_help_wrapping(&text)
}

fn insta_settings() -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    settings.add_filter(r"\b\d+\.\d+\.\d+\b", "[VERSION]");
    // Windows 上 binary 名是 cdt.exe
    settings.add_filter(r"\bcdt\.exe\b", "cdt");
    settings
}

#[test]
fn help_main() {
    let text = help_output(&["--help"]);
    insta_settings().bind(|| {
        insta::assert_snapshot!(text);
    });
}

#[test]
fn help_projects() {
    let text = help_output(&["projects", "--help"]);
    insta_settings().bind(|| {
        insta::assert_snapshot!(text);
    });
}

#[test]
fn help_sessions() {
    let text = help_output(&["sessions", "--help"]);
    insta_settings().bind(|| {
        insta::assert_snapshot!(text);
    });
}

#[test]
fn help_search() {
    let text = help_output(&["search", "--help"]);
    insta_settings().bind(|| {
        insta::assert_snapshot!(text);
    });
}

#[test]
fn help_stats() {
    let text = help_output(&["stats", "--help"]);
    insta_settings().bind(|| {
        insta::assert_snapshot!(text);
    });
}

#[test]
fn help_serve() {
    let text = help_output(&["serve", "--help"]);
    insta_settings().bind(|| {
        insta::assert_snapshot!(text);
    });
}

#[test]
fn help_mcp() {
    let text = help_output(&["mcp", "--help"]);
    insta_settings().bind(|| {
        insta::assert_snapshot!(text);
    });
}

#[test]
fn help_setup() {
    let text = help_output(&["setup", "--help"]);
    insta_settings().bind(|| {
        insta::assert_snapshot!(text);
    });
}

#[test]
fn help_completions() {
    let text = help_output(&["completions", "--help"]);
    insta_settings().bind(|| {
        insta::assert_snapshot!(text);
    });
}

#[test]
fn help_self_update() {
    let text = help_output(&["self-update", "--help"]);
    insta_settings().bind(|| {
        insta::assert_snapshot!(text);
    });
}
