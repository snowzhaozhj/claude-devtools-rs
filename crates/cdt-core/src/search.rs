//! 搜索结果类型——由 `cdt-discover` 产出，`cdt-api` 序列化给前端。

/// 单条搜索命中。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    /// 命中所在消息的 UUID。
    pub message_uuid: String,
    /// 命中在该条文本中的字节偏移。
    pub offset: usize,
    /// 前后各 50 字符的上下文预览。
    pub preview: String,
    /// 消息类型（`"user"` / `"assistant"` 等）。
    pub message_type: String,
}

/// 单个 session 的搜索结果。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchResult {
    pub session_id: String,
    pub project_id: String,
    pub session_title: String,
    pub hits: Vec<SearchHit>,
    pub total_matches: usize,
}

/// 跨 session 搜索汇总。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchSessionsResult {
    pub results: Vec<SessionSearchResult>,
    pub total_matches: usize,
    pub sessions_searched: usize,
    pub query: String,
    /// SSH 模式下可能提前返回部分结果。
    pub is_partial: bool,
}
