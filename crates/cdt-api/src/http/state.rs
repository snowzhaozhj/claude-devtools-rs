//! HTTP server 共享状态。

use std::sync::Arc;

use tokio::sync::broadcast;

use crate::ipc::{DataApi, PushEvent};

/// axum handler 共享的应用状态。
#[derive(Clone)]
pub struct AppState {
    /// `DataApi` trait 实现（由调用方注入）。
    pub api: Arc<dyn DataApi>,
    /// 推送事件广播通道（发送端）。
    pub events_tx: broadcast::Sender<PushEvent>,
}

impl AppState {
    /// 创建 `AppState`。`capacity` 为事件通道缓冲大小。
    pub fn new(api: Arc<dyn DataApi>, capacity: usize) -> Self {
        let (events_tx, _) = broadcast::channel(capacity);
        Self { api, events_tx }
    }
}
