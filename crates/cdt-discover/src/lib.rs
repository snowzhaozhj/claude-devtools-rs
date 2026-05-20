//! Discovery and search over Claude Code session data.
//!
//! 本 crate 拥有两个 baseline capability：
//! - **project-discovery** — 扫描 `~/.claude/projects/`、解码编码路径、
//!   列出每个项目的 session、按 git worktree 分组、追踪 subproject。路径
//!   解码是 best-effort；真实 cwd 从 session 文件中的 `cwd` 字段恢复。
//! - **session-search** — 三级搜索 scope + mtime LRU 缓存 + SSH 分阶段限制。
//!
//! Spec：`openspec/specs/project-discovery/spec.md`、`openspec/specs/session-search/spec.md`。

pub mod agent_configs;
pub mod error;
pub mod fs_provider;
pub mod path_compare;
pub mod path_decoder;
pub mod project_path_resolver;
pub mod project_scanner;
pub mod search_cache;
pub mod search_extract;
pub mod session_search;
pub mod worktree_grouper;
pub mod wsl;

pub use error::{DiscoverError, FsError};
pub use fs_provider::{
    DirEntry, EntryKind, FileSystemProvider, FsHandle, FsKind, FsMetadata, LocalFileSystemProvider,
    local_handle,
};
pub use path_compare::{
    normalize_path_for_compare, normalize_path_string_for_compare, path_starts_with,
    path_strip_prefix, paths_equal,
};
pub use path_decoder::{
    decode_path, encode_path, extract_base_dir, extract_project_name, get_projects_base_path,
    get_todos_base_path, home_dir, is_valid_encoded_path, looks_like_absolute_path,
};
pub use project_path_resolver::ProjectPathResolver;
pub use project_scanner::ProjectScanner;
pub use search_cache::SearchTextCache;
pub use session_search::{SearchConfig, SessionSearcher};
pub use worktree_grouper::{
    GitIdentityResolver, LocalGitIdentityResolver, RepoLookup, WorktreeGrouper,
};
pub use wsl::{WslDistroCandidate, WslDistroScanReport};
