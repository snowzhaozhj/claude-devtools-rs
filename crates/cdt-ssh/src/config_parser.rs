//! SSH config 解析器。
//!
//! 解析 `~/.ssh/config` 的 `Host` 块，提取 `HostName`、`User`、`Port`、
//! `IdentityFile` 字段。简化版本：不支持 `Include`、`Match`、`ProxyJump`。

use std::path::Path;

/// 单个 SSH Host 配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshHostConfig {
    /// Host alias（如 `myserver`）。
    pub alias: String,
    /// 实际主机名（`HostName` 字段，默认与 alias 相同）。
    pub hostname: String,
    /// 用户名（`User` 字段）。
    pub user: Option<String>,
    /// 端口（`Port` 字段，默认 22）。
    pub port: u16,
    /// 身份密钥路径列表（`IdentityFile` 字段）。
    pub identity_files: Vec<String>,
}

impl SshHostConfig {
    fn new(alias: &str) -> Self {
        Self {
            alias: alias.to_owned(),
            hostname: alias.to_owned(),
            user: None,
            port: 22,
            identity_files: Vec::new(),
        }
    }
}

/// 从 SSH config 文件内容解析 Host 配置。
pub fn parse_ssh_config(content: &str) -> Vec<SshHostConfig> {
    let mut configs = Vec::new();
    let mut current: Option<SshHostConfig> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // 跳过注释和空行
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // 分割关键字和值
        let (keyword, value) =
            if let Some(pos) = trimmed.find(|c: char| c.is_whitespace() || c == '=') {
                let k = &trimmed[..pos];
                let v = trimmed[pos..].trim_start_matches(|c: char| c.is_whitespace() || c == '=');
                (k, v)
            } else {
                continue;
            };

        match keyword.to_lowercase().as_str() {
            "host" => {
                // 保存前一个 Host 块
                if let Some(cfg) = current.take() {
                    configs.push(cfg);
                }
                current = Some(SshHostConfig::new(value));
            }
            "hostname" => {
                if let Some(ref mut cfg) = current {
                    value.clone_into(&mut cfg.hostname);
                }
            }
            "user" => {
                if let Some(ref mut cfg) = current {
                    cfg.user = Some(value.to_owned());
                }
            }
            "port" => {
                if let Some(ref mut cfg) = current {
                    if let Ok(p) = value.parse::<u16>() {
                        cfg.port = p;
                    }
                }
            }
            "identityfile" => {
                if let Some(ref mut cfg) = current {
                    cfg.identity_files.push(expand_tilde(value));
                }
            }
            _ => {}
        }
    }

    // 保存最后一个 Host 块
    if let Some(cfg) = current {
        configs.push(cfg);
    }

    configs
}

/// 从文件路径解析 SSH config。
pub async fn parse_ssh_config_file(path: &Path) -> Vec<SshHostConfig> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => parse_ssh_config(&content),
        Err(e) => {
            tracing::debug!(path = %path.display(), error = %e, "Failed to read SSH config");
            Vec::new()
        }
    }
}

/// 默认 SSH config 路径。
pub fn default_ssh_config_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".ssh")
        .join("config")
}

/// 根据 alias 查找 Host 配置。
pub fn resolve_host(configs: &[SshHostConfig], alias: &str) -> Option<SshHostConfig> {
    configs.iter().find(|c| c.alias == alias).cloned()
}

/// 列出所有非通配符 Host alias。
pub fn list_hosts(configs: &[SshHostConfig]) -> Vec<String> {
    configs
        .iter()
        .filter(|c| !c.alias.contains('*') && !c.alias.contains('?'))
        .map(|c| c.alias.clone())
        .collect()
}

/// 展开 `~` 为 home 目录；同时接受 `~/` 与 `~\`（Windows SSH config 常用）。
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        let trimmed = rest.trim_start_matches(['/', '\\']);
        if rest.is_empty() || rest.len() != trimmed.len() {
            if let Some(home) = dirs::home_dir() {
                return home.join(trimmed).to_string_lossy().into_owned();
            }
        }
    }
    path.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CONFIG: &str = r"
Host myserver
    HostName 192.168.1.100
    User admin
    Port 2222
    IdentityFile ~/.ssh/id_rsa

Host dev
    HostName dev.example.com
    User devuser

Host *
    ServerAliveInterval 60
";

    #[test]
    fn parse_multiple_hosts() {
        let configs = parse_ssh_config(SAMPLE_CONFIG);
        assert_eq!(configs.len(), 3); // myserver, dev, *
    }

    #[test]
    fn parse_host_fields() {
        let configs = parse_ssh_config(SAMPLE_CONFIG);
        let server = &configs[0];
        assert_eq!(server.alias, "myserver");
        assert_eq!(server.hostname, "192.168.1.100");
        assert_eq!(server.user, Some("admin".into()));
        assert_eq!(server.port, 2222);
        assert_eq!(server.identity_files.len(), 1);
    }

    #[test]
    fn default_port() {
        let configs = parse_ssh_config(SAMPLE_CONFIG);
        let dev = &configs[1];
        assert_eq!(dev.port, 22);
    }

    #[test]
    fn list_hosts_excludes_wildcard() {
        let configs = parse_ssh_config(SAMPLE_CONFIG);
        let hosts = list_hosts(&configs);
        assert_eq!(hosts, vec!["myserver", "dev"]);
    }

    #[test]
    fn resolve_host_found() {
        let configs = parse_ssh_config(SAMPLE_CONFIG);
        let result = resolve_host(&configs, "dev");
        assert!(result.is_some());
        assert_eq!(result.unwrap().hostname, "dev.example.com");
    }

    #[test]
    fn resolve_host_not_found() {
        let configs = parse_ssh_config(SAMPLE_CONFIG);
        assert!(resolve_host(&configs, "unknown").is_none());
    }

    #[test]
    fn parse_empty_config() {
        let configs = parse_ssh_config("");
        assert!(configs.is_empty());
    }

    #[test]
    fn expand_tilde_supports_forward_and_backslash() {
        let home = dirs::home_dir().expect("home dir resolvable");
        let forward = expand_tilde("~/foo/bar");
        let back = expand_tilde(r"~\foo\bar");
        // 验证两种前缀都能展开到 home；不断言完整路径等值（rest 段分隔符会被
        // `Path::join` 规范化到当前 OS 风格）。
        assert!(forward.starts_with(&home.to_string_lossy().into_owned()));
        assert!(back.starts_with(&home.to_string_lossy().into_owned()));
        assert!(forward.contains("foo"));
        assert!(back.contains("foo"));
    }

    #[test]
    fn expand_tilde_without_separator_keeps_original() {
        // 仅 `~username` 形式不属于 home 展开，按 TS 原版保持原样。
        let out = expand_tilde("~alice/foo");
        assert_eq!(out, "~alice/foo");
    }

    #[test]
    fn parse_comments_only() {
        let configs = parse_ssh_config("# comment\n# another");
        assert!(configs.is_empty());
    }
}
