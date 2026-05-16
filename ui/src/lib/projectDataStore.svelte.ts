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
  return [...groups]
    .sort((a, b) => (b.mostRecentSession ?? 0) - (a.mostRecentSession ?? 0))
    .map((group) => {
      const mainWorktree = group.worktrees.find((worktree) => worktree.isMainWorktree) ?? group.worktrees[0];
      return {
        ...projectFromWorktree(mainWorktree, group.name),
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
  return loading;
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
      if (refreshAfterInflight) {
        refreshAfterInflight = false;
        void loadProjectData({ refresh: true });
      }
    }
  })();

  return inflight;
}
