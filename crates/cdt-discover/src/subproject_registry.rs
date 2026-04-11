//! 同一编码目录下存在多个 `cwd` 时的拆分注册表。
//!
//! composite ID 形式冻结为 `{baseDir}::{hash8}`，其中 `hash8` 是 `cwd`
//! 的 SHA-256 十六进制摘要的前 8 个字符。
//!
//! Spec：`openspec/specs/project-discovery/spec.md` 的
//! `Represent split subprojects with a stable composite identifier` Requirement。

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::path_decoder::COMPOSITE_SEPARATOR;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprojectEntry {
    pub base_dir: String,
    pub cwd: PathBuf,
    pub session_ids: BTreeSet<String>,
}

/// 每个 `ProjectScanner` 实例持有一个注册表，不做 global，便于测试隔离
/// 与多 scanner 并存。
#[derive(Debug, Default)]
pub struct SubprojectRegistry {
    entries: HashMap<String, SubprojectEntry>,
}

impl SubprojectRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个 subproject 并返回 composite ID。
    ///
    /// 相同 `(base_dir, cwd)` 会产生同一 ID；调用两次会覆盖旧的 session 集合。
    pub fn register(
        &mut self,
        base_dir: &str,
        cwd: &Path,
        session_ids: impl IntoIterator<Item = String>,
    ) -> String {
        let composite_id = Self::compose_id(base_dir, cwd);
        let session_set: BTreeSet<String> = session_ids.into_iter().collect();
        self.entries.insert(
            composite_id.clone(),
            SubprojectEntry {
                base_dir: base_dir.to_string(),
                cwd: cwd.to_path_buf(),
                session_ids: session_set,
            },
        );
        composite_id
    }

    #[must_use]
    pub fn compose_id(base_dir: &str, cwd: &Path) -> String {
        let mut hasher = Sha256::new();
        hasher.update(cwd.to_string_lossy().as_bytes());
        let digest = hasher.finalize();
        let hex = digest
            .iter()
            .take(4)
            .fold(String::with_capacity(8), |mut acc, b| {
                use std::fmt::Write as _;
                let _ = write!(acc, "{b:02x}");
                acc
            });
        format!("{base_dir}{COMPOSITE_SEPARATOR}{hex}")
    }

    #[must_use]
    pub fn is_composite(project_id: &str) -> bool {
        project_id.contains(COMPOSITE_SEPARATOR)
    }

    #[must_use]
    pub fn get_session_filter(&self, project_id: &str) -> Option<&BTreeSet<String>> {
        self.entries.get(project_id).map(|e| &e.session_ids)
    }

    #[must_use]
    pub fn get_cwd(&self, project_id: &str) -> Option<&Path> {
        self.entries.get(project_id).map(|e| e.cwd.as_path())
    }

    #[must_use]
    pub fn get_entry(&self, project_id: &str) -> Option<&SubprojectEntry> {
        self.entries.get(project_id)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_is_deterministic_for_same_cwd() {
        let mut reg = SubprojectRegistry::new();
        let a = reg.register("-Users-foo", Path::new("/Users/foo/a"), ["s1".into()]);
        let b = reg.register("-Users-foo", Path::new("/Users/foo/a"), ["s1".into()]);
        assert_eq!(a, b);
    }

    #[test]
    fn different_cwds_produce_different_ids() {
        let mut reg = SubprojectRegistry::new();
        let a = reg.register("-Users-foo", Path::new("/Users/foo/a"), []);
        let b = reg.register("-Users-foo", Path::new("/Users/foo/b"), []);
        assert_ne!(a, b);
    }

    #[test]
    fn composite_id_shape_is_basedir_double_colon_hex8() {
        let id = SubprojectRegistry::compose_id("-Users-foo", Path::new("/Users/foo/a"));
        let (base, hash) = id.split_once("::").expect("composite form");
        assert_eq!(base, "-Users-foo");
        assert_eq!(hash.len(), 8);
        assert!(
            hash.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn session_filter_returns_none_for_plain_id() {
        let reg = SubprojectRegistry::new();
        assert!(reg.get_session_filter("-Users-foo").is_none());
    }

    #[test]
    fn session_filter_round_trip() {
        let mut reg = SubprojectRegistry::new();
        let id = reg.register(
            "-Users-foo",
            Path::new("/Users/foo/a"),
            ["s1".to_string(), "s2".to_string()],
        );
        let filter = reg.get_session_filter(&id).unwrap();
        assert_eq!(filter.len(), 2);
        assert!(filter.contains("s1"));
        assert!(filter.contains("s2"));
    }

    #[test]
    fn is_composite_detects_separator() {
        assert!(SubprojectRegistry::is_composite("-foo::abcd1234"));
        assert!(!SubprojectRegistry::is_composite("-foo"));
    }
}
