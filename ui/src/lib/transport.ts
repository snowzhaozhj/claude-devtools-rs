import { invoke as tauriInvoke, type InvokeArgs } from "@tauri-apps/api/core";
import { listen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";
import { getServerBaseUrl, isTauriRuntime } from "./runtime";

export type EventHandler = (eventName: string, payload: unknown) => void;
export type Unsubscribe = () => void;

export interface Transport {
  invoke<T>(cmd: string, args?: InvokeArgs): Promise<T>;
  subscribeEvents(handler: EventHandler): Promise<Unsubscribe>;
}

export class BrowserUnsupportedError extends Error {
  constructor(command: string) {
    super(`${command} is not available in browser runtime`);
    this.name = "BrowserUnsupportedError";
  }
}

const unsupportedBrowserCommands = new Set([
  "check_for_update",
  "is_running_under_rosetta",
  "http_server_start",
  "http_server_stop",
  "http_server_status",
  "read_agent_configs",
]);

class TauriTransport implements Transport {
  async invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
    return await tauriInvoke<T>(cmd, args);
  }

  async subscribeEvents(handler: EventHandler): Promise<Unsubscribe> {
    const unlisteners = await Promise.all([
      listen("file-change", (event) => handler("file-change", event.payload)),
      listen("notification-update", (event) => handler("notification-update", event.payload)),
      listen("notification-added", (event) => handler("notification-added", event.payload)),
      listen("updater://available", (event) => handler("updater://available", event.payload)),
    ]);
    return () => {
      for (const unlisten of unlisteners) unlisten();
    };
  }
}

class BrowserTransport implements Transport {
  async invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
    if (unsupportedBrowserCommands.has(cmd)) {
      throw new BrowserUnsupportedError(cmd);
    }
    return await invokeHttp<T>(cmd, args ?? {});
  }

  async subscribeEvents(handler: EventHandler): Promise<Unsubscribe> {
    const source = new EventSource(`${getServerBaseUrl()}/api/events`);
    source.onmessage = (event) => {
      const payload = JSON.parse(event.data) as { type?: string } & Record<string, unknown>;
      const eventName = mapPushEventName(payload.type);
      if (!eventName) return;
      const { type: _type, ...rest } = payload;
      handler(eventName, normalizePushPayload(payload.type, rest));
    };
    return () => source.close();
  }
}

const tauriTransport = new TauriTransport();
const browserTransport = new BrowserTransport();

export function getTransport(): Transport {
  return isTauriRuntime() ? tauriTransport : browserTransport;
}

export async function subscribeEvent<T>(
  eventName: string,
  callback: EventCallback<T>,
): Promise<UnlistenFn> {
  return await getTransport().subscribeEvents((name, payload) => {
    if (name === eventName) callback({ event: name, id: 0, payload: payload as T });
  });
}

async function invokeHttp<T>(cmd: string, args: InvokeArgs): Promise<T> {
  const request = httpRequestForCommand(cmd, args);
  const response = await fetch(`${getServerBaseUrl()}${request.path}`, {
    method: request.method,
    headers: request.body === undefined ? undefined : { "Content-Type": "application/json" },
    body: request.body === undefined ? undefined : JSON.stringify(request.body),
  });
  if (!response.ok) {
    const message = await errorMessage(response);
    throw new Error(message);
  }
  if (response.status === 204) return undefined as T;
  return normalizeHttpResponse(cmd, await response.json()) as T;
}

type HttpMethod = "GET" | "POST" | "PATCH" | "DELETE";

interface HttpRequest {
  method: HttpMethod;
  path: string;
  body?: unknown;
}

function httpRequestForCommand(cmd: string, args: InvokeArgs): HttpRequest {
  const a = args as Record<string, unknown>;
  switch (cmd) {
    case "list_projects":
      return { method: "GET", path: "/api/projects" };
    case "list_repository_groups":
      return { method: "GET", path: "/api/repository-groups" };
    case "list_wsl_distros":
      return { method: "GET", path: "/api/wsl-distros" };
    case "get_worktree_sessions":
      return { method: "GET", path: `/api/worktrees/${enc(a.groupId)}/sessions${paginationQuery(a)}` };
    case "list_sessions":
      return { method: "GET", path: `/api/projects/${enc(a.projectId)}/sessions${paginationQuery(a)}` };
    case "get_session_summaries_by_ids":
      return { method: "POST", path: `/api/projects/${enc(a.projectId)}/session-summaries/batch`, body: a.sessionIds };
    case "search_sessions":
      return { method: "POST", path: "/api/search", body: { projectId: a.projectId, query: a.query } };
    case "get_session_detail":
      return { method: "GET", path: `/api/sessions/${enc(a.sessionId)}` };
    case "get_project_memory":
      return { method: "GET", path: `/api/projects/${enc(a.projectId)}/memory` };
    case "read_memory_file":
      return { method: "POST", path: `/api/projects/${enc(a.projectId)}/memory-files`, body: { file: a.file } };
    case "get_subagent_trace":
      return { method: "GET", path: `/api/sessions/${enc(a.rootSessionId)}/subagents/${enc(a.subagentSessionId)}/trace` };
    case "get_image_asset":
      return { method: "GET", path: `/api/sessions/${enc(a.rootSessionId)}/subagents/${enc(a.sessionId)}/blocks/${enc(a.blockId)}/image` };
    case "get_tool_output":
      return { method: "GET", path: `/api/sessions/${enc(a.rootSessionId)}/subagents/${enc(a.sessionId)}/tools/${enc(a.toolUseId)}/output` };
    case "get_config":
      return { method: "GET", path: "/api/config" };
    case "update_config":
      return { method: "PATCH", path: "/api/config", body: { section: a.section, data: a.configData } };
    case "get_notifications":
      return { method: "GET", path: `/api/notifications?limit=${enc(a.limit ?? 50)}&offset=${enc(a.offset ?? 0)}` };
    case "mark_notification_read":
      return { method: "POST", path: `/api/notifications/${enc(a.notificationId)}/read` };
    case "delete_notification":
      return { method: "DELETE", path: `/api/notifications/${enc(a.notificationId)}` };
    case "mark_all_notifications_read":
      return { method: "POST", path: "/api/notifications/mark-all-read" };
    case "clear_notifications":
      return { method: "POST", path: "/api/notifications/clear", body: { triggerId: a.triggerId ?? null } };
    case "add_trigger":
      return { method: "POST", path: "/api/notifications/triggers", body: a.trigger };
    case "remove_trigger":
      return { method: "DELETE", path: `/api/notifications/triggers/${enc(a.triggerId)}` };
    case "get_project_session_prefs":
      return { method: "GET", path: `/api/projects/${enc(a.projectId)}/session-prefs` };
    case "pin_session":
      return { method: "POST", path: `/api/projects/${enc(a.projectId)}/sessions/${enc(a.sessionId)}/pin` };
    case "unpin_session":
      return { method: "DELETE", path: `/api/projects/${enc(a.projectId)}/sessions/${enc(a.sessionId)}/pin` };
    case "hide_session":
      return { method: "POST", path: `/api/projects/${enc(a.projectId)}/sessions/${enc(a.sessionId)}/hide` };
    case "unhide_session":
      return { method: "DELETE", path: `/api/projects/${enc(a.projectId)}/sessions/${enc(a.sessionId)}/hide` };
    default:
      throw new BrowserUnsupportedError(cmd);
  }
}

function paginationQuery(args: Record<string, unknown>): string {
  const params = new URLSearchParams();
  if (args.pageSize !== undefined) params.set("pageSize", String(args.pageSize));
  if (args.cursor !== undefined && args.cursor !== null) params.set("cursor", String(args.cursor));
  const query = params.toString();
  return query ? `?${query}` : "";
}

async function errorMessage(response: Response): Promise<string> {
  try {
    const body = await response.json() as { message?: string };
    return body.message ?? `HTTP ${response.status}`;
  } catch {
    return `HTTP ${response.status}`;
  }
}

function enc(value: unknown): string {
  return encodeURIComponent(String(value));
}

function normalizeHttpResponse(cmd: string, body: unknown): unknown {
  const obj = body as Record<string, unknown>;
  switch (cmd) {
    case "mark_notification_read":
      return obj.success;
    case "delete_notification":
      return obj.removed;
    case "mark_all_notifications_read":
      return undefined;
    case "clear_notifications":
      return obj.removed;
    default:
      return body;
  }
}

function mapPushEventName(type: string | undefined): string | null {
  switch (type) {
    case "file_change":
      return "file-change";
    case "todo_change":
      return "todo-change";
    case "new_notification":
      return "notification-added";
    case "session_metadata_update":
      return "session-metadata-update";
    case "ssh_status_change":
      return "ssh-status-change";
    default:
      return null;
  }
}

function normalizePushPayload(type: string | undefined, payload: Record<string, unknown>): unknown {
  switch (type) {
    case "file_change":
      return {
        projectId: payload.project_id,
        sessionId: payload.session_id,
        deleted: payload.deleted,
        projectListChanged: payload.project_list_changed,
      };
    case "todo_change":
      return { projectId: payload.project_id, sessionId: payload.session_id };
    case "new_notification":
      return payload.notification;
    case "session_metadata_update":
      return {
        projectId: payload.project_id,
        sessionId: payload.session_id,
        title: payload.title,
        messageCount: payload.message_count,
        isOngoing: payload.is_ongoing,
        gitBranch: payload.git_branch,
      };
    default:
      return payload;
  }
}
