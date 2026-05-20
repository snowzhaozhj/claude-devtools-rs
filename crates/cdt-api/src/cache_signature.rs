//! 文件身份签名 —— 跨平台 (mtime + size + identity) 三元组。
//!
//! 用于 notifier / metadata 缓存判定"文件是否真有变化"。`FileSignature` 字段
//! byte-equal 视为命中；任一字段不一致走 cache miss。
//!
//! `identity` 维度（仅 Unix 上为 `(dev, ino)`；Windows 与其它平台退化为
//! `None`）用于检出 `inode` 变化（典型：rename 替换文件）。详见 change
//! `multi-session-cpu-cache` design D1b/D1d/D1f：
//! - 等价性是 best-effort，inode reuse 同时撞 mtime/size 的极端场景由后续
//!   file-change 自然恢复
//! - Windows 上 `std::os::windows::fs::MetadataExt::file_index()` /
//!   `volume_serial_number()` 是 unstable feature `windows_by_handle`，stable
//!   Rust 不可用；退化为 `None` 让 Windows 仅依赖 mtime+size（D1f 修订）

use cdt_discover::FsMetadata;
use std::fs::Metadata;
use std::time::SystemTime;

/// 文件身份维度 —— Unix 上是 `(dev, ino)`；Windows 与其它平台退化为 `None`。
///
/// Windows 退化原因：`std::os::windows::fs::MetadataExt::file_index()` /
/// `volume_serial_number()` 是 unstable feature `windows_by_handle`，stable
/// Rust 不可用。引入 windows-sys + `GetFileInformationByHandle` 的成本（unsafe
/// FFI + 新依赖）超过 Windows 上 inode 检测的边际收益（详 design D1d/D1f）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileIdentity {
    #[cfg(unix)]
    Unix { dev: u64, ino: u64 },
    /// Windows 与其它平台共享的退化 variant —— identity 维度不参与签名比对，
    /// 等价性仅由 mtime+size 决定（best-effort）。
    /// `allow(dead_code)`：在 Unix 平台上此 variant 不被 `from_metadata` 构造，
    /// 但仍需保留以让跨 cfg 测试代码（如 `dummy_sig`）能引用。
    #[allow(dead_code)]
    None,
}

impl FileIdentity {
    /// 从 `Metadata` 提取平台对应的 identity。
    pub fn from_metadata(meta: &Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Self::Unix {
                dev: meta.dev(),
                ino: meta.ino(),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = meta;
            Self::None
        }
    }
}

/// 文件签名 —— mtime + size + identity 的组合。
///
/// `PartialEq` byte-equal 即视为"文件在常规 append-only 写入路径下未变"。
/// 等价性是 best-effort：详 change `multi-session-cpu-cache` design D1d。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileSignature {
    pub mtime: SystemTime,
    pub size: u64,
    pub identity: FileIdentity,
}

impl FileSignature {
    /// 从 `std::fs::Metadata` 构造签名。`mtime` 取 `modified()`，失败时退化
    /// 为 `UNIX_EPOCH`（保守判定，让缓存走 miss）。
    pub fn from_metadata(meta: &Metadata) -> Self {
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        Self {
            mtime,
            size: meta.len(),
            identity: FileIdentity::from_metadata(meta),
        }
    }

    /// 从 `cdt_discover::FsMetadata` 构造签名。SSH 远端 stat 不带 inode，
    /// `identity` 退化为 `None`（与 Windows 同处理）——等价性仅靠 mtime+size
    /// 判定，足以覆盖 append-only JSONL 的常规变化路径。
    ///
    /// `mtime` **截断到毫秒**——SSH 路径下骨架阶段从 `Session.last_modified`
    /// (epoch ms `i64`) 还原签名 vs 后台扫描走 `fs.stat` 拿到的完整 `SystemTime`
    /// (可能 sub-ms 精度) 这两路必须 byte-equal 才能命中缓存，统一规范化到
    /// ms 精度可避免"系统性 cache miss"——codex 二审 PR #178 建议修 1。
    pub fn from_fs_metadata(meta: &FsMetadata) -> Self {
        Self {
            mtime: truncate_to_ms(meta.mtime),
            size: meta.size,
            identity: FileIdentity::None,
        }
    }
}

/// 把 `SystemTime` 截断到毫秒精度——丢弃 sub-ms 余数，让来自不同精度源的
/// 同一时刻能 byte-equal 比较。`Duration::new(secs, nanos)` 的 `nanos` 必须
/// 小于 `1e9`，`subsec_millis() * 1_000_000` 落在 `0..999_000_000` 安全。
fn truncate_to_ms(t: SystemTime) -> SystemTime {
    let Ok(d) = t.duration_since(SystemTime::UNIX_EPOCH) else {
        return SystemTime::UNIX_EPOCH;
    };
    let secs = d.as_secs();
    let nanos_from_ms = d.subsec_millis() * 1_000_000;
    SystemTime::UNIX_EPOCH + std::time::Duration::new(secs, nanos_from_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn meta_for(path: &std::path::Path) -> Metadata {
        std::fs::metadata(path).expect("metadata")
    }

    #[test]
    fn same_file_yields_equal_signature() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a.jsonl");
        std::fs::write(&p, b"line1\n").unwrap();
        let s1 = FileSignature::from_metadata(&meta_for(&p));
        let s2 = FileSignature::from_metadata(&meta_for(&p));
        assert_eq!(s1, s2);
    }

    #[test]
    fn appending_changes_size_so_signature_differs() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a.jsonl");
        std::fs::write(&p, b"line1\n").unwrap();
        let s1 = FileSignature::from_metadata(&meta_for(&p));

        // append 让 size 变化（即便 mtime 巧合相同 size 也变了）
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b"line2\n").unwrap();
        f.sync_all().unwrap();
        let s2 = FileSignature::from_metadata(&meta_for(&p));

        assert_ne!(s1, s2, "size 变化必须让签名不同");
        assert_ne!(s1.size, s2.size);
    }

    #[test]
    fn truncate_makes_size_smaller_so_signature_differs() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a.jsonl");
        std::fs::write(&p, b"line1\nline2\n").unwrap();
        let s1 = FileSignature::from_metadata(&meta_for(&p));

        std::fs::write(&p, b"x\n").unwrap();
        let s2 = FileSignature::from_metadata(&meta_for(&p));

        assert_ne!(s1, s2);
        assert!(s2.size < s1.size);
    }

    #[cfg(unix)]
    #[test]
    fn rename_replace_changes_inode_so_signature_differs() {
        // file A 被 rename 替换：identity 维度（dev, ino）必然不同
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a.jsonl");
        std::fs::write(&p, b"original\n").unwrap();
        let s1 = FileSignature::from_metadata(&meta_for(&p));

        // 准备替换文件，写入相同尺寸
        let replacement = tmp.path().join("a.replace");
        std::fs::write(&replacement, b"original\n").unwrap();
        std::fs::rename(&replacement, &p).unwrap();

        let s2 = FileSignature::from_metadata(&meta_for(&p));
        // identity（inode）必然不同；即便 size 相同也应让签名不同
        assert_ne!(s1.identity, s2.identity, "rename 替换后 inode 必须不同");
        assert_ne!(s1, s2);
    }

    #[cfg(unix)]
    #[test]
    fn different_files_have_different_identity() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.jsonl");
        let b = tmp.path().join("b.jsonl");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();

        let sa = FileSignature::from_metadata(&meta_for(&a));
        let sb = FileSignature::from_metadata(&meta_for(&b));
        assert_ne!(sa.identity, sb.identity);
    }
}
