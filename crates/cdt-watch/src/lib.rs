//! 文件系统监听——100ms 去抖 + 多订阅者广播。
//!
//! Owns the **file-watching** capability
//! (`openspec/specs/file-watching/spec.md`)。监听 `~/.claude/projects/`
//! 下的 `.jsonl` 变更和 `~/.claude/todos/` 下的 `.json` 变更，
//! 经 100ms 去抖后向所有订阅者广播事件。
//!
//! 瞬时文件系统错误（权限拒绝、临时锁定）记录 warning 后继续运行。

pub mod error;
pub mod watcher;

pub use error::WatchError;
pub use watcher::FileWatcher;
