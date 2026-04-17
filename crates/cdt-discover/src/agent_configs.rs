//! agent-configs capability：扫描并解析 `.claude/agents/*.md` 文件。
//!
//! Spec：`openspec/specs/agent-configs/spec.md`。
//!
//! 支持两个作用域：
//! - Global：`~/.claude/agents/*.md`
//! - Project：`<project cwd>/.claude/agents/*.md`
//!
//! 每个文件的 YAML frontmatter 按 `key: value` 单行解析，提取 `name`、`color`、
//! `description` 三个字段；其余键忽略。若无 frontmatter，以文件名（去扩展名）
//! 作为 `name`。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Agent config 的作用域。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "projectId", rename_all = "camelCase")]
pub enum AgentConfigScope {
    Global,
    Project(String),
}

impl AgentConfigScope {
    fn sort_rank(&self) -> u8 {
        match self {
            AgentConfigScope::Global => 0,
            AgentConfigScope::Project(_) => 1,
        }
    }
}

/// 解析后的 agent 配置条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub scope: AgentConfigScope,
    pub file_path: PathBuf,
}

/// 聚合入口：同时扫全局和每个项目下的 agent configs。
///
/// `projects` 的元素为 `(project_id, cwd)`。返回值按 `(scope_global_first, name)`
/// 稳定排序。
#[must_use]
pub fn read_agent_configs(projects: &[(String, String)]) -> Vec<AgentConfig> {
    let mut all = scan_global();
    for (pid, cwd) in projects {
        all.extend(scan_project(pid, Path::new(cwd)));
    }
    all.sort_by(|a, b| {
        a.scope
            .sort_rank()
            .cmp(&b.scope.sort_rank())
            .then_with(|| a.name.cmp(&b.name))
    });
    all
}

/// 扫全局作用域 `~/.claude/agents/*.md`。目录缺失返回空。
#[must_use]
pub fn scan_global() -> Vec<AgentConfig> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let dir = home.join(".claude").join("agents");
    scan_dir(&dir, &AgentConfigScope::Global)
}

/// 扫某个项目作用域 `<cwd>/.claude/agents/*.md`。目录缺失返回空。
#[must_use]
pub fn scan_project(project_id: &str, cwd: &Path) -> Vec<AgentConfig> {
    let dir = cwd.join(".claude").join("agents");
    scan_dir(&dir, &AgentConfigScope::Project(project_id.to_owned()))
}

fn scan_dir(dir: &Path, scope: &AgentConfigScope) -> Vec<AgentConfig> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "md") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let fallback_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        let (fm, _body) = parse_frontmatter(&content);
        let name = fm
            .get("name")
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or(fallback_name);
        out.push(AgentConfig {
            name,
            color: fm.get("color").cloned(),
            description: fm.get("description").cloned(),
            scope: scope.clone(),
            file_path: path,
        });
    }
    out
}

/// 手写 YAML frontmatter 解析器。
///
/// 仅识别 `---` 分隔的 frontmatter 块；块内仅解析 `key: value` 单行，忽略其它格式。
/// 引号（双引号或单引号）会被剥离。无 frontmatter 或块未闭合时返回空 map 与原内容。
#[must_use]
pub fn parse_frontmatter(content: &str) -> (HashMap<String, String>, &str) {
    let mut map = HashMap::new();
    let trimmed_start = content
        .strip_prefix("---\n")
        .or(content.strip_prefix("---\r\n"));
    let Some(after_open) = trimmed_start else {
        return (map, content);
    };
    // 找闭合 ---
    let close_patterns = ["\n---\n", "\n---\r\n", "\n---"];
    let mut end_idx = None;
    for pat in close_patterns {
        if let Some(idx) = after_open.find(pat) {
            end_idx = Some((idx, pat.len()));
            break;
        }
    }
    let Some((fm_end, close_len)) = end_idx else {
        return (map, content);
    };
    let fm_block = &after_open[..fm_end];
    let body_start = fm_end + close_len;
    let body = if body_start <= after_open.len() {
        &after_open[body_start..]
    } else {
        ""
    };

    for line in fm_block.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || key.contains(char::is_whitespace) {
            continue;
        }
        let value = value.trim();
        let stripped = strip_quotes(value);
        map.insert(key.to_owned(), stripped.to_owned());
    }
    (map, body)
}

fn strip_quotes(s: &str) -> &str {
    if s.len() >= 2 {
        let bytes = s.as_bytes();
        let first = bytes[0];
        let last = bytes[s.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return &s[1..s.len() - 1];
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    fn write_md(dir: &Path, name: &str, contents: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), contents).unwrap();
    }

    #[test]
    fn parse_complete_frontmatter() {
        let (fm, body) = parse_frontmatter(
            "---\nname: code-reviewer\ncolor: purple\ndescription: Reviews code\n---\nBody here\n",
        );
        assert_eq!(fm.get("name").map(String::as_str), Some("code-reviewer"));
        assert_eq!(fm.get("color").map(String::as_str), Some("purple"));
        assert_eq!(
            fm.get("description").map(String::as_str),
            Some("Reviews code")
        );
        assert_eq!(body, "Body here\n");
    }

    #[test]
    fn parse_quoted_color() {
        let (fm, _) = parse_frontmatter("---\ncolor: \"#ff0000\"\n---\n");
        assert_eq!(fm.get("color").map(String::as_str), Some("#ff0000"));
        let (fm2, _) = parse_frontmatter("---\ncolor: 'red'\n---\n");
        assert_eq!(fm2.get("color").map(String::as_str), Some("red"));
    }

    #[test]
    fn parse_missing_fields_defaults_to_none() {
        let (fm, _) = parse_frontmatter("---\nname: only\n---\n");
        assert_eq!(fm.get("name").map(String::as_str), Some("only"));
        assert!(!fm.contains_key("color"));
        assert!(!fm.contains_key("description"));
    }

    #[test]
    fn parse_no_frontmatter_returns_empty_map() {
        let (fm, body) = parse_frontmatter("no frontmatter here\n");
        assert!(fm.is_empty());
        assert_eq!(body, "no frontmatter here\n");
    }

    #[test]
    fn parse_invalid_line_skipped() {
        let (fm, _) = parse_frontmatter("---\nname: ok\n# comment\n  - list\ncolor: red\n---\n");
        assert_eq!(fm.get("name").map(String::as_str), Some("ok"));
        assert_eq!(fm.get("color").map(String::as_str), Some("red"));
    }

    #[test]
    fn missing_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let nope = tmp.path().join("does-not-exist");
        let out = scan_dir(&nope, &AgentConfigScope::Global);
        assert!(out.is_empty());
    }

    #[test]
    fn scan_project_reads_md_and_uses_filename_fallback() {
        let tmp = TempDir::new().unwrap();
        let agents_dir = tmp.path().join(".claude").join("agents");
        write_md(
            &agents_dir,
            "code-reviewer.md",
            "---\nname: code-reviewer\ncolor: purple\n---\nbody",
        );
        write_md(&agents_dir, "no-frontmatter.md", "just body");
        let mut out = scan_project("proj-1", tmp.path());
        out.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "code-reviewer");
        assert_eq!(out[0].color.as_deref(), Some("purple"));
        assert_eq!(out[1].name, "no-frontmatter");
        assert!(out[1].color.is_none());
    }

    #[test]
    fn both_scopes_merged_and_sorted() {
        // 全局目录是 ~/.claude/agents——测试用 project 级多条，再确认排序按 name 字典序。
        let tmp = TempDir::new().unwrap();
        let agents_dir = tmp.path().join(".claude").join("agents");
        write_md(&agents_dir, "zebra.md", "---\nname: zebra\n---\n");
        write_md(&agents_dir, "alpha.md", "---\nname: alpha\n---\n");
        let cwd_str = tmp.path().to_string_lossy().to_string();
        let configs = read_agent_configs(&[("p1".to_owned(), cwd_str)]);
        // 仅来自项目的两条（global 目录可能存在，也可能不存在；此处只校验排序不变式）。
        let names: Vec<&str> = configs.iter().map(|c| c.name.as_str()).collect();
        let alpha = names.iter().position(|n| *n == "alpha").unwrap();
        let zebra = names.iter().position(|n| *n == "zebra").unwrap();
        assert!(alpha < zebra);
    }
}
