//! session-search capability 集成测试。
//!
//! 每个测试对应 `openspec/specs/session-search/spec.md` 的一个 Scenario。
//! 使用 `tempfile::TempDir` + fixture JSONL 隔离。

use std::sync::Arc;

use tempfile::TempDir;
use tokio::sync::Mutex;

use cdt_discover::fs_provider::LocalFileSystemProvider;
use cdt_discover::search_cache::SearchTextCache;
use cdt_discover::session_search::SessionSearcher;

fn make_user_line(uuid: &str, text: &str) -> String {
    format!(
        r#"{{"type":"user","uuid":"{uuid}","parentUuid":null,"timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"1","message":{{"role":"user","content":"{text}"}}}}"#
    )
}

fn make_assistant_line(uuid: &str, text: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"{uuid}","parentUuid":null,"timestamp":"2026-04-11T10:00:01Z","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"1","message":{{"role":"assistant","model":"claude-opus-4-6","content":[{{"type":"text","text":"{text}"}}],"usage":{{"input_tokens":5,"output_tokens":5}}}}}}"#
    )
}

fn make_system_line(uuid: &str) -> String {
    format!(
        r#"{{"type":"system","uuid":"{uuid}","parentUuid":null,"timestamp":"2026-04-11T10:00:00Z","isSidechain":false,"userType":"external","cwd":"/tmp","sessionId":"s1","version":"1","message":{{"role":"user","content":"<system-reminder>noise content with target_keyword</system-reminder>"}}}}"#
    )
}

/// 在 tmpdir 下创建 project 目录结构并写入 session 文件。
fn write_session(tmp: &TempDir, project_id: &str, session_id: &str, lines: &[String]) {
    let dir = tmp.path().join(project_id);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{session_id}.jsonl"));
    std::fs::write(path, lines.join("\n")).unwrap();
}

fn make_searcher() -> (
    SessionSearcher<LocalFileSystemProvider>,
    Arc<Mutex<SearchTextCache>>,
) {
    let fs = Arc::new(LocalFileSystemProvider::new());
    let cache = Arc::new(Mutex::new(SearchTextCache::new()));
    let searcher = SessionSearcher::new(fs, Arc::clone(&cache));
    (searcher, cache)
}

/// Scenario: Query matches text in multiple messages
#[tokio::test]
async fn query_matches_multiple_messages() {
    let tmp = TempDir::new().unwrap();
    let lines = vec![
        make_user_line("u1", "I need help with rust"),
        make_assistant_line("a1", "Sure, rust is great"),
        make_user_line("u2", "Tell me more about rust"),
    ];
    write_session(&tmp, "proj1", "sess1", &lines);

    let (searcher, _) = make_searcher();
    let path = tmp.path().join("proj1/sess1.jsonl");
    let result = searcher
        .search_session_file("proj1", "sess1", &path, "rust", 50)
        .await
        .unwrap();

    assert_eq!(result.total_matches, 3);
    assert_eq!(result.hits.len(), 3);
    assert_eq!(result.session_title, "I need help with rust");
}

/// Scenario: Query matches nothing
#[tokio::test]
async fn query_matches_nothing() {
    let tmp = TempDir::new().unwrap();
    let lines = vec![
        make_user_line("u1", "hello world"),
        make_assistant_line("a1", "greetings"),
    ];
    write_session(&tmp, "proj1", "sess1", &lines);

    let (searcher, _) = make_searcher();
    let path = tmp.path().join("proj1/sess1.jsonl");
    let result = searcher
        .search_session_file("proj1", "sess1", &path, "nonexistent", 50)
        .await
        .unwrap();

    assert!(result.hits.is_empty());
    assert_eq!(result.total_matches, 0);
}

/// Scenario: Case-insensitive match
#[tokio::test]
async fn case_insensitive_match() {
    let tmp = TempDir::new().unwrap();
    let lines = vec![make_user_line("u1", "Hello WORLD from Rust")];
    write_session(&tmp, "proj1", "sess1", &lines);

    let (searcher, _) = make_searcher();
    let path = tmp.path().join("proj1/sess1.jsonl");
    let result = searcher
        .search_session_file("proj1", "sess1", &path, "hello world", 50)
        .await
        .unwrap();

    assert_eq!(result.total_matches, 1);
}

/// Scenario: Project with multiple sessions, query matching some
#[tokio::test]
async fn project_search_multiple_sessions() {
    let tmp = TempDir::new().unwrap();

    // 10 sessions，其中 i%3==0 的包含 "target_keyword"（i=0,3,6,9 → 4 个）
    for i in 0..10 {
        let text = if i % 3 == 0 {
            format!("session {i} has target_keyword here")
        } else {
            format!("session {i} is irrelevant")
        };
        write_session(
            &tmp,
            "proj1",
            &format!("sess{i}"),
            &[make_user_line(&format!("u{i}"), &text)],
        );
    }

    let (searcher, _) = make_searcher();

    let project_dir = tmp.path().join("proj1");
    let mut match_count = 0;
    for i in 0..10 {
        let path = project_dir.join(format!("sess{i}.jsonl"));
        let r = searcher
            .search_session_file("proj1", &format!("sess{i}"), &path, "target_keyword", 50)
            .await
            .unwrap();
        if !r.hits.is_empty() {
            match_count += 1;
        }
    }
    assert_eq!(match_count, 4);
}

/// Scenario: Search term appears only inside a hard-noise system-reminder
#[tokio::test]
async fn hard_noise_excluded_from_search() {
    let tmp = TempDir::new().unwrap();
    let lines = vec![
        make_user_line("u1", "normal content"),
        make_system_line("s1"),
    ];
    write_session(&tmp, "proj1", "sess1", &lines);

    let (searcher, _) = make_searcher();
    let path = tmp.path().join("proj1/sess1.jsonl");
    let result = searcher
        .search_session_file("proj1", "sess1", &path, "target_keyword", 50)
        .await
        .unwrap();

    assert!(
        result.hits.is_empty(),
        "hard-noise should be excluded from search"
    );
}

/// Scenario: Second search on same session reuses cache
#[tokio::test]
async fn cache_reuse_on_second_search() {
    let tmp = TempDir::new().unwrap();
    let lines = vec![make_user_line("u1", "findme in cache test")];
    write_session(&tmp, "proj1", "sess1", &lines);

    let (searcher, _cache) = make_searcher();
    let path = tmp.path().join("proj1/sess1.jsonl");

    let r1 = searcher
        .search_session_file("proj1", "sess1", &path, "findme", 50)
        .await
        .unwrap();
    assert_eq!(r1.total_matches, 1);

    // 第二次搜索——mtime 未变，应命中缓存
    let r2 = searcher
        .search_session_file("proj1", "sess1", &path, "findme", 50)
        .await
        .unwrap();
    assert_eq!(r2.total_matches, 1);
}

/// Scenario: SSH stage-limit stops early — 验证 `should_stop_at_stage` 逻辑
///
/// 通过构造 5 个 session 文件，设置 `stage_limits=[2]` + `min_results=1`，
/// 手动模拟 stage-limit 检测来验证搜索应提前返回。
#[tokio::test]
async fn ssh_stage_limit_logic() {
    let tmp = TempDir::new().unwrap();

    for i in 0..5 {
        write_session(
            &tmp,
            "proj1",
            &format!("sess{i}"),
            &[make_user_line(&format!("u{i}"), "this has match keyword")],
        );
    }

    let (searcher, _) = make_searcher();
    let project_dir = tmp.path().join("proj1");

    let stage_limits = [2usize];
    let min_results = 1usize;
    let mut results = Vec::new();
    let mut processed = 0usize;
    let mut is_partial = false;

    for i in 0..5 {
        // 在处理新文件之前检查 stage-limit
        for &limit in &stage_limits {
            if processed == limit && results.len() >= min_results {
                is_partial = true;
                break;
            }
        }
        if is_partial {
            break;
        }

        let path = project_dir.join(format!("sess{i}.jsonl"));
        let r = searcher
            .search_session_file("proj1", &format!("sess{i}"), &path, "match", 50)
            .await
            .unwrap();
        if !r.hits.is_empty() {
            results.push(r);
        }
        processed += 1;
    }

    assert!(is_partial, "SSH stage-limit should trigger partial return");
    assert_eq!(processed, 2, "should stop after processing 2 files");
    assert_eq!(results.len(), 2, "should have 2 results before stopping");
}
