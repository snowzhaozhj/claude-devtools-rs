//! CLI help 输出的快照测试。
//!
//! 锁定 `cdt --help` / 子命令 `--help` 的完整输出，防止意外修改用户可见的命令行界面。
//!
//! clap 的 help 折行宽度依赖运行环境（workspace 构建启用了 clap 的 `wrap_help`，
//! 于是探测终端：dev 有 tty → 宽 → 不折行；CI 无 tty → 回退默认 100 列 → 折行），
//! 单 crate 构建甚至不折行。为不让"测试"绑架"生产 help 的自适应宽度"，**生产侧保持
//! 终端自适应不动**，改为在测试侧把环境相关的软折行 normalize 掉——见
//! [`normalize_help_wrapping`]。版本号也被过滤。这样快照与折行宽度无关，跨
//! cargo test / nextest / CI 确定性，未来新增 flag 不再 flaky。

use std::process::Command;

fn cdt_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_cdt"));
    cmd.env("RUST_LOG", "off");
    cmd
}

/// 把 clap help 的软折行规整成与宽度无关的规范形式：每个 option 的描述（无论是否被
/// 折行、也无论是 inline 列对齐格式还是 block 缩进格式）合并回单行。
///
/// 判定"软折行续行"：缩进 ≥ 10 且非空，且不是结构行——`[default: ...]` /
/// `[possible values: ...]` 以 `[` 起首，possible-value 列项以 `- ` 起首，二者保持独立；
/// 也不并入空行（段落分隔）。其余缩进 ≥ 10 的散文行都并入上一行。
fn normalize_help_wrapping(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let leading = line.len() - trimmed.len();
        let is_continuation = leading >= 10
            && !trimmed.is_empty()
            && !trimmed.starts_with('[')
            && !trimmed.starts_with("- ");
        if is_continuation {
            if let Some(prev) = lines.last_mut() {
                if !prev.trim().is_empty() {
                    prev.push(' ');
                    prev.push_str(trimmed);
                    continue;
                }
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
