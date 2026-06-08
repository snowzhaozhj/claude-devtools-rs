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
//!
//! 桥接 `cdt_fs::FsMetadata`（change `unify-fs-abstraction`）：
//! - `FileSignature::from_fs_metadata` 是首选构造路径，覆盖 local / ssh / 未来
//!   远端 backend 统一来源
//! - `FileSignature::from_metadata(&std::fs::Metadata)` 是过渡期兼容路径，
//!   PR-D 清除所有 `tokio::fs::metadata` 直调后将移除

use std::fs::Metadata;
use std::time::SystemTime;

use cdt_fs::{FsIdentity as CdtFsIdentity, FsMetadata as CdtFsMetadata};

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
    /// 但仍需保留以让跨 cfg 测试代码（如 `dummy_sig`）能引用；
    /// SSH backend 在 unify-fs-abstraction change 后也会构造此 variant。
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

    /// 从 `cdt_fs::FsIdentity` 桥接 —— `None`（SSH / Windows）→ `FileIdentity::None`，
    /// `Unix { dev, ino }` → `FileIdentity::Unix { dev, ino }`。
    #[must_use]
    pub fn from_fs_identity(identity: Option<CdtFsIdentity>) -> Self {
        match identity {
            #[cfg(unix)]
            Some(CdtFsIdentity::Unix { dev, ino }) => Self::Unix { dev, ino },
            _ => Self::None,
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
    ///
    /// **deprecated**：unify-fs-abstraction change 后 SHALL 改用
    /// [`FileSignature::from_fs_metadata`]，PR-D 完成 callsite 迁移后本路径
    /// 移除。本 PR-A 期间仍保留以避免业务代码扩散性破坏。
    #[deprecated(
        since = "0.5.6",
        note = "请改用 FileSignature::from_fs_metadata（基于 cdt_fs::FsMetadata），本路径将随 PR-D 移除"
    )]
    pub fn from_metadata(meta: &Metadata) -> Self {
        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        Self {
            mtime,
            size: meta.len(),
            identity: FileIdentity::from_metadata(meta),
        }
    }

    /// 从 `cdt_fs::FsMetadata` 构造签名 —— 覆盖 local / ssh / 未来远端 backend
    /// 统一来源。`identity` 桥接：`Unix { dev, ino }` 透传；`None`（SSH / Windows）
    /// 落到 [`FileIdentity::None`]，best-effort 维度退化为 mtime+size 比对。
    ///
    /// Spec：`openspec/specs/fs-abstraction/spec.md`
    /// §`FsMetadata.identity 字段采 best-effort 策略`。
    #[must_use]
    pub fn from_fs_metadata(meta: &CdtFsMetadata) -> Self {
        Self {
            mtime: meta.mtime,
            size: meta.size,
            identity: FileIdentity::from_fs_identity(meta.identity),
        }
    }
}

#[cfg(test)]
#[allow(deprecated)]
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

    #[test]
    fn from_fs_metadata_matches_from_metadata_for_local_file() {
        // 同一文件走两条路径构造的 FileSignature SHALL byte-equal：
        // - from_metadata(&std::fs::Metadata) → 老路径
        // - from_fs_metadata(&cdt_fs::FsMetadata) → 新路径（基于
        //   LocalFileSystemProvider::stat 的产物）
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a.jsonl");
        std::fs::write(&p, b"hello\n").unwrap();

        let std_meta = std::fs::metadata(&p).unwrap();
        let sig_old = FileSignature::from_metadata(&std_meta);

        let mtime = std_meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            Some(CdtFsIdentity::Unix {
                dev: std_meta.dev(),
                ino: std_meta.ino(),
            })
        };
        #[cfg(not(unix))]
        let identity = None;
        let fs_meta = CdtFsMetadata {
            size: std_meta.len(),
            mtime,
            created: None,
            identity,
        };
        let sig_new = FileSignature::from_fs_metadata(&fs_meta);

        assert_eq!(sig_old, sig_new, "两条路径产物 SHALL byte-equal");
    }

    #[cfg(unix)]
    #[test]
    fn from_fs_metadata_unix_identity_bridges_to_file_identity_unix() {
        // codex 二审 L1：手工构造 Unix identity 显式断言桥接精确无损
        let mtime = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let fs_meta = CdtFsMetadata {
            size: 1024,
            mtime,
            created: None,
            identity: Some(CdtFsIdentity::Unix { dev: 42, ino: 9999 }),
        };
        let sig = FileSignature::from_fs_metadata(&fs_meta);
        assert_eq!(sig.size, 1024);
        assert_eq!(sig.mtime, mtime);
        assert_eq!(sig.identity, FileIdentity::Unix { dev: 42, ino: 9999 });
    }

    #[test]
    fn from_fs_metadata_with_ssh_style_none_identity_yields_none_variant() {
        // SSH backend 永远填 identity: None；FileSignature.identity 必须落到
        // FileIdentity::None，依靠 mtime + size 完成 best-effort 等价性判定。
        let mtime = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let fs_meta = CdtFsMetadata {
            size: 4096,
            mtime,
            created: None,
            identity: None,
        };
        let sig = FileSignature::from_fs_metadata(&fs_meta);
        assert_eq!(sig.size, 4096);
        assert_eq!(sig.mtime, mtime);
        assert_eq!(sig.identity, FileIdentity::None);
    }
}
