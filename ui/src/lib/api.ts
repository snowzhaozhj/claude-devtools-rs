import { invoke } from "@tauri-apps/api/core";

export interface ProjectInfo {
  id: string;
  path: string;
  displayName: string;
  sessionCount: number;
}

export interface SessionSummary {
  sessionId: string;
  projectId: string;
  timestamp: number;
  messageCount: number;
}

export interface PaginatedResponse<T> {
  items: T[];
  nextCursor: string | null;
  total: number;
}

export async function listProjects(): Promise<ProjectInfo[]> {
  return await invoke("list_projects");
}

export async function listSessions(
  projectId: string,
  pageSize: number = 50,
  cursor?: string
): Promise<PaginatedResponse<SessionSummary>> {
  return await invoke("list_sessions", {
    projectId,
    pageSize,
    cursor: cursor ?? null,
  });
}

export async function getSessionDetail(
  projectId: string,
  sessionId: string
): Promise<any> {
  return await invoke("get_session_detail", { projectId, sessionId });
}
