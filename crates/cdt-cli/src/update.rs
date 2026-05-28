use std::env;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

const REPO: &str = "snowzhaozhj/claude-devtools-rs";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

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
        None => fetch_latest_tag().await?,
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

    let binary_bytes = download_and_extract(&url, &asset_name).await?;
    replace_binary(&install_path, &binary_bytes)?;

    println!(
        "Updated cdt to v{target} ({install_path})",
        install_path = install_path.display()
    );
    Ok(())
}

async fn fetch_latest_tag() -> Result<String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let client = build_client()?;

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

fn build_client() -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("User-Agent", "cdt-self-update".parse().unwrap());

    if let Ok(token) = env::var("GH_TOKEN").or_else(|_| env::var("GITHUB_TOKEN")) {
        let val = format!("Bearer {token}");
        headers.insert("Authorization", val.parse().context("invalid token value")?);
    }

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .context("failed to build HTTP client")
}

fn platform_asset_name() -> Result<String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    let name = match (os, arch) {
        ("macos", "aarch64") => "cdt-darwin-arm64.tar.gz",
        ("macos", "x86_64") => "cdt-darwin-x64.tar.gz",
        ("linux", "x86_64") => "cdt-linux-x64.tar.gz",
        ("windows", "x86_64") => "cdt-windows-x64.zip",
        _ => bail!("unsupported platform: {os}/{arch}"),
    };

    Ok(name.to_string())
}

async fn download_and_extract(url: &str, asset_name: &str) -> Result<Vec<u8>> {
    let client = build_client()?;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to download {url}"))?;

    if !resp.status().is_success() {
        bail!("download failed: HTTP {} for {url}", resp.status());
    }

    let expected_len = resp.content_length();
    let archive_bytes = resp.bytes().await.context("failed to read response body")?;

    if let Some(expected) = expected_len {
        if archive_bytes.len() as u64 != expected {
            bail!(
                "incomplete download: got {} bytes, expected {expected}",
                archive_bytes.len()
            );
        }
    }

    if archive_bytes.is_empty() {
        bail!("downloaded file is empty");
    }

    if asset_name.ends_with(".tar.gz") {
        extract_tar_gz(&archive_bytes)
    } else if Path::new(asset_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        extract_zip(&archive_bytes)
    } else {
        bail!("unknown archive format: {asset_name}");
    }
}

fn extract_tar_gz(data: &[u8]) -> Result<Vec<u8>> {
    let decoder = flate2::read::GzDecoder::new(data);
    let mut archive = tar::Archive::new(decoder);

    let binary_name = if cfg!(windows) { "cdt.exe" } else { "cdt" };

    for entry in archive.entries().context("failed to read tar entries")? {
        let mut entry = entry.context("corrupt tar entry")?;
        let path = entry.path().context("invalid path in tar entry")?;

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            continue;
        }

        if file_name == binary_name {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .context("failed to read binary from archive")?;
            if buf.len() < 1024 {
                bail!(
                    "extracted binary too small ({} bytes), likely corrupted",
                    buf.len()
                );
            }
            return Ok(buf);
        }
    }

    bail!("binary '{binary_name}' not found in archive");
}

fn extract_zip(data: &[u8]) -> Result<Vec<u8>> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).context("failed to read zip archive")?;

    let binary_name = if cfg!(windows) { "cdt.exe" } else { "cdt" };

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("failed to read zip entry")?;
        let name = file.name().to_string();

        if name.contains("..") {
            continue;
        }

        let file_name = Path::new(&name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if file_name == binary_name {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)
                .context("failed to read binary from zip")?;
            if buf.len() < 1024 {
                bail!(
                    "extracted binary too small ({} bytes), likely corrupted",
                    buf.len()
                );
            }
            return Ok(buf);
        }
    }

    bail!("binary '{binary_name}' not found in zip archive");
}

fn resolve_install_path() -> Result<PathBuf> {
    let exe = env::current_exe().context(
        "cannot determine current executable path. Use --install-path to specify explicitly.",
    )?;
    exe.canonicalize().or_else(|_| Ok(exe))
}

fn check_install_path(path: &Path) -> Result<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy();

    let managed_indicators = [
        "/Cellar/",
        "/homebrew/",
        "/nix/store/",
        "/snap/",
        "/.cargo/bin/",
    ];

    for indicator in &managed_indicators {
        if path_str.contains(indicator) {
            bail!(
                "cdt is installed via a package manager ({indicator} detected in path).\n\
                 Self-update would conflict with the package manager.\n\
                 Please upgrade using your package manager, or reinstall with:\n\
                 \n  curl -fsSL https://raw.githubusercontent.com/{REPO}/main/install.sh | sh"
            );
        }
    }

    let parent = path.parent().context("cannot determine parent directory")?;

    let test_file = parent.join(".cdt-update-check");
    match fs::write(&test_file, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&test_file);
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            bail!(
                "no write permission to {}.\nTry running with elevated privileges:\n\n  sudo cdt self-update",
                parent.display()
            );
        }
        Err(e) => {
            bail!("cannot write to {}: {e}", parent.display());
        }
    }

    Ok(())
}

fn validate_binary_magic(data: &[u8]) -> Result<()> {
    if data.len() < 4 {
        bail!("binary too small to validate");
    }

    let valid = match &data[..4] {
        // ELF
        [0x7f, b'E', b'L', b'F']
        // Mach-O (32/64, big/little endian)
        | [0xfe, 0xed, 0xfa, 0xce | 0xcf]
        | [0xce | 0xcf, 0xfa, 0xed, 0xfe]
        // Mach-O fat binary
        | [0xca, 0xfe, 0xba, 0xbe]
        // PE (Windows)
        | [b'M', b'Z', ..] => true,
        _ => false,
    };

    if !valid {
        bail!(
            "downloaded file does not appear to be a valid executable (unexpected magic bytes: {:02x} {:02x} {:02x} {:02x})",
            data[0],
            data[1],
            data[2],
            data[3]
        );
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
