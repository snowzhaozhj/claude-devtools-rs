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
// 每次 root switch 递增 generation。旧 root 的 in-flight 请求若晚于新
// generation 返回，不能再写回 data，也不能把旧结果返回给当前调用方。
let generation = 0;
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
  let repositoryGroups: RepositoryGroup[];
  try {
    repositoryGroups = await listRepositoryGroups();
  } catch (groupError) {
    // dev 模式下抛错让两端不对齐 / 后端 bug 立刻暴露——history：dev 模式下
    // 浏览器访问 Tauri 内置 HTTP server 拿到旧 ui/dist bundle 与新桌面端
    // 不一致时，silent fallback 把"后端返回 grouped data 但前端 store 误用
    // listProjects 平铺"伪装成"看起来还能用"，让 dev 看不出 root cause。
    // prod 模式保留 fallback 给用户兜底（避免后端短暂故障 = UI 整体崩）。
    if (import.meta.env.DEV) {
      console.error(
        "[projectDataStore] listRepositoryGroups failed in DEV; rethrowing instead of silent fallback. " +
          "Check backend logs / network tab. Fallback path is for production resilience only.",
        groupError,
      );
      throw groupError;
    }
    console.warn("[projectDataStore] listRepositoryGroups failed, fallback to listProjects:", groupError);
    return await fallbackProjectData();
  }
  if (repositoryGroups.length === 0) return await fallbackProjectData();
  return {
    repositoryGroups,
    projects: summarizeRepositoryGroups(repositoryGroups),
    worktreeProjects: flattenRepositoryGroups(repositoryGroups),
  };
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
  const requestGeneration = generation;

  inflight = (async () => {
    try {
      const next = await fetchProjectData();
      if (requestGeneration !== generation) {
        throw new Error("project data request superseded by root switch");
      }
      data = next;
      return next;
    } catch (e) {
      if (requestGeneration === generation) error = e;
      throw e;
    } finally {
      if (requestGeneration === generation) {
        inflight = null;
        loading = false;
        initialized = true;
        if (refreshAfterInflight) {
          refreshAfterInflight = false;
          void loadProjectData({ refresh: true });
        }
      }
    }
  })();

  return inflight;
}

export function clearProjectDataForRootSwitch(): void {
  generation += 1;
  data = null;
  error = null;
  inflight = null;
  refreshAfterInflight = false;
  loading = true;
  initialized = false;
}

export async function reloadProjectDataForRootSwitch(): Promise<ProjectData> {
  clearProjectDataForRootSwitch();
  return await loadProjectData({ refresh: true });
}
