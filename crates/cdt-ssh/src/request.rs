//! SSH 连接请求 payload（IPC 边界对齐 `design.md` D6）。
//!
//! Phase A 阶段在 `cdt-ssh` 自身定义新形态 `SshConnectRequest`；Phase C task 9.x
//! 时 `cdt-api` 将 import 此类型替换原有简化版（仅含 `host_alias` / `context_id`）。
//!
//! 安全约束（task 1.5 + design.md D9）：手写 `Debug` impl 把 `password` 字段渲染为
//! 固定字符串 `<redacted>`，避免 `tracing::info!(?request)` 等模式把明文密码写入
//! 日志或崩溃栈。

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::auth::AuthMethodKind;

/// SSH 连接请求 payload（与 D6 IPC 命令 `ssh_connect` 对齐）。
///
/// 序列化：camelCase；`password` 字段绝不持久化（spec `Requirement: SSH
/// authentication candidate chain` + design.md D9）。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectRequest {
    /// host alias 或裸 hostname。
    pub host: String,
    /// 端口（默认 22 由调用方填或 server 侧默认）。
    #[serde(default)]
    pub port: Option<u16>,
    /// 用户名（可选；缺省时由 `ssh -G` 解析）。
    #[serde(default)]
    pub username: Option<String>,
    /// 鉴权方式（D2 候选链或 password）。
    pub auth_method: AuthMethodKind,
    /// password（仅当 `auth_method == Password` 时填；绝不持久化）。
    #[serde(default)]
    pub password: Option<String>,
    /// 显式指定 `context_id`；缺省由调用方按 host 生成。
    #[serde(default)]
    pub context_id: Option<String>,
}

impl fmt::Debug for SshConnectRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SshConnectRequest")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("auth_method", &self.auth_method)
            .field(
                "password",
                &if self.password.is_some() {
                    "<redacted>"
                } else {
                    "<none>"
                },
            )
            .field("context_id", &self.context_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request_with_password() -> SshConnectRequest {
        SshConnectRequest {
            host: "myserver".into(),
            port: Some(2222),
            username: Some("alice".into()),
            auth_method: AuthMethodKind::Password,
            password: Some("super-secret-pw-12345".into()),
            context_id: None,
        }
    }

    #[test]
    fn debug_redacts_password_field() {
        let req = sample_request_with_password();
        let dbg = format!("{req:?}");
        assert!(
            !dbg.contains("super-secret-pw-12345"),
            "Debug must NOT contain real password: {dbg}"
        );
        assert!(dbg.contains("<redacted>"), "expected <redacted> sentinel");
    }

    #[test]
    fn debug_with_no_password_uses_none_marker() {
        let mut req = sample_request_with_password();
        req.password = None;
        let dbg = format!("{req:?}");
        assert!(dbg.contains("<none>"));
        assert!(!dbg.contains("<redacted>"));
    }

    #[test]
    fn debug_pretty_alternate_form_also_redacts() {
        let req = sample_request_with_password();
        let dbg = format!("{req:#?}");
        assert!(!dbg.contains("super-secret-pw-12345"));
        assert!(dbg.contains("<redacted>"));
    }

    #[test]
    fn serde_round_trip_preserves_password_field() {
        // 序列化路径仍保留 password — 仅 Debug 渲染做 redact。
        let req = sample_request_with_password();
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["password"], "super-secret-pw-12345");
        assert_eq!(json["authMethod"], "password");
        let back: SshConnectRequest = serde_json::from_value(json).unwrap();
        assert_eq!(back, req);
    }
}
