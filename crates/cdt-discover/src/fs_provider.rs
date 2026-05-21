//! `FileSystemProvider` 真相源住在 `cdt-fs` crate（capability `fs-abstraction`）。
//!
//! 本文件保留作为兼容 re-export shim —— 历史 import 路径 `use cdt_discover::fs_provider::*`
//! 仍然工作。新代码 SHOULD 直接 `use cdt_fs::*`。
//!
//! 设计：`openspec/changes/unify-fs-abstraction/design.md` D2。

pub use cdt_fs::{
    DirEntry, EntryKind, FileSystemProvider, FsHandle, FsIdentity, FsKind, FsMetadata,
    LocalFileSystemProvider, local_handle,
};
