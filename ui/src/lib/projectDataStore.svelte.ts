import {
  listProjects,
  listRepositoryGroups,
  type ProjectInfo,
  type RepositoryGroup,
} from "./api";

export interface ProjectData {
  projects: ProjectInfo[];
  repositoryGroups: RepositoryGroup[];
}

let data: ProjectData | null = $state(null);
let loading: boolean = $state(false);
let error: unknown = $state(null);
let inflight: Promise<ProjectData> | null = null;

function flattenRepositoryGroups(groups: RepositoryGroup[]): ProjectInfo[] {
  return groups.flatMap((group) =>
    group.worktrees.map((worktree) => ({
      id: worktree.id,
      path: worktree.path,
      displayName: worktree.name,
      sessionCount: worktree.sessions.length,
    })),
  );
}

async function fetchProjectData(): Promise<ProjectData> {
  try {
    const repositoryGroups = await listRepositoryGroups();
    return {
      repositoryGroups,
      projects: flattenRepositoryGroups(repositoryGroups),
    };
  } catch (groupError) {
    console.warn("listRepositoryGroups failed, fallback to listProjects:", groupError);
    return {
      repositoryGroups: [],
      projects: await listProjects(),
    };
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
  if (inflight) return inflight;
  if (!options.refresh && data) return Promise.resolve(data);

  loading = true;
  error = null;

  let request: Promise<ProjectData>;
  request = (async () => {
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
    }
  })();

  inflight = request;
  return request;
}
