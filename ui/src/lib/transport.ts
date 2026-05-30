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
  // pre-existing gap：add/delete memory 漏 mirror 到 HTTP route，浏览器
  // ?http=1 模式调 add/delete 没意义先 block，等单独 PR 补 axum route 后移除。
  // 详见 crates/cdt-api/tests/contract_data.rs::BROWSER_UNSUPPORTED_COMMANDS。
  "add_memory",
  "delete_memory",
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
      listen("session-metadata-update", (event) => handler("session-metadata-update", event.payload)),
      listen("ssh_status", (event) => handler("ssh_status", event.payload)),
      listen("context_changed", (event) => handler("context_changed", event.payload)),
      listen("updater://available", (event) => handler("updater://available", event.payload)),
      // `sse-lagged` 在桌面 Tauri runtime 也需订阅——src-tauri host file-change
      // bridge 在 broadcast `RecvError::Lagged` 路径 emit `sse-lagged` event
      // 让前端走 silent refresh 兜底（change
      // `enrich-file-change-with-session-list-changed::D6`）。形态：
      // `{ source: "file-change", missed: N }`，与 HTTP `PushEvent::SseLagged`
      // 对齐。原实现 Tauri runtime 不订阅 → lag 期间错过 structural 信号
      // 滞后到 LOCAL_CACHE_TTL=5min 才恢复。
      listen("sse-lagged", (event) => handler("sse-lagged", event.payload)),
    ]);
    return () => {
      for (const unlisten of unlisteners) unlisten();
    };
  }
}

/** EventSource CLOSED 后延迟重建的退避边界（ms）：min 是首次重建延迟，
 * max 是指数回退上限。避免 server 长期不可用时手动重建陷入"重建 → onerror
 * → 重建"无限风暴（codex 二审 Q3）。 */
const SSE_RECONNECT_MIN_MS = 1000;
const SSE_RECONNECT_MAX_MS = 30_000;

/** 命令名集合：发请求前 SHALL 先 await `ensureSseReady()` 让浏览器订阅好
 * `/api/events` SSE 才发 fetch——否则后端在 GET response 返回后立即 emit 的
 * `session_metadata_update` 会在 EventSource OPEN 前发生，patch 永久丢失
 * 直到下一次 file-change silent refresh 兜底（spec http-data-api §"浏览器
 * client SHALL 在首次 list_sessions 前订阅 SSE"）。 */
const LIST_SESSIONS_LIKE_COMMANDS = new Set([
  "list_sessions",
  "list_repository_groups",
  "list_group_sessions",
  "get_worktree_sessions",
]);

/** `ensureSseReady` 内部轮询粒度与总超时窗口。设计 D2：固定 onopen 监听绑死
 * 在某个特定 EventSource 实例上，重连退避期间新 source 还没创建（current source
 * 是 CLOSED 状态），监听器永远等不到 OPEN——所以每 50 ms 重读 `this.source`
 * 引用，捕捉重连过程中诞生的新 source。1000 ms 总超时是经验值，超时后不抛错
 * 放行让冷启加载继续，丢失的 metadata 由后续 file-change silent refresh 兜底。 */
const SSE_READY_POLL_MS = 50;
const SSE_READY_TIMEOUT_MS = 1000;

class BrowserTransport implements Transport {
  // 单例 EventSource + handler 集合：UI 里 5+ 个 `subscribeEvent` 调用点
  // （App.svelte 3 个 + Sidebar 1 个 + fileChangeStore 1 个等），若每次都
  // `new EventSource()` 各开一条 SSE → HTTP/1.1 同源 6 连接限制下，5 条
  // SSE 长连接占满槽位，后续 API 请求（`/api/projects/{id}/sessions` 等）
  // 永远等不到空闲连接，sidebar 卡 loading。复用同一条 SSE 流，按事件类型
  // fan-out 到所有 handler。
  private source: EventSource | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  /** 当前指数回退步数，0=首次（min），onopen 时重置（参见 ensureSource）。 */
  private reconnectAttempt = 0;
  private readonly handlers = new Set<EventHandler>();
  /** `ensureSseReady` 1000 ms 超时放行后置 true：彼时 fetch 已发出，但 SSE
   * 尚未 OPEN，后端骨架 + 异步 metadata patch 会全部丢失。SSE 真正进入
   * OPEN 时（`source.onopen`）需要给 UI 层发一次 `sse-recovered` 兜底事件，
   * 让 Sidebar 重新触发 silent refresh 拉一轮新 metadata（codex 二审 issue 1）。
   * 单标志而非队列：所有 list_sessions-like 请求共享一次重拉即可。 */
  private sseRecoveryPending = false;

  async invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
    if (unsupportedBrowserCommands.has(cmd)) {
      throw new BrowserUnsupportedError(cmd);
    }
    if (LIST_SESSIONS_LIKE_COMMANDS.has(cmd)) {
      await this.ensureSseReady();
    }
    return await invokeHttp<T>(cmd, args ?? {});
  }

  /** 阻塞调用方直到 SSE EventSource 进入 OPEN，最多等 1000 ms 后放行。
   *
   * 见 D2：每次循环重读 `this.source` 引用，重连退避期间新 source 诞生后
   * 仍能被捕捉到。CLOSED 的 source 主动 nil 后调 `ensureSource()` 重建。
   * Tauri runtime 不走这条路径（IPC event listener 是同步 register，没有
   * EventSource OPEN 等待问题）。 */
  private async ensureSseReady(): Promise<void> {
    const deadline =
      (typeof performance !== "undefined" ? performance.now() : Date.now()) + SSE_READY_TIMEOUT_MS;
    this.ensureSource();
    while (true) {
      const src = this.source;
      if (src && src.readyState === EventSource.OPEN) return;
      const now = typeof performance !== "undefined" ? performance.now() : Date.now();
      if (now >= deadline) {
        console.warn(
          "[transport] SSE not OPEN within 1000ms; proceeding without subscription—metadata patches may be lost until SSE recovers",
        );
        // 后续 onopen 时 emit `sse-recovered` 通知 UI 层兜底重拉一轮，避免
        // 后端在 timeout 窗口内已发的 metadata patch 永久丢失（codex 二审
        // issue 1）。loop 入口已 short-circuit OPEN，timeout 必然意味着新
        // source 仍在 CONNECTING；onopen 在最终成功（或下次重连成功）时触发。
        this.sseRecoveryPending = true;
        return;
      }
      await new Promise<void>((r) => setTimeout(r, SSE_READY_POLL_MS));
      // CLOSED 的旧 source 主动清掉重建——`ensureSource()` 已有自检，重复
      // 调用安全（OPEN/CONNECTING 时直接 return）。
      if (this.source && this.source.readyState === EventSource.CLOSED) {
        this.source = null;
        this.ensureSource();
      }
    }
  }

  async subscribeEvents(handler: EventHandler): Promise<Unsubscribe> {
    this.ensureSource();
    this.handlers.add(handler);
    return () => {
      this.handlers.delete(handler);
      if (this.handlers.size === 0) this.closeSource();
    };
  }

  private ensureSource(): void {
    // 现有 source 已 CLOSED（典型场景：server toggle off → axum shutdown →
    // TCP RST → EventSource 自动重连失败 N 次后进入 CLOSED 终态，浏览器
    // 不再自己重连），SHALL 主动清掉重建（codex 二审 MUST FIX 轴 4）。
    if (this.source && this.source.readyState !== EventSource.CLOSED) return;
    this.source = null;
    const source = new EventSource(`${getServerBaseUrl()}/api/events`);
    // 连接成功打开 → 重置指数回退步数，让后续真发生 CLOSED 时从首次延迟
    // 起算（避免长期连接稳定运行多年后偶发一次 CLOSED 还按"故障风暴"上限
    // 延迟）。
    source.onopen = () => {
      this.reconnectAttempt = 0;
      // 上一轮 list_sessions-like 请求 ensureSseReady 超时（SSE 还没 OPEN
      // 就放行 fetch）→ 后端在该窗口期内 emit 的 metadata patch 全部丢失。
      // 真正 OPEN 后给所有 handler 发一次 `sse-recovered` pseudo-event 让
      // Sidebar 触发 silent refresh 重拉一轮（codex 二审 issue 1）。
      if (this.sseRecoveryPending) {
        this.sseRecoveryPending = false;
        for (const h of [...this.handlers]) h("sse-recovered", {});
      }
    };
    source.onmessage = (event) => {
      const payload = JSON.parse(event.data) as { type?: string } & Record<string, unknown>;
      const eventName = mapPushEventName(payload.type);
      if (!eventName) return;
      const { type: _type, ...rest } = payload;
      const normalized = normalizePushPayload(payload.type, rest);
      // 复制 handler 集合再迭代——handler 内可能调 unsubscribe 改动 Set
      // 触发 forEach 行为依实现，复制一份避免迭代中删除引发遗漏。
      for (const h of [...this.handlers]) h(eventName, normalized);
    };
    source.onerror = (event) => {
      console.warn("[transport] SSE connection error", event);
      // CONNECTING（=0）是 EventSource 自带回退重连阶段，不干预；CLOSED
      // （=2）是终态浏览器不再重连，SHALL 主动延迟重建。OPEN（=1）短暂错误
      // 后通常自愈，无需处理。
      if (source.readyState === EventSource.CLOSED) this.scheduleReconnect(source);
    };
    this.source = source;
  }

  private scheduleReconnect(failed: EventSource): void {
    // 只在 failed === 当前 source 时重建——避免与并发 closeSource / handler
    // 全部 unsubscribe 后又有新 subscribe 的路径冲突。
    if (this.source !== failed) return;
    this.source = null;
    if (this.reconnectTimer || this.handlers.size === 0) return;
    // 指数回退：1s → 2s → 4s → ... → 30s 上限。server 持续不可用时（用户
    // toggle off 后不再 toggle on / server 进程崩溃未拉起），手动重建仍会
    // 立即 onerror → 再次 scheduleReconnect，没有上限就是每秒 burst 重连。
    const backoff = Math.min(
      SSE_RECONNECT_MIN_MS * 2 ** this.reconnectAttempt,
      SSE_RECONNECT_MAX_MS,
    );
    this.reconnectAttempt += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      if (this.handlers.size > 0) this.ensureSource();
    }, backoff);
  }

  private closeSource(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    // 清空 attempt——下次 subscribe-重连场景应从首次延迟起算，避免上一轮
    // 残留的回退步数惩罚新会话。
    this.reconnectAttempt = 0;
    if (!this.source) return;
    this.source.close();
    this.source = null;
  }
}

const tauriTransport = new TauriTransport();
const browserTransport = new BrowserTransport();

export function getTransport(): Transport {
  return isTauriRuntime() ? tauriTransport : browserTransport;
}

/** 测试专用：清掉模块单例 `browserTransport` 持有的 SSE source / handlers /
 * reconnect timer，避免测试间状态污染（前一个测试的 OPEN source 会让下一个
 * 测试的 `ensureSseReady` 跳过等待逻辑）。production 代码 SHALL NOT 调用。 */
export function __resetBrowserTransportForTests(): void {
  // BrowserTransport 私有字段，测试时强转访问；命名加 underscore 前缀避免误用。
  const inst = browserTransport as unknown as {
    source: EventSource | null;
    reconnectTimer: ReturnType<typeof setTimeout> | null;
    reconnectAttempt: number;
    handlers: Set<unknown>;
  };
  inst.handlers.clear();
  if (inst.reconnectTimer) {
    clearTimeout(inst.reconnectTimer);
    inst.reconnectTimer = null;
  }
  inst.reconnectAttempt = 0;
  if (inst.source) {
    inst.source.close();
    inst.source = null;
  }
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
    case "list_group_sessions":
      return { method: "GET", path: `/api/repository-groups/${enc(a.groupId)}/sessions${paginationQuery(a)}` };
    case "list_sessions":
      return { method: "GET", path: `/api/projects/${enc(a.projectId)}/sessions${paginationQuery(a)}` };
    case "get_session_summaries_by_ids":
      return { method: "POST", path: `/api/projects/${enc(a.projectId)}/session-summaries/batch`, body: a.sessionIds };
    case "search_sessions":
      return { method: "POST", path: "/api/search", body: { projectId: a.projectId, query: a.query } };
    case "search_group_sessions":
      return { method: "POST", path: `/api/repository-groups/${enc(a.groupId)}/search`, body: { query: a.query } };
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
    case "ssh_connect":
      return { method: "POST", path: "/api/ssh/connect", body: a.request };
    case "ssh_disconnect":
      return { method: "POST", path: "/api/ssh/disconnect", body: { context_id: a.contextId } };
    case "ssh_test_connection":
      return { method: "POST", path: "/api/ssh/test-connection", body: a.request };
    case "ssh_get_state":
      return { method: "GET", path: "/api/ssh/state" };
    case "ssh_get_config_hosts":
      return { method: "GET", path: "/api/ssh/config-hosts" };
    case "ssh_resolve_host":
      return { method: "GET", path: `/api/ssh/resolve-host?alias=${enc(a.alias)}` };
    case "ssh_save_last_connection":
      return { method: "POST", path: "/api/ssh/last-connection", body: a.request };
    case "ssh_get_last_connection":
      return { method: "GET", path: "/api/ssh/last-connection" };
    case "list_contexts":
      return { method: "GET", path: "/api/contexts" };
    case "switch_context":
      return { method: "POST", path: "/api/contexts/switch", body: { context_id: a.contextId } };
    case "get_active_context":
      return { method: "GET", path: "/api/contexts/active" };
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
    case "get_telemetry_snapshot":
      return { method: "GET", path: "/api/telemetry/snapshot" };
    case "record_correctness_events":
      return { method: "POST", path: "/api/telemetry/correctness-events", body: { items: a.items } };
    // Phase 2 frontend-context-menu：右键菜单"在终端 / 编辑器打开"+ Settings dropdown
    case "open_in_terminal":
      return { method: "POST", path: "/api/external-app/terminal", body: { path: a.path } };
    case "open_in_editor":
      return {
        method: "POST",
        path: "/api/external-app/editor",
        body: { path: a.path, line: a.line, column: a.column },
      };
    case "list_available_terminals":
      return { method: "GET", path: "/api/external-app/terminals" };
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
      return "ssh_status";
    case "context_changed":
      return "context_changed";
    case "sse_lagged":
      // 后端 broadcast 容量打满后丢弃了若干 PushEvent。`/api/events` SSE
      // handler 在 BroadcastStream Lagged 时 inline emit 一条 `sse_lagged`
      // 让 UI 知道：前后状态可能不一致，触发 silent refresh 兜底重拉一轮
      // （codex 二审 issue 2）。
      return "sse-lagged";
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
        // 后端 `PushEvent::FileChange.session_list_changed` 透传（change
        // `enrich-file-change-with-session-list-changed::D5`）。旧后端缺字段
        // 时拿 undefined，Sidebar handler 走 `?? false` 退化分支。
        sessionListChanged: payload.session_list_changed,
      };
    case "todo_change":
      return { projectId: payload.project_id, sessionId: payload.session_id };
    case "new_notification":
      return payload.notification;
    case "session_metadata_update":
      // `group_id` 是 sidebar handler 按 `selectedGroupId` 过滤 stale event 的
      // 主匹配键（`Sidebar.svelte::onMount metadataUnlisten` 内 `eventGroupId =
      // payload.groupId ?? payload.projectId`）。Tauri runtime 走 serde camelCase
      // 直接拿 `groupId`，HTTP/SSE wire 是 snake_case 必须在此 map——否则多
      // worktree group 下 `payload.projectId` 是 worktree-level id，永不等于
      // `.git` 后缀的 group.id，所有 metadata patch 被全量丢弃，sidebar 永久
      // 卡 skeleton（本 change 把 file_change 三档收缩后暴露的既有 bug：原先
      // file_change 风暴会顺带触发 sessions refetch 间接掩盖此失配）。
      return {
        groupId: payload.group_id,
        projectId: payload.project_id,
        sessionId: payload.session_id,
        title: payload.title,
        messageCount: payload.message_count,
        isOngoing: payload.is_ongoing,
        gitBranch: payload.git_branch,
      };
    case "context_changed":
      // 与 `ContextChanged` TS 类型 + 桌面 Tauri 桥 (`app.emit("context_changed", ...)`)
      // payload 形态对齐：`{ activeContextId, kind }`——前端 `contextStore`
      // 的 `refreshAfterContextChange(change)` 直接消费这两个字段。
      // 历史 bug：曾经返 `{ activeContext: payload.active_context }`，
      // listener 期望 `change.activeContextId`，永久失配，浏览器 `?http=1`
      // 模式下 contextStore 在 SSH 切换后永远 stale 在 `local`。
      return {
        activeContextId: payload.active_context_id ?? null,
        kind: payload.kind,
      };
    case "ssh_status_change":
      // 与 `session_metadata_update` 同类的 HTTP/SSE wire normalize：后端
      // `PushEvent::SshStatusChange { context_id, state }`（`crates/cdt-api/
      // src/ipc/events.rs:34`）走 enum 默认 `rename_all = "snake_case"`，wire
      // 字段是 snake_case。Tauri runtime 通过 `listen("ssh_status", ...)`
      // 拿到的是真 `cdt_ssh::SshStatusChange` struct（`#[serde(rename_all =
      // "camelCase")]`）camelCase payload，前端消费侧（如
      // `ui/src/lib/connection.svelte.ts::change.contextId / change.status`）
      // 读 camelCase。HTTP 路径 SHALL 在此 map，否则浏览器 `?http=1` 下
      // SSH 连接状态指示符永久拿 undefined。
      //
      // 备注：PushEvent::SshStatusChange 当前只 emit `{context_id, state}` 两
      // 字段（与 cdt_ssh::SshStatusChange 的 `{context_id, status, auth_chain,
      // error}` 全字段不对齐——HTTP 路径缺 auth_chain / error）。本次 normalize
      // 仅修字段名失配（最小回归暴露面），HTTP 路径补全 auth_chain / error
      // 走另一个 change（不属本 PR scope）。
      return {
        contextId: payload.context_id,
        status: payload.state,
      };
    case "sse_lagged":
      // 后端新形态 `PushEvent::SseLagged { source, missed }` 透传（change
      // `enrich-file-change-with-session-list-changed::D6`）。旧 sentinel
      // `{"type":"sse_lagged"}` 缺字段时 source / missed 为 undefined，
      // 前端 handler 按可选字段处理向后兼容。
      return {
        source: payload.source,
        missed: payload.missed,
      };
    default:
      return payload;
  }
}
