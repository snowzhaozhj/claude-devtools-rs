//! 文件监听事件的共享类型。
//!
//! 由 `cdt-watch` 生产、由 `cdt-api` / `cdt-config` 等下游 crate 消费。
//! 放在 `cdt-core` 避免下游 crate 被迫依赖 `cdt-watch`。

/// `.jsonl` 会话文件变更事件。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeEvent {
    /// 项目 ID（`~/.claude/projects/` 下的目录名）。
    pub project_id: String,
    /// 会话 ID（`.jsonl` 文件名，去掉扩展名）。
    pub session_id: String,
    /// 文件是否被删除。
    pub deleted: bool,
}

/// Todo 文件变更事件。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoChangeEvent {
    /// 会话 ID（`<sessionId>.json` 的文件名，去掉扩展名）。
    pub session_id: String,
}
