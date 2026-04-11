//! 编码目录名 / 项目 ID 的纯函数工具。
//!
//! Claude Code 把 "cwd" 编码成 `~/.claude/projects/` 下的目录名，规则是：
//! 把 `/` 换成 `-`。解码是"每个 leading `-` 换回 `/`"的 best-effort ——
//! 当路径本身含 `-` 时会歧义，真实 cwd 必须从 session JSONL 里的 `cwd`
//! 字段读取（由 `ProjectPathResolver` 完成）。
//!
//! Spec 行为参考：`openspec/specs/project-discovery/spec.md` 的
//! `Decode encoded project paths` Requirement。

use std::path::{Path, PathBuf};

/// composite project ID 的分隔符（`{baseDir}::{hash8}`）。
pub const COMPOSITE_SEPARATOR: &str = "::";

/// 把编码后的目录名还原成 best-effort 的文件系统路径。
///
/// 规则：每个 leading `-` 换成 `/`，其余 `-` 原样保留。歧义不做消解。
#[must_use]
pub fn decode_path(encoded: &str) -> PathBuf {
    if encoded.is_empty() {
        return PathBuf::new();
    }
    // 对齐 TS `decodePath`：去掉 leading `-`，再把剩下的 `-` 全部换成 `/`，
    // 最后补上 leading `/`。歧义不做消解（spec 的 best-effort 语义）。
    let trimmed = encoded.strip_prefix('-').unwrap_or(encoded);
    let replaced: String = trimmed.replace('-', "/");
    if replaced.starts_with('/') {
        PathBuf::from(replaced)
    } else {
        PathBuf::from(format!("/{replaced}"))
    }
}

/// 从任意 project ID 抽出 `baseDir` —— composite ID 去掉 `::<hash>` 后缀，
/// plain ID 原样返回。
#[must_use]
pub fn extract_base_dir(project_id: &str) -> &str {
    match project_id.find(COMPOSITE_SEPARATOR) {
        Some(idx) => &project_id[..idx],
        None => project_id,
    }
}

/// 从 `Path` 里拿最后一段作为展示名。
#[must_use]
pub fn extract_project_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |s| s.to_string_lossy().into_owned(),
    )
}

/// 是否是合法的 Claude Code 编码目录名 —— 必须以 `-` 开头。
#[must_use]
pub fn is_valid_encoded_path(name: &str) -> bool {
    name.starts_with('-')
}

/// `~/.claude/projects/` —— 根据 `$HOME` 动态解析。
#[must_use]
pub fn get_projects_base_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".claude")
        .join("projects")
}

/// `~/.claude/todos/` —— 动态解析。
#[must_use]
pub fn get_todos_base_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".claude")
        .join("todos")
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_encoded_name_decodes() {
        assert_eq!(
            decode_path("-Users-alice-code-app"),
            PathBuf::from("/Users/alice/code/app")
        );
    }

    #[test]
    fn ambiguous_name_is_best_effort() {
        // `-Users-alice-my-app` 里的中间 `-app` 无从消解，按 spec 返回全 slash 版本。
        assert_eq!(
            decode_path("-Users-alice-my-app"),
            PathBuf::from("/Users/alice/my/app")
        );
    }

    #[test]
    fn wsl_mount_path_survives_roundtrip() {
        assert_eq!(decode_path("-mnt-c-code"), PathBuf::from("/mnt/c/code"));
    }

    #[test]
    fn extract_base_dir_handles_composite_and_plain() {
        assert_eq!(extract_base_dir("-Users-foo::abcd1234"), "-Users-foo");
        assert_eq!(extract_base_dir("-Users-foo"), "-Users-foo");
    }

    #[test]
    fn is_valid_encoded_path_requires_leading_hyphen() {
        assert!(is_valid_encoded_path("-foo"));
        assert!(!is_valid_encoded_path("foo"));
        assert!(!is_valid_encoded_path(""));
    }

    #[test]
    fn extract_project_name_returns_last_segment() {
        assert_eq!(
            extract_project_name(&PathBuf::from("/Users/alice/app")),
            "app"
        );
    }
}
