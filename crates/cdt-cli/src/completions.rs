use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use clap_complete::engine::{CompletionCandidate, ValueCandidates, ValueCompleter};

use cdt_discover::path_decoder;

/// 自定义环境变量名（避免与其他工具的 COMPLETE 冲突）
pub const ENV_VAR: &str = "CDT_COMPLETE";

const MAX_SESSION_CANDIDATES: usize = 20;
const MAX_TITLE_CHARS: usize = 50;

fn projects_dir() -> PathBuf {
    path_decoder::get_projects_base_path()
}

// ─────────────────────────────────────────────────────────────────────────────
// Script generation & install
// ─────────────────────────────────────────────────────────────────────────────

/// 生成动态补全注册脚本。
///
/// 通过 `CDT_COMPLETE=<shell>` 调用自身——这是 `CompleteEnv` 的官方机制，
/// 保证输出与 shell 直接执行 `CDT_COMPLETE=zsh cdt` 完全一致。
pub fn generate_script(shell: clap_complete::Shell) -> Result<Vec<u8>> {
    let exe = std::env::current_exe().context("cannot determine current executable path")?;
    let shell_name = shell_to_env_value(shell);

    let output = std::process::Command::new(&exe)
        .env(ENV_VAR, shell_name)
        .output()
        .with_context(|| format!("failed to invoke {}", exe.display()))?;

    if !output.status.success() {
        anyhow::bail!(
            "completion generation exited with {}",
            output.status.code().unwrap_or(-1)
        );
    }

    Ok(output.stdout)
}

/// 自动检测当前 shell 并安装补全脚本到正确位置。
pub fn install(dry_run: bool) -> Result<()> {
    let (shell, path) = detect_shell_and_path()?;

    if dry_run {
        println!(
            "[dry-run] Would install {shell} completions to: {}",
            path.display()
        );
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }

    let buf = generate_script(shell)?;
    fs::write(&path, buf)
        .with_context(|| format!("failed to write completions to {}", path.display()))?;

    println!("Installed {shell} completions to: {}", path.display());
    println!("Restart your shell or run: exec {shell}");
    Ok(())
}

/// 刷新已安装的补全文件（`self-update` 后调用）。
/// 如果之前没装过（文件不存在），静默跳过。
pub fn refresh_installed() -> Result<()> {
    let Ok((shell, path)) = detect_shell_and_path() else {
        return Ok(());
    };

    if !path.exists() {
        return Ok(());
    }

    let buf = generate_script(shell)?;
    fs::write(&path, buf)
        .with_context(|| format!("failed to refresh completions at {}", path.display()))?;

    println!("Refreshed {shell} completions at: {}", path.display());
    Ok(())
}

fn shell_to_env_value(shell: clap_complete::Shell) -> &'static str {
    match shell {
        clap_complete::Shell::Zsh => "zsh",
        clap_complete::Shell::Fish => "fish",
        clap_complete::Shell::Elvish => "elvish",
        clap_complete::Shell::PowerShell => "powershell",
        _ => "bash",
    }
}

fn detect_shell_and_path() -> Result<(clap_complete::Shell, PathBuf)> {
    let shell_env = std::env::var("SHELL").unwrap_or_default();
    let shell_name = Path::new(&shell_env)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let (shell, path) = match shell_name {
        "zsh" => {
            let dir = zsh_completions_dir();
            (clap_complete::Shell::Zsh, dir.join("_cdt"))
        }
        "bash" => {
            let dir = bash_completions_dir();
            (clap_complete::Shell::Bash, dir.join("cdt"))
        }
        "fish" => {
            let home = cdt_discover::home_dir().context("cannot determine home directory")?;
            let dir = home.join(".config/fish/completions");
            (clap_complete::Shell::Fish, dir.join("cdt.fish"))
        }
        _ => {
            anyhow::bail!(
                "Cannot detect shell from $SHELL=\"{shell_env}\". \
                 Use `cdt completions <shell>` to generate manually."
            );
        }
    };

    Ok((shell, path))
}

fn zsh_completions_dir() -> PathBuf {
    if let Ok(fpath) = std::env::var("FPATH") {
        for dir in fpath.split(':') {
            let p = PathBuf::from(dir);
            if p.exists() && is_user_writable(&p) && !is_system_path(&p) {
                return p;
            }
        }
    }

    let home = cdt_discover::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = home.join(".zsh/completions");
    if !dir.exists() {
        eprintln!(
            "Hint: add this to your ~/.zshrc (before compinit):\n  \
             fpath=(~/.zsh/completions $fpath)"
        );
    }
    dir
}

fn bash_completions_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("bash-completion/completions");
    }
    let home = cdt_discover::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/bash-completion/completions")
}

fn is_user_writable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| !m.permissions().readonly())
}

fn is_system_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.starts_with("/usr/") || s.starts_with("/opt/") || s.starts_with("/nix/")
}

// ─────────────────────────────────────────────────────────────────────────────
// Project completer
// ─────────────────────────────────────────────────────────────────────────────

pub struct ProjectCompleter;

struct RawCandidate {
    name: String,
    encoded: String,
    help: String,
}

impl ValueCandidates for ProjectCompleter {
    fn candidates(&self) -> Vec<CompletionCandidate> {
        let base = projects_dir();
        let Ok(entries) = fs::read_dir(&base) else {
            return Vec::new();
        };

        let home = cdt_discover::home_dir().unwrap_or_default();

        let mut raw: Vec<RawCandidate> = Vec::new();

        for entry in entries.filter_map(Result::ok) {
            if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                continue;
            }
            let encoded = entry.file_name().to_string_lossy().to_string();
            if !path_decoder::is_valid_encoded_path(&encoded) {
                continue;
            }
            if path_decoder::is_worktree_encoded_path(&encoded) {
                continue;
            }

            let project_dir = base.join(&encoded);
            let display_name = path_decoder::resolve_project_name_from_jsonl(&project_dir)
                .unwrap_or_else(|| {
                    path_decoder::extract_project_name(&path_decoder::decode_path(&encoded))
                });

            let decoded = path_decoder::decode_path(&encoded);
            let decoded_str = decoded.to_string_lossy();
            let help = make_home_relative(&decoded_str, &home);

            raw.push(RawCandidate {
                name: display_name,
                encoded,
                help,
            });
        }

        let mut name_counts = std::collections::HashMap::<String, usize>::new();
        for c in &raw {
            *name_counts.entry(c.name.clone()).or_default() += 1;
        }

        let mut seen = std::collections::HashSet::new();
        let mut candidates = Vec::new();

        for c in raw {
            if name_counts.get(&c.name).copied().unwrap_or(0) == 1 {
                if seen.insert(c.name.clone()) {
                    candidates.push(CompletionCandidate::new(c.name).help(Some(c.help.into())));
                }
            } else {
                candidates.push(
                    CompletionCandidate::new(c.encoded)
                        .help(Some(format!("{} · {}", c.name, c.help).into())),
                );
            }
        }

        candidates
    }
}

fn make_home_relative(path: &str, home: &Path) -> String {
    let home_str = home.to_string_lossy();
    if !home_str.is_empty() {
        let normalized = home_str.replace('\\', "/");
        if let Some(rest) = path.strip_prefix(normalized.as_str()) {
            return format!("~{rest}");
        }
    }
    path.to_owned()
}

// ─────────────────────────────────────────────────────────────────────────────
// Session completer
// ─────────────────────────────────────────────────────────────────────────────

pub struct SessionCompleter;

impl ValueCompleter for SessionCompleter {
    fn complete(&self, current: &OsStr) -> Vec<CompletionCandidate> {
        let prefix = current.to_string_lossy();
        let base = projects_dir();

        let Ok(project_entries) = fs::read_dir(&base) else {
            return Vec::new();
        };

        let mut sessions: Vec<(String, String, PathBuf, SystemTime)> = Vec::new();

        for project_entry in project_entries.filter_map(Result::ok) {
            if !project_entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                continue;
            }

            let dir_name = project_entry.file_name().to_string_lossy().to_string();
            if !path_decoder::is_valid_encoded_path(&dir_name) {
                continue;
            }
            let project_name = path_decoder::resolve_project_name_from_jsonl(&project_entry.path())
                .unwrap_or_else(|| {
                    path_decoder::extract_project_name(&path_decoder::decode_path(&dir_name))
                });

            let Ok(entries) = fs::read_dir(project_entry.path()) else {
                continue;
            };

            for entry in entries.filter_map(Result::ok) {
                let fname = entry.file_name();
                let fname_str = fname.to_string_lossy();
                let Some(session_id) = fname_str.strip_suffix(".jsonl") else {
                    continue;
                };

                if !prefix.is_empty() && !session_id.starts_with(prefix.as_ref()) {
                    continue;
                }

                let mtime = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);

                sessions.push((
                    session_id.to_string(),
                    project_name.clone(),
                    entry.path(),
                    mtime,
                ));
            }
        }

        sessions.sort_unstable_by_key(|s| std::cmp::Reverse(s.3));
        sessions.truncate(MAX_SESSION_CANDIDATES);

        sessions
            .into_iter()
            .map(|(id, project_name, path, _)| {
                let title = session_first_message(&path);
                let desc = match title {
                    Some(t) => format!("[{project_name}] {t}"),
                    None => format!("[{project_name}]"),
                };
                CompletionCandidate::new(id).help(Some(desc.into()))
            })
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn session_first_message(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(20) {
        let Ok(line) = line else { continue };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        if value.get("type").and_then(|v| v.as_str()) == Some("user") {
            let message = value
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| {
                    if let Some(s) = c.as_str() {
                        return Some(s.to_string());
                    }
                    c.as_array().and_then(|arr| {
                        arr.iter().find_map(|block| {
                            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                                block.get("text").and_then(|t| t.as_str()).map(String::from)
                            } else {
                                None
                            }
                        })
                    })
                })?;

            return Some(truncate_chars(&message, MAX_TITLE_CHARS));
        }
    }
    None
}

fn truncate_chars(s: &str, max: usize) -> String {
    let boundary = s.char_indices().nth(max).map_or(s.len(), |(i, _)| i);
    if boundary < s.len() {
        format!("{}...", &s[..boundary])
    } else {
        s.to_string()
    }
}
