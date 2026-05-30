/**
 * Background Jobs 类型定义。
 * 对齐后端 cdt-core::BackgroundJob / JobState / JobChild / JobSummary。
 * IPC `list_jobs` 返回 { jobs: JobSummary[], jobsDirExists: boolean }。
 */

/** Job 状态枚举（对齐 claude agents CLI 原生语义） */
export type JobState = "working" | "idle" | "blocked" | "done" | "failed" | "stopped";

/** Job 子任务（PR 等） */
export interface JobChild {
  kind: string;
  href: string;
}

/** list_jobs 返回的单条 job 摘要（对齐后端 cdt-core::JobSummary） */
export interface JobSummary {
  id: string;
  name: string;
  state: JobState;
  detail: string;
  intent: string;
  group: JobGroup;
  children: JobChild[];
  sessionId: string;
  projectId: string;
  linkScanPath?: string;
  cwd?: string;
  tempo: string;
  inFlight: { tasks: number; queued: number; kinds: string[] } | null;
  createdAt: string;
  updatedAt: string;
}

/** list_jobs IPC 返回体（对齐后端 cdt-core::JobsResponse） */
export interface ListJobsResult {
  jobs: JobSummary[];
  badge: "red" | "amber" | "green" | "none";
  badgeCount: number;
  jobsDirExists: boolean;
}

/** 分组类型 */
export type JobGroup = "ready-for-review" | "needs-input" | "working" | "completed";

/** 分组后的数据结构 */
export interface JobGroupData {
  group: JobGroup;
  label: string;
  jobs: JobSummary[];
}

/** Badge 优先级类型 */
export type BadgePriority = "red" | "amber" | "green" | "none";
