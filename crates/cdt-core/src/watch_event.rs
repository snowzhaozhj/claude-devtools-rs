//! 文件监听事件的共享类型。
//!
//! 由 `cdt-watch` 生产、由 `cdt-api` / `cdt-config` 等下游 crate 消费。
//! 放在 `cdt-core` 避免下游 crate 被迫依赖 `cdt-watch`。

/// `.jsonl` 会话文件变更事件。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeEvent {
    /// 项目 ID（`~/.claude/projects/` 下的目录名）。
    pub project_id: String,
    /// 会话 ID（`.jsonl` 文件名，去掉扩展名）。
    pub session_id: String,
    /// 文件是否被删除。
    pub deleted: bool,
    /// 项目列表是否可能已变化。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub project_list_changed: bool,
    /// group 内 session 集合是否变化（"已知 project 下首次见 session" /
    /// 删除 / 重命名等场景）。watcher 层构造时恒为 `false`，由
    /// `cdt-api` `spawn_unified_cache_invalidator` 三档判定后 enrich。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub session_list_changed: bool,
    /// 文件 mtime（毫秒 since UNIX epoch）。watcher 在能取到时填，
    /// 取不到（删除事件 / 远端 SFTP 不返 mtime）SHALL 省略字段。
    /// 行为契约：`openspec/specs/file-watching/spec.md`；消费契约：
    /// `openspec/specs/ipc-data-api/spec.md::ProjectScanCache 维护 per-project
    /// mtime overlay 让 cache 命中路径返回新鲜 mtime`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtime_ms: Option<i64>,
}

/// Todo 文件变更事件。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoChangeEvent {
    /// 会话 ID（`<sessionId>.json` 的文件名，去掉扩展名）。
    pub session_id: String,
}
