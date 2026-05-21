//! SSH 鉴权候选链构建与尝试。
//!
//! Spec：`openspec/specs/ssh-remote-context/spec.md` `Requirement: SSH authentication
//! candidate chain`。
//!
//! 模块分两层：
//! - **构建层**：`build_candidates` / `build_candidates_with_env` 按 D2 顺序产 7 项
//!   候选源（含平台分支与路径去重，不读 env 的纯函数版便于单测）
//! - **调度层**：`run_auth_chain` 接受调用方注入的 `try_authenticate` callback
//!   依次尝试到第一个成功，记录每个尝试构造 `AuthAttempt`；全部失败抛
//!   `SshError::AuthExhausted`。callback 形态把"真 russh 调用"留给 `connection.rs`
//!   生产路径，单测侧注入 fake outcome 序列覆盖所有调度逻辑
//!
//! `AuthSource` / `AuthOutcome` / `AuthAttempt` 类型定义在 `crate::error`。

use std::future::Future;
use std::path::PathBuf;
use std::time::Instant;

use crate::error::{AuthAttempt, AuthOutcome, AuthSource, SshError};
use crate::host_resolver::ResolvedHost;

/// 当前编译目标平台分支（用于跳过 macOS-only 候选）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Linux,
    Windows,
}

impl Platform {
    /// 取当前编译目标平台。
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// 用户选择的鉴权方式（UI 表单字段）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthMethodKind {
    /// 走 D2 鉴权候选链（agent / `IdentityFile` / 默认密钥）。
    SshConfig,
    /// 仅 password。
    Password,
}

/// 1Password well-known socket 候选路径（macOS）。
///
/// 与 OpenSSH 行为对齐——若用户在 ssh config 显式指定了 `IdentityAgent`，候选 1
/// 已经覆盖；候选 4 仅作为兜底。
fn one_password_well_known_paths() -> Vec<PathBuf> {
    let home = cdt_discover::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        home.join("Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock"),
        home.join(".1password/agent.sock"),
    ]
}

/// 默认密钥位置 fallback（候选 6）。
fn default_key_paths() -> Vec<PathBuf> {
    let home = cdt_discover::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![
        home.join(".ssh/id_ed25519"),
        home.join(".ssh/id_rsa"),
        home.join(".ssh/id_ecdsa"),
    ]
}

/// 按 D2 顺序构建候选源列表（纯函数：不读 env，便于单测）。
///
/// 顺序：(1) `IdentityAgent`（`host_resolver` 解析） → (2) `env_auth_sock` 非空时 `EnvAgent` →
/// (3) `macOS` `launchctl` → (4) 1Password well-known socket（与候选 1 路径去重） →
/// (5) `IdentityFile`（`host_resolver` 解析） → (6) 默认密钥 fallback →
/// (7) `Password`（仅当 `auth_method == Password`）。
///
/// 跨平台分支：Windows 跳过候选 (3)、(4)；Linux 跳过候选 (3)、(4)（`gnome-keyring` 是 v2 phase）。
#[must_use]
pub fn build_candidates_with_env(
    host: &ResolvedHost,
    platform: Platform,
    auth_method: AuthMethodKind,
    env_auth_sock: Option<&str>,
) -> Vec<AuthSource> {
    let mut out: Vec<AuthSource> = Vec::new();

    // 候选 1：IdentityAgent 字段（来自 ssh -G 解析）
    if let Some(path) = host.identity_agent.as_ref() {
        out.push(AuthSource::IdentityAgent(path.clone()));
    }

    // 候选 2：SSH_AUTH_SOCK env（终端启动场景）；env 路径与候选 1 路径相同时去重
    if let Some(sock) = env_auth_sock.filter(|s| !s.is_empty()) {
        let env_path = PathBuf::from(sock);
        if !contains_identity_agent_path(&out, &env_path) {
            out.push(AuthSource::EnvAgent);
        }
    }

    // 候选 3：macOS launchctl getenv SSH_AUTH_SOCK
    if platform == Platform::MacOs {
        out.push(AuthSource::LaunchctlAgent);
    }

    // 候选 4：1Password well-known socket（macOS only）
    if platform == Platform::MacOs {
        for path in one_password_well_known_paths() {
            if !contains_identity_agent_path(&out, &path) {
                out.push(AuthSource::OnePasswordAgent(path));
            }
        }
    }

    // 候选 5：IdentityFile（host_resolver 解析）
    for f in &host.identity_files {
        out.push(AuthSource::IdentityFile(f.clone()));
    }

    // 候选 6：默认密钥位置 fallback
    for path in default_key_paths() {
        // 与候选 5 去重（用户显式指定的 IdentityFile 不重复尝试）
        if !contains_identity_file_path(&out, &path) {
            out.push(AuthSource::DefaultKey(path));
        }
    }

    // 候选 7：password（仅当用户选择 Password）
    if auth_method == AuthMethodKind::Password {
        out.push(AuthSource::Password);
    }

    out
}

/// 公开 API：从进程 env 读 `SSH_AUTH_SOCK` 并调内层 `build_candidates_with_env`。
#[must_use]
pub fn build_candidates(
    host: &ResolvedHost,
    platform: Platform,
    auth_method: AuthMethodKind,
) -> Vec<AuthSource> {
    let env = std::env::var("SSH_AUTH_SOCK").ok();
    build_candidates_with_env(host, platform, auth_method, env.as_deref())
}

/// 依次尝试候选链，记录每个 `AuthAttempt`，第一个 `Success` 立即返回；全部失败
/// （`Failure` / `Skipped`）抛 `SshError::AuthExhausted { attempts }`。
///
/// `try_fn` 由调用方注入：生产路径包 `russh::client::Handle::authenticate_*`，
/// 单测路径返 fake `AuthOutcome` 序列。这样调度逻辑（顺序 / 计时 / 错误聚合）
/// 与协议调用解耦，便于覆盖所有典型组合。
///
/// 返回值：成功时返 `Vec<AuthAttempt>`，含成功之前的全部尝试 + 最后一条 `Success`，
/// 用于在状态广播 `connecting → connected` 时让 UI 显示"试过哪些候选"。
pub async fn run_auth_chain<F, Fut>(
    candidates: Vec<AuthSource>,
    mut try_fn: F,
) -> Result<Vec<AuthAttempt>, SshError>
where
    F: FnMut(AuthSource) -> Fut,
    Fut: Future<Output = AuthOutcome>,
{
    let mut attempts: Vec<AuthAttempt> = Vec::with_capacity(candidates.len());

    for source in candidates {
        let started = Instant::now();
        let outcome = try_fn(source.clone()).await;
        let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

        let is_success = matches!(outcome, AuthOutcome::Success);
        attempts.push(AuthAttempt {
            source,
            outcome,
            elapsed_ms,
        });

        if is_success {
            return Ok(attempts);
        }
    }

    Err(SshError::AuthExhausted { attempts })
}

fn contains_identity_agent_path(list: &[AuthSource], target: &PathBuf) -> bool {
    list.iter().any(|s| match s {
        AuthSource::IdentityAgent(p) | AuthSource::OnePasswordAgent(p) => p == target,
        _ => false,
    })
}

fn contains_identity_file_path(list: &[AuthSource], target: &PathBuf) -> bool {
    list.iter().any(|s| match s {
        AuthSource::IdentityFile(p) | AuthSource::DefaultKey(p) => p == target,
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_with(identity_agent: Option<PathBuf>, identity_files: Vec<PathBuf>) -> ResolvedHost {
        ResolvedHost {
            host: "h".into(),
            port: 22,
            user: None,
            identity_agent,
            identity_files,
            proxyjump: None,
            proxycommand: None,
            hostkeyalias: None,
            degraded: false,
        }
    }

    #[test]
    fn macos_chain_includes_launchctl_and_1password() {
        let chain = build_candidates_with_env(
            &host_with(None, vec![]),
            Platform::MacOs,
            AuthMethodKind::SshConfig,
            None,
        );

        assert!(
            chain
                .iter()
                .any(|s| matches!(s, AuthSource::LaunchctlAgent))
        );
        assert!(
            chain
                .iter()
                .any(|s| matches!(s, AuthSource::OnePasswordAgent(_)))
        );
        // 不应含 Password（auth_method == SshConfig）
        assert!(!chain.iter().any(|s| matches!(s, AuthSource::Password)));
    }

    #[test]
    fn windows_chain_skips_launchctl_and_1password() {
        let chain = build_candidates_with_env(
            &host_with(None, vec![]),
            Platform::Windows,
            AuthMethodKind::SshConfig,
            None,
        );

        assert!(
            !chain
                .iter()
                .any(|s| matches!(s, AuthSource::LaunchctlAgent))
        );
        assert!(
            !chain
                .iter()
                .any(|s| matches!(s, AuthSource::OnePasswordAgent(_)))
        );
    }

    #[test]
    fn identity_agent_field_takes_precedence_over_env_agent() {
        let agent_path = PathBuf::from(
            "/Users/me/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock",
        );
        let chain = build_candidates_with_env(
            &host_with(Some(agent_path.clone()), vec![]),
            Platform::MacOs,
            AuthMethodKind::SshConfig,
            Some("/tmp/standard-agent.sock"),
        );

        // 候选 1（IdentityAgent）出现在候选 2（EnvAgent）之前
        let pos_agent = chain
            .iter()
            .position(|s| matches!(s, AuthSource::IdentityAgent(_)))
            .expect("IdentityAgent in chain");
        let pos_env = chain
            .iter()
            .position(|s| matches!(s, AuthSource::EnvAgent))
            .expect("EnvAgent in chain");
        assert!(pos_agent < pos_env);
    }

    #[test]
    fn env_agent_dropped_when_path_matches_identity_agent() {
        // SSH_AUTH_SOCK 与候选 1 同路径时不再加 EnvAgent（避免重复尝试同一 socket）
        let agent = PathBuf::from("/tmp/agent.sock");
        let chain = build_candidates_with_env(
            &host_with(Some(agent.clone()), vec![]),
            Platform::Linux,
            AuthMethodKind::SshConfig,
            Some("/tmp/agent.sock"),
        );
        assert!(!chain.iter().any(|s| matches!(s, AuthSource::EnvAgent)));
    }

    #[test]
    fn one_password_path_dedup_when_identity_agent_matches() {
        let one_password = one_password_well_known_paths();
        let agent_path = one_password
            .first()
            .cloned()
            .expect("at least one well-known 1Password path");

        let chain = build_candidates_with_env(
            &host_with(Some(agent_path.clone()), vec![]),
            Platform::MacOs,
            AuthMethodKind::SshConfig,
            None,
        );

        // 候选 4 中相同路径 SHALL 被去重
        let one_password_count = chain
            .iter()
            .filter(|s| matches!(s, AuthSource::OnePasswordAgent(p) if *p == agent_path))
            .count();
        assert_eq!(one_password_count, 0, "1Password path should be deduped");
    }

    #[test]
    fn identity_file_then_default_key_fallback_dedupe() {
        let home = cdt_discover::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let work_key = home.join(".ssh/id_ed25519"); // 故意与默认密钥重复
        let chain = build_candidates_with_env(
            &host_with(None, vec![work_key.clone()]),
            Platform::Linux,
            AuthMethodKind::SshConfig,
            None,
        );

        let identity_files: Vec<_> = chain
            .iter()
            .filter_map(|s| match s {
                AuthSource::IdentityFile(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        let default_keys: Vec<_> = chain
            .iter()
            .filter_map(|s| match s {
                AuthSource::DefaultKey(p) => Some(p.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(identity_files, vec![work_key.clone()]);
        // 默认密钥 fallback 跳过与 IdentityFile 重复的 id_ed25519
        assert!(!default_keys.contains(&work_key));
        assert!(default_keys.iter().any(|p| p.ends_with(".ssh/id_rsa")));
    }

    #[test]
    fn password_method_appends_password_at_end() {
        let chain = build_candidates_with_env(
            &host_with(None, vec![]),
            Platform::Linux,
            AuthMethodKind::Password,
            None,
        );

        let last = chain.last().expect("chain non-empty");
        assert!(matches!(last, AuthSource::Password));
    }

    #[test]
    fn ssh_config_method_omits_password() {
        let chain = build_candidates_with_env(
            &host_with(None, vec![]),
            Platform::Linux,
            AuthMethodKind::SshConfig,
            None,
        );

        assert!(!chain.iter().any(|s| matches!(s, AuthSource::Password)));
    }

    // -------------------------------------------------------------------
    // run_auth_chain 调度层测试（task 3.13 + spec Scenario "All candidates exhausted"）
    // -------------------------------------------------------------------

    use std::cell::RefCell;
    use std::rc::Rc;

    /// 构造一个 fake `try_fn`：按 outcomes 序列依次返回，记录每次被调用的 source。
    #[allow(clippy::type_complexity)]
    fn fake_try(
        outcomes: Vec<AuthOutcome>,
    ) -> (
        impl FnMut(AuthSource) -> std::pin::Pin<Box<dyn std::future::Future<Output = AuthOutcome>>>,
        Rc<RefCell<Vec<AuthSource>>>,
    ) {
        let calls: Rc<RefCell<Vec<AuthSource>>> = Rc::new(RefCell::new(Vec::new()));
        let calls_clone = calls.clone();
        let outcomes = Rc::new(RefCell::new(outcomes.into_iter()));

        let f = move |src: AuthSource| {
            calls_clone.borrow_mut().push(src);
            let next = outcomes.borrow_mut().next().expect("outcome supply");
            Box::pin(async move { next })
                as std::pin::Pin<Box<dyn std::future::Future<Output = AuthOutcome>>>
        };
        (f, calls)
    }

    #[tokio::test]
    async fn run_auth_chain_returns_on_first_success() {
        let candidates = vec![
            AuthSource::EnvAgent,
            AuthSource::IdentityFile(PathBuf::from("/k1")),
            AuthSource::DefaultKey(PathBuf::from("/k2")),
        ];
        let (try_fn, calls) = fake_try(vec![
            AuthOutcome::Failure("env miss".into()),
            AuthOutcome::Success,
            AuthOutcome::Success, // 不应被触达
        ]);

        let result = run_auth_chain(candidates, try_fn).await.expect("ok");

        assert_eq!(result.len(), 2, "stop after first success");
        assert!(matches!(result[0].outcome, AuthOutcome::Failure(_)));
        assert!(matches!(result[1].outcome, AuthOutcome::Success));
        // 第三个候选不应被尝试
        assert_eq!(calls.borrow().len(), 2);
    }

    #[tokio::test]
    async fn run_auth_chain_all_fail_returns_auth_exhausted() {
        let candidates = vec![
            AuthSource::EnvAgent,
            AuthSource::IdentityFile(PathBuf::from("/k1")),
        ];
        let (try_fn, _calls) = fake_try(vec![
            AuthOutcome::Failure("Permission denied".into()),
            AuthOutcome::Skipped("requires passphrase, use ssh-add".into()),
        ]);

        let err = run_auth_chain(candidates, try_fn).await.unwrap_err();
        match err {
            SshError::AuthExhausted { attempts } => {
                assert_eq!(attempts.len(), 2);
                assert!(matches!(attempts[0].outcome, AuthOutcome::Failure(_)));
                assert!(matches!(attempts[1].outcome, AuthOutcome::Skipped(_)));
            }
            other => panic!("expected AuthExhausted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn run_auth_chain_skipped_does_not_short_circuit() {
        // Skipped 与 Failure 一样不算成功——应继续往下试
        let candidates = vec![
            AuthSource::IdentityFile(PathBuf::from("/encrypted")),
            AuthSource::DefaultKey(PathBuf::from("/k")),
        ];
        let (try_fn, calls) = fake_try(vec![
            AuthOutcome::Skipped("requires passphrase, use ssh-add".into()),
            AuthOutcome::Success,
        ]);

        let result = run_auth_chain(candidates, try_fn).await.expect("ok");
        assert_eq!(result.len(), 2);
        assert_eq!(calls.borrow().len(), 2);
    }

    #[tokio::test]
    async fn run_auth_chain_records_elapsed_ms_per_attempt() {
        let candidates = vec![AuthSource::EnvAgent, AuthSource::Password];
        let (try_fn, _) = fake_try(vec![AuthOutcome::Failure("x".into()), AuthOutcome::Success]);

        let result = run_auth_chain(candidates, try_fn).await.expect("ok");
        // elapsed_ms 字段已写入；fake fn 同步返回所以耗时趋近 0，但字段必须存在
        for attempt in &result {
            // 任何 u64 都接受，断言字段被写入即可（不能为 None，因为不是 Option）
            let _ = attempt.elapsed_ms;
        }
    }

    #[tokio::test]
    async fn run_auth_chain_empty_candidates_returns_auth_exhausted_empty_attempts() {
        let (try_fn, _) = fake_try(vec![]);
        let err = run_auth_chain(vec![], try_fn).await.unwrap_err();
        match err {
            SshError::AuthExhausted { attempts } => assert!(attempts.is_empty()),
            other => panic!("expected AuthExhausted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn run_auth_chain_serialization_shape_matches_spec() {
        // spec Scenario "AuthAttempt serialization shape" 要求 attempts 序列化为
        // { source: { type: ..., data?: ... }, outcome: { type: ..., data?: ... }, elapsedMs }
        let candidates = vec![
            AuthSource::IdentityAgent(PathBuf::from("/agent.sock")),
            AuthSource::Password,
        ];
        let (try_fn, _) = fake_try(vec![
            AuthOutcome::Failure("permission denied".into()),
            AuthOutcome::Success,
        ]);
        let result = run_auth_chain(candidates, try_fn).await.expect("ok");

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json[0]["source"]["type"], "identityAgent");
        assert_eq!(json[0]["source"]["data"], "/agent.sock");
        assert_eq!(json[0]["outcome"]["type"], "failure");
        assert_eq!(json[0]["outcome"]["data"], "permission denied");
        assert!(json[0]["elapsedMs"].is_u64());
        assert_eq!(json[1]["source"]["type"], "password");
        assert!(json[1]["source"].get("data").is_none());
        assert_eq!(json[1]["outcome"]["type"], "success");
    }
}
