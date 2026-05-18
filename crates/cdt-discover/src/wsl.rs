//! WSL distro 枚举（Windows 专属）。
//!
//! 仅在 `target_os = "windows"` 上执行真实枚举；其他平台 [`list_distros`]
//! 直接返回空报告。
//!
//! Spec：`openspec/specs/wsl-distro-discovery/spec.md`。

#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WslDistroCandidate {
    pub distro: String,
    pub home_path: String,
    pub claude_root_path: String,
    pub claude_root_exists: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WslDistroScanReport {
    pub candidates: Vec<WslDistroCandidate>,
    pub distros_without_home: Vec<String>,
}

/// 枚举本机 WSL distro 并返回每个 distro 的 `~/.claude` UNC 候选路径。
///
/// 在非 Windows 平台直接返回空报告。
#[cfg_attr(not(target_os = "windows"), allow(clippy::unused_async))]
pub async fn list_distros() -> Result<WslDistroScanReport, crate::DiscoverError> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::list_distros_impl().await
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(WslDistroScanReport::default())
    }
}

const HEURISTIC_SAMPLE_BYTES: usize = 512;
const HEURISTIC_NUL_NUMER: u32 = 30;
const HEURISTIC_NUL_DENOM: u32 = 100;
#[cfg(target_os = "windows")]
const COMMAND_TIMEOUT_SECS: u64 = 4;
#[cfg(target_os = "windows")]
const HOME_TIMEOUT_SECS: u64 = 5;

/// 解码 `wsl.exe` stdout 字节流。
///
/// 算法（与 spec `wsl-distro-discovery` 的 `Requirement: 解码 wsl.exe stdout`
/// 对齐）：
/// 1. UTF-16 LE BOM (`0xFF 0xFE`) → 跳过 BOM 后按 UTF-16 LE 解码
/// 2. 否则 heuristic 检测：前 ≤ 512 字节奇数 index NUL 比例 ≥ 30% → UTF-16 LE
/// 3. 否则 UTF-8 lossy 解码
/// 4. 解码后**全局** strip 所有 `\0`
pub(crate) fn decode_wsl_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    let decoded = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        decode_utf16_le(&bytes[2..])
    } else if looks_like_utf16_le(bytes) {
        decode_utf16_le(bytes)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };

    decoded.replace('\0', "").replace('\r', "\n")
}

fn decode_utf16_le(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

fn looks_like_utf16_le(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(HEURISTIC_SAMPLE_BYTES)];
    if sample.len() < 2 {
        return false;
    }

    let mut pairs: u32 = 0;
    let mut nulls_at_odd: u32 = 0;
    let mut i = 0usize;
    while i + 1 < sample.len() {
        pairs += 1;
        if sample[i + 1] == 0 {
            nulls_at_odd += 1;
        }
        i += 2;
    }

    pairs > 0 && nulls_at_odd * HEURISTIC_NUL_DENOM >= pairs * HEURISTIC_NUL_NUMER
}

/// 解析 `wsl.exe -l*` 类命令的 stdout 文本（已经过 [`decode_wsl_output`]）为 distro 名列表。
///
/// 过滤说明行 / `*` 前缀 / `(Default)` 后缀；按小写比较去重；保留首次出现顺序。
pub(crate) fn parse_wsl_distros(stdout: &str) -> Vec<String> {
    let mut distros: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for raw_line in stdout.lines() {
        let cleaned = raw_line.replace('\0', "");
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            continue;
        }

        let after_star = trimmed.strip_prefix('*').map_or(trimmed, str::trim_start);
        let after_default = strip_default_suffix(after_star);
        let line = after_default.trim();
        if line.is_empty() {
            continue;
        }

        let lower = line.to_lowercase();
        if lower.starts_with("windows subsystem for linux")
            || lower.contains("default version")
            || lower.starts_with("the following is a list")
        {
            continue;
        }

        if seen.insert(lower) {
            distros.push(line.to_string());
        }
    }

    distros
}

fn strip_default_suffix(input: &str) -> &str {
    const SUFFIX: &str = "(default)";
    if input.len() < SUFFIX.len() {
        return input;
    }
    let tail = &input[input.len() - SUFFIX.len()..];
    if tail.eq_ignore_ascii_case(SUFFIX) {
        input[..input.len() - SUFFIX.len()].trim_end()
    } else {
        input
    }
}

/// 规范化 distro 内 home 路径：trim + 必须以 `/` 开头 + posix normalize + 去尾随 `/`。
///
/// 不合法（不以 `/` 开头）返回 `None`。
pub(crate) fn normalize_wsl_home_path(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    Some(posix_normalize(trimmed))
}

fn posix_normalize(path: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    if stack.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", stack.join("/"))
    }
}

/// 拼装 distro UNC 路径：`\\wsl.localhost\<distro>\<posix-path-with-backslashes>`。
pub(crate) fn build_unc_path(distro: &str, posix_path: &str) -> String {
    let suffix = posix_path.replace('/', "\\");
    format!("\\\\wsl.localhost\\{distro}{suffix}")
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use std::path::PathBuf;
    use std::time::Duration;

    use tokio::process::Command;
    use tokio::time::timeout;

    use super::{
        COMMAND_TIMEOUT_SECS, HOME_TIMEOUT_SECS, WslDistroCandidate, WslDistroScanReport,
        build_unc_path, decode_wsl_output, normalize_wsl_home_path, parse_wsl_distros,
    };

    pub(super) async fn list_distros_impl() -> Result<WslDistroScanReport, crate::DiscoverError> {
        let Some(distros) = list_distros_via_fallback().await else {
            return Ok(WslDistroScanReport::default());
        };

        let resolutions = futures::future::join_all(
            distros
                .into_iter()
                .map(|distro| async move {
                    let home = resolve_home(&distro).await;
                    (distro, home)
                })
                .collect::<Vec<_>>(),
        )
        .await;

        let mut candidates: Vec<WslDistroCandidate> = Vec::new();
        let mut distros_without_home: Vec<String> = Vec::new();

        for (distro, home_opt) in resolutions {
            if let Some(home_path) = home_opt {
                let claude_posix = if home_path == "/" {
                    "/.claude".to_string()
                } else {
                    format!("{home_path}/.claude")
                };
                let claude_root_path = build_unc_path(&distro, &claude_posix);
                let claude_root_exists = tokio::fs::metadata(&claude_root_path).await.is_ok();
                candidates.push(WslDistroCandidate {
                    distro,
                    home_path,
                    claude_root_path,
                    claude_root_exists,
                });
            } else {
                distros_without_home.push(distro);
            }
        }

        candidates.sort_by(|a, b| a.distro.cmp(&b.distro));

        if candidates.is_empty() && !distros_without_home.is_empty() {
            tracing::warn!(
                count = distros_without_home.len(),
                "all WSL distros failed home resolution"
            );
        }

        Ok(WslDistroScanReport {
            candidates,
            distros_without_home,
        })
    }

    async fn list_distros_via_fallback() -> Option<Vec<String>> {
        let arg_groups: [&[&str]; 3] = [&["--list", "--quiet"], &["-l", "-q"], &["-l"]];

        for args in arg_groups {
            for executable in wsl_executable_candidates() {
                match run_wsl(&executable, args, Duration::from_secs(COMMAND_TIMEOUT_SECS)).await {
                    Ok(stdout) => {
                        let parsed = parse_wsl_distros(&stdout);
                        if !parsed.is_empty() {
                            return Some(parsed);
                        }
                    }
                    Err(err) => {
                        tracing::debug!(?executable, ?args, error = %err, "wsl.exe attempt failed");
                    }
                }
            }
        }

        tracing::warn!("WSL not available or no distros found");
        None
    }

    fn wsl_executable_candidates() -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        if let Ok(windir) = std::env::var("WINDIR") {
            out.push(PathBuf::from(format!(r"{windir}\System32\wsl.exe")));
            out.push(PathBuf::from(format!(r"{windir}\Sysnative\wsl.exe")));
        }
        out.push(PathBuf::from("wsl.exe"));
        out
    }

    async fn run_wsl(
        executable: &PathBuf,
        args: &[&str],
        wait: Duration,
    ) -> Result<String, std::io::Error> {
        let result = timeout(wait, Command::new(executable).args(args).output()).await;
        let output = match result {
            Ok(io_result) => io_result?,
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "wsl.exe timed out",
                ));
            }
        };
        if !output.status.success() {
            return Err(std::io::Error::other(format!(
                "wsl.exe exited with status {:?}",
                output.status.code()
            )));
        }
        Ok(decode_wsl_output(&output.stdout))
    }

    async fn resolve_home(distro: &str) -> Option<String> {
        if let Some(home) = run_wsl_for_home(distro)
            .await
            .and_then(|raw| normalize_wsl_home_path(&raw))
        {
            return Some(home);
        }
        match std::env::var("USERNAME") {
            Ok(username) if !username.is_empty() => {
                normalize_wsl_home_path(&format!("/home/{username}"))
            }
            _ => None,
        }
    }

    async fn run_wsl_for_home(distro: &str) -> Option<String> {
        let args: [&str; 6] = ["-d", distro, "--", "sh", "-lc", "printf %s \"$HOME\""];
        for executable in wsl_executable_candidates() {
            match run_wsl(&executable, &args, Duration::from_secs(HOME_TIMEOUT_SECS)).await {
                Ok(stdout) => return Some(stdout),
                Err(err) => {
                    tracing::debug!(distro, ?executable, error = %err, "resolve_home attempt failed");
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16_le_encode(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn decode_handles_utf16_le_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(utf16_le_encode("Ubuntu\r\nDebian-12\r\n"));
        let decoded = decode_wsl_output(&bytes);
        let lines: Vec<&str> = decoded.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines, vec!["Ubuntu", "Debian-12"]);
    }

    #[test]
    fn decode_handles_utf16_le_without_bom_via_heuristic() {
        let bytes = utf16_le_encode("Ubuntu\n");
        let decoded = decode_wsl_output(&bytes);
        assert_eq!(decoded.trim(), "Ubuntu");
    }

    #[test]
    fn decode_pure_ascii_falls_back_to_utf8() {
        let bytes = b"Ubuntu\nDebian-12\n";
        let decoded = decode_wsl_output(bytes);
        let lines: Vec<&str> = decoded.lines().collect();
        assert_eq!(lines, vec!["Ubuntu", "Debian-12"]);
    }

    #[test]
    fn decode_only_bom_returns_empty() {
        let decoded = decode_wsl_output(&[0xFF, 0xFE]);
        assert_eq!(decoded, "");
    }

    #[test]
    fn decode_handles_odd_total_bytes() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(utf16_le_encode("Ubuntu"));
        bytes.push(0x00); // 多一个字节，应该被丢弃
        let decoded = decode_wsl_output(&bytes);
        assert_eq!(decoded.trim(), "Ubuntu");
    }

    #[test]
    fn decode_strips_trailing_nul() {
        let bytes = b"Ubuntu\0\0\nDebian-12\0";
        let decoded = decode_wsl_output(bytes);
        let lines: Vec<&str> = decoded.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines, vec!["Ubuntu", "Debian-12"]);
    }

    #[test]
    fn decode_strips_inline_nul_bytes_globally() {
        // ASCII 被某些版本误读为 UTF-16 后会留行内 NUL，全局 strip 应清掉
        let bytes = b"U\0b\0u\0n\0t\0u\0";
        let decoded = decode_wsl_output(bytes);
        // heuristic 命中（NUL @ odd index = 100%）走 UTF-16 LE 路径，解出
        // 6 个 ASCII 字符；strip 后是 "Ubuntu"
        assert_eq!(decoded, "Ubuntu");
    }

    #[test]
    fn decode_handles_mixed_line_endings() {
        let bytes = b"Ubuntu\nDebian-12\r\nKali\r";
        let decoded = decode_wsl_output(bytes);
        let lines: Vec<&str> = decoded.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines, vec!["Ubuntu", "Debian-12", "Kali"]);
    }

    #[test]
    fn parse_filters_header_lines() {
        let input = "Windows Subsystem for Linux Distributions:\nUbuntu (Default)\nDebian-12\n";
        assert_eq!(
            parse_wsl_distros(input),
            vec!["Ubuntu".to_string(), "Debian-12".to_string()]
        );
    }

    #[test]
    fn parse_strips_default_marker_prefix() {
        let input = "* Ubuntu\nDebian-12\n";
        assert_eq!(
            parse_wsl_distros(input),
            vec!["Ubuntu".to_string(), "Debian-12".to_string()]
        );
    }

    #[test]
    fn parse_strips_default_suffix_case_insensitive() {
        let input = "Ubuntu (DEFAULT)\n";
        assert_eq!(parse_wsl_distros(input), vec!["Ubuntu".to_string()]);
    }

    #[test]
    fn parse_dedups_repeated_distro_names() {
        let input = "Ubuntu\nubuntu\nUbuntu\n";
        assert_eq!(parse_wsl_distros(input), vec!["Ubuntu".to_string()]);
    }

    #[test]
    fn parse_filters_default_version_line() {
        let input = "Default Version: 2\nUbuntu\n";
        assert_eq!(parse_wsl_distros(input), vec!["Ubuntu".to_string()]);
    }

    #[test]
    fn normalize_returns_none_for_relative_path() {
        assert_eq!(normalize_wsl_home_path("alice"), None);
        assert_eq!(normalize_wsl_home_path("home/alice"), None);
        assert_eq!(normalize_wsl_home_path(""), None);
    }

    #[test]
    fn normalize_strips_trailing_slash() {
        assert_eq!(
            normalize_wsl_home_path("/home/alice/"),
            Some("/home/alice".to_string())
        );
    }

    #[test]
    fn normalize_collapses_double_slashes_and_dot_dot() {
        assert_eq!(
            normalize_wsl_home_path("/home//alice/./../bob"),
            Some("/home/bob".to_string())
        );
    }

    #[test]
    fn normalize_root_path_remains_root() {
        assert_eq!(normalize_wsl_home_path("/"), Some("/".to_string()));
    }

    #[test]
    fn build_unc_path_with_standard_home() {
        assert_eq!(
            build_unc_path("Ubuntu", "/home/alice/.claude"),
            r"\\wsl.localhost\Ubuntu\home\alice\.claude"
        );
    }

    #[test]
    fn build_unc_path_with_hyphenated_distro_name() {
        assert_eq!(
            build_unc_path("Debian-12", "/root/.claude"),
            r"\\wsl.localhost\Debian-12\root\.claude"
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[tokio::test]
    async fn list_distros_returns_empty_on_non_windows() {
        let report = list_distros().await.unwrap();
        assert!(report.candidates.is_empty());
        assert!(report.distros_without_home.is_empty());
    }

    #[test]
    fn report_serializes_to_camel_case() {
        let report = WslDistroScanReport {
            candidates: vec![WslDistroCandidate {
                distro: "Ubuntu".to_string(),
                home_path: "/home/alice".to_string(),
                claude_root_path: r"\\wsl.localhost\Ubuntu\home\alice\.claude".to_string(),
                claude_root_exists: true,
            }],
            distros_without_home: vec!["Debian-12".to_string()],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["candidates"][0]["distro"], "Ubuntu");
        assert_eq!(json["candidates"][0]["homePath"], "/home/alice");
        assert_eq!(
            json["candidates"][0]["claudeRootPath"],
            r"\\wsl.localhost\Ubuntu\home\alice\.claude"
        );
        assert_eq!(json["candidates"][0]["claudeRootExists"], true);
        assert_eq!(json["distrosWithoutHome"][0], "Debian-12");
    }
}
