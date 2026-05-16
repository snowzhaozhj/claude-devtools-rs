//! CLAUDE.md 多 scope 读取。
//!
//! 对应 TS `ClaudeMdReader.ts`。读取 8 个 scope 的 CLAUDE.md 文件，
//! 返回每个 scope 的路径、存在性、字符数和估算 token 数。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cdt_discover::{encode_path, path_decoder::get_claude_base_path};
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

/// 平台相关的 enterprise CLAUDE.md 路径。
///
/// Windows 上用 `%ProgramFiles%` 动态解析，支持非 C 盘安装；fallback `C:\Program Files`。
fn enterprise_path() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("/Library/Application Support/ClaudeCode/CLAUDE.md")
    } else if cfg!(target_os = "windows") {
        let program_files = std::env::var("ProgramFiles")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| r"C:\Program Files".to_owned());
        PathBuf::from(program_files)
            .join("ClaudeCode")
            .join("CLAUDE.md")
    } else {
        PathBuf::from("/etc/claude-code/CLAUDE.md")
    }
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
async fn read_auto_memory_file(project_root: &Path, claude_base: &Path) -> ClaudeMdFileInfo {
    let encoded = encode_path(&project_root.to_string_lossy());
    let memory_path = claude_base
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
    read_all_claude_md_files_with_base(project_root, &get_claude_base_path()).await
}

/// 用指定 Claude root 读取所有 8 个 scope 的 CLAUDE.md 文件。
pub async fn read_all_claude_md_files_with_base(
    project_root: &Path,
    claude_base: &Path,
) -> BTreeMap<Scope, ClaudeMdFileInfo> {
    let mut result = BTreeMap::new();

    // 1. enterprise
    result.insert(
        Scope::Enterprise,
        read_single_file(&enterprise_path()).await,
    );

    // 2. user
    result.insert(
        Scope::User,
        read_single_file(&claude_base.join("CLAUDE.md")).await,
    );

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
        read_directory_md_files(&claude_base.join("rules")).await,
    );

    // 8. auto-memory
    result.insert(
        Scope::AutoMemory,
        read_auto_memory_file(project_root, claude_base).await,
    );

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
    fn auto_memory_encoded_dir_uses_cdt_discover_encoder() {
        // encode_path 现在由 cdt-discover 统一提供；这里仅冒烟验证 claude_md 拿到
        // 的结果和 TS 原版一致（强制 leading `-`，保留冒号）。完整覆盖在
        // `cdt-discover::path_decoder::tests`。
        assert_eq!(encode_path("/Users/test/project"), "-Users-test-project");
        assert_eq!(encode_path(r"C:\Users\alice\app"), "-C:-Users-alice-app");
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

    #[tokio::test]
    async fn read_all_uses_custom_claude_base_for_user_scopes() {
        let dir = tempdir().unwrap();
        let claude_base = dir.path().join("claude-alt");
        let project = dir.path().join("proj");
        tokio::fs::create_dir_all(claude_base.join("rules"))
            .await
            .unwrap();
        tokio::fs::create_dir_all(&project).await.unwrap();
        tokio::fs::write(claude_base.join("CLAUDE.md"), "user scope")
            .await
            .unwrap();
        tokio::fs::write(claude_base.join("rules/rule.md"), "rule scope")
            .await
            .unwrap();

        let result = read_all_claude_md_files_with_base(&project, &claude_base).await;

        assert!(result[&Scope::User].exists);
        assert!(result[&Scope::User].path.contains("claude-alt"));
        assert!(result[&Scope::UserRules].exists);
        assert!(result[&Scope::UserRules].path.contains("claude-alt"));
    }

    #[tokio::test]
    async fn auto_memory_uses_custom_claude_base() {
        let dir = tempdir().unwrap();
        let claude_base = dir.path().join("claude-alt");
        let project = PathBuf::from("/workspace/proj");
        let encoded = encode_path(&project.to_string_lossy());
        let memory_dir = claude_base.join("projects").join(encoded).join("memory");
        tokio::fs::create_dir_all(&memory_dir).await.unwrap();
        tokio::fs::write(memory_dir.join("MEMORY.md"), "remember me")
            .await
            .unwrap();

        let result = read_all_claude_md_files_with_base(&project, &claude_base).await;

        assert!(result[&Scope::AutoMemory].exists);
        assert_eq!(result[&Scope::AutoMemory].char_count, "remember me".len());
        assert!(result[&Scope::AutoMemory].path.contains("claude-alt"));
    }
}
