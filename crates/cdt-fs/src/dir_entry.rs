//! 目录遍历返回的最小结果集合。
//!
//! 刻意不暴露 `std::fs::FileType` —— 保持 trait dyn-safe 与平台中立。

use crate::metadata::FsMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
    Other,
}

impl EntryKind {
    #[must_use]
    pub fn is_file(self) -> bool {
        matches!(self, EntryKind::File)
    }

    #[must_use]
    pub fn is_dir(self) -> bool {
        matches!(self, EntryKind::Dir)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub kind: EntryKind,
    pub metadata: Option<FsMetadata>,
}
