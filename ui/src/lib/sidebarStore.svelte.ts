// Sidebar 状态管理——宽度、Pin/Hide（Pin/Hide 接后端 config 持久化）
//
// Pin/Hide 走乐观更新模式：
//   1. 本地 state 立即翻转
//   2. 异步 invoke 持久化到后端 config.json
//   3. 失败时回滚并重新 load 兜底
//
// 首次访问某 project 时懒加载 pin/hide 列表到本地 state。

import { invoke } from "@tauri-apps/api/core";

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
// Pin / Hide（per-project，持久化到后端 config）
// ---------------------------------------------------------------------------

let pinnedByProject: Record<string, string[]> = $state({});
let hiddenByProject: Record<string, string[]> = $state({});
let showHidden: boolean = $state(false);

// 已经从后端 load 过的 project id 集合，避免重复请求
const loadedProjects = new Set<string>();
// 正在进行中的 load promise，避免并发竞争
const loadingProjects = new Map<string, Promise<void>>();

interface ProjectSessionPrefs {
  pinned: string[];
  hidden: string[];
}

/**
 * 首次访问某 project 时调用，从后端 config load pin/hide 列表。
 *
 * 幂等：同一个 projectId 多次调用只真正请求一次；并发调用共享同一个 promise。
 */
export function loadProjectPrefs(projectId: string): Promise<void> {
  if (!projectId || loadedProjects.has(projectId)) {
    return Promise.resolve();
  }
  const inflight = loadingProjects.get(projectId);
  if (inflight) return inflight;

  const p = (async () => {
    try {
      const prefs = await invoke<ProjectSessionPrefs>("get_project_session_prefs", { projectId });
      pinnedByProject[projectId] = prefs.pinned ?? [];
      hiddenByProject[projectId] = prefs.hidden ?? [];
      loadedProjects.add(projectId);
    } catch (e) {
      console.error(`Failed to load session prefs for project ${projectId}:`, e);
    } finally {
      loadingProjects.delete(projectId);
    }
  })();

  loadingProjects.set(projectId, p);
  return p;
}

/** 强制重新拉取某 project 的偏好（乐观更新失败后的回滚兜底）。 */
async function reloadProjectPrefs(projectId: string): Promise<void> {
  loadedProjects.delete(projectId);
  await loadProjectPrefs(projectId);
}

export function getPinnedIds(projectId: string): string[] {
  return pinnedByProject[projectId] ?? [];
}

export function isPinned(projectId: string, sessionId: string): boolean {
  return (pinnedByProject[projectId] ?? []).includes(sessionId);
}

export function togglePin(projectId: string, sessionId: string): void {
  if (!projectId) return;
  const ids = pinnedByProject[projectId] ?? [];
  const wasPinned = ids.includes(sessionId);

  // 乐观更新
  if (wasPinned) {
    pinnedByProject[projectId] = ids.filter((id) => id !== sessionId);
  } else {
    pinnedByProject[projectId] = [sessionId, ...ids];
  }

  // 确保后端已经加载过（懒加载兜底），然后异步持久化
  void loadProjectPrefs(projectId).then(async () => {
    const cmd = wasPinned ? "unpin_session" : "pin_session";
    try {
      await invoke(cmd, { projectId, sessionId });
    } catch (e) {
      console.error(`Failed to ${cmd}:`, e);
      // 回滚：重新从后端拉取作为 source of truth
      await reloadProjectPrefs(projectId);
    }
  });
}

export function isHidden(projectId: string, sessionId: string): boolean {
  return (hiddenByProject[projectId] ?? []).includes(sessionId);
}

export function toggleHide(projectId: string, sessionId: string): void {
  if (!projectId) return;
  const ids = hiddenByProject[projectId] ?? [];
  const wasHidden = ids.includes(sessionId);

  // 乐观更新
  if (wasHidden) {
    hiddenByProject[projectId] = ids.filter((id) => id !== sessionId);
  } else {
    hiddenByProject[projectId] = [sessionId, ...ids];
  }

  void loadProjectPrefs(projectId).then(async () => {
    const cmd = wasHidden ? "unhide_session" : "hide_session";
    try {
      await invoke(cmd, { projectId, sessionId });
    } catch (e) {
      console.error(`Failed to ${cmd}:`, e);
      await reloadProjectPrefs(projectId);
    }
  });
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
