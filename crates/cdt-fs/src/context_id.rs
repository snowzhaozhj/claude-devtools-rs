//! `ContextId` + `HostSignature` + `SshConfigDigestInput`：cache key 上下文身份。
//!
//! Spec：`openspec/specs/fs-abstraction/spec.md` §`ContextId` 三元组作为 cache key 前缀。
//! 设计：`openspec/changes/unify-fs-abstraction/design.md` D5 / D5b / D5b-i / D5c。

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::kind::FsKind;

/// fs-related cache key 的上下文前缀——防跨 host / 跨配置 / 跨 backend 串扰。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContextId {
    pub backend_kind: FsKind,
    pub host_signature: Option<HostSignature>,
    pub root_or_home: PathBuf,
}

impl ContextId {
    #[must_use]
    pub fn local(claude_root: PathBuf) -> Self {
        Self {
            backend_kind: FsKind::Local,
            host_signature: None,
            root_or_home: claude_root,
        }
    }

    #[must_use]
    pub fn ssh(host_signature: HostSignature, remote_home: PathBuf) -> Self {
        Self {
            backend_kind: FsKind::Ssh,
            host_signature: Some(host_signature),
            root_or_home: remote_home,
        }
    }
}

/// SSH host 的稳定身份签名——SHA-256 over resolved ssh config 影响连接行为的字段集合。
///
/// `display_label`（人类可读）SHALL NOT 参与 `Hash` / `PartialEq` —— 仅日志 / UI 展示。
#[derive(Debug, Clone)]
pub struct HostSignature {
    pub config_digest: [u8; 32],
    pub display_label: String,
}

impl HostSignature {
    /// 计算 `config_digest` —— SHA-256 over length-prefixed encoding of
    /// `hostname` / `port` / `user` / `identity_files`（字典序排序）/
    /// `proxyjump` / `proxycommand` / `hostkeyalias`。
    ///
    /// **Length-prefix encoding 防歧义**：每个字段编码为 `[u32 BE length][bytes]`，
    /// 杜绝"不同输入产相同 byte stream"的碰撞（codex 二审 M1）。`identity_files`
    /// 列表头部额外编码 `[u32 BE count]`，每个 path 用平台原生 OS bytes
    /// 编码（Unix: `OsStrExt::as_bytes`；Windows: UTF-16 LE）防止 `to_string_lossy`
    /// 的替换字符碰撞。
    ///
    /// 设计 D5b：连接行为无关字段（`loglevel` / `compression` / `connecttimeout`
    /// 等）SHALL NOT 在 `SshConfigDigestInput` 中，自然不参与 hash。
    ///
    /// `display_label = format!("{user}@{hostname}:{port}")` 仅展示，不参与 hash。
    #[must_use]
    pub fn from_ssh_config_fields(input: &SshConfigDigestInput) -> Self {
        let mut hasher = Sha256::new();
        write_field(&mut hasher, input.hostname.as_bytes());
        write_field(&mut hasher, &input.port.to_be_bytes());
        write_field(&mut hasher, input.user.as_bytes());

        // identity_files：先 count，再每个 path 按平台原生 bytes length-prefix
        let mut sorted_ids = input.identity_files.clone();
        sorted_ids.sort();
        let count_u32 = u32::try_from(sorted_ids.len()).unwrap_or(u32::MAX);
        hasher.update(count_u32.to_be_bytes());
        for id_file in &sorted_ids {
            let bytes = path_to_native_bytes(id_file);
            write_field(&mut hasher, &bytes);
        }

        write_field(
            &mut hasher,
            input.proxyjump.as_deref().unwrap_or("").as_bytes(),
        );
        write_field(
            &mut hasher,
            input.proxycommand.as_deref().unwrap_or("").as_bytes(),
        );
        write_field(
            &mut hasher,
            input.hostkeyalias.as_deref().unwrap_or("").as_bytes(),
        );

        let digest: [u8; 32] = hasher.finalize().into();
        let display_label = format!("{}@{}:{}", input.user, input.hostname, input.port);
        Self {
            config_digest: digest,
            display_label,
        }
    }
}

/// 写入 `[u32 BE length][bytes]` length-prefix 编码，防止字段拼接歧义。
fn write_field(hasher: &mut Sha256, bytes: &[u8]) {
    let len = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    hasher.update(len.to_be_bytes());
    hasher.update(bytes);
}

/// 平台原生 path bytes —— Unix `OsStrExt::as_bytes()` 直拿；Windows 走
/// `encode_wide()` UTF-16 LE。避免 `to_string_lossy` 的非法字符替换碰撞。
#[cfg(unix)]
fn path_to_native_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_to_native_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    let mut bytes = Vec::with_capacity(wide.len() * 2);
    for w in wide {
        bytes.extend_from_slice(&w.to_le_bytes());
    }
    bytes
}

#[cfg(not(any(unix, windows)))]
fn path_to_native_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

impl PartialEq for HostSignature {
    fn eq(&self, other: &Self) -> bool {
        self.config_digest == other.config_digest
    }
}

impl Eq for HostSignature {}

impl std::hash::Hash for HostSignature {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.config_digest.hash(state);
    }
}

/// `HostSignature::from_ssh_config_fields` 的最小 input 形状。
///
/// 设计 D5b-i：cdt-fs 不引用 cdt-ssh `ResolvedHost`（避免反向依赖）；调用方
/// （cdt-ssh）通过 `impl From<&ResolvedHost> for SshConfigDigestInput` 转换。
#[derive(Debug, Clone)]
pub struct SshConfigDigestInput {
    pub hostname: String,
    pub port: u16,
    pub user: String,
    /// `from_ssh_config_fields` 内部会字典序排序，调用方不必预排序。
    pub identity_files: Vec<PathBuf>,
    pub proxyjump: Option<String>,
    pub proxycommand: Option<String>,
    pub hostkeyalias: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input() -> SshConfigDigestInput {
        SshConfigDigestInput {
            hostname: "example.com".into(),
            port: 22,
            user: "alice".into(),
            identity_files: vec![PathBuf::from("/home/alice/.ssh/id_ed25519")],
            proxyjump: None,
            proxycommand: None,
            hostkeyalias: None,
        }
    }

    #[test]
    fn different_backend_kind_makes_context_unequal() {
        let local = ContextId::local(PathBuf::from("/home/u/.claude/projects"));
        let ssh = ContextId::ssh(
            HostSignature::from_ssh_config_fields(&sample_input()),
            PathBuf::from("/home/u/.claude/projects"),
        );
        assert_ne!(local, ssh);
    }

    #[test]
    fn same_user_host_port_but_different_proxyjump_digest_differs() {
        let mut a = sample_input();
        let mut b = sample_input();
        a.proxyjump = None;
        b.proxyjump = Some("bastion.example.com".into());
        let sig_a = HostSignature::from_ssh_config_fields(&a);
        let sig_b = HostSignature::from_ssh_config_fields(&b);
        assert_ne!(sig_a.config_digest, sig_b.config_digest);
        assert_ne!(sig_a, sig_b);
    }

    #[test]
    fn same_user_host_port_proxyjump_but_different_identity_file_digest_differs() {
        let mut a = sample_input();
        let mut b = sample_input();
        a.identity_files = vec![PathBuf::from("/home/alice/.ssh/work_key")];
        b.identity_files = vec![PathBuf::from("/home/alice/.ssh/personal_key")];
        let sig_a = HostSignature::from_ssh_config_fields(&a);
        let sig_b = HostSignature::from_ssh_config_fields(&b);
        assert_ne!(sig_a, sig_b);
    }

    #[test]
    fn display_label_does_not_affect_hash_eq() {
        use std::hash::{Hash, Hasher};

        let input = sample_input();
        let sig = HostSignature::from_ssh_config_fields(&input);

        let tampered = HostSignature {
            config_digest: sig.config_digest,
            display_label: "different label".into(),
        };

        assert_eq!(sig, tampered);
        let mut hasher_a = std::collections::hash_map::DefaultHasher::new();
        let mut hasher_b = std::collections::hash_map::DefaultHasher::new();
        sig.hash(&mut hasher_a);
        tampered.hash(&mut hasher_b);
        assert_eq!(hasher_a.finish(), hasher_b.finish());
    }

    #[test]
    fn identity_files_order_does_not_affect_digest() {
        let mut a = sample_input();
        let mut b = sample_input();
        a.identity_files = vec![
            PathBuf::from("/home/alice/.ssh/key1"),
            PathBuf::from("/home/alice/.ssh/key2"),
        ];
        b.identity_files = vec![
            PathBuf::from("/home/alice/.ssh/key2"),
            PathBuf::from("/home/alice/.ssh/key1"),
        ];
        let sig_a = HostSignature::from_ssh_config_fields(&a);
        let sig_b = HostSignature::from_ssh_config_fields(&b);
        assert_eq!(sig_a, sig_b);
    }

    #[test]
    fn degraded_mode_none_proxyjump_still_yields_digest() {
        let input = sample_input();
        let sig = HostSignature::from_ssh_config_fields(&input);
        // 不 panic + display_label 非空 + digest 非零即可
        assert_eq!(sig.display_label, "alice@example.com:22");
        assert_ne!(sig.config_digest, [0u8; 32]);
    }

    #[test]
    fn same_host_signature_same_root_etc_equals() {
        let sig = HostSignature::from_ssh_config_fields(&sample_input());
        let a = ContextId::ssh(sig.clone(), PathBuf::from("/home/u/.claude/projects"));
        let b = ContextId::ssh(sig, PathBuf::from("/home/u/.claude/projects"));
        assert_eq!(a, b);
    }

    #[test]
    fn display_label_format_matches_spec() {
        let input = sample_input();
        let sig = HostSignature::from_ssh_config_fields(&input);
        assert_eq!(sig.display_label, "alice@example.com:22");
    }

    #[test]
    fn clone_preserves_equality() {
        let input = sample_input();
        let sig = HostSignature::from_ssh_config_fields(&input);
        assert_eq!(sig, sig.clone());
    }
}
