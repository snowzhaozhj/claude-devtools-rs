//! Background job 的共享类型。
//!
//! 由 `cdt-api` 生产（读取 `~/.claude/jobs/*/state.json`）、由前端消费。
//! 放在 `cdt-core` 避免下游 crate 被迫依赖 `cdt-api`。

/// 后台 job 的状态枚举。
///
/// 未知值反序列化为 `Idle`（容错）。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Working,
    Blocked,
    #[default]
    Idle,
    Done,
    Failed,
    Stopped,
}

/// `state.json` 中的 `children` 条目。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JobChild {
    /// 链接地址（PR URL 等）。
    #[serde(default)]
    pub href: String,
    /// 类型标记（`"pr"` / `"issue"` 等）。
    #[serde(default)]
    pub kind: String,
}

/// `inFlight` 字段的真实结构。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JobInFlight {
    pub tasks: u32,
    pub queued: u32,
    pub kinds: Vec<String>,
}

/// 从 `~/.claude/jobs/<job_id>/state.json` 读取的原始数据。
///
/// 字段对齐 Claude Code daemon 写入的真实格式（camelCase + nullable）。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackgroundJob {
    #[serde(deserialize_with = "deserialize_job_state")]
    pub state: JobState,
    pub name: String,
    pub detail: String,
    pub intent: String,
    pub children: Option<Vec<JobChild>>,
    pub session_id: String,
    pub link_scan_path: String,
    pub cwd: String,
    pub tempo: String,
    /// blocked 时的用户动作提示（如 "reply `go` to merge"）。
    /// daemon 对非 blocked job 写 `null`，用 `deserialize_with` 容忍。
    #[serde(default, deserialize_with = "deserialize_nullable_string")]
    pub needs: String,
    pub in_flight: Option<JobInFlight>,
    pub created_at: String,
    pub updated_at: String,
}

/// 分组类别（用于前端展示）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobGroup {
    ReadyForReview,
    NeedsInput,
    Working,
    Completed,
}

/// Badge 颜色（红 > 黄 > 绿 > 无）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BadgeColor {
    Red,
    Amber,
    Green,
    None,
}

/// 前端使用的 job 摘要。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSummary {
    /// `state.json` 所在目录名（= job ID）。
    pub id: String,
    /// 任务名称。
    pub name: String,
    /// 当前步骤描述。
    pub detail: String,
    /// 用户意图。
    pub intent: String,
    /// 状态。
    pub state: JobState,
    /// 分组。
    pub group: JobGroup,
    /// 产出链接。
    pub children: Vec<JobChild>,
    /// 关联的 session ID。
    pub session_id: String,
    /// 关联的 project ID（从 `link_scan_path` / `cwd` 提取）。
    pub project_id: String,
    /// 活跃度。
    pub tempo: String,
    /// blocked 时的用户动作提示。
    pub needs: String,
    pub in_flight: Option<JobInFlight>,
    /// 创建时间。
    pub created_at: String,
    /// 最近更新时间。
    pub updated_at: String,
}

/// `list_jobs` IPC 返回值。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobsResponse {
    /// 全部 job 按分组排序。
    pub jobs: Vec<JobSummary>,
    /// badge 颜色。
    pub badge: BadgeColor,
    /// badge 数字。
    pub badge_count: usize,
    /// `~/.claude/jobs/` 目录是否存在——前端据此决定是否显示入口。
    pub jobs_dir_exists: bool,
}

/// Jobs 目录变更事件。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobChangeEvent {
    /// 变更的 job ID（目录名）。
    pub job_id: String,
}

/// 自定义反序列化：未知 state 值视为 `Idle`。
///
/// serde `deserialize_with` 要求签名为 `Result<T, D::Error>`，clippy
/// `unnecessary_wraps` 误报（不能去掉 Result）。
#[allow(clippy::unnecessary_wraps)]
fn deserialize_job_state<'de, D>(deserializer: D) -> Result<JobState, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer).unwrap_or_default();
    Ok(match s.as_str() {
        "working" => JobState::Working,
        "blocked" => JobState::Blocked,
        "done" => JobState::Done,
        "failed" => JobState::Failed,
        "stopped" => JobState::Stopped,
        _ => JobState::Idle,
    })
}

/// 容忍 JSON `null` 的 String 反序列化（null → ""）。
#[allow(clippy::unnecessary_wraps)]
fn deserialize_nullable_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

// =========================================================================
// 辅助函数
// =========================================================================

/// 从 `link_scan_path` 提取 project ID。
///
/// `link_scan_path` 格式：`/.claude/projects/<encoded_id>/<session>.jsonl`
/// 提取 `projects/` 后的第一个路径段。
pub fn extract_project_id_from_link_scan_path(link_scan_path: &str) -> Option<String> {
    // 找 "projects/" 后取第一段
    let marker = "projects/";
    let after = link_scan_path
        .find(marker)
        .map(|i| &link_scan_path[i + marker.len()..])?;
    let segment = after.split('/').next()?;
    if segment.is_empty() {
        return None;
    }
    Some(segment.to_owned())
}

/// 判定 job 的分组。
///
/// 对齐 `claude agents` CLI 的 `stateBucket` 优先级：
/// 1. Working/Idle 无 PR → Working
/// 2. Failed/Stopped/Done 无 PR → Completed
/// 3. Blocked → `NeedsInput`
/// 4. Done/Idle + 有 PR → `ReadyForReview`
pub fn classify_job_group(job: &BackgroundJob) -> JobGroup {
    let has_pr = job
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|c| c.kind == "pr");

    match job.state {
        JobState::Blocked => JobGroup::NeedsInput,
        JobState::Done | JobState::Idle if has_pr => JobGroup::ReadyForReview,
        JobState::Done | JobState::Failed | JobState::Stopped => JobGroup::Completed,
        JobState::Working | JobState::Idle => JobGroup::Working,
    }
}

/// 计算 badge 颜色和数字。
pub fn compute_badge(jobs: &[JobSummary]) -> (BadgeColor, usize) {
    let failed_count = jobs.iter().filter(|j| j.state == JobState::Failed).count();
    if failed_count > 0 {
        return (BadgeColor::Red, failed_count);
    }

    let blocked_count = jobs
        .iter()
        .filter(|j| j.group == JobGroup::NeedsInput)
        .count();
    if blocked_count > 0 {
        return (BadgeColor::Amber, blocked_count);
    }

    (BadgeColor::None, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_known_states() {
        let json = r#"{"state":"working","name":"test"}"#;
        let job: BackgroundJob = serde_json::from_str(json).unwrap();
        assert_eq!(job.state, JobState::Working);
    }

    #[test]
    fn deserialize_unknown_state_falls_back_to_idle() {
        let json = r#"{"state":"unknown_future_state","name":"test"}"#;
        let job: BackgroundJob = serde_json::from_str(json).unwrap();
        assert_eq!(job.state, JobState::Idle);
    }

    #[test]
    fn deserialize_missing_fields_uses_defaults() {
        let json = r"{}";
        let job: BackgroundJob = serde_json::from_str(json).unwrap();
        assert_eq!(job.state, JobState::Idle);
        assert!(job.name.is_empty());
        assert!(job.children.is_none());
    }

    #[test]
    fn extract_project_id_from_valid_link_scan_path() {
        let path = "/.claude/projects/-Users-alice-code-app/sess-123.jsonl";
        assert_eq!(
            extract_project_id_from_link_scan_path(path),
            Some("-Users-alice-code-app".to_owned())
        );
    }

    #[test]
    fn extract_project_id_from_empty_returns_none() {
        assert_eq!(extract_project_id_from_link_scan_path(""), None);
    }

    #[test]
    fn extract_project_id_from_no_projects_marker_returns_none() {
        assert_eq!(
            extract_project_id_from_link_scan_path("/some/random/path"),
            None
        );
    }

    #[test]
    fn classify_working_with_pr_is_still_working() {
        let job = BackgroundJob {
            state: JobState::Working,
            children: Some(vec![JobChild {
                href: "https://github.com/foo/bar/pull/1".into(),
                kind: "pr".into(),
            }]),
            ..Default::default()
        };
        assert_eq!(classify_job_group(&job), JobGroup::Working);
    }

    #[test]
    fn classify_idle_with_pr_is_ready_for_review() {
        let job = BackgroundJob {
            state: JobState::Idle,
            children: Some(vec![JobChild {
                href: "https://github.com/foo/bar/pull/1".into(),
                kind: "pr".into(),
            }]),
            ..Default::default()
        };
        assert_eq!(classify_job_group(&job), JobGroup::ReadyForReview);
    }

    #[test]
    fn classify_blocked_without_pr_is_needs_input() {
        let job = BackgroundJob {
            state: JobState::Blocked,
            ..Default::default()
        };
        assert_eq!(classify_job_group(&job), JobGroup::NeedsInput);
    }

    #[test]
    fn classify_working_without_pr_is_working() {
        let job = BackgroundJob {
            state: JobState::Working,
            ..Default::default()
        };
        assert_eq!(classify_job_group(&job), JobGroup::Working);
    }

    #[test]
    fn classify_done_with_pr_is_ready_for_review() {
        let job = BackgroundJob {
            state: JobState::Done,
            children: Some(vec![JobChild {
                href: "https://github.com/foo/bar/pull/1".into(),
                kind: "pr".into(),
            }]),
            ..Default::default()
        };
        assert_eq!(classify_job_group(&job), JobGroup::ReadyForReview);
    }

    #[test]
    fn classify_done_without_pr_is_completed() {
        let job = BackgroundJob {
            state: JobState::Done,
            ..Default::default()
        };
        assert_eq!(classify_job_group(&job), JobGroup::Completed);
    }

    #[test]
    fn badge_red_when_failed_exists() {
        let jobs = vec![
            make_summary(JobState::Working, JobGroup::Working),
            make_summary(JobState::Failed, JobGroup::Completed),
        ];
        let (color, count) = compute_badge(&jobs);
        assert_eq!(color, BadgeColor::Red);
        assert_eq!(count, 1);
    }

    #[test]
    fn badge_amber_when_blocked_no_failed() {
        let jobs = vec![
            make_summary(JobState::Blocked, JobGroup::NeedsInput),
            make_summary(JobState::Working, JobGroup::Working),
        ];
        let (color, count) = compute_badge(&jobs);
        assert_eq!(color, BadgeColor::Amber);
        assert_eq!(count, 1);
    }

    #[test]
    fn badge_none_when_only_working_or_review() {
        let jobs = vec![make_summary(JobState::Working, JobGroup::ReadyForReview)];
        let (color, count) = compute_badge(&jobs);
        assert_eq!(color, BadgeColor::None);
        assert_eq!(count, 0);
    }

    #[test]
    fn badge_none_when_only_working() {
        let jobs = vec![make_summary(JobState::Working, JobGroup::Working)];
        let (color, count) = compute_badge(&jobs);
        assert_eq!(color, BadgeColor::None);
        assert_eq!(count, 0);
    }

    fn make_summary(state: JobState, group: JobGroup) -> JobSummary {
        JobSummary {
            id: "job-1".into(),
            name: "test".into(),
            detail: String::new(),
            intent: String::new(),
            state,
            group,
            children: Vec::new(),
            session_id: String::new(),
            project_id: String::new(),
            tempo: String::new(),
            needs: String::new(),
            in_flight: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}
