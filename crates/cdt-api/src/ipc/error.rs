//! API 结构化错误。

use serde::{Deserialize, Serialize};

/// API 错误码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorCode {
    /// 输入校验失败。
    ValidationError,
    /// 资源不存在。
    NotFound,
    /// 内部错误。
    Internal,
    /// SSH 相关错误。
    SshError,
    /// 配置错误。
    ConfigError,
}

/// API 结构化错误。
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn validation(msg: impl Into<String>) -> Self {
        Self {
            code: ApiErrorCode::ValidationError,
            message: msg.into(),
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            code: ApiErrorCode::NotFound,
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: ApiErrorCode::Internal,
            message: msg.into(),
        }
    }

    pub fn ssh(msg: impl Into<String>) -> Self {
        Self {
            code: ApiErrorCode::SshError,
            message: msg.into(),
        }
    }
}
