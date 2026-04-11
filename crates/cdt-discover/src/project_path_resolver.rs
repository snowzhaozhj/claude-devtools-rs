//! 项目 ID → 规范 `cwd` 的解析器。
//!
//! 解析顺序（与 TS `ProjectPathResolver` 对齐）：
//! 1. composite subproject registry 的 `cwd`
//! 2. 内存 cache
//! 3. 外部传入的 `hint`（必须是绝对路径）
//! 4. 逐个 session 文件的前 N 行，用 `cdt_parse::parse_entry_at` 抽出 `cwd`
//!    —— SSH 模式下最多检查 1 个 session 文件，本地模式遍历全部
//! 5. `decode_path` best-effort fallback
//!
//! Spec：`openspec/specs/project-discovery/spec.md` 的
//! `Decode encoded project paths` / `Resolve subprojects and pinned sessions`
//! Requirement。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cdt_parse::parse_entry_at;

use crate::error::DiscoverError;
use crate::fs_provider::{FileSystemProvider, FsKind};
use crate::path_decoder::{decode_path, extract_base_dir};
use crate::subproject_registry::SubprojectRegistry;

/// 扫描 session 头部时最多读取的行数。对齐 TS 里的默认 `maxLines`。
const SESSION_HEAD_LINES: usize = 20;

/// 所有方法都 `&self`；cache 走内部 `Mutex`，便于在 scanner 里共享。
pub struct ProjectPathResolver {
    fs: std::sync::Arc<dyn FileSystemProvider>,
    projects_dir: PathBuf,
    cache: Mutex<HashMap<String, PathBuf>>,
}

impl ProjectPathResolver {
    pub fn new(fs: std::sync::Arc<dyn FileSystemProvider>, projects_dir: PathBuf) -> Self {
        Self {
            fs,
            projects_dir,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 解析一个 project ID 到规范 cwd。
    ///
    /// * `registry` 提供 composite ID 的 short-circuit 源。
    /// * `hint` 是可选的 cwd 提示（例如 IPC 调用方已知的绝对路径）。
    /// * `session_paths` 是可选的"已列好的 session 路径列表"，用于避免
    ///   resolver 再自己遍历目录。
    pub async fn resolve(
        &self,
        project_id: &str,
        registry: &SubprojectRegistry,
        hint: Option<&Path>,
        session_paths: Option<&[PathBuf]>,
    ) -> Result<PathBuf, DiscoverError> {
        if let Some(cwd) = registry.get_cwd(project_id) {
            let owned = cwd.to_path_buf();
            self.cache_set(project_id, &owned);
            return Ok(owned);
        }

        if let Some(cached) = self.cache_get(project_id) {
            return Ok(cached);
        }

        if let Some(h) = hint {
            if h.is_absolute() {
                let owned = h.to_path_buf();
                self.cache_set(project_id, &owned);
                return Ok(owned);
            }
        }

        let owned_session_paths: Vec<PathBuf>;
        let sessions: &[PathBuf] = if let Some(paths) = session_paths {
            paths
        } else {
            owned_session_paths = self.list_session_paths(project_id).await?;
            &owned_session_paths
        };

        let max_inspect = if self.fs.kind() == FsKind::Ssh {
            1
        } else {
            sessions.len()
        };

        for session_path in sessions.iter().take(max_inspect) {
            match self.extract_cwd_from_session(session_path).await {
                Ok(Some(cwd)) => {
                    let owned = PathBuf::from(cwd);
                    if owned.is_absolute() {
                        self.cache_set(project_id, &owned);
                        return Ok(owned);
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::debug!(path = %session_path.display(), error = ?err, "cwd extract failed");
                }
            }
        }

        let decoded = decode_path(extract_base_dir(project_id));
        self.cache_set(project_id, &decoded);
        Ok(decoded)
    }

    pub fn invalidate(&self, project_id: &str) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.remove(project_id);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    fn cache_get(&self, project_id: &str) -> Option<PathBuf> {
        self.cache
            .lock()
            .ok()
            .and_then(|map| map.get(project_id).cloned())
    }

    fn cache_set(&self, project_id: &str, path: &Path) {
        if let Ok(mut map) = self.cache.lock() {
            map.insert(project_id.to_string(), path.to_path_buf());
        }
    }

    async fn list_session_paths(&self, project_id: &str) -> Result<Vec<PathBuf>, DiscoverError> {
        let base_dir = extract_base_dir(project_id);
        let dir = self.projects_dir.join(base_dir);
        if !self.fs.exists(&dir).await {
            return Ok(Vec::new());
        }
        let entries = self.fs.read_dir(&dir).await?;
        let mut out: Vec<PathBuf> = entries
            .into_iter()
            .filter(|e| {
                e.kind.is_file()
                    && std::path::Path::new(&e.name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
            })
            .map(|e| dir.join(e.name))
            .collect();
        out.sort();
        Ok(out)
    }

    async fn extract_cwd_from_session(&self, path: &Path) -> Result<Option<String>, DiscoverError> {
        let lines = self.fs.read_lines_head(path, SESSION_HEAD_LINES).await?;
        for (idx, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let parsed = match parse_entry_at(line, idx + 1) {
                Ok(msg) => msg,
                Err(err) => {
                    tracing::debug!(path = %path.display(), line = idx + 1, error = ?err, "skip malformed jsonl line");
                    continue;
                }
            };
            if let Some(msg) = parsed {
                if let Some(cwd) = msg.cwd {
                    if !cwd.is_empty() {
                        return Ok(Some(cwd));
                    }
                }
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_provider::LocalFileSystemProvider;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn registry_short_circuits_resolution() {
        let dir = tempdir().unwrap();
        let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
        let resolver = ProjectPathResolver::new(fs, dir.path().to_path_buf());

        let mut registry = SubprojectRegistry::new();
        let id = registry.register("-Users-foo", Path::new("/Users/foo/a"), []);
        let resolved = resolver
            .resolve(&id, &registry, None, Some(&[]))
            .await
            .unwrap();
        assert_eq!(resolved, PathBuf::from("/Users/foo/a"));
    }

    #[tokio::test]
    async fn decode_fallback_when_no_sessions() {
        let dir = tempdir().unwrap();
        let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
        let resolver = ProjectPathResolver::new(fs, dir.path().to_path_buf());
        let registry = SubprojectRegistry::new();

        let resolved = resolver
            .resolve("-Users-alice-code-foo", &registry, None, Some(&[]))
            .await
            .unwrap();
        assert_eq!(resolved, PathBuf::from("/Users/alice/code/foo"));
    }

    #[tokio::test]
    async fn cwd_field_overrides_decode() {
        let dir = tempdir().unwrap();
        let project_dir = dir.path().join("-Users-alice-my-app");
        tokio::fs::create_dir_all(&project_dir).await.unwrap();
        let session = project_dir.join("s1.jsonl");
        // minimal valid JSONL user line with a cwd field
        let line = r#"{"type":"user","uuid":"u1","cwd":"/Users/alice/my-app","timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"hi"}}"#;
        tokio::fs::write(&session, format!("{line}\n"))
            .await
            .unwrap();

        let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
        let resolver = ProjectPathResolver::new(fs, dir.path().to_path_buf());
        let registry = SubprojectRegistry::new();
        let resolved = resolver
            .resolve("-Users-alice-my-app", &registry, None, None)
            .await
            .unwrap();
        assert_eq!(resolved, PathBuf::from("/Users/alice/my-app"));
    }

    #[tokio::test]
    async fn hint_takes_precedence_over_decode() {
        let dir = tempdir().unwrap();
        let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
        let resolver = ProjectPathResolver::new(fs, dir.path().to_path_buf());
        let registry = SubprojectRegistry::new();
        let resolved = resolver
            .resolve(
                "-Users-alice-my-app",
                &registry,
                Some(Path::new("/real/cwd")),
                Some(&[]),
            )
            .await
            .unwrap();
        assert_eq!(resolved, PathBuf::from("/real/cwd"));
    }
}
