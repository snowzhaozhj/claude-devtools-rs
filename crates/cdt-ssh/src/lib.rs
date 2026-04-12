//! SSH-backed remote context.
//!
//! Owns the **ssh-remote-context** capability. Implements the
//! `FileSystemProvider` trait (defined in `cdt-discover`) for remote hosts,
//! exposing the exact same read API as the local provider so downstream
//! consumers observe identical data shapes regardless of transport.
//!
//! Responsibilities:
//! - Parse `~/.ssh/config` for host aliases (hostname, user, port, identity)
//! - Connect / disconnect / test / query status
//! - Provide `SshFileSystemProvider` implementing `FileSystemProvider`
//! - Report structured connection status: disconnected / connecting /
//!   connected / error with human-readable message

pub mod config_parser;
pub mod connection;
pub mod error;
pub mod provider;

pub use config_parser::{
    SshHostConfig, default_ssh_config_path, list_hosts, parse_ssh_config, parse_ssh_config_file,
    resolve_host,
};
pub use connection::{ActiveContext, ConnectionState, ConnectionStatus, SshConnectionManager};
pub use error::SshError;
pub use provider::SshFileSystemProvider;
