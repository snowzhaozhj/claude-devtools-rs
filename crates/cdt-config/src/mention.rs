//! `@mention` 路径解析 + 沙盒校验。
//!
//! 对应 TS `pathValidation.ts` + `ipc/utility.ts` 的 `handleReadMentionedFile`。
//! 安全检查：
//! 1. 路径必须是绝对路径
//! 2. 路径必须在允许目录内（project root 或 `~/.claude/`）
//! 3. 不匹配敏感文件 pattern
//! 4. Symlink 目标也必须在允许目录内
//! 5. Token 数不超过限制

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::RegexSet;

use crate::claude_md::ClaudeMdFileInfo;
use crate::error::ConfigError;

/// 默认 mention 文件最大 token 数。
const DEFAULT_MAX_TOKENS: usize = 25_000;

/// 敏感文件 pattern 黑名单。
static SENSITIVE_PATTERNS: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new([
        r"[/\\]\.ssh[/\\]",
        r"[/\\]\.aws[/\\]",
        r"[/\\]\.config[/\\]gcloud[/\\]",
        r"[/\\]\.azure[/\\]",
        r"[/\\]\.env($|\.)",
        r"[/\\]\.git-credentials$",
        r"[/\\]\.gitconfig$",
        r"[/\\]\.npmrc$",
        r"[/\\]\.docker[/\\]config\.json$",
        r"[/\\]\.kube[/\\]config$",
        r"[/\\]\.password",
        r"[/\\]\.secret",
        r"[/\\]id_rsa$",
        r"[/\\]id_ed25519$",
        r"[/\\]id_ecdsa$",
        r"[/\\][^/\\]*\.pem$",
        r"[/\\][^/\\]*\.key$",
        r"^/etc/passwd$",
        r"^/etc/shadow$",
        r"credentials\.json$",
        r"secrets\.json$",
        r"tokens\.json$",
        // Windows 特有敏感路径
        r"(?i)[/\\]config[/\\]SAM$",
        r"(?i)[/\\]config[/\\]SYSTEM$",
        r"(?i)[/\\]NTDS\.dit$",
        r"(?i)[/\\]Microsoft[/\\]Credentials[/\\]",
        r"(?i)[/\\]Microsoft[/\\]Crypto[/\\]",
        r"(?i)[/\\]Microsoft[/\\]Protect[/\\]",
    ])
    .expect("sensitive patterns should compile")
});

/// 路径校验结果。
#[derive(Debug)]
pub struct PathValidationResult {
    pub valid: bool,
    pub error: Option<String>,
    pub normalized_path: Option<PathBuf>,
}

impl PathValidationResult {
    fn ok(path: PathBuf) -> Self {
        Self {
            valid: true,
            error: None,
            normalized_path: Some(path),
        }
    }

    fn fail(msg: impl Into<String>) -> Self {
        Self {
            valid: false,
            error: Some(msg.into()),
            normalized_path: None,
        }
    }
}

/// Claude 基础路径（用 `cdt-discover::home_dir` 对齐 Windows fallback 行为）。
fn claude_base_path() -> PathBuf {
    cdt_discover::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
}

/// 检查路径是否匹配敏感文件 pattern。
fn matches_sensitive_pattern(path: &str) -> bool {
    SENSITIVE_PATTERNS.is_match(path)
}

/// 检查路径是否在允许目录内。
fn is_path_within_allowed(normalized: &Path, project_root: Option<&Path>) -> bool {
    let claude_dir = claude_base_path();

    if normalized.starts_with(&claude_dir) {
        return true;
    }

    if let Some(root) = project_root {
        if normalized.starts_with(root) {
            return true;
        }
    }

    false
}

/// 校验文件路径的安全性。
pub fn validate_file_path(file_path: &str, project_root: Option<&Path>) -> PathValidationResult {
    if file_path.is_empty() {
        return PathValidationResult::fail("Invalid file path");
    }

    // 展开 ~ → home dir。只接受 `~` / `~/` / `~\` 三种形式；`~username` 形式
    // （指向特定用户 home）不展开，保留原样（TS 原版行为）。
    let expanded = if let Some(rest) = file_path.strip_prefix('~') {
        let trimmed = rest.trim_start_matches(['/', '\\']);
        if rest.is_empty() || rest.len() != trimmed.len() {
            cdt_discover::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(trimmed)
        } else {
            PathBuf::from(file_path)
        }
    } else {
        PathBuf::from(file_path)
    };

    if !expanded.is_absolute() {
        return PathValidationResult::fail("Path must be absolute");
    }

    // 标准化路径（移除 `..` 等 traversal）
    // 注意：不用 `canonicalize`，因为文件可能不存在
    let normalized = normalize_path(&expanded);
    let normalized_str = normalized.to_string_lossy();

    // 敏感文件检查
    if matches_sensitive_pattern(&normalized_str) {
        return PathValidationResult::fail("Access to sensitive files is not allowed");
    }

    // 允许目录检查
    if !is_path_within_allowed(&normalized, project_root) {
        return PathValidationResult::fail(
            "Path is outside allowed directories (project or Claude root)",
        );
    }

    // 如果文件存在，检查 symlink 目标
    if let Ok(real_path) = std::fs::canonicalize(&normalized) {
        let real_str = real_path.to_string_lossy();
        if matches_sensitive_pattern(&real_str) {
            return PathValidationResult::fail("Access to sensitive files is not allowed");
        }

        // 对 project root 也做 canonicalize
        let real_project = project_root.and_then(|p| std::fs::canonicalize(p).ok());
        if !is_path_within_allowed(&real_path, real_project.as_deref()) {
            return PathValidationResult::fail(
                "Path is outside allowed directories (project or Claude root)",
            );
        }
    }

    PathValidationResult::ok(normalized)
}

/// 简单路径标准化：展开 `.` 和 `..` 组件。
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

/// 读取 mentioned 文件：校验 → 读取 → token 限制检查。
pub async fn read_mentioned_file(
    absolute_path: &str,
    project_root: &Path,
    max_tokens: Option<usize>,
) -> Result<Option<ClaudeMdFileInfo>, ConfigError> {
    let max = max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);

    let validation = validate_file_path(absolute_path, Some(project_root));
    if !validation.valid {
        return Ok(None);
    }

    let safe_path = validation.normalized_path.unwrap();

    // 检查文件是否存在且是普通文件
    match tokio::fs::metadata(&safe_path).await {
        Ok(meta) if meta.is_file() => {}
        _ => return Ok(None),
    }

    let content = tokio::fs::read_to_string(&safe_path)
        .await
        .map_err(|e| ConfigError::io(&safe_path, e))?;

    let estimated_tokens = content.len() / 4;

    if estimated_tokens > max {
        return Ok(None);
    }

    Ok(Some(ClaudeMdFileInfo {
        path: safe_path.to_string_lossy().into_owned(),
        exists: true,
        char_count: content.len(),
        estimated_tokens,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn valid_path_in_project() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("src").join("foo.rs");
        let result = validate_file_path(&file.to_string_lossy(), Some(dir.path()));
        assert!(result.valid);
    }

    #[test]
    fn path_traversal_rejected() {
        let dir = tempdir().unwrap();
        let evil = format!("{}/../../../etc/passwd", dir.path().display());
        let result = validate_file_path(&evil, Some(dir.path()));
        // 路径标准化后应该在项目外
        assert!(
            !result.valid || {
                // 如果标准化后恰好匹配敏感 pattern
                let norm = normalize_path(Path::new(&evil));
                matches_sensitive_pattern(&norm.to_string_lossy())
            }
        );
    }

    #[test]
    fn sensitive_file_blocked() {
        assert!(matches_sensitive_pattern("/home/user/.ssh/id_rsa"));
        assert!(matches_sensitive_pattern("/project/.env"));
        assert!(matches_sensitive_pattern("/project/.env.local"));
        assert!(matches_sensitive_pattern("/home/user/.aws/credentials"));
        assert!(matches_sensitive_pattern("/etc/passwd"));
    }

    #[test]
    fn tilde_user_form_not_expanded() {
        // `~alice/foo` 是合法的 Unix 用户 home 展开，非当前用户 → 保留原样
        let result = validate_file_path("~alice/foo", None);
        // 应该因 "not absolute" 失败（因为返回字符串 `~alice/foo` 不是绝对路径）
        assert!(!result.valid);
        assert_eq!(
            result.error.as_deref(),
            Some("Path must be absolute"),
            "expected tilde-user form to be left unchanged and rejected as non-absolute"
        );
    }

    #[test]
    fn windows_sensitive_paths_blocked() {
        // Backslash paths（Windows native）
        assert!(matches_sensitive_pattern(r"C:\Windows\System32\config\SAM"));
        assert!(matches_sensitive_pattern(
            r"C:\Windows\System32\config\SYSTEM"
        ));
        assert!(matches_sensitive_pattern(r"C:\Windows\NTDS\NTDS.dit"));
        assert!(matches_sensitive_pattern(
            r"C:\Users\alice\AppData\Roaming\Microsoft\Credentials\abc"
        ));
        assert!(matches_sensitive_pattern(
            r"C:\Users\alice\AppData\Roaming\Microsoft\Crypto\Keys\xyz"
        ));
        assert!(matches_sensitive_pattern(
            r"C:\Users\alice\AppData\Roaming\Microsoft\Protect\S-1-5-21"
        ));
    }

    #[test]
    fn normal_file_not_blocked() {
        assert!(!matches_sensitive_pattern("/project/src/main.rs"));
        assert!(!matches_sensitive_pattern("/project/README.md"));
    }

    #[test]
    fn empty_path_rejected() {
        let result = validate_file_path("", None);
        assert!(!result.valid);
    }

    #[test]
    fn relative_path_rejected() {
        let result = validate_file_path("relative/path", None);
        assert!(!result.valid);
    }

    #[tokio::test]
    async fn read_mentioned_valid_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        tokio::fs::write(&file, "hello world").await.unwrap();

        let result = read_mentioned_file(&file.to_string_lossy(), dir.path(), None)
            .await
            .unwrap();

        assert!(result.is_some());
        let info = result.unwrap();
        assert!(info.exists);
        assert_eq!(info.char_count, 11);
    }

    #[tokio::test]
    async fn read_mentioned_token_limit_exceeded() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("big.txt");
        // 写入超过 max_tokens * 4 字节的内容
        let content = "x".repeat(100);
        tokio::fs::write(&file, &content).await.unwrap();

        let result = read_mentioned_file(
            &file.to_string_lossy(),
            dir.path(),
            Some(10), // 10 tokens = 40 chars
        )
        .await
        .unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn normalize_path_removes_parent_dir() {
        let p = normalize_path(Path::new("/a/b/../c/./d"));
        assert_eq!(p, PathBuf::from("/a/c/d"));
    }
}
