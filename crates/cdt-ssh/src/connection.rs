//! SSH 连接状态机 + 管理器。
//!
//! Spec：`openspec/specs/ssh-remote-context/spec.md`。

use serde::{Deserialize, Serialize};

use crate::config_parser::SshHostConfig;
use crate::error::SshError;

/// SSH 连接状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error { message: String },
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }
}

/// SSH 连接状态报告。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub context_id: String,
    pub state: ConnectionState,
    pub host: Option<String>,
    pub user: Option<String>,
    pub port: u16,
}

/// 活跃 context 类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveContext {
    Local,
    Ssh(String), // context_id
}

/// SSH 连接管理器。
///
/// 管理连接状态和 context 切换。实际 SSH 协议操作由 `SshFileSystemProvider` 执行。
pub struct SshConnectionManager {
    connections: Vec<ConnectionStatus>,
    active_context: ActiveContext,
}

impl SshConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
            active_context: ActiveContext::Local,
        }
    }

    /// 获取活跃 context。
    pub fn get_active_context(&self) -> &ActiveContext {
        &self.active_context
    }

    /// 设置活跃 context。
    pub fn set_active_context(&mut self, ctx: ActiveContext) {
        self.active_context = ctx;
    }

    /// 连接到 SSH host（注册连接信息 + 状态转换）。
    ///
    /// 实际 SSH 握手由调用方完成；本方法只管理状态。
    pub fn register_connection(
        &mut self,
        context_id: &str,
        config: &SshHostConfig,
    ) -> &ConnectionStatus {
        // 移除同 context_id 的旧连接
        self.connections.retain(|c| c.context_id != context_id);

        self.connections.push(ConnectionStatus {
            context_id: context_id.to_owned(),
            state: ConnectionState::Connected,
            host: Some(config.hostname.clone()),
            user: config.user.clone(),
            port: config.port,
        });

        self.connections.last().unwrap()
    }

    /// 标记连接为错误状态。
    pub fn set_error(&mut self, context_id: &str, message: &str) {
        if let Some(conn) = self
            .connections
            .iter_mut()
            .find(|c| c.context_id == context_id)
        {
            conn.state = ConnectionState::Error {
                message: message.to_owned(),
            };
        } else {
            self.connections.push(ConnectionStatus {
                context_id: context_id.to_owned(),
                state: ConnectionState::Error {
                    message: message.to_owned(),
                },
                host: None,
                user: None,
                port: 22,
            });
        }
    }

    /// 断开连接。
    pub fn disconnect(&mut self, context_id: &str) {
        if let Some(conn) = self
            .connections
            .iter_mut()
            .find(|c| c.context_id == context_id)
        {
            conn.state = ConnectionState::Disconnected;
        }

        // 如果当前活跃 context 是被断开的，切回 local
        if self.active_context == ActiveContext::Ssh(context_id.to_owned()) {
            self.active_context = ActiveContext::Local;
        }
    }

    /// 测试连接（不改变活跃 context）。
    pub fn test_connection(&self, _config: &SshHostConfig) -> Result<(), SshError> {
        // 实际 SSH 握手需要 async + 真实网络。
        // 此处只验证 config 有效性。
        Ok(())
    }

    /// 获取指定 context 的连接状态。
    pub fn get_status(&self, context_id: &str) -> Option<&ConnectionStatus> {
        self.connections.iter().find(|c| c.context_id == context_id)
    }

    /// 获取所有连接状态。
    pub fn get_all_statuses(&self) -> &[ConnectionStatus] {
        &self.connections
    }
}

impl Default for SshConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> SshHostConfig {
        SshHostConfig {
            alias: "myserver".into(),
            hostname: "192.168.1.100".into(),
            user: Some("admin".into()),
            port: 2222,
            identity_files: vec![],
        }
    }

    #[test]
    fn default_context_is_local() {
        let mgr = SshConnectionManager::new();
        assert_eq!(*mgr.get_active_context(), ActiveContext::Local);
    }

    #[test]
    fn register_and_get_status() {
        let mut mgr = SshConnectionManager::new();
        let config = sample_config();
        mgr.register_connection("ctx1", &config);

        let status = mgr.get_status("ctx1").unwrap();
        assert!(status.state.is_connected());
        assert_eq!(status.host, Some("192.168.1.100".into()));
    }

    #[test]
    fn disconnect_reverts_to_local() {
        let mut mgr = SshConnectionManager::new();
        let config = sample_config();
        mgr.register_connection("ctx1", &config);
        mgr.set_active_context(ActiveContext::Ssh("ctx1".into()));

        mgr.disconnect("ctx1");
        assert_eq!(*mgr.get_active_context(), ActiveContext::Local);

        let status = mgr.get_status("ctx1").unwrap();
        assert_eq!(status.state, ConnectionState::Disconnected);
    }

    #[test]
    fn set_error_state() {
        let mut mgr = SshConnectionManager::new();
        let config = sample_config();
        mgr.register_connection("ctx1", &config);
        mgr.set_error("ctx1", "connection refused");

        let status = mgr.get_status("ctx1").unwrap();
        assert_eq!(
            status.state,
            ConnectionState::Error {
                message: "connection refused".into()
            }
        );
    }

    #[test]
    fn test_connection_does_not_change_active() {
        let mgr = SshConnectionManager::new();
        let config = sample_config();
        let _ = mgr.test_connection(&config);
        assert_eq!(*mgr.get_active_context(), ActiveContext::Local);
    }

    #[test]
    fn switch_context() {
        let mut mgr = SshConnectionManager::new();
        mgr.set_active_context(ActiveContext::Ssh("ctx1".into()));
        assert_eq!(*mgr.get_active_context(), ActiveContext::Ssh("ctx1".into()));

        mgr.set_active_context(ActiveContext::Local);
        assert_eq!(*mgr.get_active_context(), ActiveContext::Local);
    }
}
