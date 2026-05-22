//! server-mode lifecycle —— Tauri 进程内 HTTP server 的启停 / 状态查询。
//!
//! Spec：`openspec/specs/server-mode/spec.md`。
//!
//! 串行化策略：`tokio::sync::Mutex<Option<ServerHandle>>` 保证用户连点 toggle
//! 时同一时刻只持有一个 server task；每次 `start` SHALL 先 abort 旧 handle 再 bind。
//!
//! 持久化协同（详 `configuration-management/spec.md`）：
//! - `start` 成功 → 写 `enabled=true` + `port=<入参>`
//! - `start` 失败 → **不**写持久化（保持上次成功值），仅在内存 `last_error` 记录
//! - `stop` → 写 `enabled=false`，**不**改 `port`

use std::sync::Arc;

use cdt_api::{AppState, LocalDataApi, StaticServe, serve_with_listener, spawn_event_bridge};
use cdt_config::validate_http_port;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// 事件桥抽象——把 Tauri 的 `AppHandle::emit` 与单元测试 mock 解耦。
///
/// 实测：`tauri::AppHandle` 必须由 `tauri::Builder::build` 产出，单元测试无法
/// 直接构造，因此把 emit 入口抽成 trait。production code 给 `AppHandle` 提供
/// blanket impl；测试可以注入自己实现的 emitter 验证 payload。
pub trait StatusEmitter: Send + Sync + 'static {
    fn emit_status(&self, event: &str, payload: &ServerStatus);
}

impl StatusEmitter for AppHandle {
    fn emit_status(&self, event: &str, payload: &ServerStatus) {
        if let Err(e) = self.emit(event, payload) {
            tracing::warn!(
                target: "cdt_tauri::server_mode",
                error = %e,
                event,
                "failed to emit Tauri event"
            );
        }
    }
}

/// HTTP server 事件桥广播容量。
///
/// 默认 page_size=50（`src-tauri/src/lib.rs`）+ 单次 cache miss 触发 50 条
/// metadata patch，多 project 切换或多 SSE subscriber 时 128 容量很容易被
/// 打满；提到 1024 给约 20× headroom（codex 二审 issue 2 修法之一）。仍可
/// 能 lag——`cdt-api/src/http/sse.rs` 的 `sse_lagged` sentinel 兜底通知 UI
/// 重拉数据。
const EVENT_BRIDGE_CAPACITY: usize = 1024;

/// `http_server_status` IPC 返回结构 + emit `http-server-status` event 载荷。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub running: bool,
    pub port: u16,
    /// 最近一次启动失败（含自动恢复阶段）的错误文案；成功启动后 SHALL 重置为
    /// `None`。让晚挂载的 Settings UI 错过 emit event 时仍能通过 status 查询
    /// 拿到错误原因。
    pub last_error: Option<String>,
}

struct ServerHandle {
    task: JoinHandle<()>,
    port: u16,
}

/// server-mode 全局状态。Tauri `manage` 注入后由 3 个 IPC command 共享。
pub struct ServerState {
    lifecycle: Mutex<()>,
    handle: Mutex<Option<ServerHandle>>,
    last_error: Mutex<Option<String>>,
    api: Arc<LocalDataApi>,
    static_serve: StaticServe,
    emitter: Arc<dyn StatusEmitter>,
}

impl ServerState {
    pub fn new(api: Arc<LocalDataApi>, static_serve: StaticServe, app_handle: AppHandle) -> Self {
        Self::with_emitter(api, static_serve, Arc::new(app_handle))
    }

    /// 测试可见构造器：注入任意 `StatusEmitter` 实现验证 emit 行为。
    pub fn with_emitter(
        api: Arc<LocalDataApi>,
        static_serve: StaticServe,
        emitter: Arc<dyn StatusEmitter>,
    ) -> Self {
        Self {
            lifecycle: Mutex::new(()),
            handle: Mutex::new(None),
            last_error: Mutex::new(None),
            api,
            static_serve,
            emitter,
        }
    }

    /// 用户显式开启 server-mode：启动成功后写 `enabled=true` + `port` 持久化。
    pub async fn start(&self, port: u16) -> Result<(), String> {
        let _lifecycle_guard = self.lifecycle.lock().await;
        self.start_runtime_only(port).await?;

        // 持久化失败不回滚 server task——已 bind 的 listener 仍可用，下次启动
        // 仅是 config.json 不一致；分别 warn 让运维感知。
        if let Err(e) = self.api.set_http_server_port(port).await {
            tracing::warn!(
                target: "cdt_tauri::server_mode",
                error = %e,
                "failed to persist httpServer.port"
            );
        }
        if let Err(e) = self.api.set_http_server_enabled(true).await {
            tracing::warn!(
                target: "cdt_tauri::server_mode",
                error = %e,
                "failed to persist httpServer.enabled=true"
            );
        }
        Ok(())
    }

    /// 仅启动运行时 server，不写持久化。setup 自动恢复路径使用本函数，避免
    /// 与用户刚关闭 toggle 的 `enabled=false` 意图竞态覆盖。
    async fn start_runtime_only(&self, port: u16) -> Result<(), String> {
        if let Err(e) = validate_http_port(port) {
            let msg = e.to_string();
            *self.last_error.lock().await = Some(msg.clone());
            self.emit_status(false, port, Some(msg.clone()));
            return Err(msg);
        }

        let mut handle_guard = self.handle.lock().await;
        if let Some(old) = handle_guard.take() {
            old.task.abort();
            let _ = old.task.await;
        }

        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                let msg = if e.kind() == std::io::ErrorKind::AddrInUse {
                    format!("port {port} is in use")
                } else {
                    format!("failed to bind 127.0.0.1:{port}: {e}")
                };
                *self.last_error.lock().await = Some(msg.clone());
                self.emit_status(false, port, Some(msg.clone()));
                return Err(msg);
            }
        };

        let api_dyn: Arc<dyn cdt_api::DataApi> = self.api.clone();
        let state = AppState::new(api_dyn, EVENT_BRIDGE_CAPACITY);
        let events_tx = state.events_tx.clone();
        let file_rx = self.api.subscribe_file_changes();
        let todo_rx = self.api.subscribe_todo_changes();
        let error_rx = self.api.subscribe_detected_errors();
        let metadata_rx = self.api.subscribe_session_metadata();
        let context_rx = self.api.subscribe_context_changed();
        spawn_event_bridge(
            events_tx,
            file_rx,
            todo_rx,
            error_rx,
            metadata_rx,
            context_rx,
        );

        let static_serve = self.static_serve.clone();
        let task = tokio::spawn(async move {
            if let Err(e) = serve_with_listener(state, listener, static_serve).await {
                tracing::warn!(
                    target: "cdt_tauri::server_mode",
                    error = %e,
                    "axum serve exited"
                );
            }
        });

        *handle_guard = Some(ServerHandle { task, port });
        drop(handle_guard);
        *self.last_error.lock().await = None;

        self.emit_status(true, port, None);
        Ok(())
    }

    /// 用户显式关闭 server-mode：先写 `enabled=false` 用户意图，再关闭运行时 server。
    pub async fn stop(&self) -> Result<(), String> {
        let _lifecycle_guard = self.lifecycle.lock().await;
        if let Err(e) = self.api.set_http_server_enabled(false).await {
            tracing::warn!(
                target: "cdt_tauri::server_mode",
                error = %e,
                "failed to persist httpServer.enabled=false"
            );
        }

        self.shutdown_runtime_only().await;
        let port = self.persisted_port().await;
        self.emit_status(false, port, None);
        Ok(())
    }

    /// 仅关闭运行时 server，不改持久化配置。app 退出路径使用本函数，保留
    /// `enabled=true` 用户意图，让下次启动仍能自动恢复。
    pub async fn shutdown_runtime_only(&self) {
        let mut handle_guard = self.handle.lock().await;
        if let Some(old) = handle_guard.take() {
            old.task.abort();
            let _ = old.task.await;
        }
        drop(handle_guard);
        *self.last_error.lock().await = None;
    }

    /// 当前 server 状态快照——`running` 反映运行时 task 是否存活；`port` 优先取
    /// 运行时绑定值，否则取持久化值；`lastError` 取最近一次启动失败文案。
    pub async fn status(&self) -> ServerStatus {
        let handle_guard = self.handle.lock().await;
        let running = handle_guard.is_some();
        let runtime_port = handle_guard.as_ref().map(|h| h.port);
        drop(handle_guard);

        let port = match runtime_port {
            Some(p) => p,
            None => self.persisted_port().await,
        };
        let last_error = self.last_error.lock().await.clone();
        ServerStatus {
            running,
            port,
            last_error,
        }
    }

    /// `setup` 阶段调用：若 `HttpServerConfig.enabled = true` 自动尝试启动。
    ///
    /// 自动启动失败 SHALL 仅 `tracing::warn!` + emit `http-server-status` 事件，
    /// **不**阻塞 app 启动；`enabled` 字段 SHALL 保持 `true`（用户意图未变）。
    pub async fn restore_if_enabled(&self) {
        let _lifecycle_guard = self.lifecycle.lock().await;
        let cfg = match self.api.http_server_config().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    target: "cdt_tauri::server_mode",
                    error = %e,
                    "failed to read httpServer config, skip auto-restore"
                );
                return;
            }
        };
        if !cfg.enabled {
            return;
        }
        if let Err(msg) = self.start_runtime_only(cfg.port).await {
            tracing::warn!(
                target: "cdt_tauri::server_mode",
                port = cfg.port,
                error = %msg,
                "auto-restore failed; enabled=true preserved"
            );
        }
    }

    async fn persisted_port(&self) -> u16 {
        self.api
            .http_server_config()
            .await
            .map(|c| c.port)
            .unwrap_or(3456)
    }

    fn emit_status(&self, running: bool, port: u16, last_error: Option<String>) {
        let payload = ServerStatus {
            running,
            port,
            last_error,
        };
        self.emitter.emit_status("http-server-status", &payload);
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use super::*;
    use cdt_api::LocalDataApi;
    use cdt_config::{ConfigManager, NotificationManager};
    use cdt_discover::{ProjectScanner, local_handle};
    use cdt_ssh::SshConnectionManager;

    /// 单元测试用 emitter——把 emit 调用录到 Vec 让断言对比。
    struct RecordingEmitter {
        events: StdMutex<Vec<(String, ServerStatus)>>,
    }

    impl RecordingEmitter {
        fn new() -> Self {
            Self {
                events: StdMutex::new(Vec::new()),
            }
        }

        fn snapshot(&self) -> Vec<(String, ServerStatus)> {
            self.events.lock().unwrap().clone()
        }
    }

    impl StatusEmitter for RecordingEmitter {
        fn emit_status(&self, event: &str, payload: &ServerStatus) {
            self.events
                .lock()
                .unwrap()
                .push((event.to_string(), payload.clone()));
        }
    }

    async fn build_state_with_tempdir()
    -> (Arc<ServerState>, Arc<RecordingEmitter>, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.json");
        let mut config_mgr = ConfigManager::new(Some(config_path));
        config_mgr.load().await.unwrap();

        let notif_mgr = NotificationManager::new(Some(tmp.path().join("notifications.json")));
        let ssh_mgr = SshConnectionManager::new();
        let fs = local_handle();
        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        let scanner = ProjectScanner::new(fs, projects_dir);
        let api = Arc::new(LocalDataApi::new(scanner, config_mgr, notif_mgr, ssh_mgr));

        let emitter = Arc::new(RecordingEmitter::new());
        let state = Arc::new(ServerState::with_emitter(
            api.clone(),
            StaticServe::None,
            emitter.clone() as Arc<dyn StatusEmitter>,
        ));
        (state, emitter, tmp)
    }

    #[tokio::test]
    async fn status_default_running_false_port_default() {
        let (state, _emitter, _tmp) = build_state_with_tempdir().await;
        let s = state.status().await;
        assert!(!s.running);
        assert_eq!(s.port, 3456, "default port from HttpServerConfig");
        assert!(s.last_error.is_none());
    }

    #[tokio::test]
    async fn start_with_invalid_port_rejected_does_not_persist() {
        let (state, _emitter, _tmp) = build_state_with_tempdir().await;
        let err = state.start(80).await.unwrap_err();
        assert!(
            err.contains("1024") || err.to_lowercase().contains("range"),
            "validation error mentions port range, got: {err}"
        );
        // 持久化保持原值
        let cfg = state.api.http_server_config().await.unwrap();
        assert!(!cfg.enabled);
        assert_eq!(cfg.port, 3456);
    }

    #[tokio::test]
    async fn start_then_stop_persists_then_resets_enabled() {
        let (state, emitter, _tmp) = build_state_with_tempdir().await;

        // bind 到 OS 分配的闲端口（127.0.0.1:0）会被 validate_http_port 拒绝，
        // 这里改用一个高位端口；冲突时跳过测试避免 CI 波动。
        let port = 38765;
        if tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .is_err()
        {
            eprintln!("port {port} busy, skip");
            return;
        }

        state.start(port).await.unwrap();
        let s = state.status().await;
        assert!(s.running);
        assert_eq!(s.port, port);
        assert!(s.last_error.is_none());
        let cfg = state.api.http_server_config().await.unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.port, port);

        state.stop().await.unwrap();
        let s = state.status().await;
        assert!(!s.running);
        // status.port fallback to persisted（stop 不改 port）
        assert_eq!(s.port, port);
        let cfg = state.api.http_server_config().await.unwrap();
        assert!(!cfg.enabled);
        assert_eq!(cfg.port, port, "stop SHALL 保留上次 port");

        // emit 至少两条：start success + stop
        let events = emitter.snapshot();
        assert!(events.len() >= 2, "expected ≥2 emits, got {events:?}");
        assert_eq!(events[0].0, "http-server-status");
        assert!(events[0].1.running);
        assert!(!events.last().unwrap().1.running);
    }

    #[tokio::test]
    async fn stop_when_not_running_is_idempotent() {
        let (state, _emitter, _tmp) = build_state_with_tempdir().await;
        state.stop().await.unwrap();
        let cfg = state.api.http_server_config().await.unwrap();
        assert!(!cfg.enabled, "stop SHALL 写 enabled=false 保持一致");
    }

    #[tokio::test]
    async fn start_with_port_in_use_returns_specific_error_and_records_last_error() {
        let (state, emitter, _tmp) = build_state_with_tempdir().await;

        let port = 38766;
        // 先占用端口
        let _hog = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
            Ok(l) => l,
            Err(_) => {
                eprintln!("port {port} busy from environment, skip");
                return;
            }
        };

        let err = state.start(port).await.unwrap_err();
        assert!(
            err.contains("is in use"),
            "expected 'is in use' error, got: {err}"
        );

        let s = state.status().await;
        assert!(!s.running);
        assert_eq!(s.last_error.as_deref(), Some(err.as_str()));

        // 持久化 enabled=false 不变
        let cfg = state.api.http_server_config().await.unwrap();
        assert!(!cfg.enabled, "启动失败 SHALL NOT 持久化 enabled=true");
        assert_eq!(cfg.port, 3456);

        // emit 至少一条 failure
        let events = emitter.snapshot();
        let last = events.last().unwrap();
        assert!(!last.1.running);
        assert!(last.1.last_error.is_some());
    }

    #[tokio::test]
    async fn stop_queued_after_start_wins_persisted_enabled() {
        let (state, _emitter, _tmp) = build_state_with_tempdir().await;

        let port = 38773;
        if tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .is_err()
        {
            eprintln!("port {port} busy, skip");
            return;
        }

        let lifecycle_guard = state.lifecycle.lock().await;
        let start_state = state.clone();
        let start_task = tokio::spawn(async move { start_state.start(port).await });
        tokio::task::yield_now().await;
        let stop_state = state.clone();
        let stop_task = tokio::spawn(async move { stop_state.stop().await });
        drop(lifecycle_guard);

        start_task.await.unwrap().unwrap();
        stop_task.await.unwrap().unwrap();

        let cfg = state.api.http_server_config().await.unwrap();
        assert!(!cfg.enabled, "queued stop SHALL win over prior start");
        assert_eq!(cfg.port, port);
        assert!(!state.status().await.running);
    }

    #[tokio::test]
    async fn second_start_aborts_first_handle() {
        let (state, _emitter, _tmp) = build_state_with_tempdir().await;

        let port1 = 38767;
        let port2 = 38768;
        for p in [port1, port2] {
            if tokio::net::TcpListener::bind(("127.0.0.1", p))
                .await
                .is_err()
            {
                eprintln!("port {p} busy, skip");
                return;
            }
        }

        state.start(port1).await.unwrap();
        assert!(state.status().await.running);

        // 第二次 start 应该 abort 旧 handle 再 bind 新 port
        state.start(port2).await.unwrap();
        let s = state.status().await;
        assert!(s.running);
        assert_eq!(s.port, port2, "新 port 接管");

        state.stop().await.unwrap();
    }

    #[tokio::test]
    async fn second_start_same_port_waits_old_task_release() {
        let (state, _emitter, _tmp) = build_state_with_tempdir().await;

        let port = 38771;
        if tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .is_err()
        {
            eprintln!("port {port} busy, skip");
            return;
        }

        state.start(port).await.unwrap();
        // 第二次同端口 start 应先 abort+await 旧 task，再 bind 新 listener。
        state.start(port).await.unwrap();
        let s = state.status().await;
        assert!(s.running);
        assert_eq!(s.port, port);
        state.stop().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_runtime_only_preserves_enabled_intent() {
        let (state, _emitter, _tmp) = build_state_with_tempdir().await;

        let port = 38772;
        if tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .is_err()
        {
            eprintln!("port {port} busy, skip");
            return;
        }

        state.start(port).await.unwrap();
        assert!(state.api.http_server_config().await.unwrap().enabled);

        state.shutdown_runtime_only().await;
        let cfg = state.api.http_server_config().await.unwrap();
        assert!(cfg.enabled, "app exit SHALL 保留 enabled=true 用户意图");
        assert_eq!(cfg.port, port);
        assert!(!state.status().await.running);
    }

    #[tokio::test]
    async fn start_with_invalid_port_sets_last_error_and_emits() {
        let (state, emitter, _tmp) = build_state_with_tempdir().await;
        let err = state.start(80).await.unwrap_err();
        let s = state.status().await;
        assert_eq!(s.last_error.as_deref(), Some(err.as_str()));
        let events = emitter.snapshot();
        let last = events
            .last()
            .expect("validation failure should emit status");
        assert!(!last.1.running);
        assert_eq!(last.1.last_error.as_deref(), Some(err.as_str()));
    }

    #[tokio::test]
    async fn successful_start_after_failure_resets_last_error() {
        let (state, _emitter, _tmp) = build_state_with_tempdir().await;

        let port_busy = 38769;
        let port_ok = 38770;
        let _hog = match tokio::net::TcpListener::bind(("127.0.0.1", port_busy)).await {
            Ok(l) => l,
            Err(_) => {
                eprintln!("port {port_busy} busy from environment, skip");
                return;
            }
        };
        if tokio::net::TcpListener::bind(("127.0.0.1", port_ok))
            .await
            .is_err()
        {
            eprintln!("port {port_ok} busy, skip");
            return;
        }

        // 第一次 start 失败 → lastError 设置
        let _ = state.start(port_busy).await.unwrap_err();
        let s = state.status().await;
        assert!(s.last_error.is_some());

        // 第二次 start 成功 → lastError 重置
        state.start(port_ok).await.unwrap();
        let s = state.status().await;
        assert!(s.running);
        assert!(s.last_error.is_none(), "成功启动后 SHALL 重置 lastError");

        state.stop().await.unwrap();
    }
}
