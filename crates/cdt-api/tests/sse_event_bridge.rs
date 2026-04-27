//! 集成测试：`spawn_event_bridge` 把 backend `FileChangeEvent` /
//! `TodoChangeEvent` / `DetectedError` 转发为 `PushEvent` 推到 SSE 通道。
//!
//! 覆盖 spec `http-data-api` §"Push events via Server-Sent Events":
//! - Scenario `SSE client subscribes and receives file change`
//! - Scenario `SSE client receives todo change`
//! - Scenario `SSE client receives new-notification when DetectedError fires`
//! - Scenario `Multiple concurrent SSE clients`
//! - Scenario `Producer skips lagged events without crashing`

use std::time::Duration;

use cdt_api::{PushEvent, spawn_event_bridge};
use cdt_config::{DetectedError, DetectedErrorContext};
use cdt_core::{FileChangeEvent, TodoChangeEvent};
use tokio::sync::broadcast;
use tokio::time::timeout;

fn sample_detected_error(id: &str, msg: &str) -> DetectedError {
    DetectedError {
        id: id.into(),
        timestamp: 1_700_000_000_000,
        session_id: "sess".into(),
        project_id: "proj".into(),
        file_path: "/tmp/proj/sess.jsonl".into(),
        source: "stderr".into(),
        message: msg.into(),
        line_number: None,
        tool_use_id: None,
        trigger_color: None,
        trigger_id: None,
        trigger_name: None,
        context: DetectedErrorContext {
            project_name: "proj".into(),
            cwd: None,
        },
    }
}

#[tokio::test]
async fn file_change_forwarded_as_push_event() {
    let (events_tx, mut events_rx) = broadcast::channel::<PushEvent>(64);
    let (file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
    let (_todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (_error_tx, error_rx) = broadcast::channel::<DetectedError>(16);

    spawn_event_bridge(events_tx, file_rx, todo_rx, error_rx);

    file_tx
        .send(FileChangeEvent {
            project_id: "p1".into(),
            session_id: "s1".into(),
            deleted: false,
        })
        .unwrap();

    let event = timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .expect("recv timed out")
        .expect("recv ok");
    match event {
        PushEvent::FileChange {
            project_id,
            session_id,
        } => {
            assert_eq!(project_id, "p1");
            assert_eq!(session_id, "s1");
        }
        other => panic!("expected FileChange, got {other:?}"),
    }
}

#[tokio::test]
async fn todo_change_forwarded_with_empty_project_id() {
    let (events_tx, mut events_rx) = broadcast::channel::<PushEvent>(64);
    let (_file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
    let (todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (_error_tx, error_rx) = broadcast::channel::<DetectedError>(16);

    spawn_event_bridge(events_tx, file_rx, todo_rx, error_rx);

    todo_tx
        .send(TodoChangeEvent {
            session_id: "todo-s".into(),
        })
        .unwrap();

    let event = timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .expect("recv timed out")
        .expect("recv ok");
    match event {
        PushEvent::TodoChange {
            project_id,
            session_id,
        } => {
            assert_eq!(
                project_id, "",
                "TodoChangeEvent 仅含 session_id，project_id SHALL 占位空字符串"
            );
            assert_eq!(session_id, "todo-s");
        }
        other => panic!("expected TodoChange, got {other:?}"),
    }
}

#[tokio::test]
async fn detected_error_forwarded_as_new_notification() {
    let (events_tx, mut events_rx) = broadcast::channel::<PushEvent>(64);
    let (_file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
    let (_todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (error_tx, error_rx) = broadcast::channel::<DetectedError>(16);

    spawn_event_bridge(events_tx, file_rx, todo_rx, error_rx);

    error_tx
        .send(sample_detected_error("err-1", "boom"))
        .unwrap();

    let event = timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .expect("recv timed out")
        .expect("recv ok");
    match event {
        PushEvent::NewNotification { notification } => {
            assert_eq!(
                notification.get("id").and_then(|v| v.as_str()),
                Some("err-1")
            );
            assert_eq!(
                notification.get("message").and_then(|v| v.as_str()),
                Some("boom")
            );
            // camelCase 序列化（与 DetectedError serde 配置一致）
            assert!(
                notification.get("sessionId").is_some(),
                "DetectedError SHALL 序列化为 camelCase"
            );
        }
        other => panic!("expected NewNotification, got {other:?}"),
    }
}

#[tokio::test]
async fn multiple_subscribers_each_receive_event_exactly_once() {
    let (events_tx, mut rx_a) = broadcast::channel::<PushEvent>(64);
    let mut rx_b = events_tx.subscribe();
    let mut rx_c = events_tx.subscribe();
    let (file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
    let (_todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (_error_tx, error_rx) = broadcast::channel::<DetectedError>(16);

    spawn_event_bridge(events_tx.clone(), file_rx, todo_rx, error_rx);

    file_tx
        .send(FileChangeEvent {
            project_id: "p".into(),
            session_id: "s".into(),
            deleted: false,
        })
        .unwrap();

    for rx in [&mut rx_a, &mut rx_b, &mut rx_c] {
        let event = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("recv timed out")
            .expect("recv ok");
        assert!(matches!(event, PushEvent::FileChange { .. }));
    }
}

#[tokio::test]
async fn producer_continues_after_lagged_recv() {
    // file 输入 channel capacity = 4，制造 producer 端 Lagged。
    // 关键断言：producer 没退出 loop——后续事件仍会被转发。
    let (events_tx, mut events_rx) = broadcast::channel::<PushEvent>(256);
    let (file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(4);
    let (_todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (_error_tx, error_rx) = broadcast::channel::<DetectedError>(16);

    spawn_event_bridge(events_tx, file_rx, todo_rx, error_rx);

    // 在 producer 还没 poll 之前突发塞超过 capacity 4 的事件，确保产生 lag。
    // 即便 producer 已 spawn，也尽量把窗口堆满；spinning send 不阻塞。
    for i in 0..32 {
        let _ = file_tx.send(FileChangeEvent {
            project_id: "p".into(),
            session_id: format!("burst-{i}"),
            deleted: false,
        });
    }

    // 给 producer 一点时间消费 + 命中 Lagged
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 再发尾条事件——这条 SHALL 不被丢弃（producer 仍在 loop）
    file_tx
        .send(FileChangeEvent {
            project_id: "p".into(),
            session_id: "tail".into(),
            deleted: false,
        })
        .unwrap();

    // events_rx 上至少应该收到 tail（前面突发的可能也部分被转发到 events，
    // 不强求顺序，只断言能收到 tail）
    let mut saw_tail = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            break;
        }
        match timeout(deadline - now, events_rx.recv()).await {
            Ok(Ok(PushEvent::FileChange { session_id, .. })) if session_id == "tail" => {
                saw_tail = true;
                break;
            }
            // events_rx 自己也可能 Lagged（events_tx capacity 充裕，但保险处理）
            Ok(Ok(_) | Err(broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(broadcast::error::RecvError::Closed)) | Err(_) => break,
        }
    }
    assert!(
        saw_tail,
        "producer SHALL 不因 Lagged 退出 loop；尾条事件 'tail' 应被转发"
    );
}
