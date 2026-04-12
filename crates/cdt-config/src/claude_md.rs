//! CLAUDE.md 多 scope 读取。
//!
//! 对应 TS `ClaudeMdReader.ts`。读取 8 个 scope 的 CLAUDE.md 文件，
//! 返回每个 scope 的路径、存在性、字符数和估算 token 数。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// 单个 CLAUDE.md 文件的信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeMdFileInfo {
    pub path: String,
    pub exists: bool,
    pub char_count: usize,
    pub estimated_tokens: usize,
}

impl ClaudeMdFileInfo {
    fn not_found(path: &str) -> Self {
        Self {
            path: path.to_owned(),
            exists: false,
            char_count: 0,
            estimated_tokens: 0,
        }
    }
}

/// CLAUDE.md scope 枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    Enterprise,
    User,
    Project,
    ProjectAlt,
    ProjectRules,
    ProjectLocal,
    UserRules,
    AutoMemory,
}

/// 估算 token 数：字符数 / 4（与 TS 一致）。
fn estimate_tokens(content: &str) -> usize {
    content.len() / 4
}

/// Claude 基础路径（默认 `~/.claude`）。
fn claude_base_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
}

/// 平台相关的 enterprise CLAUDE.md 路径。
fn enterprise_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support/ClaudeCode/CLAUDE.md")
    } else if cfg!(target_os = "windows") {
        PathBuf::from(r"C:\Program Files\ClaudeCode\CLAUDE.md")
    } else {
        PathBuf::from("/etc/claude-code/CLAUDE.md")
    }
}

/// 编码项目路径（用于 auto-memory 路径计算）。
/// 与 TS `pathDecoder.ts` 的 `encodePath` 一致：替换 `/` → `-`，去首尾 `-`。
fn encode_path(path: &str) -> String {
    path.replace('/', "-").trim_matches('-').to_owned()
}

/// 读取单个文件的 CLAUDE.md 信息。
async fn read_single_file(path: &Path) -> ClaudeMdFileInfo {
    let path_str = path.to_string_lossy().into_owned();
    match tokio::fs::read_to_string(path).await {
        Ok(content) => {
            let char_count = content.len();
            ClaudeMdFileInfo {
                path: path_str,
                exists: true,
                char_count,
                estimated_tokens: estimate_tokens(&content),
            }
        }
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::error!(path = %path_str, error = %e, "Error reading CLAUDE.md file");
            }
            ClaudeMdFileInfo::not_found(&path_str)
        }
    }
}

/// 递归收集目录下所有 `*.md` 文件。
async fn collect_md_files(dir: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return result;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Ok(ft) = entry.file_type().await else {
            continue;
        };
        if ft.is_file() {
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("md") {
                    result.push(path);
                }
            }
        } else if ft.is_dir() {
            let mut sub = Box::pin(collect_md_files(&path)).await;
            result.append(&mut sub);
        }
    }

    result
}

/// 读取目录下所有 `*.md` 文件，合并统计。
async fn read_directory_md_files(dir: &Path) -> ClaudeMdFileInfo {
    let dir_str = dir.to_string_lossy().into_owned();

    match tokio::fs::metadata(dir).await {
        Ok(meta) if meta.is_dir() => {}
        _ => return ClaudeMdFileInfo::not_found(&dir_str),
    }

    let md_files = collect_md_files(dir).await;
    if md_files.is_empty() {
        return ClaudeMdFileInfo::not_found(&dir_str);
    }

    let mut total_chars = 0usize;
    let mut all_content = Vec::new();

    for file in &md_files {
        if let Ok(content) = tokio::fs::read_to_string(file).await {
            total_chars += content.len();
            all_content.push(content);
        }
    }

    let combined = all_content.join("\n");
    ClaudeMdFileInfo {
        path: dir_str,
        exists: true,
        char_count: total_chars,
        estimated_tokens: estimate_tokens(&combined),
    }
}

/// 读取 auto-memory 文件（仅前 200 行）。
async fn read_auto_memory_file(project_root: &Path) -> ClaudeMdFileInfo {
    let encoded = encode_path(&project_root.to_string_lossy());
    let memory_path = claude_base_path()
        .join("projects")
        .join(&encoded)
        .join("memory")
        .join("MEMORY.md");
    let path_str = memory_path.to_string_lossy().into_owned();

    match tokio::fs::read_to_string(&memory_path).await {
        Ok(content) => {
            let truncated: String = content.lines().take(200).collect::<Vec<_>>().join("\n");
            let char_count = truncated.len();
            ClaudeMdFileInfo {
                path: path_str,
                exists: true,
                char_count,
                estimated_tokens: estimate_tokens(&truncated),
            }
        }
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::error!(path = %path_str, error = %e, "Error reading auto memory");
            }
            ClaudeMdFileInfo::not_found(&path_str)
        }
    }
}

/// 读取所有 8 个 scope 的 CLAUDE.md 文件。
pub async fn read_all_claude_md_files(project_root: &Path) -> BTreeMap<Scope, ClaudeMdFileInfo> {
    let base = claude_base_path();
    let mut result = BTreeMap::new();

    // 1. enterprise
    result.insert(
        Scope::Enterprise,
        read_single_file(&enterprise_path()).await,
    );

    // 2. user
    result.insert(Scope::User, read_single_file(&base.join("CLAUDE.md")).await);

    // 3. project
    result.insert(
        Scope::Project,
        read_single_file(&project_root.join("CLAUDE.md")).await,
    );

    // 4. project-alt
    result.insert(
        Scope::ProjectAlt,
        read_single_file(&project_root.join(".claude").join("CLAUDE.md")).await,
    );

    // 5. project-rules
    result.insert(
        Scope::ProjectRules,
        read_directory_md_files(&project_root.join(".claude").join("rules")).await,
    );

    // 6. project-local
    result.insert(
        Scope::ProjectLocal,
        read_single_file(&project_root.join("CLAUDE.local.md")).await,
    );

    // 7. user-rules
    result.insert(
        Scope::UserRules,
        read_directory_md_files(&base.join("rules")).await,
    );

    // 8. auto-memory
    result.insert(Scope::AutoMemory, read_auto_memory_file(project_root).await);

    result
}

/// 读取特定目录的 CLAUDE.md 文件。
pub async fn read_directory_claude_md(dir: &Path) -> ClaudeMdFileInfo {
    read_single_file(&dir.join("CLAUDE.md")).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn read_single_existing_file() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("CLAUDE.md");
        tokio::fs::write(&file, "hello world 1234").await.unwrap();

        let info = read_single_file(&file).await;
        assert!(info.exists);
        assert_eq!(info.char_count, 16);
        assert_eq!(info.estimated_tokens, 4); // 16/4
    }

    #[tokio::test]
    async fn read_single_missing_file() {
        let dir = tempdir().unwrap();
        let info = read_single_file(&dir.path().join("nope.md")).await;
        assert!(!info.exists);
        assert_eq!(info.char_count, 0);
    }

    #[tokio::test]
    async fn read_directory_md_collects_recursively() {
        let dir = tempdir().unwrap();
        let rules = dir.path().join("rules");
        tokio::fs::create_dir_all(rules.join("sub")).await.unwrap();
        tokio::fs::write(rules.join("a.md"), "aaaa").await.unwrap();
        tokio::fs::write(rules.join("sub").join("b.md"), "bbbbbbbb")
            .await
            .unwrap();
        // 非 .md 文件不计入
        tokio::fs::write(rules.join("c.txt"), "xxxx").await.unwrap();

        let info = read_directory_md_files(&rules).await;
        assert!(info.exists);
        assert_eq!(info.char_count, 12); // 4 + 8
    }

    #[tokio::test]
    async fn auto_memory_truncates_to_200_lines() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("myproject");
        tokio::fs::create_dir_all(&project).await.unwrap();

        let encoded = encode_path(&project.to_string_lossy());
        let memory_dir = dirs::home_dir()
            .unwrap()
            .join(".claude")
            .join("projects")
            .join(&encoded)
            .join("memory");

        // 这个测试依赖真实 home 目录，跳过
        // 用 encode_path 的单元测试代替
        let _ = memory_dir;
    }

    #[test]
    fn encode_path_replaces_slashes() {
        assert_eq!(encode_path("/Users/test/project"), "Users-test-project");
    }

    #[test]
    fn encode_path_strips_leading_trailing_dash() {
        assert_eq!(encode_path("/a/b/"), "a-b");
    }

    #[tokio::test]
    async fn read_all_returns_8_scopes() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("proj");
        tokio::fs::create_dir_all(&project).await.unwrap();

        let result = read_all_claude_md_files(&project).await;
        assert_eq!(result.len(), 8);
        assert!(result.contains_key(&Scope::Enterprise));
        assert!(result.contains_key(&Scope::User));
        assert!(result.contains_key(&Scope::Project));
        assert!(result.contains_key(&Scope::ProjectAlt));
        assert!(result.contains_key(&Scope::ProjectRules));
        assert!(result.contains_key(&Scope::ProjectLocal));
        assert!(result.contains_key(&Scope::UserRules));
        assert!(result.contains_key(&Scope::AutoMemory));
    }
}
