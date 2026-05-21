//! `ssh -G <host>` 子进程委托解析 SSH config 高级特性。
//!
//! Spec：`openspec/specs/ssh-remote-context/spec.md` `Requirement: Resolve SSH host
//! alias via ssh -G`。
//!
//! 设计参见 `openspec/changes/port-ssh-remote-browse/design.md` D3：把 `Include` /
//! `Match` / `ProxyJump` / `IdentityAgent` 等高级语法委托给系统 `ssh` 二进制，
//! 自身仅负责解析 `ssh -G` 的 stdout 与降级 `fallback`。

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

use crate::config_parser::{SshHostConfig, parse_ssh_config_file, resolve_host};
use crate::error::SshError;

/// `ssh -G` 解析结果或 fallback。
///
/// `proxyjump` / `proxycommand` / `hostkeyalias` 字段（unify-fs-abstraction
/// change D5b-i）参与 `cdt_fs::HostSignature` 计算——同 `user@host:port` 但
/// 不同 `ProxyJump` / `ProxyCommand` / `HostKeyAlias` 的连接应视为不同 fs
/// 上下文，cache key 不应串扰。退化路径 `config_parser` 拿不到这三字段时填
/// `None`，`HostSignature` 落到 degraded 等价类。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedHost {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    /// `IdentityAgent` 字段——仅在 `ssh -G` 输出非空且非 `none` 时填写。
    pub identity_agent: Option<PathBuf>,
    /// `IdentityFile` 字段（多行允许）。
    pub identity_files: Vec<PathBuf>,
    /// `ProxyJump` 字段——`ssh -G` 输出非空时填写；退化路径填 `None`。
    pub proxyjump: Option<String>,
    /// `ProxyCommand` 字段——`ssh -G` 输出非空时填写；退化路径填 `None`。
    pub proxycommand: Option<String>,
    /// `HostKeyAlias` 字段——`ssh -G` 输出非空时填写；退化路径填 `None`。
    pub hostkeyalias: Option<String>,
    /// 标记是否走了 `fallback`（`config_parser` 而非 `ssh -G`）。
    pub degraded: bool,
}

impl ResolvedHost {
    fn from_basic(cfg: SshHostConfig, degraded: bool) -> Self {
        Self {
            host: cfg.hostname,
            port: cfg.port,
            user: cfg.user,
            identity_agent: None,
            identity_files: cfg.identity_files.into_iter().map(PathBuf::from).collect(),
            proxyjump: None,
            proxycommand: None,
            hostkeyalias: None,
            degraded,
        }
    }
}

/// 转换 `ResolvedHost` → `cdt_fs::SshConfigDigestInput` —— `HostSignature` 计算
/// 的最小入参形状。设计 D5b-i：`cdt-fs` 不反向依赖 `cdt-ssh`，conversion 住在
/// 此处。`identity_files` 透传（`HostSignature::from_ssh_config_fields` 内部排序）；
/// `user` 缺失时落空串（OpenSSH `ssh -G` 默认填当前 login user，理论上不会缺）。
impl From<&ResolvedHost> for cdt_fs::SshConfigDigestInput {
    fn from(host: &ResolvedHost) -> Self {
        Self {
            hostname: host.host.clone(),
            port: host.port,
            user: host.user.clone().unwrap_or_default(),
            identity_files: host.identity_files.clone(),
            proxyjump: host.proxyjump.clone(),
            proxycommand: host.proxycommand.clone(),
            hostkeyalias: host.hostkeyalias.clone(),
        }
    }
}

/// `ssh -G` 子进程超时（与 spec 标注一致）。
pub const SSH_G_TIMEOUT: Duration = Duration::from_secs(5);

/// 通过 `ssh -G <alias>` 子进程解析 host 配置。
///
/// 失败 / 超时 / `ssh` 二进制缺失时降级到 `config_parser` 的基本字段解析。
pub async fn resolve_host_via_ssh_g(alias: &str) -> Result<ResolvedHost, SshError> {
    match run_ssh_g(alias).await {
        Ok(output) => Ok(parse_ssh_g_output(&output)),
        Err(e) => {
            tracing::debug!(alias = %alias, error = %e, "ssh -G fallback to config_parser");
            fallback_via_config_parser(alias).await
        }
    }
}

/// `ssh -G` 失败 / 缺失时的降级路径（`config_parser` 基本字段）。
async fn fallback_via_config_parser(alias: &str) -> Result<ResolvedHost, SshError> {
    let path = crate::config_parser::default_ssh_config_path();
    let configs = parse_ssh_config_file(&path).await;
    if let Some(cfg) = resolve_host(&configs, alias) {
        Ok(ResolvedHost::from_basic(cfg, true))
    } else {
        Ok(ResolvedHost {
            host: alias.to_owned(),
            port: 22,
            user: None,
            identity_agent: None,
            identity_files: vec![],
            proxyjump: None,
            proxycommand: None,
            hostkeyalias: None,
            degraded: true,
        })
    }
}

/// 跑 `ssh -G <alias>`，5s 超时，返回 stdout。
///
/// SHALL 设置 `Stdio::null()` 关闭 stdin（防 hook / 终端控制序列），
/// `Stdio::piped()` 收集 stdout，`Stdio::null()` 丢弃 stderr。
async fn run_ssh_g(alias: &str) -> Result<String, SshError> {
    let mut cmd = Command::new("ssh");
    cmd.arg("-G")
        .arg(alias)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let fut = cmd.output();
    let output = match timeout(SSH_G_TIMEOUT, fut).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            return Err(SshError::Config {
                reason: format!("ssh -G spawn failed: {e}"),
            });
        }
        Err(_) => {
            return Err(SshError::Timeout {
                stage: crate::error::TimeoutStage::Tcp,
            });
        }
    };

    if !output.status.success() {
        return Err(SshError::Config {
            reason: format!("ssh -G exited with status {}", output.status),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// 解析 `ssh -G` stdout。每行 `<keyword> <value>`，关键字大小写不敏感。
#[must_use]
pub fn parse_ssh_g_output(stdout: &str) -> ResolvedHost {
    let mut host = String::new();
    let mut port: u16 = 22;
    let mut user: Option<String> = None;
    let mut identity_agent: Option<PathBuf> = None;
    let mut identity_files: Vec<PathBuf> = Vec::new();
    let mut proxyjump: Option<String> = None;
    let mut proxycommand: Option<String> = None;
    let mut hostkeyalias: Option<String> = None;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (key, value) = match trimmed.split_once(char::is_whitespace) {
            Some((k, v)) => (k.to_lowercase(), v.trim()),
            None => continue,
        };
        match key.as_str() {
            "hostname" => value.clone_into(&mut host),
            "port" => {
                if let Ok(p) = value.parse::<u16>() {
                    port = p;
                }
            }
            "user" => user = Some(value.to_owned()),
            // OpenSSH 输出 `none` 表示显式禁用 — 不当作候选
            "identityagent" if !value.is_empty() && !value.eq_ignore_ascii_case("none") => {
                identity_agent = Some(strip_quotes_into_path(value));
            }
            "identityfile" => {
                identity_files.push(strip_quotes_into_path(value));
            }
            // OpenSSH 输出 `none` 表示显式禁用 ProxyJump / ProxyCommand
            "proxyjump" if !value.is_empty() && !value.eq_ignore_ascii_case("none") => {
                proxyjump = Some(value.to_owned());
            }
            "proxycommand" if !value.is_empty() && !value.eq_ignore_ascii_case("none") => {
                proxycommand = Some(value.to_owned());
            }
            "hostkeyalias" if !value.is_empty() => {
                hostkeyalias = Some(value.to_owned());
            }
            _ => {}
        }
    }

    ResolvedHost {
        host,
        port,
        user,
        identity_agent,
        identity_files,
        proxyjump,
        proxycommand,
        hostkeyalias,
        degraded: false,
    }
}

/// `ssh -G` 输出中含空格的路径会被 `OpenSSH` 加双引号——剥引号转 `PathBuf`。
fn strip_quotes_into_path(s: &str) -> PathBuf {
    let stripped = s
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(s);
    PathBuf::from(stripped)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SSH_G_NORMAL: &str = "\
user alice
hostname server.example.com
port 2222
identityfile ~/.ssh/id_ed25519
identityagent ~/.ssh/agent.sock
";

    const SSH_G_MULTI_KEYS: &str = "\
user bob
hostname dev.internal
port 22
identityfile ~/.ssh/work_key
identityfile ~/.ssh/personal_key
";

    const SSH_G_NONE_AGENT: &str = "\
hostname server
port 22
identityagent none
";

    const SSH_G_QUOTED_PATH: &str = "\
hostname server
port 22
identityagent \"/Users/me/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock\"
";

    const SSH_G_WITH_PROXY_JUMP: &str = "\
user alice
hostname target.internal
port 22
identityfile ~/.ssh/id_ed25519
proxyjump bastion.example.com
hostkeyalias target-canonical
";

    const SSH_G_PROXY_NONE: &str = "\
user alice
hostname server
port 22
proxyjump none
proxycommand none
";

    #[test]
    fn parses_normal_ssh_g_output() {
        let r = parse_ssh_g_output(SSH_G_NORMAL);
        assert_eq!(r.host, "server.example.com");
        assert_eq!(r.port, 2222);
        assert_eq!(r.user, Some("alice".into()));
        assert_eq!(r.identity_agent, Some(PathBuf::from("~/.ssh/agent.sock")));
        assert_eq!(r.identity_files, vec![PathBuf::from("~/.ssh/id_ed25519")]);
        assert!(r.proxyjump.is_none());
        assert!(r.proxycommand.is_none());
        assert!(r.hostkeyalias.is_none());
        assert!(!r.degraded);
    }

    #[test]
    fn parses_proxyjump_and_hostkeyalias() {
        let r = parse_ssh_g_output(SSH_G_WITH_PROXY_JUMP);
        assert_eq!(r.proxyjump.as_deref(), Some("bastion.example.com"));
        assert_eq!(r.hostkeyalias.as_deref(), Some("target-canonical"));
        assert!(r.proxycommand.is_none());
    }

    #[test]
    fn ignores_proxyjump_and_proxycommand_none() {
        let r = parse_ssh_g_output(SSH_G_PROXY_NONE);
        assert!(r.proxyjump.is_none());
        assert!(r.proxycommand.is_none());
    }

    #[test]
    fn fallback_default_resolved_host_has_none_for_new_fields() {
        // 退化路径返默认 ResolvedHost：proxyjump/proxycommand/hostkeyalias 都 None
        let r = ResolvedHost {
            host: "alias".into(),
            port: 22,
            user: None,
            identity_agent: None,
            identity_files: vec![],
            proxyjump: None,
            proxycommand: None,
            hostkeyalias: None,
            degraded: true,
        };
        assert!(r.proxyjump.is_none());
        assert!(r.proxycommand.is_none());
        assert!(r.hostkeyalias.is_none());
    }

    #[test]
    fn from_resolved_host_into_digest_input_propagates_proxy_fields() {
        let r = parse_ssh_g_output(SSH_G_WITH_PROXY_JUMP);
        let input: cdt_fs::SshConfigDigestInput = (&r).into();
        assert_eq!(input.hostname, "target.internal");
        assert_eq!(input.port, 22);
        assert_eq!(input.user, "alice");
        assert_eq!(
            input.identity_files,
            vec![PathBuf::from("~/.ssh/id_ed25519")]
        );
        assert_eq!(input.proxyjump.as_deref(), Some("bastion.example.com"));
        assert_eq!(input.hostkeyalias.as_deref(), Some("target-canonical"));
    }

    #[test]
    fn proxyjump_difference_produces_different_host_signature() {
        // 同 user@host:port 但 ProxyJump 不同的两台 host 必须产生不同 HostSignature
        let with_jump = parse_ssh_g_output(SSH_G_WITH_PROXY_JUMP);
        let mut without_jump = with_jump.clone();
        without_jump.proxyjump = None;

        let sig_with: cdt_fs::SshConfigDigestInput = (&with_jump).into();
        let sig_without: cdt_fs::SshConfigDigestInput = (&without_jump).into();

        let h_with = cdt_fs::HostSignature::from_ssh_config_fields(&sig_with);
        let h_without = cdt_fs::HostSignature::from_ssh_config_fields(&sig_without);
        assert_ne!(h_with, h_without);
    }

    #[test]
    fn degraded_path_conversion_does_not_panic() {
        let r = ResolvedHost {
            host: "alias".into(),
            port: 22,
            user: None,
            identity_agent: None,
            identity_files: vec![],
            proxyjump: None,
            proxycommand: None,
            hostkeyalias: None,
            degraded: true,
        };
        let input: cdt_fs::SshConfigDigestInput = (&r).into();
        // 退化路径 user 缺失 → 空串，HostSignature 仍可计算
        assert_eq!(input.user, "");
        let _ = cdt_fs::HostSignature::from_ssh_config_fields(&input);
    }

    #[test]
    fn parses_multiple_identity_files_in_order() {
        let r = parse_ssh_g_output(SSH_G_MULTI_KEYS);
        assert_eq!(
            r.identity_files,
            vec![
                PathBuf::from("~/.ssh/work_key"),
                PathBuf::from("~/.ssh/personal_key"),
            ]
        );
    }

    #[test]
    fn ignores_identity_agent_none() {
        let r = parse_ssh_g_output(SSH_G_NONE_AGENT);
        assert!(r.identity_agent.is_none());
    }

    #[test]
    fn strips_quotes_from_identity_agent_path() {
        let r = parse_ssh_g_output(SSH_G_QUOTED_PATH);
        assert_eq!(
            r.identity_agent,
            Some(PathBuf::from(
                "/Users/me/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock"
            ))
        );
    }

    #[test]
    fn empty_output_falls_back_to_default_port() {
        let r = parse_ssh_g_output("");
        assert_eq!(r.port, 22);
        assert!(r.host.is_empty());
        assert!(r.identity_files.is_empty());
    }

    #[test]
    fn ignores_unknown_keywords() {
        let r = parse_ssh_g_output("hostname x\nfoo bar\nbaz qux\nport 33\n");
        assert_eq!(r.host, "x");
        assert_eq!(r.port, 33);
    }

    /// 真跑 `ssh -G` 用于本地集成验证（CI 缺 ssh / sandbox 跳过）。
    /// 不在默认 test 集合，仅 `--ignored` 时跑。
    #[tokio::test]
    #[ignore = "requires system ssh binary; run locally with --ignored"]
    async fn live_ssh_g_run_falls_back_gracefully_when_alias_missing() {
        let r = resolve_host_via_ssh_g("definitely-not-a-real-host-9k2js").await;
        // ssh -G 对未配置 alias 仍会返回结果（用 alias 当 hostname），不报错
        assert!(r.is_ok());
        let out = r.unwrap();
        assert!(out.host == "definitely-not-a-real-host-9k2js" || !out.host.is_empty());
    }
}
