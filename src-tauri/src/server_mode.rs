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

use std::path::PathBuf;
use std::sync::Arc;

use cdt_api::{AppState, LocalDataApi, serve_with_listener, spawn_event_bridge};
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

/// HTTP server 事件桥广播容量。与 `cdt-cli` / 测试一致。
const EVENT_BRIDGE_CAPACITY: usize = 128;

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
    handle: Mutex<Option<ServerHandle>>,
    last_error: Mutex<Option<String>>,
    api: Arc<LocalDataApi>,
    static_dir: Option<PathBuf>,
    emitter: Arc<dyn StatusEmitter>,
}

impl ServerState {
    pub fn new(
        api: Arc<LocalDataApi>,
        static_dir: Option<PathBuf>,
        app_handle: AppHandle,
    ) -> Self {
        Self::with_emitter(api, static_dir, Arc::new(app_handle))
    }

    /// 测试可见构造器：注入任意 `StatusEmitter` 实现验证 emit 行为。
    pub fn with_emitter(
        api: Arc<LocalDataApi>,
        static_dir: Option<PathBuf>,
        emitter: Arc<dyn StatusEmitter>,
    ) -> Self {
        Self {
            handle: Mutex::new(None),
            last_error: Mutex::new(None),
            api,
            static_dir,
            emitter,
        }
    }

    /// 启动 HTTP server bind 到 `127.0.0.1:port`。
    ///
    /// 串行化：调用方持有 `Mutex<Option<ServerHandle>>` 的 lock 期间没有其它
    /// `start` / `stop` 能进入；进入后第一步 abort 旧 handle 再 bind 新 listener。
    pub async fn start(&self, port: u16) -> Result<(), String> {
        validate_http_port(port).map_err(|e| e.to_string())?;

        let mut handle_guard = self.handle.lock().await;
        if let Some(old) = handle_guard.take() {
            old.task.abort();
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
        spawn_event_bridge(events_tx, file_rx, todo_rx, error_rx);

        let static_dir = self.static_dir.clone();
        let task = tokio::spawn(async move {
            if let Err(e) = serve_with_listener(state, listener, static_dir).await {
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

        self.emit_status(true, port, None);
        Ok(())
    }

    /// 优雅关闭已运行的 server task；未运行时返回 `Ok` 保持幂等。
    pub async fn stop(&self) -> Result<(), String> {
        let mut handle_guard = self.handle.lock().await;
        if let Some(old) = handle_guard.take() {
            old.task.abort();
        }
        drop(handle_guard);
        *self.last_error.lock().await = None;

        if let Err(e) = self.api.set_http_server_enabled(false).await {
            tracing::warn!(
                target: "cdt_tauri::server_mode",
                error = %e,
                "failed to persist httpServer.enabled=false"
            );
        }
        let port = self.persisted_port().await;
        self.emit_status(false, port, None);
        Ok(())
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
        if let Err(msg) = self.start(cfg.port).await {
            tracing::warn!(
                target: "cdt_tauri::server_mode",
                port = cfg.port,
                error = %msg,
                "auto-restore failed; enabled=true preserved"
            );
            // start 内部已经 set last_error + emit；这里仅日志。
            // SHALL **不**改 enabled——用户意图保持。set_http_server_enabled 在
            // start 失败路径里**不**被调，自然不会写盘。
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

    async fn build_state_with_tempdir() -> (Arc<ServerState>, Arc<RecordingEmitter>, tempfile::TempDir)
    {
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
            None,
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
    async fn second_start_aborts_first_handle() {
        let (state, _emitter, _tmp) = build_state_with_tempdir().await;

        let port1 = 38767;
        let port2 = 38768;
        for p in [port1, port2] {
            if tokio::net::TcpListener::bind(("127.0.0.1", p)).await.is_err() {
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
        assert!(
            s.last_error.is_none(),
            "成功启动后 SHALL 重置 lastError"
        );

        state.stop().await.unwrap();
    }
}
