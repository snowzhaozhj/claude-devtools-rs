//! cdt-fs —— 文件系统抽象层（capability `fs-abstraction`）。
//!
//! 本 crate 是 local / ssh / 未来 http-server / WSL / fake-test 等所有 fs backend
//! 的**唯一**抽象住所。业务 crate（`cdt-api` / `cdt-config` / `cdt-discover`
//! / `cdt-ssh` / `cdt-cli` / `cdt-watch`）SHALL 通过本 crate 拿 fs 抽象，
//! **不得**直接调 `tokio::fs::*`（详见 `openspec/specs/fs-abstraction/spec.md` H1
//! Requirement + `crates/cdt-fs/ALLOWLIST.md` 豁免清单）。
//!
//! Spec：`openspec/specs/fs-abstraction/spec.md`。
//!
//! 公开类型：
//! - [`FileSystemProvider`] —— 所有 fs 操作的 trait seam（dyn-safe）
//! - [`LocalFileSystemProvider`] —— 本地 `tokio::fs` 实现
//! - [`FsKind`] / [`FsMetadata`] / [`FsIdentity`] / [`DirEntry`] / [`EntryKind`] / [`FsError`]
//! - [`ContextId`] / [`HostSignature`] / [`SshConfigDigestInput`] —— cache key 上下文
//! - [`BackendPolicy`] / [`InitialLoadPolicy`] / [`PrefetchPolicy`] —— PR-E 接入用契约锚点
//! - [`InstrumentedFs`] / [`FsOpCounter`] / [`with_fs_counter`] —— hot path 可观测性入口

mod backend_policy;
mod context_id;
mod dir_entry;
mod error;
mod instrumentation;
mod kind;
mod local;
mod metadata;
mod provider;

pub use backend_policy::{BackendPolicy, InitialLoadPolicy, PrefetchPolicy, StaleCheckStrategy};
pub use context_id::{ContextId, HostSignature, SshConfigDigestInput};
pub use dir_entry::{DirEntry, EntryKind};
pub use error::FsError;
pub use instrumentation::{FsOpCounter, FsOpCounts, InstrumentedFs, with_fs_counter};
pub use kind::FsKind;
pub use local::{LocalFileSystemProvider, local_handle};
pub use metadata::{FsIdentity, FsMetadata};
pub use provider::{FileSystemProvider, FsHandle};
