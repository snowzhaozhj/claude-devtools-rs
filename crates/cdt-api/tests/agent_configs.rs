//! Integration test for `LocalDataApi::read_agent_configs`.
//!
//! Covers the "Read agent configs" scenario from `ipc-data-api` spec: a
//! project-scoped `.claude/agents/*.md` file should surface in the API result.
//!
//! We do NOT override `HOME` here (工作区禁用 `unsafe`，而 `env::set_var` 在
//! 2024 edition 是 unsafe)，因此测试只断言项目级条目存在，不校验全局作用域。

use std::sync::Arc;

use cdt_api::LocalDataApi;
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::SshConnectionManager;
use tempfile::TempDir;

fn write_md(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

#[tokio::test]
async fn read_agent_configs_surfaces_project_scoped_entry() {
    let tmp = TempDir::new().unwrap();
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();

    // 构造 project：cwd 位于 tmp 下；在 cwd/.claude/agents 里写一个 md
    let project_cwd = tmp.path().join("ws/my-proj");
    std::fs::create_dir_all(&project_cwd).unwrap();
    write_md(
        &project_cwd.join(".claude/agents/code-reviewer.md"),
        "---\nname: code-reviewer-test\ncolor: purple\ndescription: PR review\n---\nbody",
    );

    // 写一个 session JSONL，包含 cwd 字段，让 ProjectScanner 能解析出 project。
    //
    // encoded 目录名用固定字面量而非 `encode_path(project_cwd)` —— Windows 上
    // `project_cwd` 含盘符 `C:\...`，`encode_path` 产出 `-C:-Users-...` 含 `:`，
    // NTFS 禁止目录名包含 `:`（Windows error 267 NotADirectory）。scanner 的
    // encoded-name → cwd 解析依赖 JSONL 的 cwd 字段，对 encoded 名本身只要求
    // 通过 `is_valid_encoded_path` 过滤；这里用纯字母 `-ws-my-proj` 即可，
    // cwd 真实磁盘路径由 JSONL 字段提供。
    let encoded_dir = projects_base.join("-ws-my-proj");
    std::fs::create_dir_all(&encoded_dir).unwrap();
    let jsonl = encoded_dir.join("sess-1.jsonl");
    let cwd_str = project_cwd.to_str().unwrap().replace('\\', "\\\\");
    write_md(
        &jsonl,
        &format!(
            "{{\"type\":\"user\",\"cwd\":\"{cwd_str}\",\"uuid\":\"u1\",\"timestamp\":\"2026-04-17T00:00:00Z\",\"message\":{{\"role\":\"user\",\"content\":\"hi\"}}}}\n"
        ),
    );

    let fs = Arc::new(LocalFileSystemProvider::new());
    let scanner = ProjectScanner::new(fs, projects_base);
    let config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    let notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
    let ssh_mgr = SshConnectionManager::new();
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);

    let configs = api.read_agent_configs().await.expect("read ok");

    let found = configs
        .iter()
        .find(|c| c.name == "code-reviewer-test")
        .unwrap_or_else(|| {
            panic!("should include project-scoped code-reviewer-test entry; got: {configs:?}")
        });
    assert_eq!(found.color.as_deref(), Some("purple"));
    assert_eq!(found.description.as_deref(), Some("PR review"));
}
