//! API 请求/响应类型。

use serde::{Deserialize, Serialize};

// =============================================================================
// 分页
// =============================================================================

/// 分页请求参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedRequest {
    pub page_size: usize,
    #[serde(default)]
    pub cursor: Option<String>,
}

/// 分页响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub total: usize,
}

// =============================================================================
// 项目
// =============================================================================

/// 项目信息（列表返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub id: String,
    pub path: String,
    pub display_name: String,
    pub session_count: usize,
}

// =============================================================================
// 会话
// =============================================================================

/// 会话摘要（列表返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub project_id: String,
    pub timestamp: i64,
    pub message_count: usize,
    /// 第一条用户消息（清洗后），用作 sidebar 标题。
    #[serde(default)]
    pub title: Option<String>,
}

/// 会话详情。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session_id: String,
    pub project_id: String,
    pub chunks: serde_json::Value,
    pub metrics: serde_json::Value,
    pub metadata: serde_json::Value,
}

// =============================================================================
// 搜索
// =============================================================================

/// 搜索请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

// =============================================================================
// 配置
// =============================================================================

/// 配置更新请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdateRequest {
    pub section: String,
    pub data: serde_json::Value,
}

// =============================================================================
// SSH
// =============================================================================

/// SSH 连接请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectRequest {
    pub host_alias: String,
    #[serde(default)]
    pub context_id: Option<String>,
}

// =============================================================================
// Context
// =============================================================================

/// Context 信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextInfo {
    pub id: String,
    pub kind: String,
    pub is_active: bool,
    #[serde(default)]
    pub host: Option<String>,
}
