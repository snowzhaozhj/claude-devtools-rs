//! File system watching with 100ms debounce.
//!
//! Owns the **file-watching** capability
//! (`openspec/specs/file-watching/spec.md`). Watches `~/.claude/projects/`
//! for `.jsonl` changes and `~/.claude/todos/` for `.json` changes,
//! broadcasts coalesced events to all subscribers (IPC renderer + SSE HTTP
//! clients), survives transient permission errors without terminating the
//! watcher.
//!
//! Debounce window is exactly 100ms — baseline invariant.
//!
//! Port status: **stub**.

/// Placeholder — replaced during `port-file-watching`.
pub fn stub() {}
