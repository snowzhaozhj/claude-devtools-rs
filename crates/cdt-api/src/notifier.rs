//! 自动通知管线：file-change → `detect_errors` → `NotificationManager::add_notification` → broadcast。
//!
//! 订阅 `cdt_watch::FileWatcher::subscribe_files()`，对每个 `FileChangeEvent`：
//! 1. 若 `deleted=true`，跳过
//! 2. 按 `~/.claude/projects/<project_id>/<session_id>.jsonl` 找文件
//! 3. 全量 `parse_file` → `detect_errors` → 逐条 `add_notification`
//! 4. 新条目（`add_notification` 返回 `Ok(true)`）通过 `error_tx` 广播
//!
//! 配合 `DetectedError` 的确定性 id + `NotificationManager` 的按 id 去重，
//! 重复扫描同一文件不会产生重复通知。

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use cdt_config::{ConfigManager, DetectedError, NotificationManager, detect_errors};
use cdt_core::FileChangeEvent;
use cdt_discover::path_decoder;
use cdt_parse::parse_file;
use tokio::sync::{Mutex, broadcast};

use crate::cache_signature::FileSignature;

/// notifier 缓存容量上限 —— 详见 change `multi-session-cpu-cache` design D2。
const NOTIFIER_CACHE_CAPACITY: usize = 200;

/// `(project_id, session_id) → FileSignature` LRU 缓存。
///
/// 命中时整段跳过 `parse_file` + `detect_errors`（D7b）；任一字段（mtime / size /
/// identity）不一致走 cache miss。命中也 bump key 到队首避免冷热混淆。
#[derive(Debug)]
struct SignatureCache {
    map: HashMap<(String, String), FileSignature>,
    order: VecDeque<(String, String)>,
    capacity: usize,
}

impl SignatureCache {
    fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    /// 命中返回 Some 并 bump key 到队首；miss 返回 None。
    fn lookup(&mut self, key: &(String, String)) -> Option<FileSignature> {
        let sig = *self.map.get(key)?;
        // bump key 到队首
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            let k = self.order.remove(pos).expect("position 已校验");
            self.order.push_front(k);
        }
        Some(sig)
    }

    /// 写入 / 更新 entry，超容量时 LRU 淘汰。
    fn insert(&mut self, key: (String, String), sig: FileSignature) {
        if self.map.contains_key(&key) {
            // 已存在：更新 sig + bump 到队首
            self.map.insert(key.clone(), sig);
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                let k = self.order.remove(pos).expect("position 已校验");
                self.order.push_front(k);
            }
            return;
        }

        if self.map.len() >= self.capacity {
            if let Some(evicted) = self.order.pop_back() {
                self.map.remove(&evicted);
            }
        }

        self.map.insert(key.clone(), sig);
        self.order.push_front(key);
    }
}

/// 自动通知管线。
pub struct NotificationPipeline {
    file_rx: broadcast::Receiver<FileChangeEvent>,
    config_mgr: Arc<Mutex<ConfigManager>>,
    notif_mgr: Arc<Mutex<NotificationManager>>,
    error_tx: broadcast::Sender<DetectedError>,
    /// `~/.claude/projects/` 的实际路径。显式参数化是为了测试可用 tmp 目录。
    projects_dir: PathBuf,
    /// `(project_id, session_id) → FileSignature` 缓存，命中即整段跳过
    /// parse + detect。详见 change `multi-session-cpu-cache` D7b。
    cache: Arc<StdMutex<SignatureCache>>,
}

impl NotificationPipeline {
    pub fn new(
        file_rx: broadcast::Receiver<FileChangeEvent>,
        config_mgr: Arc<Mutex<ConfigManager>>,
        notif_mgr: Arc<Mutex<NotificationManager>>,
        error_tx: broadcast::Sender<DetectedError>,
        projects_dir: PathBuf,
    ) -> Self {
        Self {
            file_rx,
            config_mgr,
            notif_mgr,
            error_tx,
            projects_dir,
            cache: Arc::new(StdMutex::new(SignatureCache::new(NOTIFIER_CACHE_CAPACITY))),
        }
    }

    /// 主循环：阻塞直到 `file_rx` 关闭或进程退出。
    ///
    /// `RecvError::Lagged(n)` 时记 warning 继续——丢的事件会在下次 file change 时
    /// 被全量 re-parse 覆盖，不会永久漏检。
    pub async fn run(mut self) {
        loop {
            match self.file_rx.recv().await {
                Ok(event) => {
                    self.process_file_change(&event).await;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(
                        lagged = n,
                        "notification pipeline lagged; subsequent events will re-scan affected sessions"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::info!("notification pipeline channel closed, stopping");
                    break;
                }
            }
        }
    }

    async fn process_file_change(&self, event: &FileChangeEvent) {
        if event.deleted {
            return;
        }

        let triggers = {
            let mgr = self.config_mgr.lock().await;
            mgr.get_enabled_triggers()
        };
        if triggers.is_empty() {
            return;
        }

        let base_dir = path_decoder::extract_base_dir(&event.project_id);
        let jsonl_path = self
            .projects_dir
            .join(base_dir)
            .join(format!("{}.jsonl", event.session_id));

        // FileSignature 缓存：命中即整段跳过 parse + detect（D7b）。
        // stat 失败走原路径让 parse_file 自己处理。
        let cache_key = (event.project_id.clone(), event.session_id.clone());
        let new_sig = match tokio::fs::metadata(&jsonl_path).await {
            #[allow(deprecated)]
            Ok(meta) => Some(FileSignature::from_metadata(&meta)),
            Err(err) => {
                tracing::debug!(
                    path = %jsonl_path.display(),
                    error = %err,
                    "notifier stat failed, falling through to parse"
                );
                None
            }
        };
        if let Some(sig) = new_sig {
            let cached = self
                .cache
                .lock()
                .expect("notifier cache mutex poisoned")
                .lookup(&cache_key);
            if cached == Some(sig) {
                return;
            }
        }

        let messages = match parse_file(&jsonl_path).await {
            Ok(m) => m,
            Err(err) => {
                tracing::debug!(
                    path = %jsonl_path.display(),
                    error = %err,
                    "notifier skip: parse failed"
                );
                return;
            }
        };

        let file_path_str = jsonl_path.to_string_lossy().into_owned();
        let errors = detect_errors(
            &messages,
            &triggers,
            &event.session_id,
            &event.project_id,
            &file_path_str,
        );

        // 在 detect / 通知派发完成后再写缓存。任一 `add_notification` 返回
        // `Err` 都视为本轮处理未真正落地，**不**写缓存，让下次同 FileSignature
        // 的事件再次进入此路径重试（codex 二审找到的漏通知 bug）。
        let mut all_persisted = true;

        if !errors.is_empty() {
            let mut mgr = self.notif_mgr.lock().await;
            for err in errors {
                match mgr.add_notification(err.clone()).await {
                    Ok(true) => {
                        let _ = self.error_tx.send(err);
                    }
                    Ok(false) => {
                        // duplicate, expected on re-scan
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            session_id = %event.session_id,
                            "notifier: add_notification failed"
                        );
                        all_persisted = false;
                    }
                }
            }
        }

        if all_persisted {
            if let Some(sig) = new_sig {
                self.cache
                    .lock()
                    .expect("notifier cache mutex poisoned")
                    .insert(cache_key, sig);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdt_config::{NotificationTrigger, TriggerContentType, TriggerMode};
    use tempfile::tempdir;

    fn make_error_trigger() -> NotificationTrigger {
        NotificationTrigger {
            id: "t1".into(),
            name: "Error".into(),
            enabled: true,
            content_type: TriggerContentType::ToolResult,
            mode: TriggerMode::ErrorStatus,
            require_error: Some(true),
            is_builtin: None,
            tool_name: None,
            ignore_patterns: None,
            match_field: None,
            match_pattern: None,
            token_threshold: None,
            token_type: None,
            repository_ids: None,
            color: None,
        }
    }

    async fn make_pipeline() -> (
        NotificationPipeline,
        broadcast::Sender<FileChangeEvent>,
        broadcast::Receiver<DetectedError>,
        Arc<Mutex<NotificationManager>>,
        Arc<Mutex<ConfigManager>>,
        tempfile::TempDir,
    ) {
        let tmp = tempdir().unwrap();
        let notif_path = tmp.path().join("notif.json");
        let config_path = tmp.path().join("config.json");

        let mut notif_mgr = NotificationManager::new(Some(notif_path));
        notif_mgr.load().await.unwrap();
        let notif_mgr = Arc::new(Mutex::new(notif_mgr));

        let mut config_mgr = ConfigManager::new(Some(config_path));
        config_mgr.load().await.unwrap();
        config_mgr.add_trigger(make_error_trigger()).await.unwrap();
        let config_mgr = Arc::new(Mutex::new(config_mgr));

        let (file_tx, file_rx) = broadcast::channel(16);
        let (error_tx, error_rx) = broadcast::channel(16);

        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();

        let pipeline = NotificationPipeline::new(
            file_rx,
            config_mgr.clone(),
            notif_mgr.clone(),
            error_tx,
            projects_dir,
        );

        (pipeline, file_tx, error_rx, notif_mgr, config_mgr, tmp)
    }

    #[tokio::test]
    async fn notifier_skips_deleted_events() {
        let (pipeline, _file_tx, _error_rx, _notif_mgr, _config_mgr, _tmp) = make_pipeline().await;
        pipeline
            .process_file_change(&FileChangeEvent {
                project_id: "p1".into(),
                session_id: "s1".into(),
                deleted: true,
                project_list_changed: false,
                session_list_changed: false,
                mtime_ms: None,
            })
            .await;
        // 仅断言不 panic 且不 I/O
    }

    #[tokio::test]
    async fn notifier_missing_file_is_silent() {
        // parse_file 读不到真实文件时，notifier 应记日志跳过，不 panic、不 send
        let (pipeline, _file_tx, mut error_rx, notif_mgr, _config_mgr, _tmp) =
            make_pipeline().await;
        pipeline
            .process_file_change(&FileChangeEvent {
                project_id: "does-not-exist".into(),
                session_id: "s-nope".into(),
                deleted: false,
                project_list_changed: false,
                session_list_changed: false,
                mtime_ms: None,
            })
            .await;

        assert!(error_rx.try_recv().is_err());
        assert_eq!(notif_mgr.lock().await.get_notifications(10, 0).total, 0);
    }

    // ========================================================================
    // FileSignature cache 行为测试 —— 覆盖 spec
    // `notification-triggers/spec.md::Notifier 按 FileSignature 缓存以避免重复 parse`
    // 的全部 Scenario：命中跳过 / mtime miss / size miss / identity miss /
    // stat 失败走 miss / LRU 淘汰 / 命中 bump 队首
    // ========================================================================

    use std::time::Duration;

    /// 写一个真实的 trigger 能命中的 jsonl：assistant 工具结果带 `is_error`。
    fn write_error_jsonl(path: &std::path::Path, suffix: &str) {
        // 每次构造唯一 tool_use_id 让 detect_errors 产新 DetectedError id；
        // 同一文件多次写入相同内容 add_notification 会按 id 去重。
        let content = format!(
            r#"{{"type":"assistant","uuid":"a-{suffix}","timestamp":"2026-05-03T10:00:00.000Z","sessionId":"sid","cwd":"/tmp","message":{{"role":"assistant","model":"claude","content":[{{"type":"tool_use","id":"tu-{suffix}","name":"Bash","input":{{"command":"x"}}}}]}}}}
{{"type":"user","uuid":"u-{suffix}","timestamp":"2026-05-03T10:00:01.000Z","sessionId":"sid","cwd":"/tmp","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"tu-{suffix}","content":"err","is_error":true}}]}}}}
"#
        );
        std::fs::write(path, content).unwrap();
    }

    fn build_event(project_id: &str, session_id: &str) -> FileChangeEvent {
        FileChangeEvent {
            project_id: project_id.into(),
            session_id: session_id.into(),
            deleted: false,
            project_list_changed: false,
            session_list_changed: false,
            mtime_ms: None,
        }
    }

    #[tokio::test]
    async fn cache_hit_skips_parse_and_detect() {
        let (pipeline, _file_tx, mut error_rx, notif_mgr, _config_mgr, tmp) = make_pipeline().await;
        let proj = "proj-A";
        let sess = "sess-1";
        let proj_dir = tmp.path().join("projects").join(proj);
        std::fs::create_dir_all(&proj_dir).unwrap();
        let jsonl = proj_dir.join(format!("{sess}.jsonl"));
        write_error_jsonl(&jsonl, "1");

        // 第一次：cache miss → parse + detect → 产 1 个通知
        pipeline.process_file_change(&build_event(proj, sess)).await;
        assert!(error_rx.try_recv().is_ok());
        assert_eq!(notif_mgr.lock().await.get_notifications(10, 0).total, 1);

        // 第二次：FileSignature 不变 → cache 命中整段跳过；不重跑 detect
        // 即便 detect 跑也会按 id 去重不产新通知，但跳过路径要求 error_tx 也不 send。
        pipeline.process_file_change(&build_event(proj, sess)).await;
        assert!(error_rx.try_recv().is_err(), "命中时不应再 send error");
        assert_eq!(notif_mgr.lock().await.get_notifications(10, 0).total, 1);
    }

    #[tokio::test]
    async fn cache_miss_when_size_grows() {
        let (pipeline, _file_tx, mut error_rx, _notif_mgr, _config_mgr, tmp) =
            make_pipeline().await;
        let proj = "proj-B";
        let sess = "sess-1";
        let proj_dir = tmp.path().join("projects").join(proj);
        std::fs::create_dir_all(&proj_dir).unwrap();
        let jsonl = proj_dir.join(format!("{sess}.jsonl"));
        write_error_jsonl(&jsonl, "1");

        pipeline.process_file_change(&build_event(proj, sess)).await;
        let _ = error_rx.try_recv();

        // 等到 mtime 至少推进 1ms，避免某些 FS 1s 精度下 mtime 撞车
        tokio::time::sleep(Duration::from_millis(1100)).await;
        // append 让 size 增长 → cache miss
        let mut existing = std::fs::read(&jsonl).unwrap();
        write_error_jsonl(&jsonl.with_extension("jsonl.tmp"), "2");
        let extra = std::fs::read(jsonl.with_extension("jsonl.tmp")).unwrap();
        existing.extend_from_slice(&extra);
        std::fs::write(&jsonl, existing).unwrap();

        pipeline.process_file_change(&build_event(proj, sess)).await;
        // 第二次 detect 出 2 个 error，但其中 1 个与上次相同被去重，
        // 至少有 1 个新 error 被 send
        assert!(
            error_rx.try_recv().is_ok(),
            "size 变化必须重 detect 触发新通知"
        );
    }

    #[tokio::test]
    async fn cache_miss_when_truncated() {
        let (pipeline, _file_tx, mut error_rx, _notif_mgr, _config_mgr, tmp) =
            make_pipeline().await;
        let proj = "proj-C";
        let sess = "sess-1";
        let proj_dir = tmp.path().join("projects").join(proj);
        std::fs::create_dir_all(&proj_dir).unwrap();
        let jsonl = proj_dir.join(format!("{sess}.jsonl"));
        write_error_jsonl(&jsonl, "1");

        pipeline.process_file_change(&build_event(proj, sess)).await;
        let _ = error_rx.try_recv();

        tokio::time::sleep(Duration::from_millis(1100)).await;
        // 用全新 suffix 重写，让 detect 出新 id 的 error，size 比第一次小或大都行——
        // 关键是 size 与原 cache 不同走 miss 路径
        std::fs::write(&jsonl, b"").unwrap();
        write_error_jsonl(&jsonl, "newid"); // 覆盖写

        pipeline.process_file_change(&build_event(proj, sess)).await;
        assert!(
            error_rx.try_recv().is_ok(),
            "truncate + 重写后必须重 detect 新 id 的 error"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cache_miss_when_identity_changes_via_rename() {
        // 用 rename 替换文件，让 inode 变化 —— 验证 identity 维度生效。
        // size / mtime 是否撞车不强求；inode 必不同即触发 miss。
        let (pipeline, _file_tx, mut error_rx, _notif_mgr, _config_mgr, tmp) =
            make_pipeline().await;
        let proj = "proj-D";
        let sess = "sess-1";
        let proj_dir = tmp.path().join("projects").join(proj);
        std::fs::create_dir_all(&proj_dir).unwrap();
        let jsonl = proj_dir.join(format!("{sess}.jsonl"));
        write_error_jsonl(&jsonl, "old");

        pipeline.process_file_change(&build_event(proj, sess)).await;
        let _ = error_rx.try_recv();

        // 准备替换文件并 rename 覆盖 → inode 必然不同
        let replacement = proj_dir.join("replace.jsonl");
        write_error_jsonl(&replacement, "newid");
        std::fs::rename(&replacement, &jsonl).unwrap();

        pipeline.process_file_change(&build_event(proj, sess)).await;
        assert!(
            error_rx.try_recv().is_ok(),
            "rename 替换（inode 变化）必须让 cache miss 重 detect"
        );
    }

    /// codex 二审找到的回归：当 `add_notification` 返回 `Err`（如磁盘 save 失败）
    /// 时，本轮通知未真正落地，**不**应写入缓存——否则下次同 `FileSignature`
    /// 命中即整段 return，错误永远漏通知。
    ///
    /// 触发 `add_notification` Err 的方法：让 `NotificationManager` 的 save
    /// 路径指向一个 **目录**（而非文件）—— `tokio::fs::write` 写到目录会返回
    /// `Err`，让 `save().await?` 抛错。
    #[tokio::test]
    async fn cache_not_written_when_add_notification_fails() {
        let tmp = tempdir().unwrap();
        // 故意把 notif "文件" 路径指向一个 **目录** —— save 时 write 文件会失败
        let notif_dir_as_path = tmp.path().join("notif_will_fail");
        std::fs::create_dir(&notif_dir_as_path).unwrap();
        let config_path = tmp.path().join("config.json");

        let mut notif_mgr = NotificationManager::new(Some(notif_dir_as_path));
        // load 时也会失败（read dir as file），但 NotificationManager::load 在
        // 路径不是文件时通常 fall back 到空状态——这里不细究 load 行为，重点是
        // save 必失败
        let _ = notif_mgr.load().await;
        let notif_mgr = Arc::new(Mutex::new(notif_mgr));

        let mut config_mgr = ConfigManager::new(Some(config_path));
        config_mgr.load().await.unwrap();
        config_mgr.add_trigger(make_error_trigger()).await.unwrap();
        let config_mgr = Arc::new(Mutex::new(config_mgr));

        let (_file_tx, file_rx) = broadcast::channel::<FileChangeEvent>(16);
        let (error_tx, _error_rx) = broadcast::channel::<DetectedError>(16);

        let projects_dir = tmp.path().join("projects");
        std::fs::create_dir_all(&projects_dir).unwrap();
        let proj_dir = projects_dir.join("proj-fail");
        std::fs::create_dir_all(&proj_dir).unwrap();
        let jsonl = proj_dir.join("sess.jsonl");
        write_error_jsonl(&jsonl, "f1");

        let pipeline =
            NotificationPipeline::new(file_rx, config_mgr, notif_mgr, error_tx, projects_dir);

        // 处理一次：detect 出 error，但 add_notification 因 save 失败返回 Err
        // → cache 不应写入。
        pipeline
            .process_file_change(&build_event("proj-fail", "sess"))
            .await;
        {
            let cache = pipeline.cache.lock().expect("mutex");
            assert!(
                cache.map.is_empty(),
                "add_notification 失败时不应写入缓存（避免下次命中漏通知）"
            );
        }

        // 第二次：FileSignature 不变，cache miss 重走 parse + detect。
        // NotificationManager 已修为 save 失败回滚 in-memory push（详
        // notification_manager.rs::add_notification doc），所以第二次 detect
        // 出同 id 不会被 dedup 为 Ok(false)，仍会重新尝试 save → 仍 Err →
        // cache 仍空，每次都会重试，避免永久漏通知。
        pipeline
            .process_file_change(&build_event("proj-fail", "sess"))
            .await;
        {
            let cache = pipeline.cache.lock().expect("mutex");
            assert!(
                cache.map.is_empty(),
                "save 持续失败时 cache 持续为空，每次都重试"
            );
        }
    }

    #[tokio::test]
    async fn stat_failure_falls_through_without_writing_cache() {
        // 文件不存在时，stat 失败走原路径让 parse_file 处理失败 —— 不写缓存
        let (pipeline, _file_tx, mut error_rx, _notif_mgr, _config_mgr, _tmp) =
            make_pipeline().await;
        pipeline
            .process_file_change(&build_event("proj-missing", "s"))
            .await;
        assert!(error_rx.try_recv().is_err());

        // cache 应仍为空（不应写入失败 sentinel）
        let cache = pipeline.cache.lock().expect("mutex");
        assert!(cache.map.is_empty());
        assert!(cache.order.is_empty());
    }

    // ========================================================================
    // SignatureCache LRU 行为单测 —— 直接构造数据结构测试 lookup/insert/淘汰
    // ========================================================================

    fn dummy_sig(size: u64) -> FileSignature {
        FileSignature {
            mtime: std::time::UNIX_EPOCH + Duration::from_secs(size),
            size,
            #[cfg(unix)]
            identity: crate::cache_signature::FileIdentity::Unix { dev: 1, ino: size },
            #[cfg(not(unix))]
            identity: crate::cache_signature::FileIdentity::None,
        }
    }

    #[test]
    fn signature_cache_lookup_returns_none_when_empty() {
        let mut cache = SignatureCache::new(3);
        assert!(cache.lookup(&("p".into(), "s".into())).is_none());
    }

    #[test]
    fn signature_cache_insert_and_lookup_roundtrip() {
        let mut cache = SignatureCache::new(3);
        let key = ("p".to_string(), "s".to_string());
        let sig = dummy_sig(10);
        cache.insert(key.clone(), sig);
        assert_eq!(cache.lookup(&key), Some(sig));
    }

    #[test]
    fn signature_cache_evicts_lru_when_over_capacity() {
        let mut cache = SignatureCache::new(2);
        cache.insert(("p".into(), "s1".into()), dummy_sig(1));
        cache.insert(("p".into(), "s2".into()), dummy_sig(2));
        cache.insert(("p".into(), "s3".into()), dummy_sig(3));
        // s1 应被淘汰
        assert!(cache.lookup(&("p".into(), "s1".into())).is_none());
        assert!(cache.lookup(&("p".into(), "s2".into())).is_some());
        assert!(cache.lookup(&("p".into(), "s3".into())).is_some());
        assert!(cache.map.len() <= 2);
    }

    #[test]
    fn signature_cache_lookup_bumps_hit_to_front() {
        // 验证命中 bump 到队首：连续访问 s1 在容量满时不被淘汰
        let mut cache = SignatureCache::new(2);
        cache.insert(("p".into(), "s1".into()), dummy_sig(1));
        cache.insert(("p".into(), "s2".into()), dummy_sig(2));
        // lookup s1 应 bump 到队首
        assert!(cache.lookup(&("p".into(), "s1".into())).is_some());
        // 现在插入 s3 → 应淘汰 s2 而不是 s1
        cache.insert(("p".into(), "s3".into()), dummy_sig(3));
        assert!(cache.lookup(&("p".into(), "s1".into())).is_some());
        assert!(cache.lookup(&("p".into(), "s2".into())).is_none());
    }
}
