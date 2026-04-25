import { invoke } from "@tauri-apps/api/core";

export interface ProjectInfo {
  id: string;
  path: string;
  displayName: string;
  sessionCount: number;
}

/**
 * 会话摘要。`title` / `messageCount` / `isOngoing` 在 IPC `list_sessions`
 * 返回的**骨架**态下为占位（null / 0 / false），随后由
 * `session-metadata-update` 事件按 sessionId 增量 patch 为真值。
 *
 * UI 渲染时应使用 fallback：`title || sessionId.slice(0, 8) + "…"`、
 * `C${messageCount || ""}`、`{#if isOngoing}<OngoingIndicator />{/if}`，
 * 这样骨架态也能直接展示。
 *
 * 详见 openspec/specs/ipc-data-api/spec.md §"Expose project and session
 * queries" 与 sidebar-navigation §"骨架列表快速加载"。
 */
export interface SessionSummary {
  sessionId: string;
  projectId: string;
  timestamp: number;
  messageCount: number;
  title: string | null;
  isOngoing: boolean;
}

/**
 * 单条 session 元数据增量更新，由后端 `session-metadata-update` 事件推送。
 * 前端 Sidebar 订阅后按 `sessionId` 在 `sessions[]` 中定位并 in-place patch。
 */
export interface SessionMetadataUpdate {
  projectId: string;
  sessionId: string;
  title: string | null;
  messageCount: number;
  isOngoing: boolean;
}

export interface PaginatedResponse<T> {
  items: T[];
  nextCursor: string | null;
  total: number;
}

// ---------------------------------------------------------------------------
// Chunk 类型（与 Rust cdt-core serde(tag="kind", rename_all="snake_case") 对齐）
// ---------------------------------------------------------------------------

export interface ChunkMetrics {
  inputTokens: number;
  outputTokens: number;
  cacheCreationTokens: number;
  cacheReadTokens: number;
  toolCount: number;
  costUsd: number | null;
}

export interface AssistantResponse {
  uuid: string;
  timestamp: string;
  content: string | ContentBlock[];
  toolCalls: ToolCall[];
  usage: TokenUsage | null;
  model: string | null;
}

export interface TokenUsage {
  input_tokens: number;
  output_tokens: number;
  cache_read_input_tokens: number;
  cache_creation_input_tokens: number;
}

export interface ToolCall {
  id: string;
  name: string;
  input: unknown;
  isTask: boolean;
  taskDescription: string | null;
  taskSubagentType: string | null;
}

export interface ImageSource {
  type: string; // "base64"
  media_type: string; // "image/png" 等（snake_case 与上游 Anthropic 格式一致）
  data: string; // OMIT 路径下为空字符串
  dataOmitted?: boolean; // 后端 OMIT_IMAGE_DATA=true 时为 true
}

export interface ContentBlock {
  type: string;
  text?: string;
  thinking?: string;
  id?: string;
  name?: string;
  input?: unknown;
  toolUseId?: string;
  content?: unknown;
  isError?: boolean;
  source?: ImageSource;
}

export type SemanticStep =
  | { kind: "thinking"; text: string; timestamp: string }
  | { kind: "text"; text: string; timestamp: string }
  | { kind: "tool_execution"; toolUseId: string; toolName: string; timestamp: string }
  | { kind: "subagent_spawn"; placeholderId: string; timestamp: string }
  | { kind: "interruption"; text: string; timestamp: string };

export type ToolOutput =
  | { kind: "text"; text: string }
  | { kind: "structured"; value: unknown }
  | { kind: "missing" };

export interface ToolExecution {
  toolUseId: string;
  toolName: string;
  input: unknown;
  output: ToolOutput;
  isError: boolean;
  startTs: string;
  endTs: string | null;
  sourceAssistantUuid: string;
  /** true 表示 output 已被 IPC 裁剪（inner text/value 清空但 variant kind 保留），
   *  需通过 getToolOutput(rootSessionId, sessionId, toolUseId) 懒拉取。
   *  老后端 / 回滚时为 false 或 undefined。 */
  outputOmitted?: boolean;
  /** OMIT 层在 trim 前记录的 output 原始字节长度（见 change `tool-output-omit-preserve-size`）。
   *  解析层 / HTTP 路径 / 老后端为 undefined；前端 token 估算优先用此字段除以 4，
   *  让懒加载前后 BaseItem 头部 token 数稳定。Missing variant 不填。 */
  outputBytes?: number;
}

export interface UserChunk {
  kind: "user";
  uuid: string;
  timestamp: string;
  durationMs: number | null;
  content: string | ContentBlock[];
  metrics: ChunkMetrics;
}

export interface MainSessionImpact {
  totalTokens: number;
}

export interface SubagentProcess {
  sessionId: string;
  rootTaskDescription: string | null;
  spawnTs: string;
  endTs: string | null;
  metrics: ChunkMetrics;
  team: { teamName: string; memberName: string; memberColor: string | null } | null;
  subagentType: string | null;
  /** 默认空 Vec（IPC 已裁剪）；展开 SubagentCard 时通过 getSubagentTrace 懒拉取。 */
  messages: Chunk[];
  mainSessionImpact: MainSessionImpact | null;
  isOngoing: boolean;
  durationMs: number | null;
  parentTaskId: string | null;
  description: string | null;
  /** 后端预算的 header 模型名（已 simplify，如 "haiku4.5"）。messages 缺失时仍可显示。 */
  headerModel?: string | null;
  /** 后端预算的最后一条 assistant usage 总和（input+output+cacheRead+cacheCreation）。 */
  lastIsolatedTokens?: number;
  /** 后端预算的 shutdown-only flag（team-only 极简渲染分支）。 */
  isShutdownOnly?: boolean;
  /** true 表示 messages 已被 IPC 裁剪，需 getSubagentTrace 懒拉取。 */
  messagesOmitted?: boolean;
}

export interface SlashCommand {
  name: string;
  message: string | null;
  args: string | null;
  messageUuid: string;
  timestamp: string;
  instructions: string | null;
}

export interface AIChunk {
  kind: "ai";
  timestamp: string;
  durationMs: number | null;
  responses: AssistantResponse[];
  metrics: ChunkMetrics;
  semanticSteps: SemanticStep[];
  toolExecutions: ToolExecution[];
  subagents: SubagentProcess[];
  slashCommands: SlashCommand[];
}

export interface SystemChunk {
  kind: "system";
  uuid: string;
  timestamp: string;
  durationMs: number | null;
  contentText: string;
  metrics: ChunkMetrics;
}

export interface CompactChunk {
  kind: "compact";
  uuid: string;
  timestamp: string;
  durationMs: number | null;
  summaryText: string;
  metrics: ChunkMetrics;
}

export type Chunk = UserChunk | AIChunk | SystemChunk | CompactChunk;

export interface SessionDetail {
  sessionId: string;
  projectId: string;
  chunks: Chunk[];
  metrics: Record<string, unknown>;
  metadata: Record<string, unknown>;
  contextInjections: unknown[];
  isOngoing: boolean;
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
): Promise<SessionDetail> {
  return await invoke("get_session_detail", { projectId, sessionId });
}

/**
 * 按需拉取 subagent 完整 chunks 流。SubagentCard 展开时使用——首屏 IPC 默认
 * 把 SubagentProcess.messages 裁空，前端再单独拉取，砍 60% payload。
 *
 * 找不到（subagent jsonl 不存在 / root session 不属任何已知 project）时返回 []，
 * 不报错。
 */
export async function getSubagentTrace(
  rootSessionId: string,
  subagentSessionId: string,
): Promise<Chunk[]> {
  return await invoke("get_subagent_trace", { rootSessionId, subagentSessionId });
}

/**
 * 按需拉取一个 image block 的可加载 URL。
 *
 * `get_session_detail` 默认裁剪 `ImageSource.data` 为空字符串 + 设
 * `dataOmitted=true`；ImageBlock 组件进入视口时调本方法拿到：
 *   - 成功落盘：`asset://localhost/<absolute_path>`（浏览器原生加载）
 *   - 失败 fallback：`data:<media_type>;base64,<...>` URI（兼容路径）
 *
 * blockId 编码：`"<chunkUuid>:<blockIndex>"`。
 */
export async function getImageAsset(
  rootSessionId: string,
  sessionId: string,
  blockId: string,
): Promise<string> {
  return await invoke("get_image_asset", { rootSessionId, sessionId, blockId });
}

/**
 * 按需拉取一条 tool execution 的完整 output。
 *
 * `get_session_detail` 默认裁剪 `tool_executions[].output` 内 text/value 字段
 * + 设 `outputOmitted=true`；ExecutionTrace 在用户点击展开时调本方法拿原 output。
 *
 * 失败 / 找不到 → 返回 `{ kind: "missing" }`，前端走 broken/missing 显示分支。
 */
export async function getToolOutput(
  rootSessionId: string,
  sessionId: string,
  toolUseId: string,
): Promise<ToolOutput> {
  return await invoke("get_tool_output", { rootSessionId, sessionId, toolUseId });
}

// ---------------------------------------------------------------------------
// Config 类型
// ---------------------------------------------------------------------------

export interface NotificationTrigger {
  id: string;
  name: string;
  enabled: boolean;
  contentType: string;
  mode: string;
  color?: string;
}

export interface NotificationConfig {
  enabled: boolean;
  soundEnabled: boolean;
  triggers: NotificationTrigger[];
}

export interface GeneralConfig {
  launchAtLogin: boolean;
  showDockIcon: boolean;
  theme: string;
  defaultTab: string;
  autoExpandAiGroups: boolean;
}

export interface AppConfig {
  notifications: NotificationConfig;
  general: GeneralConfig;
}

export async function getConfig(): Promise<AppConfig> {
  return await invoke("get_config");
}

export async function updateConfig(
  section: string,
  configData: Record<string, unknown>
): Promise<AppConfig> {
  return await invoke("update_config", { section, configData });
}

// ---------------------------------------------------------------------------
// Notifications 类型
// ---------------------------------------------------------------------------

export interface DetectedError {
  id: string;
  timestamp: number;
  sessionId: string;
  projectId: string;
  filePath: string;
  source: string;
  message: string;
  triggerName?: string;
  triggerColor?: string;
}

export interface StoredNotification {
  id: string;
  timestamp: number;
  sessionId: string;
  projectId: string;
  filePath: string;
  source: string;
  message: string;
  triggerName?: string;
  triggerColor?: string;
  isRead: boolean;
  createdAt: number;
}

export interface GetNotificationsResult {
  notifications: StoredNotification[];
  total: number;
  totalCount: number;
  unreadCount: number;
  hasMore: boolean;
}

export async function getNotifications(
  limit: number = 50,
  offset: number = 0
): Promise<GetNotificationsResult> {
  return await invoke("get_notifications", { limit, offset });
}

export async function markNotificationRead(
  notificationId: string
): Promise<boolean> {
  return await invoke("mark_notification_read", { notificationId });
}

export async function deleteNotification(
  notificationId: string
): Promise<boolean> {
  return await invoke("delete_notification", { notificationId });
}

export async function markAllNotificationsRead(): Promise<void> {
  return await invoke("mark_all_notifications_read");
}

export async function clearNotifications(
  triggerId?: string
): Promise<number> {
  return await invoke("clear_notifications", { triggerId: triggerId ?? null });
}

// ---------------------------------------------------------------------------
// Trigger CRUD
// ---------------------------------------------------------------------------

export interface NewTrigger {
  id: string;
  name: string;
  enabled: boolean;
  contentType: string;
  mode: string;
  requireError?: boolean;
  matchField?: string;
  matchPattern?: string;
  tokenThreshold?: number;
  tokenType?: string;
  color?: string;
}

export async function addTrigger(trigger: NewTrigger): Promise<AppConfig> {
  return await invoke("add_trigger", { trigger });
}

export async function removeTrigger(triggerId: string): Promise<AppConfig> {
  return await invoke("remove_trigger", { triggerId });
}
