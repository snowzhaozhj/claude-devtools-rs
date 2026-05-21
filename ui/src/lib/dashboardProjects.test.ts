import { describe, expect, test } from "vitest";
import type { ProjectData } from "./projectDataStore.svelte";
import type { ProjectInfo, RepositoryGroup, Worktree } from "./api";
import {
  deriveDashboardProjects,
  sortDashboardProjects,
  filterDashboardProjects,
  formatRelativeTime,
  type DashboardProject,
} from "./dashboardProjects";

function wt(over: Partial<Worktree> & { id: string; path: string; name: string }): Worktree {
  return {
    gitBranch: null,
    isMainWorktree: true,
    sessions: [],
    createdAt: null,
    mostRecentSession: null,
    ...over,
  };
}

function group(over: Partial<RepositoryGroup> & { id: string; name: string; worktrees: Worktree[] }): RepositoryGroup {
  return {
    identity: null,
    mostRecentSession: null,
    totalSessions: over.worktrees.reduce((acc, w) => acc + w.sessions.length, 0),
    ...over,
  };
}

function pi(over: Partial<ProjectInfo> & { id: string }): ProjectInfo {
  return {
    path: "/tmp/" + over.id,
    displayName: over.id,
    sessionCount: 0,
    ...over,
  };
}

describe("deriveDashboardProjects", () => {
  test("null data 返回空数组", () => {
    expect(deriveDashboardProjects(null)).toEqual([]);
  });

  test("fallback 路径（无 repositoryGroups）按 projects 平铺，lastModified=null worktreeCount=1", () => {
    const data: ProjectData = {
      projects: [pi({ id: "a", sessionCount: 3 }), pi({ id: "b", sessionCount: 7 })],
      worktreeProjects: [],
      repositoryGroups: [],
    };
    const out = deriveDashboardProjects(data);
    expect(out).toHaveLength(2);
    expect(out[0]).toMatchObject({ id: "a", sessionCount: 3, lastModified: null, worktreeCount: 1 });
    expect(out[1]).toMatchObject({ id: "b", sessionCount: 7, lastModified: null, worktreeCount: 1 });
  });

  test("grouped 路径：单 worktree group 用 main worktree 作 id+path，group name 作 displayName", () => {
    const data: ProjectData = {
      projects: [],
      worktreeProjects: [],
      repositoryGroups: [
        group({
          id: "g1",
          name: "My Repo",
          worktrees: [wt({ id: "w-main", path: "/home/me/repo", name: "main", isMainWorktree: true, sessions: ["s1", "s2", "s3"] })],
          mostRecentSession: 1_700_000_000_000,
          totalSessions: 3,
        }),
      ],
    };
    const out = deriveDashboardProjects(data);
    expect(out).toHaveLength(1);
    // change `simplify-repository-as-project::D5/D7`：dashboard id 走 group.id
    // 让 sidebar 点击后 selectedGroupId 命中 list_group_sessions(groupId, ...)；
    // 单 worktree group 时 group.id === worktrees[0].id 退化为字符串相同。
    expect(out[0]).toEqual({
      id: "g1",
      path: "/home/me/repo",
      displayName: "My Repo",
      sessionCount: 3,
      lastModified: 1_700_000_000_000,
      worktreeCount: 1,
    });
  });

  test("多 worktree group：id 用 group.id，sessionCount 聚合 group.totalSessions", () => {
    const data: ProjectData = {
      projects: [],
      worktreeProjects: [],
      repositoryGroups: [
        group({
          id: "g2",
          name: "Big Repo",
          worktrees: [
            wt({ id: "wt-feat", path: "/home/me/big-feat", name: "feat", isMainWorktree: false, sessions: ["sf1", "sf2"] }),
            wt({ id: "wt-main", path: "/home/me/big", name: "main", isMainWorktree: true, sessions: ["s1"] }),
            wt({ id: "wt-exp", path: "/home/me/big-exp", name: "exp", isMainWorktree: false, sessions: ["se1", "se2"] }),
          ],
          mostRecentSession: 1_700_000_500_000,
          totalSessions: 5,
        }),
      ],
    };
    const out = deriveDashboardProjects(data);
    // change `simplify-repository-as-project::D5/D7`：id 是 group.id，
    // sessionCount 是 group.totalSessions=5（与 sidebar 切到该 group 后
    // list_group_sessions 返回的合并条数一致），path 仍用 mainWorktree.path
    // 作为 dashboard 行展示锚点。
    expect(out[0]).toMatchObject({
      id: "g2",
      path: "/home/me/big",
      displayName: "Big Repo",
      sessionCount: 5,
      worktreeCount: 3,
    });
  });

  test("无 isMainWorktree 时仍走 group.id（path 回退到第一个 worktree）", () => {
    const data: ProjectData = {
      projects: [],
      worktreeProjects: [],
      repositoryGroups: [
        group({
          id: "g3",
          name: "No-main",
          worktrees: [
            wt({ id: "wt-a", path: "/x/a", name: "a", isMainWorktree: false }),
            wt({ id: "wt-b", path: "/x/b", name: "b", isMainWorktree: false }),
          ],
        }),
      ],
    };
    const out = deriveDashboardProjects(data);
    expect(out[0].id).toBe("g3");
    expect(out[0].path).toBe("/x/a"); // path 仍取第一个 worktree 作为展示锚点
    expect(out[0].worktreeCount).toBe(2);
  });
});

describe("sortDashboardProjects", () => {
  const projects: DashboardProject[] = [
    { id: "a", path: "/a", displayName: "Alpha", sessionCount: 3, lastModified: 100, worktreeCount: 1 },
    { id: "b", path: "/b", displayName: "Bravo", sessionCount: 7, lastModified: 300, worktreeCount: 2 },
    { id: "c", path: "/c", displayName: "charlie", sessionCount: 7, lastModified: null, worktreeCount: 1 },
    { id: "d", path: "/d", displayName: "Delta", sessionCount: 1, lastModified: 200, worktreeCount: 1 },
  ];

  test("recent 排序：mostRecent 在前，null 沉底", () => {
    const out = sortDashboardProjects(projects, "recent");
    expect(out.map((p) => p.id)).toEqual(["b", "d", "a", "c"]);
  });

  test("sessions 排序：sessionCount 倒序，相等保持输入顺序（稳定）", () => {
    const out = sortDashboardProjects(projects, "sessions");
    // b 与 c 同 7：保持输入顺序 b 在前
    expect(out.map((p) => p.id)).toEqual(["b", "c", "a", "d"]);
  });

  test("name 排序：locale 字典序，大小写不敏感", () => {
    const out = sortDashboardProjects(projects, "name");
    expect(out.map((p) => p.id)).toEqual(["a", "b", "c", "d"]);
  });

  test("不修改输入数组", () => {
    const snapshot = projects.map((p) => p.id);
    sortDashboardProjects(projects, "name");
    expect(projects.map((p) => p.id)).toEqual(snapshot);
  });
});

describe("filterDashboardProjects", () => {
  const projects: DashboardProject[] = [
    { id: "1", path: "/code/cdt-rs", displayName: "claude-devtools-rs", sessionCount: 1, lastModified: 0, worktreeCount: 1 },
    { id: "2", path: "/work/foo", displayName: "spike-router", sessionCount: 2, lastModified: 0, worktreeCount: 1 },
  ];

  test("空 query 返回原列表", () => {
    expect(filterDashboardProjects(projects, "")).toBe(projects);
    expect(filterDashboardProjects(projects, "   ")).toBe(projects);
  });

  test("按 displayName 不区分大小写匹配", () => {
    expect(filterDashboardProjects(projects, "CLAUDE")).toHaveLength(1);
    expect(filterDashboardProjects(projects, "spike")).toHaveLength(1);
  });

  test("按 path 匹配", () => {
    expect(filterDashboardProjects(projects, "cdt-rs")).toHaveLength(1);
    expect(filterDashboardProjects(projects, "/work/")).toHaveLength(1);
  });

  test("无匹配返回空", () => {
    expect(filterDashboardProjects(projects, "xxxxx")).toHaveLength(0);
  });
});

describe("formatRelativeTime", () => {
  // 固定 now：2026-05-17 14:00:00 Asia/Shanghai = 2026-05-17 06:00:00 UTC
  // 用 toLocaleTimeString 渲染的"HH:MM"对 TZ 敏感——所以下方只断言模式存在性。
  const now = new Date("2026-05-17T06:00:00Z").getTime();

  test("null 返回空串", () => {
    expect(formatRelativeTime(null, now)).toBe("");
  });

  test("不到 1 分钟返回 '刚刚'", () => {
    expect(formatRelativeTime(now - 30_000, now)).toBe("刚刚");
    expect(formatRelativeTime(now, now)).toBe("刚刚");
  });

  test("未来时间也归 '刚刚'（避免负数显示异常）", () => {
    expect(formatRelativeTime(now + 5_000, now)).toBe("刚刚");
  });

  test("小于 1 小时返回 'N 分钟前'", () => {
    expect(formatRelativeTime(now - 5 * 60_000, now)).toBe("5 分钟前");
    expect(formatRelativeTime(now - 59 * 60_000, now)).toBe("59 分钟前");
  });

  test("当天稍早时间返回 '今天 HH:MM'", () => {
    const out = formatRelativeTime(now - 3 * 3600_000, now);
    expect(out).toMatch(/^今天\s\d{2}:\d{2}$/);
  });

  test("昨天返回 '昨天 HH:MM'", () => {
    const yest = new Date(now);
    yest.setDate(yest.getDate() - 1);
    yest.setHours(10, 30, 0, 0);
    const out = formatRelativeTime(yest.getTime(), now);
    expect(out).toMatch(/^昨天\s\d{2}:\d{2}$/);
  });

  test("同年但更早返回月日（不含年）", () => {
    const earlier = new Date("2026-03-14T10:00:00Z").getTime();
    const out = formatRelativeTime(earlier, now);
    expect(out).toMatch(/月/);
    expect(out).not.toMatch(/2026/);
    expect(out).not.toMatch(/2025/);
  });

  test("跨年返回年月", () => {
    const lastYear = new Date("2025-08-01T10:00:00Z").getTime();
    const out = formatRelativeTime(lastYear, now);
    expect(out).toMatch(/2025/);
  });
});
