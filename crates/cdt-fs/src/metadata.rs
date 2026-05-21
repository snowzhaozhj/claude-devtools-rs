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
    pub identity: Option<FsIdentity>,
}

impl FsMetadata {
    /// mtime 折算成 epoch 毫秒——对齐 TS 侧 `Project.mostRecentSession` 类型。
    #[must_use]
    pub fn mtime_ms(&self) -> i64 {
        self.mtime
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_millis()).ok())
            .unwrap_or(0)
    }
}
