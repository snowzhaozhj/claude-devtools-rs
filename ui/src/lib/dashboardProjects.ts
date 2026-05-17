import type { ProjectData } from "./projectDataStore.svelte";

/**
 * Dashboard 视角的项目记录：在 RepositoryGroup 与 ProjectInfo 两条数据通路之上
 * 抹平为单一形态，承载工作台需要的丰富 metadata（mtime / worktreeCount /
 * totalSessions）。所有字段直接派生自 `projectDataStore` 已 cache 的数据，
 * 无额外 IPC，无后端聚合。
 *
 * `lastModified === null` 标记 fallback 路径（`list_repository_groups` 失败退
 * 回 `list_projects`）——此时无 mtime 数据，UI 应跳过相对时间显示。
 * `worktreeCount === 1` 在 fallback 下也代表"未知"——dashboard 仅在 `>1` 时
 * 显示 chip，所以默认 1 安全。
 */
export interface DashboardProject {
  id: string;
  path: string;
  displayName: string;
  sessionCount: number;
  lastModified: number | null;
  worktreeCount: number;
}

export type DashboardSortKey = "recent" | "sessions" | "name";

export function deriveDashboardProjects(data: ProjectData | null): DashboardProject[] {
  if (!data) return [];
  if (data.repositoryGroups.length === 0) {
    return data.projects.map((p) => ({
      id: p.id,
      path: p.path,
      displayName: p.displayName,
      sessionCount: p.sessionCount,
      lastModified: null,
      worktreeCount: 1,
    }));
  }
  return data.repositoryGroups.map((group) => {
    const mainWorktree = group.worktrees.find((w) => w.isMainWorktree) ?? group.worktrees[0];
    return {
      id: mainWorktree.id,
      path: mainWorktree.path,
      displayName: group.name,
      sessionCount: group.totalSessions,
      lastModified: group.mostRecentSession,
      worktreeCount: group.worktrees.length,
    };
  });
}

/**
 * 稳定排序：相等键时保持输入顺序（store 已按 mostRecentSession DESC 排好）。
 * `recent` 排序中 `lastModified == null` 沉到末尾。
 */
export function sortDashboardProjects(
  projects: DashboardProject[],
  key: DashboardSortKey,
): DashboardProject[] {
  const indexed = projects.map((p, i) => ({ p, i }));
  indexed.sort((a, b) => {
    let cmp = 0;
    switch (key) {
      case "recent": {
        const av = a.p.lastModified ?? -Infinity;
        const bv = b.p.lastModified ?? -Infinity;
        cmp = bv - av;
        break;
      }
      case "sessions":
        cmp = b.p.sessionCount - a.p.sessionCount;
        break;
      case "name":
        cmp = a.p.displayName.localeCompare(b.p.displayName, "zh-CN");
        break;
    }
    return cmp !== 0 ? cmp : a.i - b.i;
  });
  return indexed.map(({ p }) => p);
}

export function filterDashboardProjects(
  projects: DashboardProject[],
  query: string,
): DashboardProject[] {
  const q = query.trim().toLowerCase();
  if (!q) return projects;
  return projects.filter(
    (p) =>
      p.displayName.toLowerCase().includes(q) || p.path.toLowerCase().includes(q),
  );
}

/**
 * 相对时间渲染：dashboard 工作台空间宽，用中文长格式（"3 分钟前 / 今天 14:02 /
 * 昨天 16:45 / 5月14日 / 2025年3月"），可读性优于 sidebar 紧凑 "3m/1h/1d"。
 * `now` 注入便于单测；生产代码省略走 `Date.now()`。
 */
export function formatRelativeTime(
  timestamp: number | null,
  now: number = Date.now(),
): string {
  if (timestamp == null) return "";
  const diff = now - timestamp;
  if (diff < 0) return "刚刚";

  const sec = Math.floor(diff / 1000);
  if (sec < 60) return "刚刚";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} 分钟前`;

  const date = new Date(timestamp);
  const nowDate = new Date(now);

  const sameDay =
    date.getFullYear() === nowDate.getFullYear() &&
    date.getMonth() === nowDate.getMonth() &&
    date.getDate() === nowDate.getDate();
  if (sameDay) {
    return `今天 ${date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false })}`;
  }

  const yesterday = new Date(now);
  yesterday.setDate(nowDate.getDate() - 1);
  const isYesterday =
    date.getFullYear() === yesterday.getFullYear() &&
    date.getMonth() === yesterday.getMonth() &&
    date.getDate() === yesterday.getDate();
  if (isYesterday) {
    return `昨天 ${date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false })}`;
  }

  if (date.getFullYear() === nowDate.getFullYear()) {
    return date.toLocaleDateString("zh-CN", { month: "long", day: "numeric" });
  }
  return date.toLocaleDateString("zh-CN", { year: "numeric", month: "long" });
}
