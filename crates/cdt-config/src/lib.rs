//! Configuration, CLAUDE.md reading, notification triggers.
//!
//! Owns two baseline capabilities:
//! - **configuration-management** — persisted app config, CLAUDE.md reads
//!   (8 scopes: enterprise / user / project / project-alt / project-rules /
//!   project-local / user-rules / auto-memory), `@mention` path resolution with
//!   sandboxing, corruption backup on load failure. Port note: the TS
//!   implementation skips the backup step — Rust MUST implement it per spec.
//! - **notification-triggers** — error detection over tool executions,
//!   trigger evaluation (literal + regex with RE2-style safety validation),
//!   historical preview without side effects, persisted read/unread state.

pub mod claude_md;
pub mod defaults;
pub mod error;
pub mod manager;
pub mod mention;
pub mod regex_safety;
pub mod trigger;
pub mod types;
pub mod validation;

pub use claude_md::{ClaudeMdFileInfo, Scope, read_all_claude_md_files, read_directory_claude_md};
pub use error::ConfigError;
pub use manager::ConfigManager;
pub use mention::{read_mentioned_file, validate_file_path};
pub use regex_safety::{create_safe_regex, validate_regex_pattern};
pub use trigger::{TriggerManager, TriggerValidationResult, merge_triggers, validate_trigger};
pub use types::{
    AppConfig, ConfigSection, DisplayConfig, GeneralConfig, HttpServerConfig, NotificationConfig,
    NotificationTrigger, SessionsConfig, SshPersistConfig, TriggerContentType, TriggerMode,
};
pub use validation::{normalize_claude_root_path, validate_http_port, validate_section};
