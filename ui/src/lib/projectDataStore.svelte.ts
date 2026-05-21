import {
  listProjects,
  listRepositoryGroups,
  type ProjectInfo,
  type RepositoryGroup,
} from "./api";

export interface ProjectData {
  projects: ProjectInfo[];
  worktreeProjects: ProjectInfo[];
  repositoryGroups: RepositoryGroup[];
}

let data: ProjectData | null = $state(null);
let loading: boolean = $state(false);
let error: unknown = $state(null);
// 首次 `loadProjectData` 完成前为 false。`isProjectDataLoading()` 在 initialized
// 之前一律返回 true，避免首帧调用者（如 `UnifiedTitleBar` 的 ProjectSwitcher）
// 在 `loading=false + data=null` 的 1 帧窗口内误显"无项目"（codex PR #140 二审）。
let initialized: boolean = $state(false);
let inflight: Promise<ProjectData> | null = null;
let refreshAfterInflight = false;

function projectFromWorktree(worktree: RepositoryGroup["worktrees"][number], displayName = worktree.name): ProjectInfo {
  return {
    id: worktree.id,
    path: worktree.path,
    displayName,
    sessionCount: worktree.sessions.length,
  };
}

function flattenRepositoryGroups(groups: RepositoryGroup[]): ProjectInfo[] {
  return groups
    .flatMap((group) => group.worktrees)
    .sort((a, b) => (b.mostRecentSession ?? 0) - (a.mostRecentSession ?? 0))
    .map((worktree) => projectFromWorktree(worktree));
}

function summarizeRepositoryGroups(groups: RepositoryGroup[]): ProjectInfo[] {
  // change `simplify-repository-as-project::D5/D7`：ProjectSwitcher 下拉项
  // 的 `id` SHALL 是 `group.id`（不再是 mainWorktree.id），让 App 顶层导航
  // 持 `selectedGroupId` 与 sidebar `list_group_sessions(groupId, ...)` 对齐。
  // 单 worktree group 时 `group.id === worktrees[0].id`（grouper 在 standalone
  // 场景下设定），所以单 project 用户无感知 ID 变化。
  return [...groups]
    .sort((a, b) => (b.mostRecentSession ?? 0) - (a.mostRecentSession ?? 0))
    .map((group) => {
      const anchor = group.worktrees.find((w) => w.isRepoRoot)
        ?? group.worktrees.find((w) => w.isMainWorktree)
        ?? group.worktrees[0];
      return {
        id: group.id,
        path: anchor.path,
        displayName: group.name,
        sessionCount: group.totalSessions,
      };
    });
}

async function fallbackProjectData(): Promise<ProjectData> {
  const projects = await listProjects();
  return {
    repositoryGroups: [],
    projects,
    worktreeProjects: projects,
  };
}

async function fetchProjectData(): Promise<ProjectData> {
  try {
    const repositoryGroups = await listRepositoryGroups();
    if (repositoryGroups.length === 0) return await fallbackProjectData();
    return {
      repositoryGroups,
      projects: summarizeRepositoryGroups(repositoryGroups),
      worktreeProjects: flattenRepositoryGroups(repositoryGroups),
    };
  } catch (groupError) {
    console.warn("listRepositoryGroups failed, fallback to listProjects:", groupError);
    return await fallbackProjectData();
  }
}

export function getProjectData(): ProjectData | null {
  return data;
}

export function isProjectDataLoading(): boolean {
  return !initialized || loading;
}

export function getProjectDataError(): unknown {
  return error;
}

export function loadProjectData(options: { refresh?: boolean } = {}): Promise<ProjectData> {
  if (inflight) {
    if (options.refresh) refreshAfterInflight = true;
    return inflight;
  }
  if (!options.refresh && data) return Promise.resolve(data);

  loading = true;
  error = null;

  inflight = (async () => {
    try {
      const next = await fetchProjectData();
      data = next;
      return next;
    } catch (e) {
      error = e;
      throw e;
    } finally {
      inflight = null;
      loading = false;
      initialized = true;
      if (refreshAfterInflight) {
        refreshAfterInflight = false;
        void loadProjectData({ refresh: true });
      }
    }
  })();

  return inflight;
}
