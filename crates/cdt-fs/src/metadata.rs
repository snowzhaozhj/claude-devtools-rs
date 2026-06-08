//! `stat` 返回的跨平台 metadata + 文件身份维度。
//!
//! `identity` 字段是 best-effort —— 详 spec `fs-abstraction/spec.md`
//! §`FsMetadata.identity` 字段采 best-effort 策略。Local Unix 填
//! `Some(Unix { dev, ino })`；Local Windows 与所有 SSH 场景填 `None`。
//!
//! cache 用 identity 做"两次 stat 是否同一文件实体"判断时 SHALL 当作辅助
//! 维度，主签名仍是 mtime + size——`None` 不算 cache miss，回退到 mtime/size
//! 比较。

use std::time::{SystemTime, UNIX_EPOCH};

/// 文件身份维度——Unix 上是 `(dev, ino)`；其它平台退化为 `None` variant。
///
/// Windows 退化原因：`std::os::windows::fs::MetadataExt::file_index()` /
/// `volume_serial_number()` 是 unstable feature `windows_by_handle`，stable
/// Rust 不可用；引入 windows-sys 的 unsafe FFI 成本超过 Windows 平台 inode
/// 检测的边际收益。
///
/// SSH 退化原因：SFTP 协议不暴露 inode 等价物。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsIdentity {
    #[cfg(unix)]
    Unix { dev: u64, ino: u64 },
    /// Windows 与所有 SSH 场景共享的退化 variant。
    ///
    /// `allow(dead_code)`：Unix 平台上此 variant 不被 `LocalFileSystemProvider`
    /// 构造，但仍需保留以让跨 cfg 测试代码能引用，以及 SSH provider 使用。
    #[allow(dead_code)]
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsMetadata {
    pub size: u64,
    pub mtime: SystemTime,
    pub created: Option<SystemTime>,
    pub identity: Option<FsIdentity>,
}

impl FsMetadata {
    #[must_use]
    pub fn mtime_ms(&self) -> i64 {
        Self::system_time_to_ms(self.mtime)
    }

    /// `min(created, mtime)` 的 epoch 毫秒。`created = None` 时 fallback 到 mtime。
    #[must_use]
    pub fn created_ms(&self) -> i64 {
        match self.created {
            Some(c) => std::cmp::min(Self::system_time_to_ms(c), self.mtime_ms()),
            None => self.mtime_ms(),
        }
    }

    fn system_time_to_ms(t: SystemTime) -> i64 {
        t.duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_millis()).ok())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn meta(mtime_secs: u64, created: Option<u64>) -> FsMetadata {
        FsMetadata {
            size: 100,
            mtime: UNIX_EPOCH + Duration::from_secs(mtime_secs),
            created: created.map(|s| UNIX_EPOCH + Duration::from_secs(s)),
            identity: None,
        }
    }

    #[test]
    fn created_ms_returns_created_when_before_mtime() {
        let m = meta(2000, Some(1000));
        assert_eq!(m.created_ms(), 1_000_000);
        assert_eq!(m.mtime_ms(), 2_000_000);
    }

    #[test]
    fn created_ms_fallback_to_mtime_when_none() {
        let m = meta(2000, None);
        assert_eq!(m.created_ms(), m.mtime_ms());
    }

    #[test]
    fn created_ms_normalizes_when_created_after_mtime() {
        let m = meta(1000, Some(2000));
        assert_eq!(
            m.created_ms(),
            1_000_000,
            "should return min(created, mtime)"
        );
    }
}
