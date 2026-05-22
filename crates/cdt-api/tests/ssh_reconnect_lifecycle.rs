//! 集成测试 `fix-ssh-active-context-dispatch` change spec
//! `ssh-remote-context::Reconnect lifecycle preserves SFTP session integrity`
//! 的 reproducer：模拟 `ssh_connect` → `list_repository_groups` → `ssh_disconnect`
//! → `ssh_connect` 同名重连 → `list_repository_groups`，断言第二次成功且
//! 数据来自 v2 fake provider（不复用 v1 旧 Arc）。

use std::sync::Arc;

use async_trait::async_trait;
use cdt_api::{DataApi, LocalDataApi};
use cdt_config::{ConfigManager, NotificationManager};
use cdt_discover::{EntryKind, FsMetadata, LocalFileSystemProvider, ProjectScanner};
use cdt_ssh::{
    RemoteEntry, SftpClient, SftpClientError, SshConnectionManager, SshFileSystemProvider,
};
use tempfile::TempDir;

#[derive(Default)]
struct FakeRemoteSftp {
    files: std::collections::HashMap<String, Vec<u8>>,
    dirs: std::collections::HashMap<String, Vec<RemoteEntry>>,
}

impl FakeRemoteSftp {
    fn with_session(remote_home: &str, project_id: &str, label: &str, content: String) -> Self {
        let session_id = format!("session-{label}");
        let mut fake = Self::default();
        let project_dir = format!("{remote_home}/{project_id}");
        let file_path = format!("{project_dir}/{session_id}.jsonl");
        fake.dirs.insert(
            remote_home.to_owned(),
            vec![RemoteEntry {
                name: project_id.to_owned(),
                kind: EntryKind::Dir,
                metadata: None,
                mtime_missing: false,
            }],
        );
        fake.dirs.insert(
            project_dir,
            vec![RemoteEntry {
                name: format!("{session_id}.jsonl"),
                kind: EntryKind::File,
                metadata: Some(FsMetadata {
                    size: content.len() as u64,
                    mtime: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000),
                    identity: None,
                }),
                mtime_missing: false,
            }],
        );
        fake.files.insert(file_path, content.into_bytes());
        fake
    }
}

#[async_trait]
impl SftpClient for FakeRemoteSftp {
    async fn metadata(&self, path: &str) -> Result<FsMetadata, SftpClientError> {
        if let Some(bytes) = self.files.get(path) {
            Ok(FsMetadata {
                size: bytes.len() as u64,
                mtime: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000),
                identity: None,
            })
        } else if self.dirs.contains_key(path) {
            Ok(FsMetadata {
                size: 0,
                mtime: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000),
                identity: None,
            })
        } else {
            Err(SftpClientError::NoSuchFile)
        }
    }

    async fn try_exists(&self, path: &str) -> Result<bool, SftpClientError> {
        Ok(self.files.contains_key(path) || self.dirs.contains_key(path))
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>, SftpClientError> {
        self.files
            .get(path)
            .cloned()
            .ok_or(SftpClientError::NoSuchFile)
    }

    async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, SftpClientError> {
        self.dirs
            .get(path)
            .cloned()
            .ok_or(SftpClientError::NoSuchFile)
    }

    async fn read_lines_head(
        &self,
        path: &str,
        max: usize,
    ) -> Result<Vec<String>, SftpClientError> {
        let bytes = self.read(path).await?;
        let content =
            String::from_utf8(bytes).map_err(|e| SftpClientError::Other(e.to_string()))?;
        Ok(content.lines().take(max).map(ToOwned::to_owned).collect())
    }
    async fn write(&self, _path: &str, _data: &[u8]) -> Result<(), SftpClientError> {
        Err(SftpClientError::Other(
            "write not used in reconnect fake".into(),
        ))
    }
    async fn mkdir(&self, _path: &str) -> Result<(), SftpClientError> {
        Err(SftpClientError::Other(
            "mkdir not used in reconnect fake".into(),
        ))
    }
    async fn remove(&self, _path: &str) -> Result<(), SftpClientError> {
        Err(SftpClientError::Other(
            "remove not used in reconnect fake".into(),
        ))
    }
    async fn rename(&self, _src: &str, _dst: &str) -> Result<(), SftpClientError> {
        Err(SftpClientError::Other(
            "rename not used in reconnect fake".into(),
        ))
    }
}

async fn setup_api() -> (Arc<LocalDataApi>, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let projects_base = tmp.path().join("projects");
    std::fs::create_dir_all(&projects_base).unwrap();
    let scanner = ProjectScanner::new(
        Arc::new(LocalFileSystemProvider::new()),
        projects_base.clone(),
    );
    let mut config_mgr = ConfigManager::new(Some(tmp.path().join("config.json")));
    config_mgr.load().await.expect("config load");
    let notif_mgr = NotificationManager::new(None);
    let ssh_mgr = SshConnectionManager::new();
    let api = LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr);
    (Arc::new(api), tmp)
}

fn jsonl_line(session_id: &str, cwd: &str, text: &str) -> String {
    format!(
        r#"{{"type":"user","uuid":"{session_id}","parentUuid":null,"timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"userType":"external","cwd":"{cwd}","sessionId":"{session_id}","version":"1","message":{{"role":"user","content":"{text}"}}}}"#,
    )
}

/// Reproducer for spec `ssh-remote-context::Reconnect lifecycle preserves
/// SFTP session integrity`：同 host 重连后 `list_repository_groups` 不复用旧
/// closed session，数据来自 v2 provider。
#[tokio::test]
async fn same_host_reconnect_does_not_leak_closed_session() {
    let (api, _tmp) = setup_api().await;
    let remote_home = "/remote/home/.claude/projects";
    let project_id = "-remote-project";
    let context_id = "host-a";

    // v1：第一次连接 - cwd 是 `/srv/v1`
    let line_v1 = jsonl_line("session-v1", "/srv/v1", "from v1 connection");
    let fake_v1 =
        FakeRemoteSftp::with_session(remote_home, project_id, "v1", format!("{line_v1}\n"));
    let provider_v1 = SshFileSystemProvider::with_client(
        context_id,
        Arc::new(fake_v1),
        std::path::PathBuf::from(remote_home),
    );
    api.insert_test_ssh_context(
        context_id,
        "remote-host",
        22,
        Some("alice".into()),
        std::path::PathBuf::from(remote_home),
        provider_v1,
    )
    .await;

    let groups_v1 = api.list_repository_groups().await.unwrap();
    assert!(
        !groups_v1.is_empty(),
        "第一次连接 list_repository_groups SHALL 返回 v1 远端项目"
    );
    let v1_cwd_match = groups_v1.iter().any(|g| {
        g.worktrees
            .iter()
            .any(|w| w.path.to_string_lossy() == "/srv/v1")
    });
    assert!(
        v1_cwd_match,
        "v1 结果 SHALL 含 cwd=/srv/v1, actual: {groups_v1:?}"
    );

    // disconnect host-a
    api.ssh_disconnect(context_id).await.unwrap();

    // v2：重连同名 context，cwd 换成 `/srv/v2` 验证不复用旧 fake
    let line_v2 = jsonl_line("session-v2", "/srv/v2", "from v2 connection");
    let fake_v2 =
        FakeRemoteSftp::with_session(remote_home, project_id, "v2", format!("{line_v2}\n"));
    let provider_v2 = SshFileSystemProvider::with_client(
        context_id,
        Arc::new(fake_v2),
        std::path::PathBuf::from(remote_home),
    );
    api.insert_test_ssh_context(
        context_id,
        "remote-host",
        22,
        Some("alice".into()),
        std::path::PathBuf::from(remote_home),
        provider_v2,
    )
    .await;

    // 关键断言：第二次 list_repository_groups 必须返回 v2 数据
    let groups_v2 = api.list_repository_groups().await.unwrap();
    assert!(
        !groups_v2.is_empty(),
        "重连后 list_repository_groups SHALL 返回 v2 远端项目，而不是失败 / 空 / v1 缓存"
    );
    let v2_cwd_match = groups_v2.iter().any(|g| {
        g.worktrees
            .iter()
            .any(|w| w.path.to_string_lossy() == "/srv/v2")
    });
    assert!(
        v2_cwd_match,
        "重连后 SHALL 拿到 v2 fixture 的 cwd=/srv/v2（证明 Arc 已替换不复用 v1），actual: {groups_v2:?}"
    );
    let v1_cwd_leaked = groups_v2.iter().any(|g| {
        g.worktrees
            .iter()
            .any(|w| w.path.to_string_lossy() == "/srv/v1")
    });
    assert!(
        !v1_cwd_leaked,
        "重连后 SHALL NOT 看到 v1 的 cwd=/srv/v1，否则说明旧 provider 泄漏"
    );
}
