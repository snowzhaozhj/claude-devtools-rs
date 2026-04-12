//! SSE 事件端点。
//!
//! Spec：`openspec/specs/http-data-api/spec.md`
//! §"`Push events via Server-Sent Events`"。

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use super::state::AppState;

/// SSE handler：`GET /api/events`。
///
/// 每个连接的客户端获得独立的 `broadcast::Receiver`，
/// 多客户端并发时每个都收到完整事件流。
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let rx = state.events_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(event) => {
                let data = serde_json::to_string(&event).unwrap_or_default();
                Some(Ok(Event::default().data(data)))
            }
            Err(_) => None, // lagged receiver，跳过
        }
    });
    Sse::new(stream)
}
