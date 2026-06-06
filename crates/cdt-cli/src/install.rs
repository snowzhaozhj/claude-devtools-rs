//! Shared CLI binary installation utilities.
//!
//! Provides platform detection, download, extraction, and binary replacement
//! for both `cdt self-update` and Tauri desktop "Install CLI" feature.

use std::env;
use std::io::Read as _;
use std::path::Path;

use anyhow::{Context, Result, bail};

pub const REPO: &str = "snowzhaozhj/claude-devtools-rs";

pub const DEFAULT_DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub fn platform_asset_name() -> Result<String> {
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

pub async fn download_and_extract(url: &str, asset_name: &str) -> Result<Vec<u8>> {
    let client = build_client(
        reqwest::redirect::Policy::default(),
        Some(DEFAULT_DOWNLOAD_TIMEOUT),
    )?;
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

pub async fn download_and_extract_with_timeout(
    url: &str,
    asset_name: &str,
    timeout: std::time::Duration,
) -> Result<Vec<u8>> {
    let client = build_client(reqwest::redirect::Policy::default(), Some(timeout))?;
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

pub fn extract_tar_gz(data: &[u8]) -> Result<Vec<u8>> {
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

pub fn extract_zip(data: &[u8]) -> Result<Vec<u8>> {
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

pub fn validate_binary_magic(data: &[u8]) -> Result<()> {
    if data.len() < 4 {
        bail!("binary too small to validate");
    }

    let valid = matches!(
        &data[..4],
        [0x7f, b'E', b'L', b'F']
            | [0xfe, 0xed, 0xfa, 0xce | 0xcf]
            | [0xce | 0xcf, 0xfa, 0xed, 0xfe]
            | [0xca, 0xfe, 0xba, 0xbe]
            | [b'M', b'Z', ..]
    );

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

pub fn build_client(
    redirect: reqwest::redirect::Policy,
    timeout: Option<std::time::Duration>,
) -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("User-Agent", "cdt-self-update".parse().unwrap());

    if let Ok(token) = env::var("GH_TOKEN").or_else(|_| env::var("GITHUB_TOKEN")) {
        let val = format!("Bearer {token}");
        headers.insert("Authorization", val.parse().context("invalid token value")?);
    }

    let mut builder = reqwest::Client::builder()
        .default_headers(headers)
        .redirect(redirect);

    if let Some(t) = timeout {
        builder = builder.timeout(t);
    }

    builder.build().context("failed to build HTTP client")
}
