//! SSH-backed remote context.
//!
//! Owns the **ssh-remote-context** capability. Implements the
//! `FileSystemProvider` trait (defined in `cdt-discover`) for remote hosts,
//! exposing the exact same read API as the local provider so downstream
//! consumers observe identical data shapes regardless of transport.
//!
//! Responsibilities:
//! - Parse `~/.ssh/config` for host alias listing (UI combobox)
//! - Delegate full host resolution (`Include` / `Match` / `ProxyJump` / `IdentityAgent`)
//!   to system `ssh -G <host>` subprocess (`host_resolver`)
//! - Build the 7-source authentication candidate chain (`auth`) per
//!   `openspec/specs/ssh-remote-context/spec.md::Requirement: SSH authentication
//!   candidate chain`
//! - Connect / disconnect / test / query status via real `russh` 0.52 protocol
//!   stack — Phase B / `connection.rs`
//! - Provide `SshFileSystemProvider` implementing `FileSystemProvider` over
//!   `russh-sftp` — Phase B / `provider.rs`
//! - Watch remote project directories via SFTP polling (`polling_watcher`)
//! - Report structured connection status with structured `SshError`
//!   classification

pub mod auth;
pub mod config_parser;
pub mod connection;
pub mod error;
pub mod host_resolver;
pub mod polling_watcher;
pub mod provider;
pub mod request;
pub mod session;

pub use auth::{AuthMethodKind, Platform, build_candidates, run_auth_chain};
pub use config_parser::{
    SshHostConfig, default_ssh_config_path, list_hosts, parse_ssh_config, parse_ssh_config_file,
    resolve_host,
};
pub use connection::{ActiveContext, ConnectionState, ConnectionStatus, SshConnectionManager};
pub use error::{AuthAttempt, AuthOutcome, AuthSource, SshError, TimeoutStage};
pub use host_resolver::{ResolvedHost, parse_ssh_g_output, resolve_host_via_ssh_g};
pub use polling_watcher::{CancelToken, RemotePollingWatcher, RemoteWatcherHandle};
pub use provider::{RemoteEntry, SftpClient, SftpClientError, SshFileSystemProvider, with_retry};
pub use request::SshConnectRequest;
pub use session::{
    ContextChanged, ContextKind, SHUTDOWN_TIMEOUT, SshContextState, SshSessionManager, SshStatus,
    SshStatusChange,
};
