import { describe, test, expect } from "vitest";
import {
  classifyJob,
  groupJobs,
  computeBadge,
  stateToColor,
  extractProjectId,
  formatAge,
} from "./jobsStore.svelte";
import type { JobSummary, JobState } from "./types/jobs";

function makeJob(overrides: Partial<JobSummary> = {}): JobSummary {
  return {
    id: "job-test",
    name: "test-job",
    state: "working",
    detail: "",
    intent: "",
    group: "working",
    children: [],
    sessionId: "",
    projectId: "",
    tempo: "",
    inFlight: "",
    createdAt: new Date(Date.now() - 60_000).toISOString(),
    updatedAt: new Date().toISOString(),
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// classifyJob
// ---------------------------------------------------------------------------

describe("classifyJob", () => {
  test("job with PR child → ready-for-review", () => {
    const job = makeJob({ children: [{ kind: "pr", href: "https://github.com/x/pull/1" }], group: "ready-for-review" });
    expect(classifyJob(job)).toBe("ready-for-review");
  });

  test("blocked state → needs-input", () => {
    const job = makeJob({ state: "blocked", group: "needs-input" });
    expect(classifyJob(job)).toBe("needs-input");
  });

  test("working state → working", () => {
    const job = makeJob({ state: "working", group: "working" });
    expect(classifyJob(job)).toBe("working");
  });

  test("idle state → working", () => {
    const job = makeJob({ state: "idle", group: "working" });
    expect(classifyJob(job)).toBe("working");
  });

  test("done state → completed", () => {
    const job = makeJob({ state: "done", group: "completed" });
    expect(classifyJob(job)).toBe("completed");
  });

  test("failed state → completed", () => {
    const job = makeJob({ state: "failed", group: "completed" });
    expect(classifyJob(job)).toBe("completed");
  });

  test("stopped state → completed", () => {
    const job = makeJob({ state: "stopped", group: "completed" });
    expect(classifyJob(job)).toBe("completed");
  });

  test("PR child 优先于 done 状态（D4: PR → ready-for-review）", () => {
    const job = makeJob({
      state: "done",
      group: "ready-for-review",
      children: [{ kind: "pr", href: "https://github.com/x/pull/2" }],
    });
    expect(classifyJob(job)).toBe("ready-for-review");
  });
});

// ---------------------------------------------------------------------------
// groupJobs
// ---------------------------------------------------------------------------

describe("groupJobs", () => {
  test("空列表返回空组", () => {
    expect(groupJobs([])).toEqual([]);
  });

  test("按正确顺序分组", () => {
    const jobs = [
      makeJob({ id: "1", state: "working", group: "working", updatedAt: "2026-01-01T00:01:00Z" }),
      makeJob({ id: "2", state: "blocked", group: "needs-input", updatedAt: "2026-01-01T00:02:00Z" }),
      makeJob({ id: "3", state: "done", group: "completed", updatedAt: "2026-01-01T00:03:00Z" }),
      makeJob({ id: "4", group: "ready-for-review", children: [{ kind: "pr", href: "https://x" }], updatedAt: "2026-01-01T00:04:00Z" }),
    ];
    const groups = groupJobs(jobs);
    expect(groups).toHaveLength(4);
    expect(groups[0].group).toBe("ready-for-review");
    expect(groups[1].group).toBe("needs-input");
    expect(groups[2].group).toBe("working");
    expect(groups[3].group).toBe("completed");
  });

  test("组内按 updatedAt 降序", () => {
    const jobs = [
      makeJob({ id: "a", state: "working", group: "working", updatedAt: "2026-01-01T00:01:00Z" }),
      makeJob({ id: "b", state: "working", group: "working", updatedAt: "2026-01-01T00:03:00Z" }),
      makeJob({ id: "c", state: "working", group: "working", updatedAt: "2026-01-01T00:02:00Z" }),
    ];
    const groups = groupJobs(jobs);
    expect(groups[0].jobs.map((j) => j.id)).toEqual(["b", "c", "a"]);
  });

  test("空组不出现在结果中", () => {
    const jobs = [makeJob({ id: "1", state: "working", group: "working" })];
    const groups = groupJobs(jobs);
    expect(groups).toHaveLength(1);
    expect(groups[0].group).toBe("working");
  });
});

// ---------------------------------------------------------------------------
// computeBadge
// ---------------------------------------------------------------------------

describe("computeBadge", () => {
  test("空列表 → none", () => {
    expect(computeBadge([])).toBe("none");
  });

  test("failed → red（最高优先级）", () => {
    const jobs = [
      makeJob({ state: "failed" }),
      makeJob({ state: "blocked" }),
      makeJob({ children: [{ kind: "pr", href: "https://x" }] }),
    ];
    expect(computeBadge(jobs)).toBe("red");
  });

  test("blocked 无 failed → amber", () => {
    const jobs = [
      makeJob({ state: "blocked" }),
      makeJob({ children: [{ kind: "pr", href: "https://x" }] }),
    ];
    expect(computeBadge(jobs)).toBe("amber");
  });

  test("有 PR 无 failed 无 blocked → green", () => {
    const jobs = [makeJob({ children: [{ kind: "pr", href: "https://x" }] })];
    expect(computeBadge(jobs)).toBe("green");
  });

  test("working 只有 → none", () => {
    const jobs = [makeJob({ state: "working" })];
    expect(computeBadge(jobs)).toBe("none");
  });

  test("done 只有 → none", () => {
    const jobs = [makeJob({ state: "done" })];
    expect(computeBadge(jobs)).toBe("none");
  });
});

// ---------------------------------------------------------------------------
// stateToColor
// ---------------------------------------------------------------------------

describe("stateToColor", () => {
  const cases: [JobState, string][] = [
    ["working", "var(--color-accent-blue)"],
    ["blocked", "var(--color-warning)"],
    ["idle", "var(--color-text-muted)"],
    ["done", "var(--color-success-bright)"],
    ["failed", "var(--color-danger)"],
    ["stopped", "var(--color-text-muted)"],
  ];

  test.each(cases)("%s → %s", (state, expected) => {
    expect(stateToColor(state)).toBe(expected);
  });
});

// ---------------------------------------------------------------------------
// extractProjectId
// ---------------------------------------------------------------------------

describe("extractProjectId", () => {
  test("直接使用 job.projectId", () => {
    const job = makeJob({ projectId: "proj-abc" });
    expect(extractProjectId(job)).toBe("proj-abc");
  });

  test("projectId 为空时从 linkScanPath 提取", () => {
    const job = makeJob({
      projectId: "",
      linkScanPath: "/home/user/.claude/projects/abc123def/sessions/sess-1",
    });
    expect(extractProjectId(job)).toBe("abc123def");
  });

  test("linkScanPath 也无时 fallback 到 cwd", () => {
    const job = makeJob({
      projectId: "",
      cwd: "/Users/dev/my-project",
    });
    expect(extractProjectId(job)).toBe("/Users/dev/my-project");
  });

  test("全部为空 → null", () => {
    const job = makeJob({ projectId: "" });
    expect(extractProjectId(job)).toBeNull();
  });

  test("linkScanPath 无 projects/ 路径时 fallback cwd", () => {
    const job = makeJob({
      projectId: "",
      linkScanPath: "/some/other/path",
      cwd: "/Users/dev/fallback",
    });
    expect(extractProjectId(job)).toBe("/Users/dev/fallback");
  });
});

// ---------------------------------------------------------------------------
// computeBadge — 边界用例
// ---------------------------------------------------------------------------

describe("computeBadge edge cases", () => {
  test("failed + PR + blocked → 红色（最高优先级 red 胜出）", () => {
    const jobs = [
      makeJob({ state: "failed" }),
      makeJob({ state: "blocked" }),
      makeJob({ children: [{ kind: "pr", href: "https://x" }] }),
    ];
    expect(computeBadge(jobs)).toBe("red");
  });

  test("多个 failed → 仍 red（不会升级到其他优先级）", () => {
    const jobs = [
      makeJob({ state: "failed", id: "1" }),
      makeJob({ state: "failed", id: "2" }),
    ];
    expect(computeBadge(jobs)).toBe("red");
  });

  test("idle + stopped + done 全部终态 → none", () => {
    const jobs = [
      makeJob({ state: "idle" }),
      makeJob({ state: "stopped" }),
      makeJob({ state: "done" }),
    ];
    expect(computeBadge(jobs)).toBe("none");
  });

  test("PR child 但 job 是 failed 状态 → 同时触发 red 和 green，red 胜出", () => {
    const jobs = [
      makeJob({ state: "failed", children: [{ kind: "pr", href: "https://x" }] }),
    ];
    expect(computeBadge(jobs)).toBe("red");
  });
});

// ---------------------------------------------------------------------------
// classifyJob — PR 优先于各种 state
// ---------------------------------------------------------------------------

describe("classifyJob PR-priority edge cases", () => {
  test("blocked + PR → ready-for-review（PR 优先于 blocked）", () => {
    const job = makeJob({
      state: "blocked",
      group: "ready-for-review",
      children: [{ kind: "pr", href: "https://github.com/x/pull/1" }],
    });
    expect(classifyJob(job)).toBe("ready-for-review");
  });

  test("failed + PR → ready-for-review（PR 优先于 failed）", () => {
    const job = makeJob({
      state: "failed",
      group: "ready-for-review",
      children: [{ kind: "pr", href: "https://github.com/x/pull/1" }],
    });
    expect(classifyJob(job)).toBe("ready-for-review");
  });

  test("non-pr child 不触发 ready-for-review", () => {
    const job = makeJob({
      state: "working",
      group: "working",
      children: [{ kind: "issue", href: "https://github.com/x/issues/1" }],
    });
    expect(classifyJob(job)).toBe("working");
  });
});

// ---------------------------------------------------------------------------
// extractProjectId — 更多路径模式
// ---------------------------------------------------------------------------

describe("extractProjectId edge cases", () => {
  test("多段 projects/ 路径取第一个", () => {
    const job = makeJob({
      projectId: "",
      linkScanPath: "/home/user/.claude/projects/abc/sessions/projects/xyz/file",
    });
    expect(extractProjectId(job)).toBe("abc");
  });

  test("Windows 风格 linkScanPath", () => {
    const job = makeJob({
      projectId: "",
      linkScanPath: "/.claude/projects/-C:-Users-alice-code/sess.jsonl",
    });
    expect(extractProjectId(job)).toBe("-C:-Users-alice-code");
  });

  test("linkScanPath 以 projects/ 结尾无 segment → fallback cwd", () => {
    const job = makeJob({
      projectId: "",
      linkScanPath: "/some/path/projects/",
      cwd: "/Users/dev/fallback",
    });
    expect(extractProjectId(job)).toBe("/Users/dev/fallback");
  });
});

// ---------------------------------------------------------------------------
// formatAge
// ---------------------------------------------------------------------------

describe("formatAge", () => {
  test("< 1 分钟 → just now", () => {
    expect(formatAge(new Date(Date.now() - 30_000).toISOString())).toBe("just now");
  });

  test("5 分钟 → 5m", () => {
    expect(formatAge(new Date(Date.now() - 5 * 60_000).toISOString())).toBe("5m");
  });

  test("2 小时 → 2h", () => {
    expect(formatAge(new Date(Date.now() - 2 * 3_600_000).toISOString())).toBe("2h");
  });

  test("3 天 → 3d", () => {
    expect(formatAge(new Date(Date.now() - 3 * 86_400_000).toISOString())).toBe("3d");
  });

  test("未来时间 → just now", () => {
    expect(formatAge(new Date(Date.now() + 60_000).toISOString())).toBe("just now");
  });

  test("兼容 number 入参", () => {
    expect(formatAge(Date.now() - 5 * 60_000)).toBe("5m");
  });
});
