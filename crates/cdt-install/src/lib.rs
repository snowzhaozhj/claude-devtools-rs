//! Shared CLI binary installation utilities.
//!
//! Provides platform detection, download, extraction, and binary replacement
//! for both `cdt self-update` and Tauri desktop "Install CLI" feature.

mod error;

use std::env;
use std::io::Read as _;
use std::path::Path;

pub use error::InstallError;

pub type Result<T> = std::result::Result<T, InstallError>;

pub const REPO: &str = "snowzhaozhj/claude-devtools-rs";

pub const DEFAULT_DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);
pub const DEFAULT_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadErrorKind {
    Timeout,
    Dns,
    Connection,
    RateLimit,
    NotFound,
    Forbidden,
    Other,
}

pub fn classify_download_error(raw: &str) -> DownloadErrorKind {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") || lower.contains("deadline") {
        DownloadErrorKind::Timeout
    } else if lower.contains("dns")
        || lower.contains("failed to lookup")
        || lower.contains("name resolution")
        || lower.contains("no such host")
    {
        DownloadErrorKind::Dns
    } else if lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("failed to connect")
        || lower.contains("error sending request")
        || lower.contains("network")
        || lower.contains("tls")
        || lower.contains("certificate")
    {
        DownloadErrorKind::Connection
    } else if lower.contains("rate limit") {
        DownloadErrorKind::RateLimit
    } else if lower.contains("http 404") || lower.contains("download failed: http 404") {
        DownloadErrorKind::NotFound
    } else if lower.contains("http 403") || lower.contains("forbidden") {
        DownloadErrorKind::Forbidden
    } else {
        DownloadErrorKind::Other
    }
}

pub fn platform_asset_name() -> Result<String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    let name = match (os, arch) {
        ("macos", "aarch64") => "cdt-darwin-arm64.tar.gz",
        ("macos", "x86_64") => "cdt-darwin-x64.tar.gz",
        ("linux", "x86_64") => "cdt-linux-x64.tar.gz",
        ("windows", "x86_64") => "cdt-windows-x64.zip",
        _ => return Err(InstallError::UnsupportedPlatform { os, arch }),
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
        .map_err(|e| InstallError::network(format!("failed to download {url}"), e))?;

    if !resp.status().is_success() {
        return Err(InstallError::HttpStatus {
            status: resp.status(),
            url: url.to_string(),
        });
    }

    let expected_len = resp.content_length();
    let archive_bytes = resp
        .bytes()
        .await
        .map_err(|e| InstallError::network("failed to read response body", e))?;

    if let Some(expected) = expected_len {
        if archive_bytes.len() as u64 != expected {
            return Err(InstallError::Download(format!(
                "incomplete download: got {} bytes, expected {expected}",
                archive_bytes.len()
            )));
        }
    }

    if archive_bytes.is_empty() {
        return Err(InstallError::Download("downloaded file is empty".into()));
    }

    if asset_name.ends_with(".tar.gz") {
        extract_tar_gz(&archive_bytes)
    } else if Path::new(asset_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        extract_zip(&archive_bytes)
    } else {
        Err(InstallError::Archive(format!(
            "unknown archive format: {asset_name}"
        )))
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
        .map_err(|e| InstallError::network(format!("failed to download {url}"), e))?;

    if !resp.status().is_success() {
        return Err(InstallError::HttpStatus {
            status: resp.status(),
            url: url.to_string(),
        });
    }

    let expected_len = resp.content_length();
    let archive_bytes = resp
        .bytes()
        .await
        .map_err(|e| InstallError::network("failed to read response body", e))?;

    if let Some(expected) = expected_len {
        if archive_bytes.len() as u64 != expected {
            return Err(InstallError::Download(format!(
                "incomplete download: got {} bytes, expected {expected}",
                archive_bytes.len()
            )));
        }
    }

    if archive_bytes.is_empty() {
        return Err(InstallError::Download("downloaded file is empty".into()));
    }

    if asset_name.ends_with(".tar.gz") {
        extract_tar_gz(&archive_bytes)
    } else if Path::new(asset_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        extract_zip(&archive_bytes)
    } else {
        Err(InstallError::Archive(format!(
            "unknown archive format: {asset_name}"
        )))
    }
}

pub fn extract_tar_gz(data: &[u8]) -> Result<Vec<u8>> {
    let decoder = flate2::read::GzDecoder::new(data);
    let mut archive = tar::Archive::new(decoder);

    let binary_name = if cfg!(windows) { "cdt.exe" } else { "cdt" };

    for entry in archive
        .entries()
        .map_err(|e| InstallError::Archive(format!("failed to read tar entries: {e}")))?
    {
        let mut entry =
            entry.map_err(|e| InstallError::Archive(format!("corrupt tar entry: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| InstallError::Archive(format!("invalid path in tar entry: {e}")))?;

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
            entry.read_to_end(&mut buf).map_err(|e| {
                InstallError::Archive(format!("failed to read binary from archive: {e}"))
            })?;
            if buf.len() < 1024 {
                return Err(InstallError::Archive(format!(
                    "extracted binary too small ({} bytes), likely corrupted",
                    buf.len()
                )));
            }
            return Ok(buf);
        }
    }

    Err(InstallError::Archive(format!(
        "binary '{binary_name}' not found in archive"
    )))
}

pub fn extract_zip(data: &[u8]) -> Result<Vec<u8>> {
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| InstallError::Archive(format!("failed to read zip archive: {e}")))?;

    let binary_name = if cfg!(windows) { "cdt.exe" } else { "cdt" };

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| InstallError::Archive(format!("failed to read zip entry: {e}")))?;
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
            file.read_to_end(&mut buf).map_err(|e| {
                InstallError::Archive(format!("failed to read binary from zip: {e}"))
            })?;
            if buf.len() < 1024 {
                return Err(InstallError::Archive(format!(
                    "extracted binary too small ({} bytes), likely corrupted",
                    buf.len()
                )));
            }
            return Ok(buf);
        }
    }

    Err(InstallError::Archive(format!(
        "binary '{binary_name}' not found in zip archive"
    )))
}

pub fn validate_binary_magic(data: &[u8]) -> Result<()> {
    if data.len() < 4 {
        return Err(InstallError::Validation(
            "binary too small to validate".into(),
        ));
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
        return Err(InstallError::Validation(
            "downloaded file is not a valid executable for this platform".into(),
        ));
    }
    Ok(())
}

pub fn validate_binary_arch(data: &[u8]) -> Result<()> {
    let current_arch = env::consts::ARCH;
    let current_os = env::consts::OS;

    let magic = if data.len() >= 4 {
        [data[0], data[1], data[2], data[3]]
    } else {
        return Err(InstallError::Validation(
            "binary too small to validate architecture".into(),
        ));
    };

    match magic {
        [0xfe, 0xed, 0xfa, 0xce | 0xcf] => validate_macho_arch(data, current_arch, false),
        [0xce | 0xcf, 0xfa, 0xed, 0xfe] => validate_macho_arch(data, current_arch, true),
        [0xca, 0xfe, 0xba, 0xbe] => validate_fat_macho_arch(data, current_arch),
        [0x7f, b'E', b'L', b'F'] => validate_elf_arch(data, current_arch),
        [b'M', b'Z', ..] => validate_pe_arch(data, current_arch),
        _ => Err(InstallError::Validation(format!(
            "architecture mismatch: unrecognized binary format for {current_os}"
        ))),
    }
}

const MACHO_CPU_TYPE_X86_64: u32 = 0x0100_0007;
const MACHO_CPU_TYPE_ARM64: u32 = 0x0100_000C;
const ELF_EM_X86_64: u16 = 62;
const ELF_EM_AARCH64: u16 = 183;
const PE_MACHINE_AMD64: u16 = 0x8664;

fn expected_macho_cputype(arch: &str) -> Option<u32> {
    match arch {
        "x86_64" => Some(MACHO_CPU_TYPE_X86_64),
        "aarch64" => Some(MACHO_CPU_TYPE_ARM64),
        _ => None,
    }
}

fn validate_macho_arch(data: &[u8], arch: &str, little_endian: bool) -> Result<()> {
    if data.len() < 8 {
        return Err(InstallError::Validation("Mach-O header truncated".into()));
    }
    let cputype = if little_endian {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]])
    } else {
        u32::from_be_bytes([data[4], data[5], data[6], data[7]])
    };

    let Some(expected) = expected_macho_cputype(arch) else {
        return Ok(());
    };

    if cputype != expected {
        let actual_name = match cputype {
            MACHO_CPU_TYPE_X86_64 => "x86_64",
            MACHO_CPU_TYPE_ARM64 => "arm64",
            _ => "unknown",
        };
        return Err(InstallError::Validation(format!(
            "architecture mismatch: binary is for {actual_name}, but this system is {arch}"
        )));
    }
    Ok(())
}

fn validate_fat_macho_arch(data: &[u8], arch: &str) -> Result<()> {
    if data.len() < 8 {
        return Err(InstallError::Validation(
            "fat Mach-O header truncated".into(),
        ));
    }
    let Some(expected) = expected_macho_cputype(arch) else {
        return Ok(());
    };

    let nfat = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if nfat > 20 {
        return Err(InstallError::Validation(format!(
            "fat Mach-O header has too many architectures ({nfat}), likely corrupted"
        )));
    }

    let mut offset = 8usize;
    for _ in 0..nfat {
        if offset + 4 > data.len() {
            return Err(InstallError::Validation(
                "fat Mach-O arch entry truncated".into(),
            ));
        }
        let cputype = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        if cputype == expected {
            return Ok(());
        }
        offset += 20;
    }

    Err(InstallError::Validation(format!(
        "architecture mismatch: universal binary does not contain a slice for {arch}"
    )))
}

fn validate_elf_arch(data: &[u8], arch: &str) -> Result<()> {
    if data.len() < 20 {
        return Err(InstallError::Validation("ELF header truncated".into()));
    }
    let little_endian = match data[5] {
        1 => true,
        2 => false,
        _ => {
            return Err(InstallError::Validation(
                "invalid ELF endianness indicator".into(),
            ));
        }
    };
    let e_machine = if little_endian {
        u16::from_le_bytes([data[18], data[19]])
    } else {
        u16::from_be_bytes([data[18], data[19]])
    };

    let expected = match arch {
        "x86_64" => ELF_EM_X86_64,
        "aarch64" => ELF_EM_AARCH64,
        _ => return Ok(()),
    };

    if e_machine != expected {
        let actual_name = match e_machine {
            ELF_EM_X86_64 => "x86_64",
            ELF_EM_AARCH64 => "aarch64",
            _ => "unknown",
        };
        return Err(InstallError::Validation(format!(
            "architecture mismatch: binary is for {actual_name}, but this system is {arch}"
        )));
    }
    Ok(())
}

fn validate_pe_arch(data: &[u8], arch: &str) -> Result<()> {
    if data.len() < 64 {
        return Err(InstallError::Validation("PE header truncated".into()));
    }
    let pe_offset = u32::from_le_bytes([data[60], data[61], data[62], data[63]]) as usize;
    let pe_end = pe_offset
        .checked_add(6)
        .ok_or_else(|| InstallError::Validation("PE offset overflow".into()))?;
    if pe_end > data.len() {
        return Err(InstallError::Validation("PE header truncated".into()));
    }
    if data[pe_offset..pe_offset + 4] != [b'P', b'E', 0, 0] {
        return Err(InstallError::Validation("invalid PE signature".into()));
    }
    let machine = u16::from_le_bytes([data[pe_offset + 4], data[pe_offset + 5]]);

    let expected = match arch {
        "x86_64" => PE_MACHINE_AMD64,
        _ => return Ok(()),
    };

    if machine != expected {
        return Err(InstallError::Validation(
            "architecture mismatch: binary is for a different CPU architecture".into(),
        ));
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
        headers.insert("Authorization", val.parse()?);
    }

    let mut builder = reqwest::Client::builder()
        .default_headers(headers)
        .redirect(redirect)
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT);

    if let Some(t) = timeout {
        builder = builder.timeout(t);
    }

    builder
        .build()
        .map_err(|e| InstallError::network("failed to build HTTP client", e))
}

#[cfg(test)]
mod tests {
    use super::{
        DownloadErrorKind, classify_download_error, validate_binary_arch, validate_binary_magic,
    };

    #[test]
    fn classifies_timeout_errors() {
        assert_eq!(
            classify_download_error("Operation timed out (os error 60)"),
            DownloadErrorKind::Timeout
        );
        assert_eq!(
            classify_download_error("request deadline exceeded"),
            DownloadErrorKind::Timeout
        );
    }

    #[test]
    fn classifies_dns_errors() {
        assert_eq!(
            classify_download_error("failed to lookup address"),
            DownloadErrorKind::Dns
        );
        assert_eq!(
            classify_download_error("no such host is known"),
            DownloadErrorKind::Dns
        );
    }

    #[test]
    fn classifies_connection_errors() {
        assert_eq!(
            classify_download_error("connection refused"),
            DownloadErrorKind::Connection
        );
        assert_eq!(
            classify_download_error("error sending request for url"),
            DownloadErrorKind::Connection
        );
        assert_eq!(
            classify_download_error("network is unreachable"),
            DownloadErrorKind::Connection
        );
        assert_eq!(
            classify_download_error("tls handshake failure"),
            DownloadErrorKind::Connection
        );
    }

    #[test]
    fn classifies_rate_limit() {
        assert_eq!(
            classify_download_error("GitHub API rate limit exceeded"),
            DownloadErrorKind::RateLimit
        );
    }

    #[test]
    fn classifies_http_404_but_not_archive_not_found() {
        assert_eq!(
            classify_download_error("download failed: HTTP 404 Not Found"),
            DownloadErrorKind::NotFound
        );
        assert_eq!(
            classify_download_error("binary 'cdt' not found in archive"),
            DownloadErrorKind::Other
        );
        assert_eq!(
            classify_download_error("binary 'cdt.exe' not found in zip archive"),
            DownloadErrorKind::Other
        );
    }

    #[test]
    fn classifies_forbidden() {
        assert_eq!(
            classify_download_error("HTTP 403 Forbidden"),
            DownloadErrorKind::Forbidden
        );
    }

    #[test]
    fn unknown_errors_classify_as_other() {
        assert_eq!(
            classify_download_error("some completely unknown error"),
            DownloadErrorKind::Other
        );
    }

    #[test]
    fn validate_magic_rejects_without_hex_leak() {
        let bad = b"not a binary at all!!!";
        let err = validate_binary_magic(bad).unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains("6e 6f"), "hex bytes leaked in error: {msg}");
        assert!(msg.contains("not a valid executable"));
    }

    #[test]
    fn validate_arch_accepts_current_platform_macho_arm64() {
        let mut data = vec![0xcf, 0xfa, 0xed, 0xfe];
        data.extend_from_slice(&0x0100_000Cu32.to_le_bytes());
        data.extend_from_slice(&[0; 24]);
        let result = validate_binary_arch(&data);
        if cfg!(target_arch = "aarch64") {
            assert!(result.is_ok(), "should accept arm64 on arm64: {result:?}");
        } else {
            assert!(result.is_err(), "should reject arm64 on non-arm64");
        }
    }

    #[test]
    fn validate_arch_accepts_current_platform_macho_x86_64() {
        let mut data = vec![0xcf, 0xfa, 0xed, 0xfe];
        data.extend_from_slice(&0x0100_0007u32.to_le_bytes());
        data.extend_from_slice(&[0; 24]);
        let result = validate_binary_arch(&data);
        if cfg!(target_arch = "x86_64") {
            assert!(result.is_ok(), "should accept x86_64 on x86_64: {result:?}");
        } else {
            assert!(result.is_err(), "should reject x86_64 on non-x86_64");
        }
    }

    #[test]
    fn validate_arch_rejects_truncated() {
        let data = vec![0xcf, 0xfa, 0xed];
        assert!(validate_binary_arch(&data).is_err());
    }

    #[test]
    fn validate_arch_fat_macho_with_matching_slice() {
        let mut data = vec![0xca, 0xfe, 0xba, 0xbe];
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&0x0100_000Cu32.to_be_bytes());
        data.extend_from_slice(&[0; 16]);
        let result = validate_binary_arch(&data);
        if cfg!(target_arch = "aarch64") {
            assert!(
                result.is_ok(),
                "fat with arm64 slice should pass on arm64: {result:?}"
            );
        } else {
            assert!(
                result.is_err(),
                "fat with only arm64 should fail on non-arm64"
            );
        }
    }

    #[test]
    fn validate_arch_elf_x86_64() {
        let mut data = vec![0x7f, b'E', b'L', b'F'];
        data.push(2);
        data.push(1);
        data.extend_from_slice(&[0; 12]);
        data.extend_from_slice(&62u16.to_le_bytes());
        let result = validate_binary_arch(&data);
        if cfg!(target_arch = "x86_64") {
            assert!(
                result.is_ok(),
                "ELF x86_64 should pass on x86_64: {result:?}"
            );
        } else {
            assert!(result.is_err(), "ELF x86_64 should fail on non-x86_64");
        }
    }

    #[test]
    fn validate_arch_error_no_hex_leak() {
        let mut data = vec![0xcf, 0xfa, 0xed, 0xfe];
        data.extend_from_slice(&0x0100_0007u32.to_le_bytes());
        data.extend_from_slice(&[0; 24]);
        if cfg!(target_arch = "aarch64") {
            let err = validate_binary_arch(&data).unwrap_err();
            let msg = err.to_string();
            assert!(!msg.contains("0x"), "hex leaked in arch error: {msg}");
            assert!(msg.contains("architecture mismatch"));
        }
    }
}
