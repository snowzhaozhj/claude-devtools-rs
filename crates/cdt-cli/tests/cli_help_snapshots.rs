//! CLI help 输出的快照测试。
//!
//! 锁定 `cdt --help` / 子命令 `--help` 的完整输出，
//! 防止意外修改用户可见的命令行界面。版本号会被过滤。

use std::process::Command;

fn cdt_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cdt"));
    cmd.env("RUST_LOG", "off");
    cmd
}

fn help_output(args: &[&str]) -> String {
    let output = cdt_bin().args(args).output().unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn insta_settings() -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    settings.add_filter(r"\d+\.\d+\.\d+", "[VERSION]");
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
