/**
 * Background Jobs store — 管理 jobs 列表、分组、badge。
 *
 * 数据来源：
 * - `list_jobs` IPC → 全量刷新（后端已算好 badge + group）
 * - `jobs-update` event → 增量触发 re-fetch
 *
 * Svelte 5 runes（$state / $derived）。
 */

import { getTransport } from "./transport";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  BadgePriority,
  JobGroup,
  JobGroupData,
  JobState,
  JobSummary,
  ListJobsResult,
} from "./types/jobs";

const invoke = <T>(cmd: string, args?: Record<string, unknown>) =>
  getTransport().invoke<T>(cmd, args);

// ---------------------------------------------------------------------------
// 响应式状态
// ---------------------------------------------------------------------------

let jobs: JobSummary[] = $state([]);
let badgeColor: BadgePriority = $state("none");
let badgeCount: number = $state(0);
let jobsDirExists: boolean = $state(false);
let loading: boolean = $state(false);
let error: string | null = $state(null);

// ---------------------------------------------------------------------------
// 纯函数（导出供测试）
// ---------------------------------------------------------------------------

/**
 * 对单个 job 进行分组（前端 fallback——后端已在 job.group 给出）。
 * D4：Ready for review → Needs input → Working → Completed
 */
export function classifyJob(job: JobSummary): JobGroup {
  // 优先使用后端已计算的 group
  if (job.group) return job.group;
  // fallback 逻辑
  if (job.children.some((c) => c.kind === "pr")) {
    return "ready-for-review";
  }
  if (job.state === "blocked") {
    return "needs-input";
  }
  if (job.state === "done" || job.state === "failed" || job.state === "stopped") {
    return "completed";
  }
  return "working";
}

/** 将 ISO8601 字符串转为 Unix ms（兼容 number 和 string） */
function toMs(value: string | number): number {
  if (typeof value === "number") return value;
  const ms = new Date(value).getTime();
  return Number.isNaN(ms) ? 0 : ms;
}

/** 分组 + 排序（组内按 updatedAt 降序） */
export function groupJobs(jobsList: JobSummary[]): JobGroupData[] {
  const groups: Record<JobGroup, JobSummary[]> = {
    "ready-for-review": [],
    "needs-input": [],
    "working": [],
    "completed": [],
  };

  for (const job of jobsList) {
    groups[classifyJob(job)].push(job);
  }

  // 组内按 updatedAt 降序
  for (const g of Object.values(groups)) {
    g.sort((a, b) => toMs(b.updatedAt) - toMs(a.updatedAt));
  }

  const result: JobGroupData[] = [];
  if (groups["ready-for-review"].length > 0) {
    result.push({ group: "ready-for-review", label: "Ready for review", jobs: groups["ready-for-review"] });
  }
  if (groups["needs-input"].length > 0) {
    result.push({ group: "needs-input", label: "Needs input", jobs: groups["needs-input"] });
  }
  if (groups["working"].length > 0) {
    result.push({ group: "working", label: "Working", jobs: groups["working"] });
  }
  if (groups["completed"].length > 0) {
    result.push({ group: "completed", label: "Completed", jobs: groups["completed"] });
  }

  return result;
}

/**
 * Badge 优先级计算（D5）——前端 fallback，正常情况下直接用后端返回的 badge。
 * failed → red > blocked → amber > ready-for-review → green > 无 badge
 */
export function computeBadge(jobsList: JobSummary[]): BadgePriority {
  let hasFailed = false;
  let hasBlocked = false;
  let hasReady = false;

  for (const job of jobsList) {
    if (job.state === "failed") hasFailed = true;
    if (job.state === "blocked") hasBlocked = true;
    if (job.children.some((c) => c.kind === "pr")) hasReady = true;
  }

  if (hasFailed) return "red";
  if (hasBlocked) return "amber";
  if (hasReady) return "green";
  return "none";
}

/** 状态 → CSS var 映射 */
export function stateToColor(state: JobState): string {
  switch (state) {
    case "working": return "var(--color-accent-blue)";
    case "blocked": return "var(--color-warning)";
    case "idle": return "var(--color-text-muted)";
    case "done": return "var(--color-success-bright)";
    case "failed": return "var(--color-danger)";
    case "stopped": return "var(--color-text-muted)";
  }
}

/** 从 job 获取 projectId（后端已算好 job.projectId，fallback linkScanPath/cwd） */
export function extractProjectId(job: JobSummary): string | null {
  // 后端已提供 projectId
  if (job.projectId) return job.projectId;
  // fallback：linkScanPath 形如 ".../projects/<encoded_path>/sessions/..."
  if (job.linkScanPath) {
    const match = job.linkScanPath.match(/projects\/([^/]+)/);
    if (match) return match[1];
  }
  // fallback：cwd
  if (job.cwd) {
    return job.cwd;
  }
  return null;
}

/**
 * 人类可读时间间隔（"2m" / "1h" / "3d"）。
 * 接受 ISO8601 字符串或 Unix ms。
 */
export function formatAge(timestamp: string | number): string {
  const ms = toMs(timestamp);
  if (ms === 0) return "—";
  const now = Date.now();
  const diffMs = now - ms;
  if (diffMs < 0) return "just now";

  const minutes = Math.floor(diffMs / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;

  const days = Math.floor(hours / 24);
  return `${days}d`;
}

// ---------------------------------------------------------------------------
// 副作用
// ---------------------------------------------------------------------------

let unlistenJobs: UnlistenFn | null = null;

/** 加载 jobs 列表 */
async function loadJobs(): Promise<void> {
  loading = true;
  error = null;
  try {
    const result = await invoke<ListJobsResult>("list_jobs");
    jobs = result.jobs;
    badgeColor = result.badge;
    badgeCount = result.badgeCount;
    jobsDirExists = result.jobsDirExists;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    if (msg.includes("not implemented") || msg.includes("unknownCommand")) {
      console.warn("[jobs] list_jobs not available:", msg);
      jobsDirExists = false;
      jobs = [];
      badgeColor = "none";
      badgeCount = 0;
      error = null;
    } else {
      error = msg;
    }
  } finally {
    loading = false;
  }
}

/** 订阅 jobs-update 事件 */
async function subscribeJobsUpdate(): Promise<void> {
  if (unlistenJobs) return;
  try {
    unlistenJobs = await listen("jobs-update", () => {
      void loadJobs();
    });
  } catch (e) {
    console.warn("[jobs] failed to subscribe to jobs-update events:", e);
  }
}

// ---------------------------------------------------------------------------
// 公开 API
// ---------------------------------------------------------------------------

export function getJobs(): JobSummary[] {
  return jobs;
}

export function getJobsDirExists(): boolean {
  return jobsDirExists;
}

export function getBadgeColor(): BadgePriority {
  return badgeColor;
}

export function getBadgeCount(): number {
  return badgeCount;
}

export function getJobsLoading(): boolean {
  return loading;
}

export function getJobsError(): string | null {
  return error;
}

export async function initializeJobs(): Promise<void> {
  await loadJobs();
  await subscribeJobsUpdate();
}

export async function refreshJobs(): Promise<void> {
  await loadJobs();
}

export async function stopJob(jobId: string): Promise<void> {
  await invoke("stop_job", { jobId });
  await refreshJobs();
}

export function cleanupJobs(): void {
  if (unlistenJobs) {
    unlistenJobs();
    unlistenJobs = null;
  }
}
