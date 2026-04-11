//! Discovery and search over Claude Code session data.
//!
//! Owns two baseline capabilities:
//! - **project-discovery** — scan `~/.claude/projects/`, decode encoded
//!   paths, list sessions per project, group by git worktree, track pinned
//!   sessions. Path decoding is best-effort; authoritative cwd comes from
//!   the `cwd` field inside session entries.
//! - **session-search** — in-session, per-project, and cross-project search
//!   with noise exclusion, mtime-aware cache, and staged limits for SSH
//!   contexts.
//!
//! Port status: **stub**.

pub mod projects {
    //! project-discovery capability.
}

pub mod search {
    //! session-search capability.
}
