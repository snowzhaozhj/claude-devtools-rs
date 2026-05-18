//! SSH 鉴权候选链构建与尝试。
//!
//! Spec：`openspec/specs/ssh-remote-context/spec.md` `Requirement: SSH authentication
//! candidate chain`。本文件目前仅提供候选源构建（D2 7 项有序列表 + 平台分支 + 去重），
//! 真握手（`russh::client::Handle::authenticate_*`）由 Phase B 的 `connection.rs::run_auth_chain`
//! 接入。
//!
//! `AuthSource` / `AuthOutcome` / `AuthAttempt` 类型定义在 `crate::error`，本文件只构建。

use std::path::PathBuf;

use crate::error::AuthSource;
use crate::host_resolver::ResolvedHost;

/// 当前编译目标平台分支（用于跳过 macOS-only 候选）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Linux,
    Windows,
}

impl Platform {
    /// 取当前编译目标平台。
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// 用户选择的鉴权方式（UI 表单字段）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthMethodKind {
    /// 走 D2 鉴权候选链（agent / `IdentityFile` / 默认密钥）。
    SshConfig,
    /// 仅 password。
    Password,
}

/// 1Password well-known socket 候选路径（macOS）。
///
/// 与 OpenSSH 行为对齐——若用户在 ssh config 显式指定了 `IdentityAgent`，候选 1
/// 已经覆盖；候选 4 仅作为兜底。
fn one_password_well_known_paths() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        home.join("Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock"),
        home.join(".1password/agent.sock"),
    ]
}

/// 默认密钥位置 fallback（候选 6）。
fn default_key_paths() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        home.join(".ssh/id_ed25519"),
        home.join(".ssh/id_rsa"),
        home.join(".ssh/id_ecdsa"),
    ]
}

/// 按 D2 顺序构建候选源列表（纯函数：不读 env，便于单测）。
///
/// 顺序：(1) `IdentityAgent`（`host_resolver` 解析） → (2) `env_auth_sock` 非空时 `EnvAgent` →
/// (3) `macOS` `launchctl` → (4) 1Password well-known socket（与候选 1 路径去重） →
/// (5) `IdentityFile`（`host_resolver` 解析） → (6) 默认密钥 fallback →
/// (7) `Password`（仅当 `auth_method == Password`）。
///
/// 跨平台分支：Windows 跳过候选 (3)、(4)；Linux 跳过候选 (3)、(4)（`gnome-keyring` 是 v2 phase）。
#[must_use]
pub fn build_candidates_with_env(
    host: &ResolvedHost,
    platform: Platform,
    auth_method: AuthMethodKind,
    env_auth_sock: Option<&str>,
) -> Vec<AuthSource> {
    let mut out: Vec<AuthSource> = Vec::new();

    // 候选 1：IdentityAgent 字段（来自 ssh -G 解析）
    if let Some(path) = host.identity_agent.as_ref() {
        out.push(AuthSource::IdentityAgent(path.clone()));
    }

    // 候选 2：SSH_AUTH_SOCK env（终端启动场景）；env 路径与候选 1 路径相同时去重
    if let Some(sock) = env_auth_sock.filter(|s| !s.is_empty()) {
        let env_path = PathBuf::from(sock);
        if !contains_identity_agent_path(&out, &env_path) {
            out.push(AuthSource::EnvAgent);
        }
    }

    // 候选 3：macOS launchctl getenv SSH_AUTH_SOCK
    if platform == Platform::MacOs {
        out.push(AuthSource::LaunchctlAgent);
    }

    // 候选 4：1Password well-known socket（macOS only）
    if platform == Platform::MacOs {
        for path in one_password_well_known_paths() {
            if !contains_identity_agent_path(&out, &path) {
                out.push(AuthSource::OnePasswordAgent(path));
            }
        }
    }

    // 候选 5：IdentityFile（host_resolver 解析）
    for f in &host.identity_files {
        out.push(AuthSource::IdentityFile(f.clone()));
    }

    // 候选 6：默认密钥位置 fallback
    for path in default_key_paths() {
        // 与候选 5 去重（用户显式指定的 IdentityFile 不重复尝试）
        if !contains_identity_file_path(&out, &path) {
            out.push(AuthSource::DefaultKey(path));
        }
    }

    // 候选 7：password（仅当用户选择 Password）
    if auth_method == AuthMethodKind::Password {
        out.push(AuthSource::Password);
    }

    out
}

/// 公开 API：从进程 env 读 `SSH_AUTH_SOCK` 并调内层 `build_candidates_with_env`。
#[must_use]
pub fn build_candidates(
    host: &ResolvedHost,
    platform: Platform,
    auth_method: AuthMethodKind,
) -> Vec<AuthSource> {
    let env = std::env::var("SSH_AUTH_SOCK").ok();
    build_candidates_with_env(host, platform, auth_method, env.as_deref())
}

fn contains_identity_agent_path(list: &[AuthSource], target: &PathBuf) -> bool {
    list.iter().any(|s| match s {
        AuthSource::IdentityAgent(p) | AuthSource::OnePasswordAgent(p) => p == target,
        _ => false,
    })
}

fn contains_identity_file_path(list: &[AuthSource], target: &PathBuf) -> bool {
    list.iter().any(|s| match s {
        AuthSource::IdentityFile(p) | AuthSource::DefaultKey(p) => p == target,
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_with(identity_agent: Option<PathBuf>, identity_files: Vec<PathBuf>) -> ResolvedHost {
        ResolvedHost {
            host: "h".into(),
            port: 22,
            user: None,
            identity_agent,
            identity_files,
            degraded: false,
        }
    }

    #[test]
    fn macos_chain_includes_launchctl_and_1password() {
        let chain = build_candidates_with_env(
            &host_with(None, vec![]),
            Platform::MacOs,
            AuthMethodKind::SshConfig,
            None,
        );

        assert!(
            chain
                .iter()
                .any(|s| matches!(s, AuthSource::LaunchctlAgent))
        );
        assert!(
            chain
                .iter()
                .any(|s| matches!(s, AuthSource::OnePasswordAgent(_)))
        );
        // 不应含 Password（auth_method == SshConfig）
        assert!(!chain.iter().any(|s| matches!(s, AuthSource::Password)));
    }

    #[test]
    fn windows_chain_skips_launchctl_and_1password() {
        let chain = build_candidates_with_env(
            &host_with(None, vec![]),
            Platform::Windows,
            AuthMethodKind::SshConfig,
            None,
        );

        assert!(
            !chain
                .iter()
                .any(|s| matches!(s, AuthSource::LaunchctlAgent))
        );
        assert!(
            !chain
                .iter()
                .any(|s| matches!(s, AuthSource::OnePasswordAgent(_)))
        );
    }

    #[test]
    fn identity_agent_field_takes_precedence_over_env_agent() {
        let agent_path = PathBuf::from(
            "/Users/me/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock",
        );
        let chain = build_candidates_with_env(
            &host_with(Some(agent_path.clone()), vec![]),
            Platform::MacOs,
            AuthMethodKind::SshConfig,
            Some("/tmp/standard-agent.sock"),
        );

        // 候选 1（IdentityAgent）出现在候选 2（EnvAgent）之前
        let pos_agent = chain
            .iter()
            .position(|s| matches!(s, AuthSource::IdentityAgent(_)))
            .expect("IdentityAgent in chain");
        let pos_env = chain
            .iter()
            .position(|s| matches!(s, AuthSource::EnvAgent))
            .expect("EnvAgent in chain");
        assert!(pos_agent < pos_env);
    }

    #[test]
    fn env_agent_dropped_when_path_matches_identity_agent() {
        // SSH_AUTH_SOCK 与候选 1 同路径时不再加 EnvAgent（避免重复尝试同一 socket）
        let agent = PathBuf::from("/tmp/agent.sock");
        let chain = build_candidates_with_env(
            &host_with(Some(agent.clone()), vec![]),
            Platform::Linux,
            AuthMethodKind::SshConfig,
            Some("/tmp/agent.sock"),
        );
        assert!(!chain.iter().any(|s| matches!(s, AuthSource::EnvAgent)));
    }

    #[test]
    fn one_password_path_dedup_when_identity_agent_matches() {
        let one_password = one_password_well_known_paths();
        let agent_path = one_password
            .first()
            .cloned()
            .expect("at least one well-known 1Password path");

        let chain = build_candidates_with_env(
            &host_with(Some(agent_path.clone()), vec![]),
            Platform::MacOs,
            AuthMethodKind::SshConfig,
            None,
        );

        // 候选 4 中相同路径 SHALL 被去重
        let one_password_count = chain
            .iter()
            .filter(|s| matches!(s, AuthSource::OnePasswordAgent(p) if *p == agent_path))
            .count();
        assert_eq!(one_password_count, 0, "1Password path should be deduped");
    }

    #[test]
    fn identity_file_then_default_key_fallback_dedupe() {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let work_key = home.join(".ssh/id_ed25519"); // 故意与默认密钥重复
        let chain = build_candidates_with_env(
            &host_with(None, vec![work_key.clone()]),
            Platform::Linux,
            AuthMethodKind::SshConfig,
            None,
        );

        let identity_files: Vec<_> = chain
            .iter()
            .filter_map(|s| match s {
                AuthSource::IdentityFile(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        let default_keys: Vec<_> = chain
            .iter()
            .filter_map(|s| match s {
                AuthSource::DefaultKey(p) => Some(p.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(identity_files, vec![work_key.clone()]);
        // 默认密钥 fallback 跳过与 IdentityFile 重复的 id_ed25519
        assert!(!default_keys.contains(&work_key));
        assert!(default_keys.iter().any(|p| p.ends_with(".ssh/id_rsa")));
    }

    #[test]
    fn password_method_appends_password_at_end() {
        let chain = build_candidates_with_env(
            &host_with(None, vec![]),
            Platform::Linux,
            AuthMethodKind::Password,
            None,
        );

        let last = chain.last().expect("chain non-empty");
        assert!(matches!(last, AuthSource::Password));
    }

    #[test]
    fn ssh_config_method_omits_password() {
        let chain = build_candidates_with_env(
            &host_with(None, vec![]),
            Platform::Linux,
            AuthMethodKind::SshConfig,
            None,
        );

        assert!(!chain.iter().any(|s| matches!(s, AuthSource::Password)));
    }
}
