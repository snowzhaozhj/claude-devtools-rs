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
  title: string | null;
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
}

export type SemanticStep =
  | { kind: "thinking"; text: string; timestamp: string }
  | { kind: "text"; text: string; timestamp: string }
  | { kind: "tool_execution"; toolUseId: string; toolName: string; timestamp: string }
  | { kind: "subagent_spawn"; placeholderId: string; timestamp: string };

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
}

export interface UserChunk {
  kind: "user";
  uuid: string;
  timestamp: string;
  durationMs: number | null;
  content: string | ContentBlock[];
  metrics: ChunkMetrics;
}

export interface SubagentProcess {
  sessionId: string;
  rootTaskDescription: string | null;
  spawnTs: string;
  endTs: string | null;
  metrics: ChunkMetrics;
  team: { teamName: string; memberName: string; memberColor: string | null } | null;
}

export interface SlashCommand {
  name: string;
  message: string | null;
  args: string | null;
  messageUuid: string;
  timestamp: string;
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
