//! SSE 事件端点。
//!
//! Spec：`openspec/specs/http-data-api/spec.md`
//! §"`Push events via Server-Sent Events`"。

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use super::state::AppState;
use crate::ipc::PushEvent;

/// `BroadcastStream` Lagged 时 inline emit 的 sentinel payload。
///
/// 容量被打满 + slow client 跟不上时（默认 1024 capacity，但同时多个
/// project 切换 + 每 `page_size=50` 条 metadata 仍可能溢出），原实现 `None`
/// 静默吞掉事件——UI 永远停在缺失 metadata 的骨架。改为 inline emit 一条
/// `{"type":"sse_lagged"}`，UI 收到后触发 silent refresh 重拉数据
/// （codex 二审 issue 2 修法）。
pub(crate) const SSE_LAGGED_SENTINEL: &str = r#"{"type":"sse_lagged"}"#;

/// 把 `BroadcastStream` 一次 poll 的结果转成可发送的 SSE `Event`。
///
/// `Ok(PushEvent)` 序列化为 JSON 作 data，`Err(Lagged)` 转 `sse_lagged`
/// sentinel 让 UI 兜底重拉，**不**静默吞 None（codex 二审 issue 2）。
fn convert_broadcast_result(result: Result<PushEvent, BroadcastStreamRecvError>) -> Event {
    match result {
        Ok(event) => {
            let data = serde_json::to_string(&event).unwrap_or_default();
            Event::default().data(data)
        }
        Err(BroadcastStreamRecvError::Lagged(skipped)) => {
            tracing::warn!(
                target: "cdt_api::http::sse",
                skipped,
                "broadcast stream lagged; emit sse_lagged sentinel"
            );
            Event::default().data(SSE_LAGGED_SENTINEL)
        }
    }
}

/// SSE handler：`GET /api/events`。
///
/// 每个连接的客户端获得独立的 `broadcast::Receiver`，
/// 多客户端并发时每个都收到完整事件流。
///
/// `BroadcastStream` Lagged 处理：channel 容量被打满 + 当前 receiver 跟不上
/// 速度时 emit 一条 `sse_lagged` sentinel 让 UI 兜底重拉，**不**静默丢弃
/// （codex 二审 issue 2）。
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let rx = state.events_tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|result| Ok(convert_broadcast_result(result)));
    Sse::new(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_push_event_emits_json_payload() {
        let event = PushEvent::TodoChange {
            project_id: String::new(),
            session_id: "s".into(),
        };
        let sse = convert_broadcast_result(Ok(event));
        let dbg = format!("{sse:?}");
        assert!(
            dbg.contains("todo_change"),
            "JSON payload SHALL 含 PushEvent type tag, got: {dbg}"
        );
    }

    #[test]
    fn lagged_emits_sse_lagged_sentinel_not_dropped() {
        // codex 二审 issue 2：原实现 Err(Lagged) 走 None 静默吞，UI 永久卡
        // 在缺 metadata 的骨架。新实现 SHALL 转 sentinel let UI 重拉。
        let sse = convert_broadcast_result(Err(BroadcastStreamRecvError::Lagged(7)));
        let dbg = format!("{sse:?}");
        assert!(
            dbg.contains("sse_lagged"),
            "Lagged SHALL emit sse_lagged sentinel, got: {dbg}"
        );
    }
}
