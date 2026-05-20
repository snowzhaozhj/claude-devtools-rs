//! 项目 ID → 规范 `cwd` 的解析器。
//!
//! 解析顺序（与 TS `ProjectPathResolver` 对齐，已移除 composite registry 短路）：
//! 1. 内存 cache
//! 2. 外部传入的 `hint`（必须是绝对路径）
//! 3. 逐个 session 文件的前 N 行，用 `cdt_parse::parse_entry_at` 抽出 `cwd`
//!    —— SSH 模式下最多检查 1 个 session 文件，本地模式遍历全部
//! 4. `decode_path` best-effort fallback
//!
//! Spec：`openspec/specs/project-discovery/spec.md` 的
//! `Decode encoded project paths` / `Compare paths case-insensitively on Windows`
//! Requirement。
//!
//! Change `merge-composite-projects` 已移除 composite ID 拆分；本 resolver 不再
//! 接收 `SubprojectRegistry`，直接从 cache / hint / session jsonl / decode 链路
//! 解析。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use cdt_parse::parse_entry_at;

use crate::error::DiscoverError;
use crate::fs_provider::{FileSystemProvider, FsKind};
use crate::path_compare::normalize_path_string_for_compare;
use crate::path_decoder::{decode_path, extract_base_dir, looks_like_absolute_path};

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
    /// * `hint` 是可选的 cwd 提示（例如 IPC 调用方已知的绝对路径）。
    /// * `session_paths` 是可选的"已列好的 session 路径列表"，用于避免
    ///   resolver 再自己遍历目录。
    pub async fn resolve(
        &self,
        project_id: &str,
        hint: Option<&Path>,
        session_paths: Option<&[PathBuf]>,
    ) -> Result<PathBuf, DiscoverError> {
        if let Some(cached) = self.cache_get(project_id) {
            return Ok(cached);
        }

        if let Some(h) = hint {
            // 跨平台绝对路径识别 —— Windows 上 `Path::is_absolute()` 拒绝 POSIX
            // 风格，但 SSH 远端 / JSONL cwd 字段可能携带 POSIX 路径，必须接受
            if looks_like_absolute_path(&h.to_string_lossy()) {
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
                    if looks_like_absolute_path(&cwd) {
                        let owned = PathBuf::from(cwd);
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
            let key = normalize_path_string_for_compare(project_id);
            cache.remove(key.as_ref());
        }
    }

    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    fn cache_get(&self, project_id: &str) -> Option<PathBuf> {
        let key = normalize_path_string_for_compare(project_id);
        self.cache
            .lock()
            .ok()
            .and_then(|map| map.get(key.as_ref()).cloned())
    }

    fn cache_set(&self, project_id: &str, path: &Path) {
        if let Ok(mut map) = self.cache.lock() {
            // Cache key 在 Windows 上规范化 ASCII 小写以容忍 `project_id` 大小写
            // 漂移；非 Windows 平台保持原 `project_id`。Spec：`project-discovery::
            // Compare paths case-insensitively on Windows`。
            let key = normalize_path_string_for_compare(project_id).into_owned();
            map.insert(key, path.to_path_buf());
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
    async fn decode_fallback_when_no_sessions() {
        let dir = tempdir().unwrap();
        let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
        let resolver = ProjectPathResolver::new(fs, dir.path().to_path_buf());

        let resolved = resolver
            .resolve("-Users-alice-code-foo", None, Some(&[]))
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
        let line = r#"{"type":"user","uuid":"u1","cwd":"/Users/alice/my-app","timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"hi"}}"#;
        tokio::fs::write(&session, format!("{line}\n"))
            .await
            .unwrap();

        let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
        let resolver = ProjectPathResolver::new(fs, dir.path().to_path_buf());
        let resolved = resolver
            .resolve("-Users-alice-my-app", None, None)
            .await
            .unwrap();
        assert_eq!(resolved, PathBuf::from("/Users/alice/my-app"));
    }

    #[tokio::test]
    async fn hint_takes_precedence_over_decode() {
        let dir = tempdir().unwrap();
        let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
        let resolver = ProjectPathResolver::new(fs, dir.path().to_path_buf());
        let resolved = resolver
            .resolve(
                "-Users-alice-my-app",
                Some(Path::new("/real/cwd")),
                Some(&[]),
            )
            .await
            .unwrap();
        assert_eq!(resolved, PathBuf::from("/real/cwd"));
    }

    /// Spec：`project-discovery::Compare paths case-insensitively on Windows::
    /// 跨大小写命中同一 ProjectPathResolver 缓存`。
    #[tokio::test]
    async fn cache_handles_case_per_platform() {
        let dir = tempdir().unwrap();
        let fs: Arc<dyn FileSystemProvider> = Arc::new(LocalFileSystemProvider::new());
        let resolver = ProjectPathResolver::new(fs, dir.path().to_path_buf());

        let first = resolver
            .resolve(
                "-C:-Users-Alice-app",
                Some(Path::new(r"C:\Users\Alice\app")),
                Some(&[]),
            )
            .await
            .unwrap();
        assert_eq!(first, PathBuf::from(r"C:\Users\Alice\app"));

        let second = resolver
            .resolve("-C:-users-alice-app", None, Some(&[]))
            .await
            .unwrap();

        #[cfg(target_os = "windows")]
        assert_eq!(
            second, first,
            "Windows: 不同大小写的 project_id 应命中同一 cache 条目"
        );
        #[cfg(not(target_os = "windows"))]
        assert_ne!(
            second, first,
            "非 Windows: 不同大小写的 project_id 应视为不同 key（cache miss）"
        );
    }
}
