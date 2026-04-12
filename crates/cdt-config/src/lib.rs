//! Configuration, CLAUDE.md reading, notification triggers.
//!
//! Owns two baseline capabilities:
//! - **configuration-management** — persisted app config, CLAUDE.md reads
//!   (8 scopes: enterprise / user / project / project-alt / project-rules /
//!   project-local / user-rules / auto-memory), `@mention` path resolution with
//!   sandboxing, corruption backup on load failure.
//! - **notification-triggers** — error detection over tool executions
//!   (`is_error` flag + content pattern + token threshold), trigger evaluation,
//!   regex cache with LRU eviction, notification persistence with read/unread
//!   state and paging.

pub mod claude_md;
pub mod defaults;
pub mod detected_error;
pub mod error;
pub mod error_detector;
pub mod error_trigger_checker;
pub mod manager;
pub mod mention;
pub mod notification_manager;
pub mod regex_safety;
pub mod trigger;
pub mod trigger_matcher;
pub mod types;
pub mod validation;

pub use claude_md::{ClaudeMdFileInfo, Scope, read_all_claude_md_files, read_directory_claude_md};
pub use detected_error::{DetectedError, DetectedErrorContext};
pub use error::ConfigError;
pub use error_detector::{detect_errors, detect_errors_with_trigger};
pub use manager::ConfigManager;
pub use mention::{read_mentioned_file, validate_file_path};
pub use notification_manager::{GetNotificationsResult, NotificationManager, StoredNotification};
pub use regex_safety::{create_safe_regex, validate_regex_pattern};
pub use trigger::{TriggerManager, TriggerValidationResult, merge_triggers, validate_trigger};
pub use trigger_matcher::{matches_ignore_patterns, matches_pattern};
pub use types::{
    AppConfig, ConfigSection, DisplayConfig, GeneralConfig, HttpServerConfig, NotificationConfig,
    NotificationTrigger, SessionsConfig, SshPersistConfig, TriggerContentType, TriggerMode,
};
pub use validation::{normalize_claude_root_path, validate_http_port, validate_section};
