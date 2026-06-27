//! Workflow manifest 读取 + `FileSignature` 缓存。
//!
//! manifest `<session_dir>/workflows/wf_<runId>.json` immutable——
//! 写入后内容不变。首次 stat+read 后按 `FileSignature` 缓存，
//! 后续只 stat 比对。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cdt_core::workflow::{
    WorkflowAgent, WorkflowAgentState, WorkflowItem, WorkflowPhase, WorkflowStatus,
};
use cdt_discover::FileSystemProvider;
use cdt_fs::FsError;

use super::workflow_script::{ScriptMeta, parse_script_meta};
use crate::cache_signature::FileSignature;

/// `scriptPreview` 截断上限——bounds IPC payload 与缓存内存。超限按 UTF-8 边界
/// 截断并追加可见 marker（见 `truncate_script_preview`）。
const SCRIPT_PREVIEW_MAX_BYTES: usize = 32 * 1024;

/// scriptPath 文件读取上限——远高于 Workflow inline script 512 KB 合法上限，覆盖一切
/// 合法脚本。超此值不全量 `read_to_string`（bound 异常大文件的瞬态内存，codex #7）。
const MAX_SCRIPT_READ_BYTES: usize = 1024 * 1024;

/// 从一次 script 文件读派生的两份产物——共缓存避免双读（design D4）。
#[derive(Clone)]
struct ScriptData {
    /// 静态 meta（name + phases，Tier 1）。`None` = 无 meta 块 / 解析失败。
    meta: Option<ScriptMeta>,
    /// 截断后的脚本预览（含截断 marker）。`None` = 文件读失败。
    preview: Option<String>,
}

struct CacheEntry {
    sig: FileSignature,
    item: WorkflowItem,
}

struct JournalCacheEntry {
    sig: FileSignature,
    agents: Vec<WorkflowAgent>,
}

struct ScriptCacheEntry {
    sig: FileSignature,
    /// 含「读/解析失败」的负缓存——同样缓存以免每次 poll 重复读/解析坏 script。
    data: ScriptData,
}

#[derive(Default)]
pub struct WorkflowManifestCache {
    entries: HashMap<PathBuf, CacheEntry>,
    /// 运行态 journal 派生的合成 agents 缓存（manifest 缺失降级路径）。
    /// journal append-only → 每次 append 签名变化 → 自动失效重读；
    /// 仅在 journal 未变化（如非 journal 触发的 refresh）时复用。
    ///
    /// **不返回 stale 计数的依据是 `FileSignature` 的 size 维度**：journal 严格
    /// append-only，每次 append 后 `size` 单调增、永不回退，故即使 mtime 秒级精度
    /// 撞同秒，size 变化仍让签名 miss 重读。若未来 journal 改为 rotate/rewrite
    /// （size 可能回退到旧值）则此不变量失效，须改用内容哈希或 offset 续读。
    journal_entries: HashMap<PathBuf, JournalCacheEntry>,
    /// 运行态 script meta 解析缓存（Tier 1）。script immutable → 一辈子只解析一次。
    script_entries: HashMap<PathBuf, ScriptCacheEntry>,
}

impl WorkflowManifestCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn get(&self, path: &Path, sig: &FileSignature) -> Option<WorkflowItem> {
        self.entries
            .get(path)
            .filter(|e| &e.sig == sig)
            .map(|e| e.item.clone())
    }

    fn insert(&mut self, path: PathBuf, sig: FileSignature, item: WorkflowItem) {
        self.entries.insert(path, CacheEntry { sig, item });
    }

    fn get_journal(&self, path: &Path, sig: &FileSignature) -> Option<Vec<WorkflowAgent>> {
        self.journal_entries
            .get(path)
            .filter(|e| &e.sig == sig)
            .map(|e| e.agents.clone())
    }

    fn insert_journal(&mut self, path: PathBuf, sig: FileSignature, agents: Vec<WorkflowAgent>) {
        self.journal_entries
            .insert(path, JournalCacheEntry { sig, agents });
    }

    fn get_script(&self, path: &Path, sig: &FileSignature) -> Option<ScriptData> {
        self.script_entries
            .get(path)
            .filter(|e| &e.sig == sig)
            .map(|e| e.data.clone())
    }

    fn insert_script(&mut self, path: PathBuf, sig: FileSignature, data: ScriptData) {
        self.script_entries
            .insert(path, ScriptCacheEntry { sig, data });
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawManifest {
    #[serde(default)]
    workflow_progress: Vec<serde_json::Value>,
    #[serde(default)]
    logs: Vec<String>,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    duration_ms: u64,
    #[serde(default)]
    workflow_name: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

pub fn parse_manifest(run_id: &str, content: &str) -> Result<WorkflowItem, String> {
    let raw: RawManifest =
        serde_json::from_str(content).map_err(|e| format!("manifest JSON parse: {e}"))?;

    let failed_indices = extract_failed_indices(&raw.logs);

    let mut phases: Vec<WorkflowPhase> = Vec::new();
    let mut agents: Vec<WorkflowAgent> = Vec::new();

    for entry_val in &raw.workflow_progress {
        let Some(type_str) = entry_val.get("type").and_then(serde_json::Value::as_str) else {
            continue;
        };
        match type_str {
            "workflow_phase" => {
                if let (Some(index), Some(title)) = (
                    entry_val.get("index").and_then(serde_json::Value::as_u64),
                    entry_val.get("title").and_then(serde_json::Value::as_str),
                ) {
                    #[allow(clippy::cast_possible_truncation)]
                    phases.push(WorkflowPhase {
                        index: index as u32,
                        title: title.to_owned(),
                    });
                }
            }
            "workflow_agent" => {
                let label = entry_val
                    .get("label")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                #[allow(clippy::cast_possible_truncation)]
                let phase_index = entry_val
                    .get("phaseIndex")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as u32;
                let state_str = entry_val
                    .get("state")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("pending");
                let tokens = entry_val
                    .get("tokens")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let tool_calls = entry_val
                    .get("toolCalls")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let duration_ms = entry_val
                    .get("durationMs")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let result_preview = entry_val
                    .get("resultPreview")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
                let queued_at = entry_val
                    .get("queuedAt")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
                let session_id = entry_val
                    .get("agentId")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);

                let agent_index = agents.len();
                let failed_by_log = failed_indices.contains(&(agent_index + 1));
                let failed_by_heuristic = matches!(state_str, "completed" | "done")
                    && tokens == 0
                    && tool_calls == 0
                    && result_preview.is_none();
                let failed = failed_by_log || failed_by_heuristic;

                let state = if failed {
                    WorkflowAgentState::Failed
                } else {
                    match state_str {
                        "completed" | "done" => WorkflowAgentState::Completed,
                        "running" => WorkflowAgentState::Running,
                        _ => WorkflowAgentState::Pending,
                    }
                };

                agents.push(WorkflowAgent {
                    label,
                    phase_index,
                    state,
                    tokens,
                    tool_calls,
                    duration_ms,
                    result_preview,
                    queued_at,
                    failed,
                    session_id,
                });
            }
            _ => {}
        }
    }

    let any_failed = agents.iter().any(|a| a.failed);
    let all_done = !agents.is_empty()
        && agents.iter().all(|a| {
            matches!(
                a.state,
                WorkflowAgentState::Completed | WorkflowAgentState::Failed
            )
        });
    let any_running = agents
        .iter()
        .any(|a| a.state == WorkflowAgentState::Running);
    let top_level_completed = raw.status.as_deref() == Some("completed");

    let status = if agents.is_empty() {
        match raw.status.as_deref() {
            Some("completed") => WorkflowStatus::Completed,
            Some("failed") => WorkflowStatus::Failed,
            _ => WorkflowStatus::Pending,
        }
    } else if any_failed && all_done {
        WorkflowStatus::PartialFailure
    } else if all_done || (top_level_completed && !any_running) {
        WorkflowStatus::Completed
    } else {
        WorkflowStatus::Running
    };

    let error = if any_failed {
        raw.logs.iter().find(|l| l.contains("failed")).cloned()
    } else {
        None
    };

    Ok(WorkflowItem {
        run_id: run_id.to_owned(),
        name: raw.workflow_name,
        status,
        phases,
        agents,
        total_tokens: raw.total_tokens,
        duration_ms: raw.duration_ms,
        error,
        // preview 由 resolve_single wrapper 统一填充（design D1），parse 阶段不设。
        script_preview: None,
    })
}

fn extract_failed_indices(logs: &[String]) -> Vec<usize> {
    logs.iter()
        .filter_map(|l| extract_index_from_log(l))
        .collect()
}

fn extract_index_from_log(log: &str) -> Option<usize> {
    for prefix in &["parallel[", "pipeline["] {
        if let Some(start) = log.find(prefix) {
            let after = &log[start + prefix.len()..];
            if let Some(end) = after.find(']') {
                if let Ok(n) = after[..end].parse::<usize>() {
                    if log[start..].contains("failed") {
                        return Some(n);
                    }
                }
            }
        }
    }
    None
}

/// workflow 解析候选——按 `run_id` 去重，`script_path` / `inline_script` 各取第一个
/// 非空值。
pub struct WorkflowCandidate {
    pub run_id: String,
    /// `scriptPath` 形态：脚本文件路径，供 name 剥取 + preview 读文件。
    pub script_path: Option<String>,
    /// inline `{script}` 形态：脚本内容（来自 `tool_use.input.script`），供 preview 零 I/O 取用。
    pub inline_script: Option<String>,
}

/// 收集 workflow 候选——按 `run_id` 去重，`script_path` / `inline_script` 各取第一个非空值。
///
/// `script_path` 供运行态降级（manifest 缺失）时剥取 workflow name；`inline_script`
/// 供 inline `{script}` 形态零文件 I/O 填充 `scriptPreview`。
pub fn collect_workflow_candidates(chunks: &[cdt_core::Chunk]) -> Vec<WorkflowCandidate> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<WorkflowCandidate> = Vec::new();
    for chunk in chunks {
        if let cdt_core::Chunk::Ai(ai) = chunk {
            for exec in &ai.tool_executions {
                let Some(run_id) = exec.workflow_run_id.as_ref() else {
                    continue;
                };
                let inline_script = exec
                    .input
                    .get("script")
                    .and_then(serde_json::Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
                if seen.insert(run_id.clone()) {
                    out.push(WorkflowCandidate {
                        run_id: run_id.clone(),
                        script_path: exec.workflow_script_path.clone(),
                        inline_script,
                    });
                } else if let Some(slot) = out.iter_mut().find(|c| &c.run_id == run_id) {
                    // 同 run_id 已存在但来源缺失时，补上后续 exec 的非空值
                    if slot.script_path.is_none() {
                        slot.script_path.clone_from(&exec.workflow_script_path);
                    }
                    if slot.inline_script.is_none() {
                        slot.inline_script = inline_script;
                    }
                }
            }
        }
    }
    out
}

pub async fn resolve_workflow_items(
    chunks: &[cdt_core::Chunk],
    session_dir: &Path,
    fs: &dyn FileSystemProvider,
    cache: &std::sync::Mutex<WorkflowManifestCache>,
) -> Vec<WorkflowItem> {
    let candidates = collect_workflow_candidates(chunks);
    if candidates.is_empty() {
        return Vec::new();
    }

    let workflows_dir = session_dir.join("workflows");
    let mut items = Vec::with_capacity(candidates.len());

    for cand in &candidates {
        let run_id = &cand.run_id;
        let manifest_path = workflows_dir.join(format!("{run_id}.json"));
        let journal_path = session_dir
            .join("subagents")
            .join("workflows")
            .join(run_id)
            .join("journal.jsonl");
        let item = resolve_single(
            run_id,
            &manifest_path,
            &journal_path,
            cand.script_path.as_deref(),
            cand.inline_script.as_deref(),
            fs,
            cache,
        )
        .await;
        items.push(item);
    }

    items
}

/// 完整解析单个 workflow（manifest + journal + script）。
///
/// 对外暴露给 `get_workflow_detail` IPC command 使用。该路径不携带 chunks，
/// `script_path`/`inline_script` 均传 `None`（detail preview = None，见 design D6）。
pub async fn resolve_single_detail(
    run_id: &str,
    manifest_path: &Path,
    journal_path: &Path,
    script_path: Option<&str>,
    fs: &dyn FileSystemProvider,
    cache: &std::sync::Mutex<WorkflowManifestCache>,
) -> WorkflowItem {
    resolve_single(
        run_id,
        manifest_path,
        journal_path,
        script_path,
        None,
        fs,
        cache,
    )
    .await
}

/// wrapper：inner 产出 item（不含 preview）后，单点统一填充 `script_preview`
/// （design D1）。preview 来源（inline 内存 / scriptPath 文件）与 manifest 读取
/// 相互独立——即便 inner 走错误 placeholder，仍按独立来源填 preview（design D5）。
async fn resolve_single(
    run_id: &str,
    manifest_path: &Path,
    journal_path: &Path,
    script_path: Option<&str>,
    inline_script: Option<&str>,
    fs: &dyn FileSystemProvider,
    cache: &std::sync::Mutex<WorkflowManifestCache>,
) -> WorkflowItem {
    let mut item =
        resolve_single_inner(run_id, manifest_path, journal_path, script_path, fs, cache).await;
    item.script_preview = resolve_script_preview(script_path, inline_script, fs, cache).await;
    item
}

async fn resolve_single_inner(
    run_id: &str,
    manifest_path: &Path,
    journal_path: &Path,
    script_path: Option<&str>,
    fs: &dyn FileSystemProvider,
    cache: &std::sync::Mutex<WorkflowManifestCache>,
) -> WorkflowItem {
    // manifest **缺失**（NotFound）→ 进运行态降级路径（journal + scriptPath 合成）。
    // 非 NotFound（权限 / IO / SSH 连接抖动）：manifest 文件可能真实存在只是读不到，
    // **不能**误判成「运行中」合成虚假 Running 卡片——降级为 pending placeholder + warn。
    let fs_meta = match fs.stat(manifest_path).await {
        Ok(meta) => meta,
        Err(FsError::NotFound(_)) => {
            tracing::debug!(
                run_id,
                path = %manifest_path.display(),
                "workflow manifest absent, degrading to running-state synthesis"
            );
            return resolve_running_state(run_id, journal_path, script_path, fs, cache).await;
        }
        Err(e) => {
            tracing::warn!(
                run_id,
                path = %manifest_path.display(),
                error = %e,
                "workflow manifest stat failed (not NotFound), using pending placeholder"
            );
            return WorkflowItem::pending(run_id.to_owned());
        }
    };

    let sig = FileSignature::from_fs_metadata(&fs_meta);

    {
        let Ok(guard) = cache.lock() else {
            return WorkflowItem::pending(run_id.to_owned());
        };
        if let Some(cached) = guard.get(manifest_path, &sig) {
            return cached;
        }
    }

    let content = match fs.read_to_string(manifest_path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                run_id,
                path = %manifest_path.display(),
                error = %e,
                "workflow manifest read failed, using pending placeholder"
            );
            return WorkflowItem::pending(run_id.to_owned());
        }
    };

    let item = match parse_manifest(run_id, &content) {
        Ok(item) => item,
        Err(e) => {
            tracing::warn!(
                run_id,
                path = %manifest_path.display(),
                error = %e,
                "workflow manifest parse failed, using pending placeholder"
            );
            WorkflowItem::pending(run_id.to_owned())
        }
    };

    if let Ok(mut guard) = cache.lock() {
        guard.insert(manifest_path.to_owned(), sig, item.clone());
    }

    item
}

/// manifest 缺失时的运行态降级解析（Tier 0）。
///
/// 状态判定**只看 journal**，独立于 manifest 完成态的失败启发式：
/// - journal 缺失 → `Pending`（agent 刚启动 journal 尚未 append）
/// - journal 含 ≥1 `started` → `Running`，合成匿名 agents
///
/// 合成 agent 的 `Completed` 语义是「已结束」而非「已成功」——journal `result`
/// 对失败 agent 也 append，运行态不区分成败（成败裁定是 manifest 职责）。
async fn resolve_running_state(
    run_id: &str,
    journal_path: &Path,
    script_path: Option<&str>,
    fs: &dyn FileSystemProvider,
    cache: &std::sync::Mutex<WorkflowManifestCache>,
) -> WorkflowItem {
    let agents = read_journal_agents(journal_path, fs, cache).await;
    let status = if agents.is_empty() {
        WorkflowStatus::Pending
    } else {
        WorkflowStatus::Running
    };

    // Tier 1：解析 script meta 取 name + phases（失败静默降回 Tier 0）。
    let meta = match script_path {
        Some(p) => read_script_data(Path::new(p), fs, cache).await.meta,
        None => None,
    };
    // name 优先 meta.name（Tier 1 权威），否则从 scriptPath basename 剥取（Tier 0）。
    let name = meta
        .as_ref()
        .and_then(|m| m.name.clone())
        .or_else(|| script_path.and_then(|p| workflow_name_from_script_path(p, run_id)));
    let phases = meta.map(|m| m.phases).unwrap_or_default();

    WorkflowItem {
        run_id: run_id.to_owned(),
        name,
        status,
        phases,
        agents,
        total_tokens: 0,
        duration_ms: 0,
        error: None,
        // preview 由 resolve_single wrapper 统一填充（design D1）。
        script_preview: None,
    }
}

/// `scriptPreview` 来源分流（design D2）：inline `{script}` 优先（零文件 I/O），
/// 否则读 scriptPath 文件（缓存）。两者皆无 → `None`。
async fn resolve_script_preview(
    script_path: Option<&str>,
    inline_script: Option<&str>,
    fs: &dyn FileSystemProvider,
    cache: &std::sync::Mutex<WorkflowManifestCache>,
) -> Option<String> {
    // inline 优先：input.script 已驻 ToolExecution.input 内存，零文件 I/O。
    if let Some(inline) = inline_script.filter(|s| !s.is_empty()) {
        return Some(truncate_script_preview(inline));
    }
    let path = script_path?;
    read_script_data(Path::new(path), fs, cache).await.preview
}

/// 把脚本文本截断到 `SCRIPT_PREVIEW_MAX_BYTES`（UTF-8 字符边界），超限时尾部追加
/// 可见 marker（标注原始总字节数）。≤ 上限时原样返回。
fn truncate_script_preview(content: &str) -> String {
    if content.len() <= SCRIPT_PREVIEW_MAX_BYTES {
        return content.to_owned();
    }
    // 找 ≤ 上限的最大 UTF-8 字符边界，避免切出半个多字节字符（脚本含中文）。
    let mut end = SCRIPT_PREVIEW_MAX_BYTES;
    while end > 0 && !content.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n\n/* … script truncated, {} bytes total … */",
        &content[..end],
        content.len()
    )
}

/// 异常大脚本（> `MAX_SCRIPT_READ_BYTES`）的 preview——不全量读入内存，仅一行 marker。
fn oversize_script_marker(total_bytes: u64) -> String {
    format!("/* script too large to preview, {total_bytes} bytes total */")
}

/// 读 script 文件一次，派生 meta（Tier 1 name+phases）+ 截断 preview，按 `FileSignature`
/// 缓存整个 `ScriptData`（含负缓存）。script immutable → 稳态命中缓存复用、不重读盘
/// （并发 miss 窗口可能短暂双读，幂等无害，design D4）。stat/read 失败 → `{meta:None,
/// preview:None}`；文件 > `MAX_SCRIPT_READ_BYTES` → 不全量读入内存，preview 仅 oversize
/// marker、meta 降回 Tier 0（design D3/D5）。
async fn read_script_data(
    script_path: &Path,
    fs: &dyn FileSystemProvider,
    cache: &std::sync::Mutex<WorkflowManifestCache>,
) -> ScriptData {
    let empty = ScriptData {
        meta: None,
        preview: None,
    };
    let Ok(fs_meta) = fs.stat(script_path).await else {
        return empty;
    };
    let sig = FileSignature::from_fs_metadata(&fs_meta);

    if let Ok(guard) = cache.lock() {
        if let Some(cached) = guard.get_script(script_path, &sig) {
            return cached;
        }
    }

    // 异常大文件：不全量读入内存（bound 瞬态内存），preview 仅 marker、meta 降 Tier 0。
    let data = if fs_meta.size > MAX_SCRIPT_READ_BYTES as u64 {
        ScriptData {
            meta: None,
            preview: Some(oversize_script_marker(fs_meta.size)),
        }
    } else {
        // read 失败（异常）与 json5 解析失败（预期 graceful，如 backtick 值）都降回 Tier 0，
        // 但两者性质不同：read 失败留 `debug!` 信号，便于排查「运行中 workflow 缺编排器名/phases」。
        match fs.read_to_string(script_path).await {
            Ok(content) => ScriptData {
                meta: parse_script_meta(&content),
                preview: Some(truncate_script_preview(&content)),
            },
            Err(e) => {
                tracing::debug!(
                    path = %script_path.display(),
                    error = %e,
                    "workflow script read failed, falling back to Tier 0 (basename-derived name)"
                );
                empty
            }
        }
    };

    if let Ok(mut guard) = cache.lock() {
        guard.insert_script(script_path.to_owned(), sig, data.clone());
    }
    data
}

/// 读 journal.jsonl 合成匿名 agents，按 `FileSignature` 缓存。journal **缺失**（`NotFound`）
/// → 空 Vec（刚启动 journal 尚未 append，调用方据此判 `Pending`）。stat/read 非 `NotFound`
/// 失败（权限 / IO / 连接抖动）：文件可能存在却读不到，仍返回空 Vec 但 `warn!` 留信号——
/// 否则「运行中」会被静默误降级为 `Pending`，且异常无任何痕迹可排查。
async fn read_journal_agents(
    journal_path: &Path,
    fs: &dyn FileSystemProvider,
    cache: &std::sync::Mutex<WorkflowManifestCache>,
) -> Vec<WorkflowAgent> {
    let fs_meta = match fs.stat(journal_path).await {
        Ok(meta) => meta,
        Err(FsError::NotFound(_)) => return Vec::new(),
        Err(e) => {
            tracing::warn!(
                path = %journal_path.display(),
                error = %e,
                "workflow journal stat failed (not NotFound), treating as no agents"
            );
            return Vec::new();
        }
    };
    let sig = FileSignature::from_fs_metadata(&fs_meta);

    if let Ok(guard) = cache.lock() {
        if let Some(cached) = guard.get_journal(journal_path, &sig) {
            return cached;
        }
    }

    // stat 已成功 → 文件存在；read 失败是强异常信号（权限 / 截断 / 损坏），必须留痕。
    let content = match fs.read_to_string(journal_path).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                path = %journal_path.display(),
                error = %e,
                "workflow journal read failed, treating as no agents"
            );
            return Vec::new();
        }
    };
    let agents = parse_journal(&content);

    if let Ok(mut guard) = cache.lock() {
        guard.insert_journal(journal_path.to_owned(), sig, agents.clone());
    }
    agents
}

/// 轻量解析 journal.jsonl——**不做 JSON 全解析**（`result` 行内嵌完整 agent
/// 输出可能很大）。仅按行首 `{"type":"started"` / `{"type":"result"` 判类型 +
/// 子串提取顶层 `agentId`。按 agentId 首见顺序去重；任一 `result` → `Completed`，
/// 仅 `started` → `Running`。半截行 / 无 agentId 行静默跳过（graceful）。
fn parse_journal(content: &str) -> Vec<WorkflowAgent> {
    let mut order: Vec<String> = Vec::new();
    let mut completed: HashMap<String, bool> = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        let is_result = line.starts_with(r#"{"type":"result""#);
        let is_started = line.starts_with(r#"{"type":"started""#);
        if !is_result && !is_started {
            continue;
        }
        let Some(agent_id) = extract_journal_agent_id(line) else {
            continue;
        };
        if !completed.contains_key(agent_id) {
            order.push(agent_id.to_owned());
            completed.insert(agent_id.to_owned(), false);
        }
        if is_result {
            completed.insert(agent_id.to_owned(), true);
        }
    }

    order
        .into_iter()
        .map(|id| WorkflowAgent {
            label: String::new(),
            phase_index: 0,
            state: if completed.get(&id).copied().unwrap_or(false) {
                WorkflowAgentState::Completed
            } else {
                WorkflowAgentState::Running
            },
            tokens: 0,
            tool_calls: 0,
            duration_ms: 0,
            result_preview: None,
            queued_at: None,
            failed: false,
            session_id: Some(id),
        })
        .collect()
}

/// 从 journal 行子串提取顶层 `"agentId":"<id>"`。result 行里嵌套的 agent 输出中
/// 若含 `agentId` 也会被 JSON 转义（`\"agentId\":`），未转义的顶层 key 先于嵌套
/// 出现，`find` 命中的即顶层值。
fn extract_journal_agent_id(line: &str) -> Option<&str> {
    const KEY: &str = r#""agentId":""#;
    let start = line.find(KEY)? + KEY.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    let id = &rest[..end];
    if id.is_empty() { None } else { Some(id) }
}

/// 从 scriptPath basename 精确剥取 workflow name：先 `strip_suffix(".js")`，
/// 再 `strip_suffix("-<run_id>")`。任一不匹配（runId 与文件名后缀不一致 /
/// resume 场景）→ `None`，绝不模糊匹配剥出半截垃圾。
fn workflow_name_from_script_path(script_path: &str, run_id: &str) -> Option<String> {
    let file_name = Path::new(script_path).file_name()?.to_str()?;
    let stem = file_name.strip_suffix(".js")?;
    let name = stem.strip_suffix(&format!("-{run_id}"))?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest_full_success() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_phase", "index": 1, "title": "Build"},
                {"type": "workflow_agent", "label": "agent-1", "phaseIndex": 1, "state": "completed", "tokens": 3000, "toolCalls": 8, "durationMs": 20000, "resultPreview": "Done"}
            ],
            "status": "completed",
            "logs": [],
            "result": {"output": "success"},
            "agentCount": 1,
            "totalTokens": 3000,
            "durationMs": 20000,
            "workflowName": "Code Review"
        }"#;
        let item = parse_manifest("wf_test1", json).unwrap();

        assert_eq!(item.run_id, "wf_test1");
        assert_eq!(item.name.as_deref(), Some("Code Review"));
        assert_eq!(item.status, WorkflowStatus::Completed);
        assert_eq!(item.phases.len(), 1);
        assert_eq!(item.phases[0].title, "Build");
        assert_eq!(item.agents.len(), 1);
        assert_eq!(item.agents[0].label, "agent-1");
        assert!(!item.agents[0].failed);
        assert_eq!(item.total_tokens, 3000);
    }

    #[test]
    fn parse_manifest_agent_failed_by_heuristic() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_agent", "label": "agent-dead", "phaseIndex": 1, "state": "completed", "tokens": 0, "toolCalls": 0}
            ],
            "status": "completed",
            "logs": [],
            "totalTokens": 0,
            "durationMs": 5000
        }"#;
        let item = parse_manifest("wf_fail1", json).unwrap();

        assert_eq!(item.status, WorkflowStatus::PartialFailure);
        assert!(item.agents[0].failed);
        assert_eq!(item.agents[0].state, WorkflowAgentState::Failed);
    }

    #[test]
    fn parse_manifest_agent_failed_by_log() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_agent", "label": "agent-1", "phaseIndex": 1, "state": "completed", "tokens": 5000, "toolCalls": 10, "durationMs": 30000, "resultPreview": "OK"}
            ],
            "status": "completed",
            "logs": ["parallel[1] failed: timeout"],
            "totalTokens": 5000,
            "durationMs": 30000
        }"#;
        let item = parse_manifest("wf_fail2", json).unwrap();

        assert_eq!(item.status, WorkflowStatus::PartialFailure);
        assert!(item.agents[0].failed);
        assert_eq!(item.error.as_deref(), Some("parallel[1] failed: timeout"));
    }

    #[test]
    fn parse_manifest_invalid_json() {
        let result = parse_manifest("wf_bad", "not json {{{");
        assert!(result.is_err());
    }

    #[test]
    fn extract_failed_indices_various_patterns() {
        let logs = vec![
            "parallel[1] failed: error".to_owned(),
            "pipeline[3] failed: timeout".to_owned(),
            "some other log".to_owned(),
        ];
        let indices = extract_failed_indices(&logs);
        assert_eq!(indices, vec![1, 3]);
    }

    #[test]
    fn cache_hit_and_miss() {
        use crate::cache_signature::FileIdentity;
        use std::time::SystemTime;

        let sig1 = FileSignature {
            mtime: SystemTime::UNIX_EPOCH,
            size: 100,
            identity: FileIdentity::None,
        };
        let sig2 = FileSignature {
            mtime: SystemTime::UNIX_EPOCH,
            size: 200,
            identity: FileIdentity::None,
        };

        let mut cache = WorkflowManifestCache::new();
        let path = PathBuf::from("/tmp/wf_test.json");
        let item = WorkflowItem::pending("wf_test".into());

        cache.insert(path.clone(), sig1, item.clone());

        assert_eq!(cache.get(&path, &sig1), Some(item));
        assert_eq!(cache.get(&path, &sig2), None);
    }

    #[test]
    fn collect_workflow_candidates_dedupes_and_picks_first_script_path() {
        use cdt_core::chunk::{AIChunk, Chunk, ChunkMetrics};
        use cdt_core::tool_execution::{ToolExecution, ToolOutput};
        use chrono::{TimeZone, Utc};

        let ts = Utc.with_ymd_and_hms(2026, 5, 29, 0, 0, 0).unwrap();
        let exec = |run_id: &str, script: Option<&str>| ToolExecution {
            tool_use_id: format!("tu_{run_id}"),
            tool_name: "Workflow".into(),
            input: serde_json::json!({}),
            output: ToolOutput::Missing,
            is_error: false,
            start_ts: ts,
            end_ts: None,
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
            error_message: None,
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
            workflow_run_id: Some(run_id.to_owned()),
            workflow_script_path: script.map(str::to_owned),
        };

        let chunks = vec![Chunk::Ai(AIChunk {
            chunk_id: "c1".into(),
            timestamp: ts,
            duration_ms: None,
            responses: vec![],
            metrics: ChunkMetrics::default(),
            semantic_steps: vec![],
            // wf_a 首见 script 缺失，后续 exec 补上；wf_b 自带 script
            tool_executions: vec![
                exec("wf_a", None),
                exec("wf_b", Some("/x/b-wf_b.js")),
                exec("wf_a", Some("/x/a-wf_a.js")),
            ],
            subagents: vec![],
            slash_commands: vec![],
            teammate_messages: vec![],
        })];

        let cands = collect_workflow_candidates(&chunks);
        let got: Vec<(String, Option<String>)> = cands
            .iter()
            .map(|c| (c.run_id.clone(), c.script_path.clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("wf_a".to_owned(), Some("/x/a-wf_a.js".to_owned())),
                ("wf_b".to_owned(), Some("/x/b-wf_b.js".to_owned())),
            ]
        );
        // 本用例 exec input 为空对象 → inline_script 均 None
        assert!(cands.iter().all(|c| c.inline_script.is_none()));
    }

    #[test]
    fn empty_chunks_returns_empty() {
        let cands = collect_workflow_candidates(&[]);
        assert!(cands.is_empty());
    }

    #[test]
    fn parse_manifest_agent_state_done_recognized_as_completed() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_phase", "index": 1, "title": "Assess"},
                {"type": "workflow_agent", "label": "assess:bugs", "phaseIndex": 1, "state": "done", "tokens": 80000, "toolCalls": 5, "durationMs": 90000, "resultPreview": "Found 3 bugs"},
                {"type": "workflow_agent", "label": "assess:perf", "phaseIndex": 1, "state": "done", "tokens": 75000, "toolCalls": 3, "durationMs": 85000, "resultPreview": "No issues"}
            ],
            "status": "completed",
            "logs": [],
            "totalTokens": 155000,
            "durationMs": 95000,
            "workflowName": "bug-hunt"
        }"#;
        let item = parse_manifest("wf_done_test", json).unwrap();

        assert_eq!(item.status, WorkflowStatus::Completed);
        assert_eq!(item.agents.len(), 2);
        assert_eq!(item.agents[0].state, WorkflowAgentState::Completed);
        assert_eq!(item.agents[1].state, WorkflowAgentState::Completed);
        assert!(!item.agents[0].failed);
        assert!(!item.agents[1].failed);
    }

    #[test]
    fn parse_manifest_top_level_status_fallback() {
        let json = r#"{
            "workflowProgress": [],
            "status": "completed",
            "logs": [],
            "totalTokens": 50000,
            "durationMs": 10000,
            "workflowName": "empty-progress"
        }"#;
        let item = parse_manifest("wf_status_fb", json).unwrap();

        assert_eq!(item.status, WorkflowStatus::Completed);
        assert_eq!(item.agents.len(), 0);
    }

    // ---- Tier 0 运行态降级：name 剥取 ----

    #[test]
    fn name_from_script_path_strips_runid_suffix() {
        let name = workflow_name_from_script_path(
            "/x/workflows/scripts/explore-workflow-rendering-wf_a3fbf671-153.js",
            "wf_a3fbf671-153",
        );
        assert_eq!(name.as_deref(), Some("explore-workflow-rendering"));
    }

    #[test]
    fn name_from_script_path_runid_mismatch_returns_none() {
        // resume 场景：当前 runId 与文件名后缀不一致 → strip_suffix 失败 → None
        let name =
            workflow_name_from_script_path("/x/scripts/foo-wf_aaaaaaaa-111.js", "wf_bbbbbbbb-222");
        assert_eq!(name, None);
    }

    #[test]
    fn name_from_script_path_no_js_or_empty_returns_none() {
        assert_eq!(
            workflow_name_from_script_path("/x/foo-wf_a.txt", "wf_a"),
            None
        );
        // basename == "-wf_a.js" → 剥后空 name → None
        assert_eq!(workflow_name_from_script_path("/x/-wf_a.js", "wf_a"), None);
    }

    // ---- Tier 0 运行态降级：journal 解析 ----

    #[test]
    fn parse_journal_started_and_result_mixed() {
        let journal = concat!(
            r#"{"type":"started","key":"v2:k1","agentId":"a1"}"#,
            "\n",
            r#"{"type":"started","key":"v2:k2","agentId":"a2"}"#,
            "\n",
            r#"{"type":"started","key":"v2:k3","agentId":"a3"}"#,
            "\n",
            r#"{"type":"result","key":"v2:k1","agentId":"a1","result":{"ok":true}}"#,
            "\n",
        );
        let agents = parse_journal(journal);
        assert_eq!(agents.len(), 3);
        // 首见顺序 a1/a2/a3；a1 有 result → Completed，a2/a3 仅 started → Running
        assert_eq!(agents[0].state, WorkflowAgentState::Completed);
        assert_eq!(agents[1].state, WorkflowAgentState::Running);
        assert_eq!(agents[2].state, WorkflowAgentState::Running);
        assert!(agents.iter().all(|a| !a.failed && a.label.is_empty()));
    }

    #[test]
    fn parse_journal_all_result_all_completed() {
        let journal = concat!(
            r#"{"type":"started","key":"v2:k1","agentId":"a1"}"#,
            "\n",
            r#"{"type":"result","key":"v2:k1","agentId":"a1","result":{}}"#,
            "\n",
            r#"{"type":"result","key":"v2:k2","agentId":"a2","result":{}}"#,
            "\n",
        );
        let agents = parse_journal(journal);
        assert_eq!(agents.len(), 2);
        assert!(
            agents
                .iter()
                .all(|a| a.state == WorkflowAgentState::Completed)
        );
    }

    #[test]
    fn parse_journal_skips_garbage_and_truncated_lines() {
        let journal = concat!(
            r#"{"type":"started","key":"v2:k1","agentId":"a1"}"#,
            "\n",
            "not json at all\n",
            r#"{"type":"started","key":"v2:k2","#, // 截断的半行（无 agentId 闭合）
            "\n",
            r#"{"type":"started","agentId":"a2"}"#,
            "\n",
        );
        let agents = parse_journal(journal);
        // a1 + a2 提取成功，垃圾行与截断行跳过
        assert_eq!(agents.len(), 2);
    }

    #[test]
    fn parse_journal_dedup_multiple_started_same_agent() {
        let journal = concat!(
            r#"{"type":"started","key":"v2:k1","agentId":"a1"}"#,
            "\n",
            r#"{"type":"started","key":"v2:k1b","agentId":"a1"}"#,
            "\n",
        );
        let agents = parse_journal(journal);
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].state, WorkflowAgentState::Running);
    }

    #[test]
    fn parse_journal_does_not_count_nested_escaped_agent_id() {
        // result 行内嵌的 agent 输出含被转义的 "agentId"——不应被当顶层 agentId 误计。
        // 顶层 agentId 在 nested result 之前，find 命中顶层值。
        let journal = concat!(
            r#"{"type":"result","key":"v2:k1","agentId":"top1","result":{"text":"see \"agentId\":\"nested\" inside"}}"#,
            "\n",
        );
        let agents = parse_journal(journal);
        assert_eq!(agents.len(), 1);
    }

    #[test]
    fn extract_journal_agent_id_picks_top_level_over_nested_object_key() {
        // 实证 journal result 行格式：顶层 agentId 始终在 result 字段**之前**
        //（design.md §journal 实证）。即便 result 是 JSON **对象**且内含未转义的
        // "agentId" key，`find` 先命中顶层值。正面锁定 review 对「嵌套对象 key
        // 误提取」的担忧——只要实证的「agentId 先于 result」不变量成立即安全。
        let line = r#"{"type":"result","key":"v2:k1","agentId":"top1","result":{"agentId":"nested","output":"x"}}"#;
        assert_eq!(extract_journal_agent_id(line), Some("top1"));
    }

    #[test]
    fn parse_journal_empty_returns_empty() {
        assert!(parse_journal("").is_empty());
        assert!(parse_journal("\n\n").is_empty());
    }

    // ---- Tier 0 运行态降级：resolve（fs + TempDir）----

    fn write_journal(dir: &std::path::Path, run_id: &str, lines: &[&str]) -> PathBuf {
        let jdir = dir.join("subagents").join("workflows").join(run_id);
        std::fs::create_dir_all(&jdir).unwrap();
        let jpath = jdir.join("journal.jsonl");
        std::fs::write(&jpath, lines.join("\n")).unwrap();
        jpath
    }

    #[tokio::test]
    async fn resolve_running_state_with_journal_is_running() {
        use cdt_discover::LocalFileSystemProvider;
        let tmp = tempfile::TempDir::new().unwrap();
        let run_id = "wf_a04767d2-4f1";
        let jpath = write_journal(
            tmp.path(),
            run_id,
            &[
                r#"{"type":"started","key":"v2:k1","agentId":"a1"}"#,
                r#"{"type":"started","key":"v2:k2","agentId":"a2"}"#,
                r#"{"type":"result","key":"v2:k1","agentId":"a1","result":{}}"#,
            ],
        );
        let script = format!("/x/workflows/scripts/assess-migration-{run_id}.js");
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());

        let item = resolve_running_state(run_id, &jpath, Some(&script), &fs, &cache).await;
        assert_eq!(item.status, WorkflowStatus::Running);
        assert_eq!(item.name.as_deref(), Some("assess-migration"));
        assert_eq!(item.agents.len(), 2);
        assert_eq!(item.agents[0].state, WorkflowAgentState::Completed);
        assert_eq!(item.agents[1].state, WorkflowAgentState::Running);
        assert!(item.phases.is_empty());
    }

    #[tokio::test]
    async fn resolve_running_state_no_journal_is_pending() {
        use cdt_discover::LocalFileSystemProvider;
        let tmp = tempfile::TempDir::new().unwrap();
        let run_id = "wf_none";
        let jpath = tmp
            .path()
            .join("subagents/workflows")
            .join(run_id)
            .join("journal.jsonl"); // 不创建该文件
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());

        let item = resolve_running_state(run_id, &jpath, None, &fs, &cache).await;
        assert_eq!(item.status, WorkflowStatus::Pending);
        assert!(item.agents.is_empty());
        assert_eq!(item.name, None);
    }

    #[tokio::test]
    async fn resolve_running_state_race_all_done_still_running() {
        use cdt_discover::LocalFileSystemProvider;
        let tmp = tempfile::TempDir::new().unwrap();
        let run_id = "wf_race";
        let jpath = write_journal(
            tmp.path(),
            run_id,
            &[
                r#"{"type":"started","key":"v2:k1","agentId":"a1"}"#,
                r#"{"type":"result","key":"v2:k1","agentId":"a1","result":{}}"#,
            ],
        );
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());

        let item = resolve_running_state(run_id, &jpath, None, &fs, &cache).await;
        // manifest 未写 → 仍 Running，即使 journal 全 result
        assert_eq!(item.status, WorkflowStatus::Running);
        assert_eq!(item.agents.len(), 1);
        assert_eq!(item.agents[0].state, WorkflowAgentState::Completed);
    }

    #[tokio::test]
    async fn resolve_single_prefers_manifest_when_present() {
        use cdt_discover::LocalFileSystemProvider;
        let tmp = tempfile::TempDir::new().unwrap();
        let run_id = "wf_done";
        // 同时存在 journal（全 running）与 manifest（completed）——manifest 优先
        let jpath = write_journal(
            tmp.path(),
            run_id,
            &[r#"{"type":"started","key":"v2:k1","agentId":"a1"}"#],
        );
        let wf_dir = tmp.path().join("workflows");
        std::fs::create_dir_all(&wf_dir).unwrap();
        let manifest_path = wf_dir.join(format!("{run_id}.json"));
        std::fs::write(
            &manifest_path,
            r#"{"workflowProgress":[{"type":"workflow_agent","label":"real","phaseIndex":0,"state":"done","tokens":100,"toolCalls":2,"resultPreview":"ok"}],"status":"completed","logs":[],"totalTokens":100,"durationMs":5,"workflowName":"done-wf"}"#,
        )
        .unwrap();
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());

        let item = resolve_single(run_id, &manifest_path, &jpath, None, None, &fs, &cache).await;
        // 走 manifest 完成态路径：status Completed，agent label 来自 manifest 不是匿名
        assert_eq!(item.status, WorkflowStatus::Completed);
        assert_eq!(item.agents.len(), 1);
        assert_eq!(item.agents[0].label, "real");
    }

    #[tokio::test]
    async fn resolve_running_state_tier1_parses_script_phases() {
        use cdt_discover::LocalFileSystemProvider;
        let tmp = tempfile::TempDir::new().unwrap();
        let run_id = "wf_t1run";
        let jpath = write_journal(
            tmp.path(),
            run_id,
            &[r#"{"type":"started","key":"v2:k1","agentId":"a1"}"#],
        );
        // 真写一个 script 文件含 meta（phases + name）
        let sdir = tmp.path().join("workflows").join("scripts");
        std::fs::create_dir_all(&sdir).unwrap();
        let spath = sdir.join(format!("my-flow-{run_id}.js"));
        std::fs::write(
            &spath,
            "export const meta = {\n  name: 'meta-name',\n  phases: [\n    { title: 'Assess', detail: 'x' },\n    { title: 'Synthesize' },\n  ],\n}\nphase('Assess')\n",
        )
        .unwrap();
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());

        let item =
            resolve_running_state(run_id, &jpath, Some(spath.to_str().unwrap()), &fs, &cache).await;
        assert_eq!(item.status, WorkflowStatus::Running);
        // Tier 1 成功：name 优先 meta.name，phases 取静态列表
        assert_eq!(item.name.as_deref(), Some("meta-name"));
        assert_eq!(item.phases.len(), 2);
        assert_eq!(item.phases[0].title, "Assess");
        assert_eq!(item.phases[1].title, "Synthesize");
    }

    #[tokio::test]
    async fn resolve_running_state_tier1_missing_script_falls_back_to_tier0_name() {
        use cdt_discover::LocalFileSystemProvider;
        let tmp = tempfile::TempDir::new().unwrap();
        let run_id = "wf_t0run";
        let jpath = write_journal(
            tmp.path(),
            run_id,
            &[r#"{"type":"started","key":"v2:k1","agentId":"a1"}"#],
        );
        // script 路径指向不存在的文件 → Tier 1 失败 → 降回 Tier 0（剥文件名得 name，phases 空）
        let script = format!("/nonexistent/scripts/fallback-flow-{run_id}.js");
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());

        let item = resolve_running_state(run_id, &jpath, Some(&script), &fs, &cache).await;
        assert_eq!(item.status, WorkflowStatus::Running);
        assert_eq!(item.name.as_deref(), Some("fallback-flow"));
        assert!(item.phases.is_empty());
    }

    // ---- 降级路径错误分流：非 NotFound 不能误判成运行态（codex 替代二审 SFH #1/#2）----

    /// 注错 fs：可让指定 path 的 `stat` 返回非 `NotFound` 的 Io error，或让 `stat`
    /// 成功但 `read_to_string` 失败——`LocalFileSystemProvider` + `TempDir` 无法
    /// 可靠制造「文件存在却读不到」，故需此 mock。
    struct FaultyFs {
        /// `stat` 返回非 `NotFound` 的 Io error（模拟权限 / IO / 连接抖动）。
        stat_io_err: Vec<PathBuf>,
        /// `stat` 成功（伪 metadata）但 `read_to_string` 返回 Io error。
        read_io_err: Vec<PathBuf>,
        /// 正常文件：`stat` ok + `read_to_string` 返回内容。
        files: Vec<(PathBuf, String)>,
    }

    impl FaultyFs {
        fn io_err(path: &Path) -> FsError {
            FsError::Io {
                path: path.to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "injected fault"),
            }
        }
        fn fake_meta(size: u64) -> cdt_fs::FsMetadata {
            cdt_fs::FsMetadata {
                size,
                mtime: std::time::SystemTime::UNIX_EPOCH,
                created: None,
                identity: None,
            }
        }
    }

    #[async_trait::async_trait]
    impl FileSystemProvider for FaultyFs {
        fn kind(&self) -> cdt_fs::FsKind {
            cdt_fs::FsKind::Local
        }
        async fn exists(&self, path: &Path) -> bool {
            self.files.iter().any(|(p, _)| p == path) || self.read_io_err.iter().any(|p| p == path)
        }
        async fn read_dir(&self, path: &Path) -> Result<Vec<cdt_fs::DirEntry>, FsError> {
            Err(Self::io_err(path))
        }
        async fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
            if self.read_io_err.iter().any(|p| p == path) {
                return Err(Self::io_err(path));
            }
            self.files
                .iter()
                .find(|(p, _)| p == path)
                .map(|(_, c)| c.clone())
                .ok_or_else(|| FsError::NotFound(path.to_path_buf()))
        }
        async fn stat(&self, path: &Path) -> Result<cdt_fs::FsMetadata, FsError> {
            if self.stat_io_err.iter().any(|p| p == path) {
                return Err(Self::io_err(path));
            }
            if self.read_io_err.iter().any(|p| p == path) {
                return Ok(Self::fake_meta(1));
            }
            self.files
                .iter()
                .find(|(p, _)| p == path)
                .map(|(_, c)| Self::fake_meta(c.len() as u64))
                .ok_or_else(|| FsError::NotFound(path.to_path_buf()))
        }
        async fn read_lines_head(&self, path: &Path, _max: usize) -> Result<Vec<String>, FsError> {
            Err(Self::io_err(path))
        }
        async fn open_read(
            &self,
            path: &Path,
        ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, FsError> {
            Err(Self::io_err(path))
        }
        async fn write_atomic(&self, _path: &Path, _content: &[u8]) -> Result<(), FsError> {
            unimplemented!("FaultyFs 不走写路径")
        }
        async fn create_dir_all(&self, _path: &Path) -> Result<(), FsError> {
            unimplemented!("FaultyFs 不走写路径")
        }
        async fn remove_file(&self, _path: &Path) -> Result<(), FsError> {
            unimplemented!("FaultyFs 不走写路径")
        }
    }

    #[tokio::test]
    async fn resolve_single_non_notfound_manifest_stat_error_does_not_synthesize_running() {
        // manifest stat 失败为**非 NotFound**（权限 / IO）→ manifest 可能真实存在只是
        // 读不到，即便 journal 有 started 也**不能**误判成 Running，须降级 pending placeholder。
        let run_id = "wf_faulty";
        let manifest_path = PathBuf::from("/wf/workflows/wf_faulty.json");
        let journal_path = PathBuf::from("/wf/subagents/workflows/wf_faulty/journal.jsonl");
        let fs = FaultyFs {
            stat_io_err: vec![manifest_path.clone()],
            read_io_err: vec![],
            files: vec![(
                journal_path.clone(),
                r#"{"type":"started","key":"v2:k1","agentId":"a1"}"#.to_owned(),
            )],
        };
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());
        let item = resolve_single(
            run_id,
            &manifest_path,
            &journal_path,
            None,
            None,
            &fs,
            &cache,
        )
        .await;
        // 修复前：进 resolve_running_state + journal 有 started → 虚假 Running。
        // 修复后：非 NotFound → pending placeholder。
        assert_eq!(item.status, WorkflowStatus::Pending);
        assert!(item.agents.is_empty());
    }

    #[tokio::test]
    async fn resolve_single_journal_read_error_treated_as_no_agents_not_running() {
        // manifest 真缺失（NotFound）→ 正常降级；但 journal stat 成功、read 失败（截断 /
        // 权限）→ read_journal_agents 返回空（留 warn），status 落 Pending 而非误判 Running。
        let run_id = "wf_jread";
        let manifest_path = PathBuf::from("/wf/workflows/wf_jread.json"); // 不在 files → NotFound
        let journal_path = PathBuf::from("/wf/subagents/workflows/wf_jread/journal.jsonl");
        let fs = FaultyFs {
            stat_io_err: vec![],
            read_io_err: vec![journal_path.clone()],
            files: vec![],
        };
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());
        let item = resolve_single(
            run_id,
            &manifest_path,
            &journal_path,
            None,
            None,
            &fs,
            &cache,
        )
        .await;
        assert_eq!(item.status, WorkflowStatus::Pending);
        assert!(item.agents.is_empty());
    }

    #[test]
    fn parse_manifest_extracts_agent_id_as_session_id() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_agent", "label": "reviewer", "phaseIndex": 1, "state": "done", "tokens": 100, "toolCalls": 2, "durationMs": 5000, "agentId": "ad34cb14a1ae5b192"}
            ],
            "status": "completed",
            "logs": [],
            "totalTokens": 100,
            "durationMs": 5000
        }"#;
        let item = parse_manifest("wf_test", json).unwrap();
        assert_eq!(
            item.agents[0].session_id.as_deref(),
            Some("ad34cb14a1ae5b192")
        );
    }

    #[test]
    fn parse_manifest_missing_agent_id_yields_none() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_agent", "label": "old-agent", "phaseIndex": 1, "state": "done", "tokens": 100, "toolCalls": 2, "durationMs": 5000}
            ],
            "status": "completed",
            "logs": [],
            "totalTokens": 100,
            "durationMs": 5000
        }"#;
        let item = parse_manifest("wf_test", json).unwrap();
        assert_eq!(item.agents[0].session_id, None);
    }

    #[test]
    fn parse_journal_populates_session_id() {
        let content = r#"{"type":"started","agentId":"abc123","key":"k1"}
{"type":"started","agentId":"def456","key":"k2"}
{"type":"result","agentId":"abc123","key":"k1","result":"ok"}
"#;
        let agents = parse_journal(content);
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].session_id.as_deref(), Some("abc123"));
        assert_eq!(agents[1].session_id.as_deref(), Some("def456"));
    }

    #[test]
    fn parse_manifest_running_agent_not_failed_by_heuristic() {
        let json = r#"{
            "workflowProgress": [
                {"type": "workflow_agent", "label": "agent-active", "phaseIndex": 0, "state": "running", "tokens": 0, "toolCalls": 0}
            ],
            "status": "running",
            "logs": [],
            "totalTokens": 0,
            "durationMs": 1000
        }"#;
        let item = parse_manifest("wf_running", json).unwrap();

        assert!(!item.agents[0].failed);
        assert_eq!(item.agents[0].state, WorkflowAgentState::Running);
    }

    // ---- scriptPreview 填充（issue #561）----

    #[test]
    fn truncate_script_preview_under_limit_unchanged() {
        let s = "export const meta = { name: 'x' }\nphase('A')";
        assert_eq!(truncate_script_preview(s), s);
    }

    #[test]
    fn truncate_script_preview_over_limit_appends_marker_at_char_boundary() {
        // 用多字节字符（中文）填充超上限，验证截断落在 UTF-8 边界 + marker 含总字节数
        let unit = "中"; // 3 bytes
        let total_chars = (SCRIPT_PREVIEW_MAX_BYTES / unit.len()) + 100;
        let big: String = unit.repeat(total_chars);
        let out = truncate_script_preview(&big);
        // 截断主体 ≤ 上限；含 marker；marker 标注原始总字节数
        assert!(out.contains("script truncated"));
        assert!(out.contains(&big.len().to_string()));
        // 主体部分（marker 前）是合法 UTF-8 且不超上限
        let body = out.split("\n\n/* ").next().unwrap();
        assert!(body.len() <= SCRIPT_PREVIEW_MAX_BYTES);
        assert!(std::str::from_utf8(body.as_bytes()).is_ok());
    }

    #[tokio::test]
    async fn resolve_script_preview_inline_zero_io() {
        use cdt_discover::LocalFileSystemProvider;
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());
        // script_path 指向不存在文件，但 inline 非空 → 取 inline，不读文件
        let preview = resolve_script_preview(
            Some("/nonexistent/should-not-be-read.js"),
            Some("inline body here"),
            &fs,
            &cache,
        )
        .await;
        assert_eq!(preview.as_deref(), Some("inline body here"));
    }

    #[tokio::test]
    async fn resolve_script_preview_scriptpath_reads_file() {
        use cdt_discover::LocalFileSystemProvider;
        let tmp = tempfile::TempDir::new().unwrap();
        let spath = tmp.path().join("flow-wf_x.js");
        std::fs::write(&spath, "export const meta = { name: 'fromfile' }").unwrap();
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());

        let preview =
            resolve_script_preview(Some(spath.to_str().unwrap()), None, &fs, &cache).await;
        assert_eq!(
            preview.as_deref(),
            Some("export const meta = { name: 'fromfile' }")
        );
    }

    #[tokio::test]
    async fn resolve_script_preview_no_source_is_none() {
        use cdt_discover::LocalFileSystemProvider;
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());
        assert_eq!(resolve_script_preview(None, None, &fs, &cache).await, None);
    }

    #[tokio::test]
    async fn resolve_script_preview_scriptpath_read_fail_is_none() {
        use cdt_discover::LocalFileSystemProvider;
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());
        // 文件不存在（stat NotFound）→ preview None，不 panic
        let preview =
            resolve_script_preview(Some("/nonexistent/scripts/x-wf_x.js"), None, &fs, &cache).await;
        assert_eq!(preview, None);
    }

    #[tokio::test]
    async fn read_script_data_oversize_file_marker_not_full_read() {
        use cdt_discover::LocalFileSystemProvider;
        let tmp = tempfile::TempDir::new().unwrap();
        let spath = tmp.path().join("huge-wf_big.js");
        // 写一个 > MAX_SCRIPT_READ_BYTES 的文件
        let big = "x".repeat(MAX_SCRIPT_READ_BYTES + 1024);
        std::fs::write(&spath, &big).unwrap();
        let fs = LocalFileSystemProvider::new();
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());

        let data = read_script_data(&spath, &fs, &cache).await;
        assert!(data.meta.is_none(), "oversize → meta 降 Tier 0");
        let preview = data.preview.expect("oversize → marker preview");
        assert!(preview.contains("too large"));
        assert!(preview.contains(&big.len().to_string()));
        // marker 远小于文件本身（证明没把全文塞进 preview）
        assert!(preview.len() < 200);
    }

    #[test]
    fn collect_workflow_candidates_extracts_inline_script() {
        use cdt_core::chunk::{AIChunk, Chunk, ChunkMetrics};
        use cdt_core::tool_execution::{ToolExecution, ToolOutput};
        use chrono::{TimeZone, Utc};

        let ts = Utc.with_ymd_and_hms(2026, 5, 29, 0, 0, 0).unwrap();
        let exec = ToolExecution {
            tool_use_id: "tu_1".into(),
            tool_name: "Workflow".into(),
            input: serde_json::json!({ "script": "export const meta = {}" }),
            output: ToolOutput::Missing,
            is_error: false,
            start_ts: ts,
            end_ts: None,
            source_assistant_uuid: "a1".into(),
            result_agent_id: None,
            error_message: None,
            output_omitted: false,
            output_bytes: None,
            teammate_spawn: None,
            workflow_run_id: Some("wf_inline".into()),
            workflow_script_path: None,
        };
        let chunks = vec![Chunk::Ai(AIChunk {
            chunk_id: "c1".into(),
            timestamp: ts,
            duration_ms: None,
            responses: vec![],
            metrics: ChunkMetrics::default(),
            semantic_steps: vec![],
            tool_executions: vec![exec],
            subagents: vec![],
            slash_commands: vec![],
            teammate_messages: vec![],
        })];

        let cands = collect_workflow_candidates(&chunks);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].run_id, "wf_inline");
        assert_eq!(cands[0].script_path, None);
        assert_eq!(
            cands[0].inline_script.as_deref(),
            Some("export const meta = {}")
        );
    }

    /// 计数型 fs：统计 `read_to_string` 真实调用次数，验证 script 缓存复用不重读盘。
    struct CountingFs {
        path: PathBuf,
        content: String,
        reads: std::sync::atomic::AtomicUsize,
    }

    #[async_trait::async_trait]
    impl FileSystemProvider for CountingFs {
        fn kind(&self) -> cdt_fs::FsKind {
            cdt_fs::FsKind::Local
        }
        async fn exists(&self, path: &Path) -> bool {
            path == self.path
        }
        async fn read_dir(&self, path: &Path) -> Result<Vec<cdt_fs::DirEntry>, FsError> {
            Err(FsError::NotFound(path.to_path_buf()))
        }
        async fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
            if path == self.path {
                self.reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(self.content.clone())
            } else {
                Err(FsError::NotFound(path.to_path_buf()))
            }
        }
        async fn stat(&self, path: &Path) -> Result<cdt_fs::FsMetadata, FsError> {
            if path == self.path {
                Ok(cdt_fs::FsMetadata {
                    size: self.content.len() as u64,
                    mtime: std::time::SystemTime::UNIX_EPOCH,
                    created: None,
                    identity: None,
                })
            } else {
                Err(FsError::NotFound(path.to_path_buf()))
            }
        }
        async fn read_lines_head(&self, path: &Path, _max: usize) -> Result<Vec<String>, FsError> {
            Err(FsError::NotFound(path.to_path_buf()))
        }
        async fn open_read(
            &self,
            path: &Path,
        ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, FsError> {
            Err(FsError::NotFound(path.to_path_buf()))
        }
        async fn write_atomic(&self, _path: &Path, _content: &[u8]) -> Result<(), FsError> {
            unimplemented!("CountingFs 不走写路径")
        }
        async fn create_dir_all(&self, _path: &Path) -> Result<(), FsError> {
            unimplemented!("CountingFs 不走写路径")
        }
        async fn remove_file(&self, _path: &Path) -> Result<(), FsError> {
            unimplemented!("CountingFs 不走写路径")
        }
    }

    #[tokio::test]
    async fn read_script_data_reuses_cache_no_double_read() {
        let path = PathBuf::from("/x/scripts/flow-wf_c.js");
        let fs = CountingFs {
            path: path.clone(),
            content: "export const meta = { name: 'cached' }".to_owned(),
            reads: std::sync::atomic::AtomicUsize::new(0),
        };
        let cache = std::sync::Mutex::new(WorkflowManifestCache::new());

        let d1 = read_script_data(&path, &fs, &cache).await;
        let d2 = read_script_data(&path, &fs, &cache).await;
        assert_eq!(d1.preview, d2.preview);
        assert_eq!(
            fs.reads.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "稳态下同一 script 仅一次真实读盘，第二次命中缓存"
        );
    }
}
