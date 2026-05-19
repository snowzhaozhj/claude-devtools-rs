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
    handle: client::Handle<RusshClientHandler>,
    /// SFTP 会话——通过 `channel.request_subsystem("sftp")` 建立。
    /// `Mutex` 包一层是因为 `SftpSession` 内部状态非 `Sync`，但我们要在多个 IPC
    /// 命令之间共享同一会话句柄。
    sftp: Arc<Mutex<SftpSession>>,
    /// 远端 `~/.claude/projects` 路径（4 fallback 中第一个存在的）。
    remote_home: PathBuf,
    /// 连接元信息 —— 显示给 UI（Phase C task 9.x 读）。
    host: String,
    port: u16,
    user: Option<String>,
    /// 鉴权链结果（成功路径下 attempts 含 success；error 路径下 attempts 全部失败）。
    auth_chain: Vec<AuthAttempt>,
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

    /// 获取活跃 context id（None 表示 Local）。
    pub async fn active_context_id(&self) -> Option<String> {
        self.active.lock().await.clone()
    }

    /// 列出所有已注册 SSH context id。
    pub async fn registered_context_ids(&self) -> Vec<String> {
        self.sessions.lock().await.keys().cloned().collect()
    }

    pub async fn context_states(&self) -> Vec<SshContextState> {
        self.sessions
            .lock()
            .await
            .iter()
            .map(|(context_id, resources)| SshContextState {
                context_id: context_id.clone(),
                host: resources.host.clone(),
                port: resources.port,
                username: resources.user.clone(),
                remote_home: resources.remote_home.clone(),
                status: SshStatus::Connected,
                auth_chain: resources.auth_chain.clone(),
            })
            .collect()
    }

    pub async fn context_state(&self, context_id: &str) -> Option<SshContextState> {
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
                status: SshStatus::Connected,
                auth_chain: resources.auth_chain.clone(),
            })
    }

    pub async fn provider(&self, context_id: &str) -> Option<SshFileSystemProvider> {
        self.sessions.lock().await.get(context_id).map(|resources| {
            SshFileSystemProvider::new(
                context_id.to_owned(),
                resources.sftp.clone(),
                resources.remote_home.clone(),
            )
        })
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

        let outer = timeout(OUTER_TIMEOUT, self.connect_inner(&context_id, request)).await;

        let resources = match outer {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
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
                self.emit_status(SshStatusChange {
                    context_id: context_id.clone(),
                    status: SshStatus::Error,
                    auth_chain: vec![],
                    error: Some(err.clone()),
                });
                return Err(err);
            }
        };

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
        _context_id: &str,
        request: SshConnectRequest,
    ) -> Result<SshSessionResources, SshError> {
        // 阶段 0：解析 host alias（`ssh -G` 委托）
        let resolved = resolve_host_via_ssh_g(&request.host).await?;
        let port = request.port.or(Some(resolved.port)).unwrap_or(22);
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

        Ok(SshSessionResources {
            handle,
            sftp,
            remote_home,
            host,
            port,
            user: Some(username),
            auth_chain,
        })
    }

    /// 主动断开一个 context：关闭 SFTP / transport / TCP；若被断开的是 active 切回 Local。
    pub async fn disconnect(&self, context_id: &str) -> Result<(), SshError> {
        let resources = self.sessions.lock().await.remove(context_id);
        if let Some(r) = resources {
            // 关闭 SFTP（drop Arc 即释放）— SftpSession 没有显式 close，drop 自动结束 channel
            drop(r.sftp);
            // 关闭 russh handle
            let _ = r
                .handle
                .disconnect(russh::Disconnect::ByApplication, "", "")
                .await;
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
    if !path.exists() {
        return AuthOutcome::Skipped(format!("not found: {}", path.display()));
    }
    let key = match russh::keys::load_secret_key(path, None) {
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

    if !socket_path.exists() {
        return AuthOutcome::Skipped(format!("agent socket missing: {}", socket_path.display()));
    }
    let stream = match UnixStream::connect(socket_path).await {
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
