//! 远端 SFTP polling watcher（3s 间隔 + 30s catch-up timer）。
//!
//! Spec：`openspec/specs/ssh-remote-context/spec.md` `Requirement: Watch remote project
//! directories via SFTP polling`。
//!
//! Phase A 仅占位骨架——`RemotePollingWatcher::spawn` 与 `FileFingerprint` 类型在
//! Phase B（task 6.x）填入真实实现，与 `cdt-watch` 接入。

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// 远端文件指纹 baseline 条目：size + mtime（mtime 缺失时退化为 size-only）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFingerprint {
    pub size: u64,
    pub mtime: Option<SystemTime>,
}

/// 主 polling 周期（design.md D5）。
pub const POLL_INTERVAL: Duration = Duration::from_secs(3);

/// catch-up 兜底周期（design.md D5）。
pub const CATCH_UP_INTERVAL: Duration = Duration::from_secs(30);

/// 取消 token 退出最大延迟（spec Scenario "Polling stops on disconnect"）。
pub const CANCEL_DEADLINE: Duration = Duration::from_secs(1);

/// `RemotePollingWatcher` handle —— Phase B 填入真实 spawn。
#[derive(Debug)]
pub struct RemotePollingWatcher {
    pub remote_projects_root: PathBuf,
}
