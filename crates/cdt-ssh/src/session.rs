//! 真 SSH session manager（持有 russh handle + SFTP + remote home + 状态广播）。
//!
//! Spec：`openspec/specs/ssh-remote-context/spec.md` 全部 SSH 真握手 / 断开 / 状态推送
//! Requirement。
//!
//! Phase A 的 `connection.rs::SshConnectionManager` 是 placeholder（仅状态机 + alias
//! 解析），保留供现有 `cdt-api` 暂不破坏使用；Phase C task 9.x 时由本 `SshSessionManager`
//! 替换。两者命名错开（Connection vs Session）便于 grep。
//!
//! 5 阶段 connect 流程（design.md D1 + spec `Requirement: Establish and tear down`）：
//! 1. TCP probe 5s    `tokio::net::TcpStream::connect_timeout`
//! 2. russh transport 握手
//! 3. 鉴权候选链      `auth::run_auth_chain` + `try_authenticate_via_handle`
//! 4. SFTP open 8s    `Channel::request_subsystem("sftp")` + `SftpSession::new`
//! 5. remote home probe 4 fallback (`<home>/.claude/projects` / `/home/<user>/...` /
//!    `/Users/<user>/...` / `/root/.claude/projects`)
//!
//! 外层硬超时 25s（spec）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use russh::client;
use russh::keys::PrivateKeyWithHashAlg;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, broadcast};
use tokio::time::timeout;

use crate::auth::{Platform, build_candidates};
use crate::error::{AuthAttempt, AuthOutcome, AuthSource, SshError, TimeoutStage};
use crate::host_resolver::resolve_host_via_ssh_g;
use crate::provider::SshFileSystemProvider;
use crate::request::SshConnectRequest;

/// `connect()` 各阶段超时（design.md D1 + spec `Requirement: Establish`）。
pub const TCP_TIMEOUT: Duration = Duration::from_secs(5);
pub const SFTP_TIMEOUT: Duration = Duration::from_secs(8);
pub const OUTER_TIMEOUT: Duration = Duration::from_secs(25);
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
/// 状态广播 `broadcast::channel` 容量（design.md D6 备注 128）。
pub const STATUS_CHANNEL_CAP: usize = 128;

/// 与 `connect` payload 对齐 `SshSessionManager::connect` 的状态变更事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshStatusChange {
    pub context_id: String,
    pub status: SshStatus,
    /// connecting 阶段携带已尝试候选源 outcome（spec `Connecting state carries auth chain progress`）；
    /// 成功时省略；error 时含全部 attempts。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth_chain: Vec<AuthAttempt>,
    /// error 时附带结构化错误（IPC 序列化）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<SshError>,
}

/// 单个 context 的连接状态（与 `connection::ConnectionState` 平行；新形态走 `SshStatus`
/// 让事件 payload schema 与 spec 对齐）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SshStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// 活跃 context 切换事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextChanged {
    pub active_context_id: Option<String>,
    pub kind: ContextKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContextKind {
    Local,
    Ssh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshContextState {
    pub context_id: String,
    pub host: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    pub remote_home: PathBuf,
    pub status: SshStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth_chain: Vec<AuthAttempt>,
}

/// 单个 SSH 连接持有的资源（真 russh handle + sftp 句柄 + 远端 home 探测结果）。
///
/// `remote_home` / `host` / `port` / `user` 字段供 Phase C task 9.x 的 IPC 状态
/// 查询读取（UI 展示当前连接元信息）；当前 phase 只 disconnect 时用到 handle。
#[allow(dead_code)]
struct SshSessionResources {
    /// russh client handle —— 真协议栈，Drop 时自动断 transport。
    handle: Option<client::Handle<RusshClientHandler>>,
    /// SFTP provider。生产路径包装 `russh_sftp::SftpSession`，测试路径可注入 fake。
    provider: SshFileSystemProvider,
    /// 远端 `~/.claude/projects` 路径（4 fallback 中第一个存在的）。
    remote_home: PathBuf,
    status: SshStatus,
    /// 连接元信息 —— 显示给 UI（Phase C task 9.x 读）。
    host: String,
    port: u16,
    user: Option<String>,
    /// 鉴权链结果（成功路径下 attempts 含 success；error 路径下 attempts 全部失败）。
    auth_chain: Vec<AuthAttempt>,
    /// SSH host 的稳定身份签名 —— `connect_inner` 在 stage 0 之后立即按
    /// `SshConfigDigestInput::from(&resolved)` + `HostSignature::from_ssh_config_fields`
    /// 计算并存入；用于 `SshSessionManager::context_id(&str)` 派生 `ContextId::ssh(...)`
    /// 给 fs-related cache 作 key 前缀（详 change `metadata-cache-context-prefix` design D6）。
    host_signature: cdt_fs::HostSignature,
}

/// `russh::client::Handler` 实现：v1 SHALL 接受任意 server key（host key 校验留 v2）。
///
/// design.md OQ：v1 不持久化 `known_hosts` 也不弹窗确认；与 TS 原版当前行为一致
/// （TS 原版同样接受任意 host key）。spec 未要求 host key 校验，留 v2。
struct RusshClientHandler;

impl client::Handler for RusshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// SSH session 管理器：真握手 + 资源生命周期 + 状态广播。
///
/// 与 `connection::SshConnectionManager`（占位）独立——Phase C 时 `cdt-api` 切换。
pub struct SshSessionManager {
    /// `context_id` → 资源；连接成功才插入。
    sessions: Arc<Mutex<HashMap<String, SshSessionResources>>>,
    /// 当前活跃 context（None=Local；Some(ctx)=Ssh<ctx>）。
    active: Arc<Mutex<Option<String>>>,
    failed_states: Arc<Mutex<HashMap<String, SshContextState>>>,
    failed_auth_chains: Arc<Mutex<HashMap<String, Vec<AuthAttempt>>>>,
    /// 状态变更广播。
    status_tx: broadcast::Sender<SshStatusChange>,
    /// 活跃 context 切换广播。
    context_tx: broadcast::Sender<ContextChanged>,
}

impl Default for SshSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SshSessionManager {
    pub fn new() -> Self {
        let (status_tx, _) = broadcast::channel(STATUS_CHANNEL_CAP);
        let (context_tx, _) = broadcast::channel(STATUS_CHANNEL_CAP);
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            active: Arc::new(Mutex::new(None)),
            failed_states: Arc::new(Mutex::new(HashMap::new())),
            failed_auth_chains: Arc::new(Mutex::new(HashMap::new())),
            status_tx,
            context_tx,
        }
    }

    /// 订阅 `ssh_status` 事件流（多订阅者各自独立收事件 — spec Scenario "Status
    /// broadcast to multiple subscribers"）。
    pub fn subscribe_status(&self) -> broadcast::Receiver<SshStatusChange> {
        self.status_tx.subscribe()
    }

    /// 订阅 `context_changed` 事件流。
    pub fn subscribe_context_changed(&self) -> broadcast::Receiver<ContextChanged> {
        self.context_tx.subscribe()
    }

    fn emit_status(&self, change: SshStatusChange) {
        // broadcast::Sender::send 在没有订阅者时会返 Err；忽略——状态广播是 fire-and-forget
        let _ = self.status_tx.send(change);
    }

    fn emit_context(&self, change: ContextChanged) {
        let _ = self.context_tx.send(change);
    }

    async fn record_error_context(
        &self,
        context_id: &str,
        request: &SshConnectRequest,
        err: &SshError,
    ) {
        let mut auth_chain = error_auth_chain(err);
        if auth_chain.is_empty() {
            auth_chain = self
                .failed_auth_chains
                .lock()
                .await
                .get(context_id)
                .cloned()
                .unwrap_or_default();
        }
        self.failed_states.lock().await.insert(
            context_id.to_owned(),
            SshContextState {
                context_id: context_id.to_owned(),
                host: request.host.clone(),
                port: request.port.unwrap_or(22),
                username: request.username.clone(),
                remote_home: PathBuf::new(),
                status: SshStatus::Error,
                auth_chain,
            },
        );
    }

    /// 获取活跃 context id（None 表示 Local）。
    pub async fn active_context_id(&self) -> Option<String> {
        self.active.lock().await.clone()
    }

    /// 列出所有已注册 SSH context id。
    pub async fn registered_context_ids(&self) -> Vec<String> {
        self.sessions.lock().await.keys().cloned().collect()
    }

    pub async fn context_states(&self) -> Vec<SshContextState> {
        let mut states: Vec<SshContextState> = self
            .sessions
            .lock()
            .await
            .iter()
            .map(|(context_id, resources)| SshContextState {
                context_id: context_id.clone(),
                host: resources.host.clone(),
                port: resources.port,
                username: resources.user.clone(),
                remote_home: resources.remote_home.clone(),
                status: resources.status.clone(),
                auth_chain: resources.auth_chain.clone(),
            })
            .collect();
        states.extend(self.failed_states.lock().await.values().cloned());
        states
    }

    pub async fn context_state(&self, context_id: &str) -> Option<SshContextState> {
        if let Some(state) =
            self.sessions
                .lock()
                .await
                .get(context_id)
                .map(|resources| SshContextState {
                    context_id: context_id.to_owned(),
                    host: resources.host.clone(),
                    port: resources.port,
                    username: resources.user.clone(),
                    remote_home: resources.remote_home.clone(),
                    status: resources.status.clone(),
                    auth_chain: resources.auth_chain.clone(),
                })
        {
            return Some(state);
        }
        self.failed_states.lock().await.get(context_id).cloned()
    }

    pub async fn provider(&self, context_id: &str) -> Option<SshFileSystemProvider> {
        self.sessions
            .lock()
            .await
            .get(context_id)
            .map(|resources| resources.provider.clone())
    }

    /// 派生当前 SSH context 的 `ContextId::ssh(host_signature, remote_home)` —— 给
    /// fs-related cache 作 key 前缀。未注册（未 connect 或已 disconnect）的 context
    /// 返回 `None`；本方法 SHALL NOT 调用 `resolve_host_via_ssh_g` 子进程
    /// （`HostSignature` 在 `connect_inner` 已计算并缓存于 `SshSessionResources`）。
    ///
    /// 详 change `metadata-cache-context-prefix` ssh-remote-context spec delta
    /// §`SshSessionManager 暴露 HostSignature 派生的 ContextId 查询`。
    pub async fn context_id(&self, context_id: &str) -> Option<cdt_fs::ContextId> {
        self.sessions
            .lock()
            .await
            .get(context_id)
            .map(|r| cdt_fs::ContextId::ssh(r.host_signature.clone(), r.remote_home.clone()))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_test_context(
        &self,
        context_id: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        username: Option<String>,
        remote_home: PathBuf,
        provider: SshFileSystemProvider,
        host_signature: Option<cdt_fs::HostSignature>,
    ) {
        let context_id = context_id.into();
        let host_str = host.into();
        let host_signature = host_signature.unwrap_or_else(|| {
            // fake digest：用真算法 + (host, port, user) 字段构造，确保不同 host 自然
            // 产不同 digest；不直接造 raw bytes，避免 fake 与生产路径行为分叉。
            cdt_fs::HostSignature::from_ssh_config_fields(&cdt_fs::SshConfigDigestInput {
                hostname: host_str.clone(),
                port,
                user: username.clone().unwrap_or_default(),
                identity_files: vec![],
                proxyjump: None,
                proxycommand: None,
                hostkeyalias: None,
            })
        });
        self.sessions.lock().await.insert(
            context_id.clone(),
            SshSessionResources {
                handle: None,
                provider,
                remote_home,
                status: SshStatus::Connected,
                host: host_str,
                port,
                user: username,
                auth_chain: vec![],
                host_signature,
            },
        );
        self.set_active_inner(Some(context_id)).await;
    }

    /// 真握手 5 阶段连接到远端 host。
    ///
    /// spec `Requirement: Establish and tear down SSH connections` + design.md D1 5 阶段。
    pub async fn connect(&self, request: SshConnectRequest) -> Result<String, SshError> {
        let context_id = request
            .context_id
            .clone()
            .unwrap_or_else(|| request.host.clone());

        // 强制单 active SSH：连接新 host 前先 disconnect 当前 active SSH context
        // （spec Scenario "Connecting new host while another SSH context is active"）。
        if let Some(prev) = self.active.lock().await.clone() {
            if prev != context_id {
                let _ = self.disconnect(&prev).await;
            }
        }

        self.emit_status(SshStatusChange {
            context_id: context_id.clone(),
            status: SshStatus::Connecting,
            auth_chain: vec![],
            error: None,
        });

        let request_for_error_state = request.clone();
        let outer = timeout(OUTER_TIMEOUT, self.connect_inner(&context_id, request)).await;

        let resources = match outer {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                self.record_error_context(&context_id, &request_for_error_state, &e)
                    .await;
                self.emit_status(SshStatusChange {
                    context_id: context_id.clone(),
                    status: SshStatus::Error,
                    auth_chain: error_auth_chain(&e),
                    error: Some(e.clone()),
                });
                return Err(e);
            }
            Err(_) => {
                let err = SshError::Timeout {
                    stage: TimeoutStage::Tcp,
                };
                self.record_error_context(&context_id, &request_for_error_state, &err)
                    .await;
                self.emit_status(SshStatusChange {
                    context_id: context_id.clone(),
                    status: SshStatus::Error,
                    auth_chain: vec![],
                    error: Some(err.clone()),
                });
                return Err(err);
            }
        };

        self.failed_states.lock().await.remove(&context_id);
        self.failed_auth_chains.lock().await.remove(&context_id);
        self.sessions
            .lock()
            .await
            .insert(context_id.clone(), resources);
        self.set_active_inner(Some(context_id.clone())).await;
        self.emit_status(SshStatusChange {
            context_id: context_id.clone(),
            status: SshStatus::Connected,
            auth_chain: vec![],
            error: None,
        });

        Ok(context_id)
    }

    /// 测试连通性：跑同 connect 流程，成功立即关闭，**不**注册 active context。
    /// 返回 `auth_chain` 让 UI 显示"试过哪些候选源"诊断（spec `Test connection without persisting`）。
    pub async fn test_connection(
        &self,
        request: SshConnectRequest,
    ) -> Result<Vec<AuthAttempt>, SshError> {
        let context_id = request
            .context_id
            .clone()
            .unwrap_or_else(|| format!("{}-test", request.host));

        let outer = timeout(OUTER_TIMEOUT, self.connect_inner(&context_id, request)).await;
        match outer {
            Ok(Ok(resources)) => {
                let chain = resources.auth_chain.clone();
                drop(resources); // 立即关闭
                Ok(chain)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(SshError::Timeout {
                stage: TimeoutStage::Tcp,
            }),
        }
    }

    async fn set_active_inner(&self, ctx: Option<String>) {
        *self.active.lock().await = ctx.clone();
        self.emit_context(ContextChanged {
            active_context_id: ctx.clone(),
            kind: if ctx.is_some() {
                ContextKind::Ssh
            } else {
                ContextKind::Local
            },
        });
    }

    /// 5 阶段内部实现（不带外层 timeout）。
    ///
    /// `_context_id` 仅用于错误诊断的 `tracing::span`（v1 暂不接 span，留前缀避免 warn）。
    async fn connect_inner(
        &self,
        context_id: &str,
        request: SshConnectRequest,
    ) -> Result<SshSessionResources, SshError> {
        // 阶段 0：解析 host alias（`ssh -G` 委托）
        let resolved = resolve_host_via_ssh_g(&request.host).await?;
        // stage 0 resolve 后立即计算 HostSignature —— 用于 fs-related cache key
        // 前缀（详 change `metadata-cache-context-prefix` design D6）。无论 ssh -G
        // 成功还是 degraded fallback 路径都会产 32-byte digest，by-design 落不同
        // cache namespace 防串扰。
        let host_signature_input: cdt_fs::SshConfigDigestInput = (&resolved).into();
        let host_signature = cdt_fs::HostSignature::from_ssh_config_fields(&host_signature_input);
        let port = resolved_connect_port(request.port, resolved.port);
        let username = request
            .username
            .clone()
            .or_else(|| resolved.user.clone())
            .unwrap_or_else(whoami_fallback);
        let host = if resolved.host.is_empty() {
            request.host.clone()
        } else {
            resolved.host.clone()
        };

        // 阶段 1：TCP probe 5s
        let socket_addr = format!("{host}:{port}");
        let tcp = match timeout(TCP_TIMEOUT, TcpStream::connect(&socket_addr)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                return Err(SshError::Tcp {
                    host: host.clone(),
                    reason: e.to_string(),
                });
            }
            Err(_) => {
                return Err(SshError::Timeout {
                    stage: TimeoutStage::Tcp,
                });
            }
        };

        // 阶段 2：russh transport 握手
        let config = Arc::new(client::Config::default());
        let mut handle = client::connect_stream(config, tcp, RusshClientHandler)
            .await
            .map_err(|e| SshError::Tcp {
                host: host.clone(),
                reason: format!("russh transport: {e}"),
            })?;

        // 阶段 3：鉴权候选链（inline 调度——`run_auth_chain` 的 callback 形态与
        // `&mut handle` 串行 borrow 冲突，公共 API `run_auth_chain` 仍保留供调度逻辑
        // 单测；此处直接 inline 跑同一逻辑：first success 立即 break，否则全失败抛
        // `AuthExhausted`）
        let candidates = build_candidates(&resolved, Platform::current(), request.auth_method);
        let mut auth_chain: Vec<AuthAttempt> = Vec::with_capacity(candidates.len());
        let mut authenticated = false;
        for source in candidates {
            let started = Instant::now();
            let outcome = try_authenticate_via_handle(
                &mut handle,
                source.clone(),
                &username,
                request.password.as_deref(),
            )
            .await;
            let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let is_success = matches!(outcome, AuthOutcome::Success);
            auth_chain.push(AuthAttempt {
                source,
                outcome,
                elapsed_ms,
            });
            if is_success {
                authenticated = true;
                break;
            }
        }
        if !authenticated {
            return Err(SshError::AuthExhausted {
                attempts: auth_chain,
            });
        }
        self.failed_auth_chains
            .lock()
            .await
            .insert(context_id.to_owned(), auth_chain.clone());

        // 阶段 4：SFTP open 8s
        let sftp = match timeout(SFTP_TIMEOUT, open_sftp(&mut handle)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                return Err(SshError::SftpInit {
                    reason: e.to_string(),
                });
            }
            Err(_) => {
                return Err(SshError::Timeout {
                    stage: TimeoutStage::Sftp,
                });
            }
        };
        let sftp = Arc::new(Mutex::new(sftp));

        // 阶段 5：remote home probe（4 fallback）
        let remote_home = probe_remote_home(&sftp, &mut handle, &username).await?;

        let provider = SshFileSystemProvider::new(context_id.to_owned(), sftp, remote_home.clone());
        Ok(SshSessionResources {
            handle: Some(handle),
            provider,
            remote_home,
            status: SshStatus::Connected,
            host,
            port,
            user: Some(username),
            auth_chain,
            host_signature,
        })
    }

    /// 主动断开一个 context：关闭 SFTP / transport / TCP；若被断开的是 active 切回 Local。
    pub async fn disconnect(&self, context_id: &str) -> Result<(), SshError> {
        self.failed_states.lock().await.remove(context_id);
        self.failed_auth_chains.lock().await.remove(context_id);
        let resources = self.sessions.lock().await.remove(context_id);
        if let Some(r) = resources {
            drop(r.provider);
            if let Some(handle) = r.handle {
                let _ = handle
                    .disconnect(russh::Disconnect::ByApplication, "", "")
                    .await;
            }
            self.emit_status(SshStatusChange {
                context_id: context_id.to_owned(),
                status: SshStatus::Disconnected,
                auth_chain: vec![],
                error: None,
            });
        }
        // 若被断开的是 active context，切回 Local
        let mut active = self.active.lock().await;
        if active.as_deref() == Some(context_id) {
            *active = None;
            drop(active);
            self.emit_context(ContextChanged {
                active_context_id: None,
                kind: ContextKind::Local,
            });
        }
        Ok(())
    }

    /// 切换活跃 context；目标必须已存在于 sessions 或为 None（Local）。
    pub async fn switch_context(&self, target: Option<String>) -> Result<(), SshError> {
        if let Some(id) = target.as_ref() {
            if !self.sessions.lock().await.contains_key(id) {
                return Err(SshError::Config {
                    reason: format!("context {id} not registered"),
                });
            }
        }
        self.set_active_inner(target).await;
        Ok(())
    }

    /// 应用关闭时的优雅断开（spec `Graceful disconnect on app exit`）：并发断开所有
    /// SSH context，最长等待 `SHUTDOWN_TIMEOUT`（3s）。
    pub async fn shutdown_all(&self, deadline: Duration) {
        let ids: Vec<String> = self.sessions.lock().await.keys().cloned().collect();
        let futures = ids
            .into_iter()
            .map(|id| async move {
                let _ = self.disconnect(&id).await;
            })
            .collect::<Vec<_>>();

        let _ = timeout(deadline, futures::future::join_all(futures)).await;
    }
}

/// 提取 `error` 路径下的 auth chain（如果是 AuthExhausted）；其他错误返空。
fn error_auth_chain(err: &SshError) -> Vec<AuthAttempt> {
    match err {
        SshError::AuthExhausted { attempts } => attempts.clone(),
        _ => vec![],
    }
}

/// `username` fallback：当 `ssh -G` 无 user 字段且用户表单未填时——`whoami` 返回当前
/// shell 用户（Tauri 桌面进程一定有）。
fn whoami_fallback() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "root".into())
}

fn resolved_connect_port(request_port: Option<u16>, resolved_port: u16) -> u16 {
    request_port.filter(|p| *p != 22).unwrap_or(resolved_port)
}

fn expand_local_ssh_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s == "~" {
        return cdt_discover::home_dir().unwrap_or_else(|| PathBuf::from(s.as_ref()));
    }
    if let Some(rest) = s.strip_prefix("~/").or_else(|| s.strip_prefix("~\\")) {
        if let Some(home) = cdt_discover::home_dir() {
            return rest
                .split(['/', '\\'])
                .fold(home, |path, component| path.join(component));
        }
    }
    path.to_path_buf()
}

/// `russh::client::connect_stream` 替代 `connect` 用，把已建立的 `TcpStream` 直接传入。
mod _ensure_connect_stream_exists {
    // russh 0.52 提供 `client::connect_stream`，签名 `pub async fn connect_stream<H, S>(
    // config: Arc<Config>, stream: S, handler: H) -> Result<Handle<H>, Error>`；
    // 此处 mod 只是占位文档。
}

/// 真 SFTP open（阶段 4）。
async fn open_sftp(
    handle: &mut client::Handle<RusshClientHandler>,
) -> Result<SftpSession, russh::Error> {
    let channel = handle.channel_open_session().await?;
    channel.request_subsystem(true, "sftp").await?;
    SftpSession::new(channel.into_stream())
        .await
        .map_err(|e| russh::Error::IO(std::io::Error::other(e.to_string())))
}

/// 远端 home 探测：先发 `printf %s "$HOME"` 唯一允许在远端跑的命令拿到真 home，
/// 再依次试 `<home>/.claude/projects` / `/home/<user>/.claude/projects` /
/// `/Users/<user>/.claude/projects` / `/root/.claude/projects`。
async fn probe_remote_home(
    sftp: &Arc<Mutex<SftpSession>>,
    handle: &mut client::Handle<RusshClientHandler>,
    user: &str,
) -> Result<PathBuf, SshError> {
    // 通过 exec 拿真 $HOME（spec 显式允许的唯一远端命令）
    let real_home = run_remote_command(handle, "printf %s \"$HOME\"")
        .await
        .ok()
        .filter(|s| !s.is_empty());

    let mut tried: Vec<PathBuf> = Vec::new();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(h) = real_home.as_ref() {
        candidates.push(PathBuf::from(format!("{h}/.claude/projects")));
    }
    candidates.extend([
        PathBuf::from(format!("/home/{user}/.claude/projects")),
        PathBuf::from(format!("/Users/{user}/.claude/projects")),
        PathBuf::from("/root/.claude/projects"),
    ]);

    let sftp = sftp.lock().await;
    for path in &candidates {
        let path_str = path.to_string_lossy().into_owned();
        match sftp.metadata(&path_str).await {
            Ok(meta) if meta.is_dir() => return Ok(path.clone()),
            _ => tried.push(path.clone()),
        }
    }
    Err(SshError::RemoteHomeMissing { tried })
}

/// `printf %s "$HOME"` exec helper（极少数远端 home 探测场景；spec 显式允许）。
async fn run_remote_command(
    handle: &mut client::Handle<RusshClientHandler>,
    cmd: &str,
) -> Result<String, russh::Error> {
    let channel = handle.channel_open_session().await?;
    channel.exec(true, cmd).await?;
    let mut stdout: Vec<u8> = Vec::new();
    let mut reader = channel.into_stream();
    reader
        .read_to_end(&mut stdout)
        .await
        .map_err(russh::Error::IO)?;
    Ok(String::from_utf8_lossy(&stdout).trim().to_owned())
}

/// 真 russh 鉴权：把 `AuthSource` 映射到 `russh::client::Handle::authenticate_*`。
///
/// agent 路径走 `russh::keys::agent::client::AgentClient::connect`；私钥路径走
/// `russh::keys::load_secret_key` + `PrivateKeyWithHashAlg`；password 路径直传。
/// passphrase 加密私钥返 `Skipped("requires passphrase, use ssh-add")`，不弹窗（D9）。
async fn try_authenticate_via_handle(
    handle: &mut client::Handle<RusshClientHandler>,
    source: AuthSource,
    username: &str,
    password: Option<&str>,
) -> AuthOutcome {
    match source {
        AuthSource::Password => match password {
            None => AuthOutcome::Skipped("password not provided".into()),
            Some(pw) => match handle.authenticate_password(username, pw).await {
                Ok(success) if success.success() => AuthOutcome::Success,
                Ok(_) => AuthOutcome::Failure("permission denied".into()),
                Err(e) => AuthOutcome::Failure(e.to_string()),
            },
        },
        AuthSource::IdentityFile(path) | AuthSource::DefaultKey(path) => {
            authenticate_with_key(handle, &path, username).await
        }
        AuthSource::IdentityAgent(path) | AuthSource::OnePasswordAgent(path) => {
            authenticate_with_agent(handle, &path, username).await
        }
        AuthSource::EnvAgent => match std::env::var("SSH_AUTH_SOCK") {
            Ok(p) if !p.is_empty() => {
                authenticate_with_agent(handle, &PathBuf::from(p), username).await
            }
            _ => AuthOutcome::Skipped("SSH_AUTH_SOCK not set".into()),
        },
        AuthSource::LaunchctlAgent => match query_launchctl_ssh_auth_sock().await {
            Some(p) if !p.is_empty() => {
                authenticate_with_agent(handle, &PathBuf::from(p), username).await
            }
            _ => AuthOutcome::Skipped("launchctl SSH_AUTH_SOCK empty".into()),
        },
    }
}

async fn authenticate_with_key(
    handle: &mut client::Handle<RusshClientHandler>,
    path: &Path,
    username: &str,
) -> AuthOutcome {
    let path = expand_local_ssh_path(path);
    if !path.exists() {
        return AuthOutcome::Skipped(format!("not found: {}", path.display()));
    }
    let key = match russh::keys::load_secret_key(&path, None) {
        Ok(k) => k,
        Err(e) => {
            let msg = e.to_string();
            // russh-keys 对 passphrase 加密私钥的错误形态——保守按字符串匹配。
            if msg.to_lowercase().contains("passphrase") || msg.to_lowercase().contains("encrypted")
            {
                return AuthOutcome::Skipped("requires passphrase, use ssh-add".into());
            }
            return AuthOutcome::Failure(format!("decode key: {msg}"));
        }
    };
    let hash = match handle.best_supported_rsa_hash().await {
        Ok(h) => h.flatten(),
        Err(e) => return AuthOutcome::Failure(format!("rsa hash: {e}")),
    };
    let key_with_hash = PrivateKeyWithHashAlg::new(Arc::new(key), hash);
    match handle.authenticate_publickey(username, key_with_hash).await {
        Ok(success) if success.success() => AuthOutcome::Success,
        Ok(_) => AuthOutcome::Failure("permission denied".into()),
        Err(e) => AuthOutcome::Failure(e.to_string()),
    }
}

#[cfg(unix)]
async fn authenticate_with_agent(
    handle: &mut client::Handle<RusshClientHandler>,
    socket_path: &Path,
    username: &str,
) -> AuthOutcome {
    use russh::keys::agent::client::AgentClient;
    use tokio::net::UnixStream;

    let socket_path = expand_local_ssh_path(socket_path);
    if !socket_path.exists() {
        return AuthOutcome::Skipped(format!("agent socket missing: {}", socket_path.display()));
    }
    let stream = match UnixStream::connect(&socket_path).await {
        Ok(s) => s,
        Err(e) => return AuthOutcome::Failure(format!("agent connect: {e}")),
    };
    let mut agent = AgentClient::connect(stream);
    let identities = match agent.request_identities().await {
        Ok(ids) => ids,
        Err(e) => return AuthOutcome::Failure(format!("agent identities: {e}")),
    };
    if identities.is_empty() {
        return AuthOutcome::Skipped("agent has no identities".into());
    }
    for id in identities {
        match handle
            .authenticate_publickey_with(username, id, None, &mut agent)
            .await
        {
            Ok(success) if success.success() => return AuthOutcome::Success,
            Ok(_) => {}
            Err(e) => return AuthOutcome::Failure(format!("agent auth: {e}")),
        }
    }
    AuthOutcome::Failure("all agent identities rejected".into())
}

#[cfg(not(unix))]
async fn authenticate_with_agent(
    _handle: &mut client::Handle<RusshClientHandler>,
    _socket_path: &Path,
    _username: &str,
) -> AuthOutcome {
    std::future::ready(AuthOutcome::Skipped(
        "named-pipe agent not supported in v1".into(),
    ))
    .await
}

/// macOS `launchctl getenv SSH_AUTH_SOCK` 子进程包装（v1 D2 候选 3）。
async fn query_launchctl_ssh_auth_sock() -> Option<String> {
    use std::process::Stdio;
    use tokio::process::Command;

    let output = Command::new("launchctl")
        .args(["getenv", "SSH_AUTH_SOCK"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = timeout(Duration::from_secs(5), output).await.ok()?.ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthMethodKind;

    #[tokio::test]
    async fn manager_default_active_is_none_local() {
        let mgr = SshSessionManager::new();
        assert!(mgr.active_context_id().await.is_none());
        assert!(mgr.registered_context_ids().await.is_empty());
    }

    #[tokio::test]
    async fn switch_context_to_unregistered_returns_config_error() {
        let mgr = SshSessionManager::new();
        let err = mgr
            .switch_context(Some("ghost".into()))
            .await
            .expect_err("should fail");
        assert!(matches!(err, SshError::Config { .. }));
    }

    #[tokio::test]
    async fn switch_context_to_local_emits_event() {
        let mgr = SshSessionManager::new();
        let mut rx = mgr.subscribe_context_changed();
        mgr.switch_context(None).await.expect("ok");
        let evt = tokio::time::timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("event")
            .expect("ok");
        assert!(evt.active_context_id.is_none());
        assert_eq!(evt.kind, ContextKind::Local);
    }

    #[test]
    fn default_port_does_not_override_ssh_config_port() {
        assert_eq!(resolved_connect_port(None, 2200), 2200);
        assert_eq!(resolved_connect_port(Some(22), 2200), 2200);
        assert_eq!(resolved_connect_port(Some(2222), 2200), 2222);
    }

    #[test]
    fn expands_tilde_for_local_ssh_paths() {
        let home = cdt_discover::home_dir().expect("home dir resolvable");
        assert_eq!(expand_local_ssh_path(Path::new("~")), home);
        assert_eq!(
            expand_local_ssh_path(Path::new("~/.ssh/id_ed25519")),
            home.join(".ssh/id_ed25519")
        );
        assert_eq!(
            expand_local_ssh_path(Path::new(r"~\.ssh\id_ed25519")),
            home.join(".ssh/id_ed25519")
        );
        assert_eq!(
            expand_local_ssh_path(Path::new("/tmp/id_ed25519")),
            PathBuf::from("/tmp/id_ed25519")
        );
    }

    // change `metadata-cache-context-prefix` ssh-remote-context spec delta：
    // SshSessionManager 暴露 HostSignature 派生的 ContextId 查询。

    /// 最小 fake `SftpClient` —— 仅供 `host_signature` / `context_id` 相关测试构造
    /// `SshFileSystemProvider`，不真做远端 I/O；所有方法返 `NotFound` 哨兵。
    struct DummySftpClient;

    #[async_trait::async_trait]
    impl crate::provider::SftpClient for DummySftpClient {
        async fn metadata(
            &self,
            _path: &str,
        ) -> Result<cdt_fs::FsMetadata, crate::provider::SftpClientError> {
            Err(crate::provider::SftpClientError::Other("dummy fake".into()))
        }
        async fn try_exists(&self, _path: &str) -> Result<bool, crate::provider::SftpClientError> {
            Ok(false)
        }
        async fn read(&self, _path: &str) -> Result<Vec<u8>, crate::provider::SftpClientError> {
            Err(crate::provider::SftpClientError::Other("dummy fake".into()))
        }
        async fn read_dir(
            &self,
            _path: &str,
        ) -> Result<Vec<crate::provider::RemoteEntry>, crate::provider::SftpClientError> {
            Ok(vec![])
        }
        async fn read_lines_head(
            &self,
            _path: &str,
            _max: usize,
        ) -> Result<Vec<String>, crate::provider::SftpClientError> {
            Ok(vec![])
        }
    }

    fn fake_provider(context_id: &str, remote_home: &str) -> SshFileSystemProvider {
        SshFileSystemProvider::with_client(
            context_id,
            std::sync::Arc::new(DummySftpClient),
            PathBuf::from(remote_home),
        )
    }

    #[tokio::test]
    async fn insert_test_context_with_explicit_host_signature_round_trips() {
        let mgr = SshSessionManager::new();
        let sig = cdt_fs::HostSignature::from_ssh_config_fields(&cdt_fs::SshConfigDigestInput {
            hostname: "explicit-host".into(),
            port: 2222,
            user: "u".into(),
            identity_files: vec![],
            proxyjump: Some("bastion".into()),
            proxycommand: None,
            hostkeyalias: None,
        });
        mgr.insert_test_context(
            "ctx-1",
            "explicit-host",
            2222,
            Some("u".into()),
            PathBuf::from("/remote/home"),
            fake_provider("ctx-1", "/remote/home"),
            Some(sig.clone()),
        )
        .await;

        let ctx = mgr.context_id("ctx-1").await.expect("registered context");
        assert_eq!(ctx.backend_kind, cdt_fs::FsKind::Ssh);
        assert_eq!(ctx.host_signature.as_ref().unwrap(), &sig);
        assert_eq!(ctx.root_or_home, PathBuf::from("/remote/home"));
    }

    #[tokio::test]
    async fn insert_test_context_default_host_signature_derives_from_host_port_user() {
        // 缺省 host_signature SHALL 用真算法构造，不同 host 自然产不同 digest。
        let mgr = SshSessionManager::new();
        mgr.insert_test_context(
            "host-a",
            "host-a.example",
            22,
            Some("u".into()),
            PathBuf::from("/h"),
            fake_provider("host-a", "/h"),
            None,
        )
        .await;
        mgr.insert_test_context(
            "host-b",
            "host-b.example",
            22,
            Some("u".into()),
            PathBuf::from("/h"),
            fake_provider("host-b", "/h"),
            None,
        )
        .await;
        let ctx_a = mgr.context_id("host-a").await.expect("ctx-a");
        let ctx_b = mgr.context_id("host-b").await.expect("ctx-b");
        assert_ne!(ctx_a.host_signature, ctx_b.host_signature);
    }

    #[tokio::test]
    async fn context_id_returns_none_for_unregistered() {
        let mgr = SshSessionManager::new();
        assert!(mgr.context_id("missing").await.is_none());
    }

    #[test]
    fn degraded_fallback_digest_differs_from_ssh_g_success() {
        // change `metadata-cache-context-prefix` ssh-remote-context spec delta
        // §`degraded fallback 与 ssh -G 路径产 ContextId 安全不等` —— success
        // 路径 ResolvedHost 含 proxyjump，fallback 路径三字段全 None，digest 不等。
        let success_input = cdt_fs::SshConfigDigestInput {
            hostname: "host.example".into(),
            port: 22,
            user: "u".into(),
            identity_files: vec![PathBuf::from("/home/u/.ssh/id_ed25519")],
            proxyjump: Some("bastion".into()),
            proxycommand: None,
            hostkeyalias: None,
        };
        let fallback_input = cdt_fs::SshConfigDigestInput {
            hostname: "host.example".into(),
            port: 22,
            user: "u".into(),
            identity_files: vec![PathBuf::from("/home/u/.ssh/id_ed25519")],
            proxyjump: None,
            proxycommand: None,
            hostkeyalias: None,
        };
        let sig_success = cdt_fs::HostSignature::from_ssh_config_fields(&success_input);
        let sig_fallback = cdt_fs::HostSignature::from_ssh_config_fields(&fallback_input);
        assert_ne!(
            sig_success.config_digest, sig_fallback.config_digest,
            "ssh -G 成功路径与 degraded fallback 路径 digest SHALL 不同（by-design safe miss）"
        );
        let ctx_success = cdt_fs::ContextId::ssh(sig_success, PathBuf::from("/h"));
        let ctx_fallback = cdt_fs::ContextId::ssh(sig_fallback, PathBuf::from("/h"));
        assert_ne!(ctx_success, ctx_fallback);
    }

    #[tokio::test]
    async fn shutdown_all_completes_quickly_with_no_sessions() {
        let mgr = SshSessionManager::new();
        let started = Instant::now();
        mgr.shutdown_all(Duration::from_secs(3)).await;
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn status_broadcast_to_two_subscribers() {
        // spec `Status broadcast to multiple subscribers` Scenario：
        // 两个订阅者各自独立收到事件，任一滞后不影响另一投递。
        let mgr = SshSessionManager::new();
        let mut rx1 = mgr.subscribe_status();
        let mut rx2 = mgr.subscribe_status();

        mgr.emit_status(SshStatusChange {
            context_id: "ctx1".into(),
            status: SshStatus::Connected,
            auth_chain: vec![],
            error: None,
        });

        let e1 = tokio::time::timeout(Duration::from_millis(100), rx1.recv())
            .await
            .expect("e1")
            .expect("ok");
        let e2 = tokio::time::timeout(Duration::from_millis(100), rx2.recv())
            .await
            .expect("e2")
            .expect("ok");
        assert_eq!(e1.context_id, "ctx1");
        assert_eq!(e2.context_id, "ctx1");
        assert_eq!(e1.status, SshStatus::Connected);
        assert_eq!(e2.status, SshStatus::Connected);
    }

    #[tokio::test]
    async fn post_auth_failure_keeps_successful_auth_chain() {
        let mgr = SshSessionManager::new();
        let req = SshConnectRequest {
            host: "missing-home".into(),
            port: Some(22),
            username: Some("alice".into()),
            auth_method: AuthMethodKind::SshConfig,
            password: None,
            context_id: Some("ctx-missing-home".into()),
        };
        let attempts = vec![AuthAttempt {
            source: AuthSource::EnvAgent,
            outcome: AuthOutcome::Success,
            elapsed_ms: 2,
        }];
        mgr.failed_auth_chains
            .lock()
            .await
            .insert("ctx-missing-home".into(), attempts.clone());
        mgr.record_error_context(
            "ctx-missing-home",
            &req,
            &SshError::RemoteHomeMissing { tried: vec![] },
        )
        .await;

        let state = mgr.context_state("ctx-missing-home").await.expect("state");
        assert_eq!(state.status, SshStatus::Error);
        assert_eq!(state.auth_chain, attempts);
    }

    #[tokio::test]
    async fn failed_context_remains_queryable_with_auth_chain() {
        let mgr = SshSessionManager::new();
        let err = SshError::AuthExhausted {
            attempts: vec![AuthAttempt {
                source: AuthSource::EnvAgent,
                outcome: AuthOutcome::Skipped("SSH_AUTH_SOCK not set".into()),
                elapsed_ms: 1,
            }],
        };
        let req = SshConnectRequest {
            host: "bad-host".into(),
            port: Some(2222),
            username: Some("alice".into()),
            auth_method: AuthMethodKind::SshConfig,
            password: None,
            context_id: Some("ctx-bad".into()),
        };

        mgr.record_error_context("ctx-bad", &req, &err).await;

        let state = mgr.context_state("ctx-bad").await.expect("state");
        assert_eq!(state.status, SshStatus::Error);
        assert_eq!(state.host, "bad-host");
        assert_eq!(state.port, 2222);
        assert_eq!(state.auth_chain.len(), 1);
    }

    #[tokio::test]
    async fn ssh_status_change_serializes_with_camel_case() {
        let change = SshStatusChange {
            context_id: "ctx".into(),
            status: SshStatus::Connecting,
            auth_chain: vec![],
            error: None,
        };
        let json = serde_json::to_value(&change).unwrap();
        assert_eq!(json["contextId"], "ctx");
        assert_eq!(json["status"], "connecting");
        assert!(json.get("auth_chain").is_none()); // skip if empty
    }

    #[tokio::test]
    async fn context_changed_serializes_with_camel_case() {
        let change = ContextChanged {
            active_context_id: Some("myhost".into()),
            kind: ContextKind::Ssh,
        };
        let json = serde_json::to_value(&change).unwrap();
        assert_eq!(json["activeContextId"], "myhost");
        assert_eq!(json["kind"], "ssh");
    }

    /// 真 SSH server 集成测试占位（需本地 docker `linuxserver/openssh-server`）。
    #[tokio::test]
    #[ignore = "requires running docker SSH server; run locally with --ignored"]
    async fn live_connect_to_local_docker() {
        let mgr = SshSessionManager::new();
        let req = SshConnectRequest {
            host: "localhost".into(),
            port: Some(2222),
            username: Some("test".into()),
            auth_method: AuthMethodKind::Password,
            password: Some("test".into()),
            context_id: None,
        };
        let _ctx = mgr.connect(req).await.expect("connect ok");
    }
}
