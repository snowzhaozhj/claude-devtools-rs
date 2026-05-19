//! `ssh-remote-context` capability 的结构化错误分类。
//!
//! Spec：`openspec/specs/ssh-remote-context/spec.md` `Requirement: Structured SSH error
//! classification`。每个变体序列化为 `{ "code": "ssh_<...>", ... }` 形态，与
//! `cdt-api::ApiError.code` `snake_case` 约定一致；`AuthExhausted` 携带每个候选源的
//! 详细 `AuthAttempt`，便于 UI 渲染逐项诊断。

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// SSH 失败的结构化分类。
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum SshError {
    /// TCP probe 失败（host 不可达 / 拒绝连接）。
    #[error("TCP probe to {host} failed: {reason}")]
    #[serde(rename = "ssh_tcp_failure")]
    Tcp { host: String, reason: String },

    /// 鉴权候选链全部失败。
    #[error("SSH auth exhausted ({} attempts)", attempts.len())]
    #[serde(rename = "ssh_auth_exhausted")]
    AuthExhausted { attempts: Vec<AuthAttempt> },

    /// SFTP subsystem open 失败。
    #[error("SFTP init failed: {reason}")]
    #[serde(rename = "ssh_sftp_init")]
    SftpInit { reason: String },

    /// 远端 `~/.claude/projects` 与多个 fallback 候选都不存在。
    #[error("remote home not found, tried {} paths", tried.len())]
    #[serde(rename = "ssh_remote_home_missing")]
    RemoteHomeMissing { tried: Vec<PathBuf> },

    /// 用户主动取消。
    #[error("SSH operation cancelled")]
    #[serde(rename = "ssh_cancelled")]
    Cancelled,

    /// 按 stage 区分的超时（TCP / Auth / SFTP）。
    #[error("SSH timeout at stage {stage:?}")]
    #[serde(rename = "ssh_timeout")]
    Timeout { stage: TimeoutStage },

    /// SSH config 解析或 `ssh -G` 失败。
    #[error("SSH config error: {reason}")]
    #[serde(rename = "ssh_config")]
    Config { reason: String },
}

/// 超时阶段（与连接 5 阶段对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimeoutStage {
    Tcp,
    Auth,
    Sftp,
    RemoteHome,
}

/// 单次鉴权候选尝试的诊断记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthAttempt {
    pub source: AuthSource,
    pub outcome: AuthOutcome,
    pub elapsed_ms: u64,
}

/// 鉴权候选源（D2 鉴权链 7 项）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum AuthSource {
    /// `ssh -G` 解析得到的 `IdentityAgent` 字段（unix socket 路径）。
    IdentityAgent(PathBuf),
    /// `SSH_AUTH_SOCK` env 指向的 socket。
    EnvAgent,
    /// macOS `launchctl getenv SSH_AUTH_SOCK` 返回的 socket。
    LaunchctlAgent,
    /// 1Password well-known socket（macOS）。
    OnePasswordAgent(PathBuf),
    /// `ssh -G` 解析得到的 `IdentityFile` 候选私钥。
    IdentityFile(PathBuf),
    /// 默认密钥位置 fallback (`id_ed25519` / `id_rsa` / `id_ecdsa`)。
    DefaultKey(PathBuf),
    /// 用户在 UI 选择 `password` auth method 时尝试。
    Password,
}

/// 单次鉴权尝试的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum AuthOutcome {
    Success,
    Failure(String),
    Skipped(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_error_tcp_serializes_with_snake_case_code() {
        let err = SshError::Tcp {
            host: "unreachable.example.com".into(),
            reason: "connection refused".into(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "ssh_tcp_failure");
        assert_eq!(json["host"], "unreachable.example.com");
        assert_eq!(json["reason"], "connection refused");
    }

    #[test]
    fn ssh_error_auth_exhausted_serializes() {
        let err = SshError::AuthExhausted {
            attempts: vec![AuthAttempt {
                source: AuthSource::EnvAgent,
                outcome: AuthOutcome::Failure("socket missing".into()),
                elapsed_ms: 12,
            }],
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "ssh_auth_exhausted");
        assert_eq!(json["attempts"][0]["source"]["type"], "envAgent");
        assert_eq!(json["attempts"][0]["outcome"]["type"], "failure");
        assert_eq!(json["attempts"][0]["outcome"]["data"], "socket missing");
        assert_eq!(json["attempts"][0]["elapsedMs"], 12);
    }

    #[test]
    fn ssh_error_remote_home_missing_serializes() {
        let err = SshError::RemoteHomeMissing {
            tried: vec![PathBuf::from("/home/alice/.claude/projects")],
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "ssh_remote_home_missing");
        assert_eq!(json["tried"][0], "/home/alice/.claude/projects");
    }

    #[test]
    fn ssh_error_timeout_carries_stage_camel_case() {
        let err = SshError::Timeout {
            stage: TimeoutStage::Auth,
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "ssh_timeout");
        assert_eq!(json["stage"], "auth");
    }

    #[test]
    fn ssh_error_cancelled_carries_only_code() {
        let err = SshError::Cancelled;
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "ssh_cancelled");
        // 单变体序列化结果只含 `code` 字段
        assert_eq!(json.as_object().unwrap().len(), 1);
    }

    #[test]
    fn auth_source_identity_agent_carries_path() {
        let src = AuthSource::IdentityAgent(PathBuf::from("/Users/me/agent.sock"));
        let json = serde_json::to_value(&src).unwrap();
        assert_eq!(json["type"], "identityAgent");
        assert_eq!(json["data"], "/Users/me/agent.sock");
    }

    #[test]
    fn auth_source_env_agent_no_data() {
        let src = AuthSource::EnvAgent;
        let json = serde_json::to_value(&src).unwrap();
        assert_eq!(json["type"], "envAgent");
        assert!(json.get("data").is_none());
    }

    #[test]
    fn auth_source_one_password_agent_carries_path() {
        let src = AuthSource::OnePasswordAgent(PathBuf::from(
            "/Users/me/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock",
        ));
        let json = serde_json::to_value(&src).unwrap();
        assert_eq!(json["type"], "onePasswordAgent");
    }

    #[test]
    fn auth_outcome_success_no_data() {
        let json = serde_json::to_value(AuthOutcome::Success).unwrap();
        assert_eq!(json["type"], "success");
        assert!(json.get("data").is_none());
    }

    #[test]
    fn auth_outcome_skipped_carries_reason() {
        let json = serde_json::to_value(AuthOutcome::Skipped(
            "requires passphrase, use ssh-add".into(),
        ))
        .unwrap();
        assert_eq!(json["type"], "skipped");
        assert_eq!(json["data"], "requires passphrase, use ssh-add");
    }

    #[test]
    fn auth_attempt_uses_camel_case_elapsed_ms() {
        let attempt = AuthAttempt {
            source: AuthSource::DefaultKey(PathBuf::from("/Users/me/.ssh/id_ed25519")),
            outcome: AuthOutcome::Success,
            elapsed_ms: 234,
        };
        let json = serde_json::to_value(&attempt).unwrap();
        assert!(json.get("elapsed_ms").is_none());
        assert_eq!(json["elapsedMs"], 234);
        assert_eq!(json["source"]["type"], "defaultKey");
    }
}
