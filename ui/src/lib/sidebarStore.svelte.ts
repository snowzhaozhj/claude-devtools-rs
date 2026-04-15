// Sidebar 状态管理——宽度、Pin/Hide（内存级，不跨重启持久化）

const MIN_WIDTH = 200;
const MAX_WIDTH = 500;

// ---------------------------------------------------------------------------
// 宽度
// ---------------------------------------------------------------------------

let sidebarWidth: number = $state(280);

export function getSidebarWidth(): number {
  return sidebarWidth;
}

export function setSidebarWidth(w: number): void {
  sidebarWidth = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, w));
}

// ---------------------------------------------------------------------------
// Pin / Hide（per-project）
// ---------------------------------------------------------------------------

let pinnedByProject: Record<string, string[]> = $state({});
let hiddenByProject: Record<string, string[]> = $state({});
let showHidden: boolean = $state(false);

export function getPinnedIds(projectId: string): string[] {
  return pinnedByProject[projectId] ?? [];
}

export function isPinned(projectId: string, sessionId: string): boolean {
  return (pinnedByProject[projectId] ?? []).includes(sessionId);
}

export function togglePin(projectId: string, sessionId: string): void {
  const ids = pinnedByProject[projectId] ?? [];
  if (ids.includes(sessionId)) {
    pinnedByProject[projectId] = ids.filter((id) => id !== sessionId);
  } else {
    pinnedByProject[projectId] = [sessionId, ...ids];
  }
}

export function isHidden(projectId: string, sessionId: string): boolean {
  return (hiddenByProject[projectId] ?? []).includes(sessionId);
}

export function toggleHide(projectId: string, sessionId: string): void {
  const ids = hiddenByProject[projectId] ?? [];
  if (ids.includes(sessionId)) {
    hiddenByProject[projectId] = ids.filter((id) => id !== sessionId);
  } else {
    hiddenByProject[projectId] = [sessionId, ...ids];
  }
}

export function getShowHidden(): boolean {
  return showHidden;
}

export function toggleShowHidden(): void {
  showHidden = !showHidden;
}

export function getHiddenCount(projectId: string): number {
  return (hiddenByProject[projectId] ?? []).length;
}
