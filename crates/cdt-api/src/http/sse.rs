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
    let broadcast_stream =
        BroadcastStream::new(rx).map(|result| Ok(convert_broadcast_result(result)));
    // 立即 emit 一条 SSE comment（`:open`）作为 prelude，让 axum / 反向代理 /
    // vite dev proxy 立刻 flush HTTP response chunked 给浏览器，浏览器
    // EventSource 收到 response headers 后立即进入 OPEN。否则首条真实 event
    // 之前 response body 可能被中间 proxy 缓冲，浏览器 EventSource 永远卡在
    // CONNECTING（readyState=0）→ 前端 `ensureSseReady` 1000ms 超时放行 →
    // 后端在该窗口 emit 的 `session_metadata_update` 全部丢失 → 列表项
    // title=null 永久 fallback 到 sessionId（codex 二审 round 3 报"会话名
    // 变 sessionId"的修法）。SSE comment 行以 `:` 开头，符合 W3C spec，
    // 浏览器静默忽略不当成 event 派发。
    let prelude = futures::stream::once(async { Ok(Event::default().comment("open")) });
    let stream = prelude.chain(broadcast_stream);
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

    /// `ContextChanged` SHALL 走 SSE 推到浏览器——历史 bug：HTTP server 缺
    /// 这个桥，浏览器 `?http=1` 模式下 contextStore 在 SSH 切换后永远 stale。
    /// 序列化 payload 形态与桌面 Tauri 桥保持一致：
    /// `{"type":"context_changed", "active_context_id": "...", "kind": "ssh"}`。
    #[test]
    fn context_changed_serializes_with_snake_case_payload() {
        let event = PushEvent::ContextChanged {
            active_context_id: Some("localhost".into()),
            kind: "ssh".into(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(
            json.contains("\"type\":\"context_changed\""),
            "SHALL tag with snake_case type, got: {json}"
        );
        assert!(
            json.contains("\"active_context_id\":\"localhost\""),
            "SHALL emit active_context_id field, got: {json}"
        );
        assert!(
            json.contains("\"kind\":\"ssh\""),
            "SHALL emit kind field, got: {json}"
        );
    }

    #[test]
    fn context_changed_local_serializes_with_null_active() {
        let event = PushEvent::ContextChanged {
            active_context_id: None,
            kind: "local".into(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(
            json.contains("\"active_context_id\":null"),
            "Local SHALL serialize active_context_id=null, got: {json}"
        );
        assert!(
            json.contains("\"kind\":\"local\""),
            "Local SHALL serialize kind=local, got: {json}"
        );
    }
}
