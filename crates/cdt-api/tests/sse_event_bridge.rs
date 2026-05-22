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

use cdt_api::{PushEvent, SessionMetadataUpdate, spawn_event_bridge};
use cdt_config::{DetectedError, DetectedErrorContext};
use cdt_core::{FileChangeEvent, TodoChangeEvent};
use cdt_ssh::ContextChanged;
use tokio::sync::broadcast;
use tokio::time::timeout;

fn spawn_test_event_bridge(
    events_tx: broadcast::Sender<PushEvent>,
    file_rx: broadcast::Receiver<FileChangeEvent>,
    todo_rx: broadcast::Receiver<TodoChangeEvent>,
    error_rx: broadcast::Receiver<DetectedError>,
) {
    let (_metadata_tx, metadata_rx) = broadcast::channel::<SessionMetadataUpdate>(16);
    let (_context_tx, context_rx) = broadcast::channel::<ContextChanged>(16);
    spawn_event_bridge(
        events_tx,
        file_rx,
        todo_rx,
        error_rx,
        metadata_rx,
        context_rx,
    );
}

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

    spawn_test_event_bridge(events_tx, file_rx, todo_rx, error_rx);

    file_tx
        .send(FileChangeEvent {
            project_id: "p1".into(),
            session_id: "s1".into(),
            deleted: false,
            project_list_changed: false,
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
            deleted,
            project_list_changed,
        } => {
            assert_eq!(project_id, "p1");
            assert_eq!(session_id, "s1");
            assert!(!deleted);
            assert!(!project_list_changed);
        }
        other => panic!("expected FileChange, got {other:?}"),
    }
}

#[tokio::test]
async fn project_list_changed_forwarded_to_sse() {
    let (events_tx, mut events_rx) = broadcast::channel::<PushEvent>(64);
    let (file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
    let (_todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (_error_tx, error_rx) = broadcast::channel::<DetectedError>(16);

    spawn_test_event_bridge(events_tx, file_rx, todo_rx, error_rx);

    file_tx
        .send(FileChangeEvent {
            project_id: "p-new".into(),
            session_id: String::new(),
            deleted: false,
            project_list_changed: true,
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
            deleted,
            project_list_changed,
        } => {
            assert_eq!(project_id, "p-new");
            assert_eq!(session_id, "");
            assert!(!deleted);
            assert!(project_list_changed);
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

    spawn_test_event_bridge(events_tx, file_rx, todo_rx, error_rx);

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

    spawn_test_event_bridge(events_tx, file_rx, todo_rx, error_rx);

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
async fn session_metadata_forwarded_as_push_event() {
    let (events_tx, mut events_rx) = broadcast::channel::<PushEvent>(64);
    let (_file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
    let (_todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (_error_tx, error_rx) = broadcast::channel::<DetectedError>(16);
    let (metadata_tx, metadata_rx) = broadcast::channel::<SessionMetadataUpdate>(16);
    let (_context_tx, context_rx) = broadcast::channel::<ContextChanged>(16);

    spawn_event_bridge(
        events_tx,
        file_rx,
        todo_rx,
        error_rx,
        metadata_rx,
        context_rx,
    );

    metadata_tx
        .send(SessionMetadataUpdate {
            project_id: "p1".into(),
            session_id: "s1".into(),
            title: Some("hello".into()),
            message_count: 42,
            is_ongoing: true,
            git_branch: Some("main".into()),
            group_id: Some("g1".into()),
        })
        .unwrap();

    let event = timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .expect("recv timed out")
        .expect("recv ok");
    match event {
        PushEvent::SessionMetadataUpdate {
            project_id,
            session_id,
            title,
            message_count,
            is_ongoing,
            git_branch,
            group_id,
        } => {
            assert_eq!(project_id, "p1");
            assert_eq!(session_id, "s1");
            assert_eq!(title.as_deref(), Some("hello"));
            assert_eq!(message_count, 42);
            assert!(is_ongoing);
            assert_eq!(git_branch.as_deref(), Some("main"));
            // spec sidebar-navigation §"selectedGroupId 与 worktree id 分层维护"
            // Scenario "SSE patch 按 groupId filter"：bridge MUST 透传 group_id
            // 字段——前端按此过滤当前 group 的 patch。
            assert_eq!(
                group_id.as_deref(),
                Some("g1"),
                "SSE event SHALL 透传 group_id"
            );
        }
        other => panic!("expected SessionMetadataUpdate, got {other:?}"),
    }
}

/// 验证 `ssh_mgr.subscribe_context_changed` 的 broadcast 真的被
/// `spawn_context_changed_bridge` 转成 `PushEvent::ContextChanged` 喂给 SSE。
/// 修历史 bug：HTTP server 缺这个桥让浏览器 `?http=1` 模式下 contextStore
/// 在 SSH 切换后永远 stale 在 local。
#[tokio::test]
async fn context_changed_forwarded_as_push_event_ssh() {
    let (events_tx, mut events_rx) = broadcast::channel::<PushEvent>(64);
    let (_file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
    let (_todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (_error_tx, error_rx) = broadcast::channel::<DetectedError>(16);
    let (_metadata_tx, metadata_rx) = broadcast::channel::<SessionMetadataUpdate>(16);
    let (context_tx, context_rx) = broadcast::channel::<ContextChanged>(16);

    spawn_event_bridge(
        events_tx,
        file_rx,
        todo_rx,
        error_rx,
        metadata_rx,
        context_rx,
    );

    context_tx
        .send(ContextChanged {
            active_context_id: Some("ctx-A".into()),
            kind: cdt_ssh::ContextKind::Ssh,
        })
        .unwrap();

    let event = timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .expect("recv timed out")
        .expect("recv ok");
    match event {
        PushEvent::ContextChanged {
            active_context_id,
            kind,
        } => {
            assert_eq!(active_context_id.as_deref(), Some("ctx-A"));
            assert_eq!(kind, "ssh");
        }
        other => panic!("expected ContextChanged, got {other:?}"),
    }
}

/// disconnect / `switch_context("local")` 路径——`active_context_id=None` +
/// `kind=local`。前端 `refreshAfterContextChange` 看到 null active 不更新
/// `contextStore.activeContextId`（`context.svelte.ts:30` guard），靠
/// `loadContexts()` 异步刷新拿权威 active。
#[tokio::test]
async fn context_changed_forwarded_as_push_event_local() {
    let (events_tx, mut events_rx) = broadcast::channel::<PushEvent>(64);
    let (_file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
    let (_todo_tx, todo_rx) = broadcast::channel::<TodoChangeEvent>(16);
    let (_error_tx, error_rx) = broadcast::channel::<DetectedError>(16);
    let (_metadata_tx, metadata_rx) = broadcast::channel::<SessionMetadataUpdate>(16);
    let (context_tx, context_rx) = broadcast::channel::<ContextChanged>(16);

    spawn_event_bridge(
        events_tx,
        file_rx,
        todo_rx,
        error_rx,
        metadata_rx,
        context_rx,
    );

    context_tx
        .send(ContextChanged {
            active_context_id: None,
            kind: cdt_ssh::ContextKind::Local,
        })
        .unwrap();

    let event = timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .expect("recv timed out")
        .expect("recv ok");
    match event {
        PushEvent::ContextChanged {
            active_context_id,
            kind,
        } => {
            assert!(active_context_id.is_none());
            assert_eq!(kind, "local");
        }
        other => panic!("expected ContextChanged, got {other:?}"),
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

    spawn_test_event_bridge(events_tx.clone(), file_rx, todo_rx, error_rx);

    file_tx
        .send(FileChangeEvent {
            project_id: "p".into(),
            session_id: "s".into(),
            deleted: false,
            project_list_changed: false,
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

    spawn_test_event_bridge(events_tx, file_rx, todo_rx, error_rx);

    // 在 producer 还没 poll 之前突发塞超过 capacity 4 的事件，确保产生 lag。
    // 即便 producer 已 spawn，也尽量把窗口堆满；spinning send 不阻塞。
    for i in 0..32 {
        let _ = file_tx.send(FileChangeEvent {
            project_id: "p".into(),
            session_id: format!("burst-{i}"),
            deleted: false,
            project_list_changed: false,
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
            project_list_changed: false,
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
