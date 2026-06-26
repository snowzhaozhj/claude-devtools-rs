use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use cdt_cli::install::{
    DownloadErrorKind, REPO, build_client, classify_download_error, download_and_extract,
    platform_asset_name, validate_binary_magic,
};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const VERSION_CHECK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn friendly_error(raw: &str) -> String {
    match classify_download_error(raw) {
        DownloadErrorKind::Timeout => {
            "Network timeout, please check your connection and try again".to_string()
        }
        DownloadErrorKind::Dns => "DNS resolution failed, please check your network".to_string(),
        DownloadErrorKind::Connection => {
            "Cannot connect to the update server, please check your network or proxy settings"
                .to_string()
        }
        DownloadErrorKind::RateLimit => {
            "API rate limit exceeded. Set GH_TOKEN or GITHUB_TOKEN to increase the limit"
                .to_string()
        }
        DownloadErrorKind::NotFound => {
            "Release not found. The requested version may not exist".to_string()
        }
        DownloadErrorKind::Forbidden => {
            "Access denied. Check your network or proxy settings, or set GH_TOKEN / GITHUB_TOKEN"
                .to_string()
        }
        DownloadErrorKind::Other => "Update failed, please try again later".to_string(),
    }
}

pub struct UpdateOptions {
    pub check_only: bool,
    pub target_version: Option<String>,
    pub install_path: Option<PathBuf>,
}

pub async fn run(opts: UpdateOptions) -> Result<()> {
    let current =
        semver::Version::parse(CURRENT_VERSION).context("failed to parse current version")?;

    let target_tag = match &opts.target_version {
        Some(v) => {
            if v.starts_with('v') {
                v.clone()
            } else {
                format!("v{v}")
            }
        }
        None => match fetch_latest_tag().await {
            Ok(tag) => tag,
            Err(e) => {
                tracing::warn!(target: "cdt_cli::update", error = %e, "version check failed");
                bail!("{}", friendly_error(&format!("{e:#}")));
            }
        },
    };

    let target_ver_str = target_tag.strip_prefix('v').unwrap_or(&target_tag);
    let target = semver::Version::parse(target_ver_str)
        .with_context(|| format!("invalid version: {target_ver_str}"))?;

    if target <= current && opts.target_version.is_none() {
        println!("Already up to date (v{current}).");
        return Ok(());
    }

    if target == current {
        println!("Already at v{current}.");
        return Ok(());
    }

    if opts.check_only {
        if target > current {
            println!("Update available: v{current} → v{target}");
        } else {
            println!(
                "v{target} is older than current v{current}. Use --version v{target} without --check to downgrade."
            );
        }
        return Ok(());
    }

    let install_path = match opts.install_path {
        Some(p) => p,
        None => resolve_install_path()?,
    };

    check_install_path(&install_path)?;

    println!("Updating cdt v{current} → v{target}...");

    let asset_name = platform_asset_name()?;
    let url = format!("https://github.com/{REPO}/releases/download/{target_tag}/{asset_name}");

    let binary_bytes = match download_and_extract(&url, &asset_name).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!(target: "cdt_cli::update", error = %e, "download failed");
            bail!("{}", friendly_error(&format!("{e:#}")));
        }
    };
    replace_binary(&install_path, &binary_bytes)?;

    println!(
        "Updated cdt to v{target} ({install_path})",
        install_path = install_path.display()
    );

    // 自动刷新已安装的 shell 补全
    if let Err(e) = crate::completions::refresh_installed() {
        eprintln!("Warning: failed to refresh shell completions: {e}");
    }

    Ok(())
}

async fn fetch_latest_tag() -> Result<String> {
    // 优先走 `releases/latest` 的 302 重定向探测最新 tag——github.com 网页跳转不消耗
    // GitHub REST API 的 60 次/小时未认证额度（共享出口 IP 下极易耗尽，是 `cdt self-update`
    // 不带 --version 必报 rate-limit 的根因）。探测失败再 fallback 到 API（带 token 时 5000/小时）。
    match fetch_latest_tag_via_redirect().await {
        Ok(tag) => Ok(tag),
        Err(err) => {
            tracing::debug!(
                "latest-tag redirect probe failed, falling back to GitHub API: {err:#}"
            );
            fetch_latest_tag_via_api().await
        }
    }
}

/// 通过 `https://github.com/<repo>/releases/latest` 的 302 重定向拿最新 tag。
///
/// 关 redirect-follow，读 `Location` 头——形如 `https://github.com/<repo>/releases/tag/vX.Y.Z`。
/// 仓库无 release 时该 endpoint 返回 404（非重定向），此处 `bail!` 后由调用方 fallback。
///
/// 用 `GET`（不消费 body）而非 `HEAD`：企业透明代理可能把 `HEAD` 改写为 `GET` 并自动跟随
/// 重定向，丢失 302 + `Location`，让本探测静默降级回 API；`GET` 更不易被代理篡改。
async fn fetch_latest_tag_via_redirect() -> Result<String> {
    let url = format!("https://github.com/{REPO}/releases/latest");
    let client = build_client(
        reqwest::redirect::Policy::none(),
        Some(VERSION_CHECK_TIMEOUT),
    )?;

    let resp = client
        .get(&url)
        .send()
        .await
        .context("failed to reach github.com")?;

    let status = resp.status();
    if !status.is_redirection() {
        bail!("expected redirect from releases/latest, got HTTP {status}");
    }

    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .context("redirect response missing Location header")?;

    parse_tag_from_location(location)
}

/// 从 `releases/tag/<tag>` 形式的重定向目标里解析出 tag 段。
fn parse_tag_from_location(location: &str) -> Result<String> {
    let tag = location
        .rsplit_once("/releases/tag/")
        .map(|(_, rest)| {
            // 剥离可能的 query string / fragment，再去掉 trailing slash。
            let rest = rest.split(['?', '#']).next().unwrap_or(rest);
            rest.trim_end_matches('/')
        })
        .filter(|tag| !tag.is_empty())
        .with_context(|| format!("could not parse tag from redirect Location: {location}"))?;
    Ok(tag.to_string())
}

async fn fetch_latest_tag_via_api() -> Result<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let client = build_client(
        reqwest::redirect::Policy::default(),
        Some(VERSION_CHECK_TIMEOUT),
    )?;

    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("failed to reach GitHub API")?;

    if resp.status().as_u16() == 403 {
        bail!(
            "GitHub API rate limit exceeded. Set GH_TOKEN or GITHUB_TOKEN environment variable, \
             or specify a version with --version vX.Y.Z"
        );
    }

    if !resp.status().is_success() {
        bail!("GitHub API returned {}", resp.status());
    }

    let body: serde_json::Value = resp.json().await.context("invalid JSON from GitHub API")?;
    let tag = body["tag_name"]
        .as_str()
        .context("missing tag_name in release response")?;

    Ok(tag.to_string())
}

fn resolve_install_path() -> Result<PathBuf> {
    let exe = env::current_exe().context(
        "cannot determine current executable path. Use --install-path to specify explicitly.",
    )?;
    exe.canonicalize().or_else(|_| Ok(exe))
}

fn managed_install_path() -> Option<PathBuf> {
    let home = cdt_discover::home_dir()?;
    let managed = home.join(".local").join("bin").join("cdt");
    if managed.exists() {
        Some(managed)
    } else {
        None
    }
}

fn check_install_path(path: &Path) -> Result<()> {
    let parent = path.parent().context("cannot determine parent directory")?;

    let test_file = parent.join(".cdt-update-check");
    match fs::write(&test_file, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&test_file);
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            if let Some(managed) = managed_install_path() {
                bail!(
                    "No write permission to {}.\n\n\
                     A desktop-managed installation exists at {}.\n\
                     You can update it with:\n\n  {} self-update\n\n\
                     Or update via the desktop app's Settings page.",
                    parent.display(),
                    managed.display(),
                    managed.display(),
                );
            }
            bail!(
                "No write permission to {}.\n\
                 Try running with elevated privileges:\n\n  sudo cdt self-update",
                parent.display(),
            );
        }
        Err(e) => {
            bail!("Cannot write to {}: {e}", parent.display());
        }
    }

    // Warn (non-blocking) if running from a different path but managed install exists
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Some(managed) = managed_install_path() {
        let managed_canonical = managed.canonicalize().unwrap_or_else(|_| managed.clone());
        if canonical != managed_canonical {
            eprintln!(
                "Note: a desktop-managed cdt exists at {}.\n\
                 You are updating {} instead.\n\
                 To avoid maintaining two installations, consider removing this copy\n\
                 and using the desktop-managed version.\n",
                managed.display(),
                canonical.display(),
            );
        }
    }

    Ok(())
}

fn replace_binary(target: &Path, new_bytes: &[u8]) -> Result<()> {
    validate_binary_magic(new_bytes)?;

    let parent = target.parent().context("no parent directory")?;
    let stem = target.file_name().unwrap_or_default().to_string_lossy();
    let pid = std::process::id();
    let backup = parent.join(format!("{stem}.old"));
    let temp_path = parent.join(format!(".{stem}.{pid}.tmp"));

    if temp_path.exists() {
        fs::remove_file(&temp_path).ok();
    }

    fs::write(&temp_path, new_bytes)
        .with_context(|| format!("failed to write temp file: {}", temp_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))
            .context("failed to set executable permission")?;
    }

    if target.exists() {
        if backup.exists() {
            let _ = fs::remove_file(&backup);
        }
        fs::rename(target, &backup).context(
            "failed to backup current binary. On Windows, close any other cdt processes and retry.",
        )?;
    }

    if let Err(e) = fs::rename(&temp_path, target) {
        if let Err(rb_err) = fs::rename(&backup, target) {
            let _ = fs::remove_file(&temp_path);
            bail!(
                "CRITICAL: failed to install new binary ({e}) AND failed to restore backup ({rb_err}).\n\
                 Your original binary is at: {}\n\
                 Manually restore it with: mv {} {}",
                backup.display(),
                backup.display(),
                target.display(),
            );
        }
        let _ = fs::remove_file(&temp_path);
        return Err(e).context("failed to install new binary (rolled back successfully)");
    }

    let _ = fs::remove_file(&backup);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{friendly_error, parse_tag_from_location};

    #[test]
    fn parses_tag_from_releases_latest_redirect() {
        let tag =
            parse_tag_from_location("https://github.com/owner/repo/releases/tag/v0.6.0").unwrap();
        assert_eq!(tag, "v0.6.0");
    }

    #[test]
    fn parses_tag_ignoring_trailing_slash() {
        let tag =
            parse_tag_from_location("https://github.com/owner/repo/releases/tag/v1.2.3/").unwrap();
        assert_eq!(tag, "v1.2.3");
    }

    #[test]
    fn parses_tag_stripping_query_and_fragment() {
        let tag = parse_tag_from_location(
            "https://github.com/owner/repo/releases/tag/v2.0.0?utm_source=redirect",
        )
        .unwrap();
        assert_eq!(tag, "v2.0.0");
        let tag =
            parse_tag_from_location("https://github.com/owner/repo/releases/tag/v2.0.0#notes")
                .unwrap();
        assert_eq!(tag, "v2.0.0");
    }

    #[test]
    fn rejects_location_without_tag_segment() {
        // 仓库无 release 时 releases/latest 返回 404；即便拿到非 tag 的 Location 也不应误判出 tag。
        assert!(parse_tag_from_location("https://github.com/owner/repo/releases/").is_err());
        assert!(parse_tag_from_location("https://github.com/owner/repo/releases/tag/").is_err());
        assert!(parse_tag_from_location("https://example.com/").is_err());
    }

    #[test]
    fn friendly_error_never_leaks_url() {
        let raw = "download failed: HTTP 500 Internal Server Error for https://github.com/owner/repo/releases/download/v1.0.0/cdt-darwin-arm64.tar.gz";
        let msg = friendly_error(raw);
        assert!(
            !msg.contains("github.com"),
            "URL leaked in friendly error: {msg}"
        );
    }

    #[test]
    fn friendly_error_maps_timeout() {
        assert!(friendly_error("Operation timed out (os error 60)").contains("timeout"));
        assert!(friendly_error("request deadline exceeded").contains("timeout"));
    }

    #[test]
    fn friendly_error_maps_dns() {
        assert!(friendly_error("failed to lookup address").contains("DNS"));
    }

    #[test]
    fn friendly_error_maps_connection() {
        assert!(friendly_error("connection refused").contains("connect"));
        assert!(friendly_error("error sending request for url").contains("connect"));
        assert!(friendly_error("network is unreachable").contains("connect"));
    }

    #[test]
    fn friendly_error_maps_rate_limit() {
        assert!(friendly_error("GitHub API rate limit exceeded").contains("rate limit"));
    }

    #[test]
    fn friendly_error_does_not_misclassify_extraction_not_found() {
        let msg = friendly_error("binary 'cdt' not found in archive");
        assert!(
            !msg.contains("not exist"),
            "extraction error misclassified as missing release: {msg}"
        );
    }

    #[test]
    fn friendly_error_maps_http_404() {
        let msg = friendly_error("download failed: HTTP 404 Not Found for https://example.com");
        assert!(
            msg.contains("not exist"),
            "HTTP 404 should map to not-exist: {msg}"
        );
    }

    #[test]
    fn friendly_error_fallback_is_generic() {
        let msg = friendly_error("some unknown error happened");
        assert!(
            !msg.contains("some unknown error"),
            "raw error leaked: {msg}"
        );
    }

    #[test]
    fn check_install_path_writable_dir_succeeds() {
        let dir = std::env::temp_dir();
        let fake_exe = dir.join("cdt-test-check");
        std::fs::write(&fake_exe, b"").unwrap();
        let result = super::check_install_path(&fake_exe);
        let _ = std::fs::remove_file(&fake_exe);
        assert!(result.is_ok(), "writable dir should pass: {result:?}");
    }

    #[test]
    fn check_install_path_no_url_in_errors() {
        // Permission-denied path can't easily be tested portably, but we can
        // verify the friendly_error paths don't leak URLs by checking all
        // error variants.
        let forbidden_msg = friendly_error("HTTP 403 Forbidden");
        assert!(
            !forbidden_msg.contains("private repo"),
            "should not mention private repo: {forbidden_msg}"
        );
        assert!(
            !forbidden_msg.contains("github.com"),
            "URL leaked: {forbidden_msg}"
        );
    }
}
