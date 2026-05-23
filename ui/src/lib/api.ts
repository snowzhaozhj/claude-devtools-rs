import type { UnlistenFn } from "@tauri-apps/api/event";
import { getTransport, subscribeEvent } from "./transport";
import type {
  ContextChanged,
  ContextSummary,
  ResolvedHost,
  SshConnectionRequest,
  SshConnectionResult,
  SshLastConnection as IpcSshLastConnection,
  SshState,
  SshStatusChange,
} from "./types/ssh";

const invoke = <T>(cmd: string, args?: Record<string, unknown>) => getTransport().invoke<T>(cmd, args);

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
  /**
   * 会话最后一条消息所在的 git 分支。骨架态为 null，由
   * `session-metadata-update` 异步 patch 填充。详见
   * `openspec/specs/ipc-data-api/spec.md` §"Expose git branch on
   * session summary and metadata updates"。
   */
  gitBranch: string | null;
  /**
   * 当 session 属于某 `RepositoryGroup` 内的 worktree 时，记录 worktree 的
   * project id（与 `Worktree.id == Project.id` 一致）。`list_repository_groups`
   * 未跑过时 fallback 为 `projectId`（IPC handler join 缓存未填）。
   * change `simplify-repository-as-project::D2/D7`。
   */
  worktreeId?: string;
  /** 同 `worktreeId`，记录 worktree 的人类展示名（`Worktree.name`）。 */
  worktreeName?: string;
  /**
   * 该 session 所属 `RepositoryGroup.id`，让前端按 group 维度过滤 SSE event /
   * cache key。`list_repository_groups` 未跑过时 fallback 为 `projectId`。
   * change `simplify-repository-as-project::D7`。
   */
  groupId?: string;
  /**
   * 该 session 所属 `Worktree.cwd_relative_to_repo_root`（如 `crates`、
   * `.claude/worktrees/feat-x`）。repo 根本身或解析失败时省略。
   * 由 IPC handler 通过 `worktree_meta_cache` join 填入（scheme c）。
   * change `simplify-repository-as-project::D2`。
   */
  cwdRelativeToRepoRoot?: string;
  /**
   * session jsonl 首条带 `cwd` 字段消息的 `cwd` 值；缺失时省略。
   * 由后端 `ProjectScanner` 通过 head-read 填充。Sidebar 行尾不再展示该字段
   * （change `simplify-repository-as-project::D8` 已切换到 cwd hint chip），
   * SessionDetail 顶部 badge 仍消费。
   */
  cwd?: string;
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
  gitBranch: string | null;
  /**
   * 该 session 所属 `RepositoryGroup.id`。前端按此字段匹配 `selectedGroupId`
   * 过滤 SSE event；`projectId` 仍保留供 detail / cache 路径用。
   * `list_repository_groups` 未跑过时缺省，前端 fallback 到 `projectId` 匹配。
   * change `simplify-repository-as-project::D7`。
   */
  groupId?: string;
}

export interface PaginatedResponse<T> {
  items: T[];
  nextCursor: string | null;
  total: number;
}

export interface SessionSearchResult {
  sessionId: string;
  projectId: string;
  sessionTitle: string;
  totalMatches: number;
}

export interface SearchSessionsResult {
  results: SessionSearchResult[];
  totalMatches: number;
  sessionsSearched: number;
  query: string;
  isPartial: boolean;
}

export type MemoryLayerKind = "index" | "entry" | "orphan";

export interface MemoryLayer {
  file: string;
  title: string;
  hook: string | null;
  kind: MemoryLayerKind;
}

export interface ProjectMemory {
  projectId: string;
  hasMemory: boolean;
  count: number;
  defaultFile: string | null;
  layers: MemoryLayer[];
}

export interface MemoryFileContent {
  projectId: string;
  file: string;
  filePath: string;
  content: string;
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

export interface TeammateSpawnInfo {
  /** 队友成员名（如 "member-1"）。 */
  name: string;
  /** 队友色（teamColors 调色板键）；缺失时 UI 退化到 muted 视觉。 */
  color?: string | null;
}

export interface ToolExecution {
  toolUseId: string;
  toolName: string;
  input: unknown;
  output: ToolOutput;
  isError: boolean;
  errorMessage?: string | null;
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
  /** 当 tool_result.toolUseResult.status === "teammate_spawned" 时由后端
   *  抽出的队友派生元数据。前端检测到非空 → 渲染极简单行（圆点 + member-X
   *  badge + Teammate spawned）替代 DefaultToolViewer，对齐原版 LinkedToolItem.tsx。 */
  teammateSpawn?: TeammateSpawnInfo | null;
}

export interface UserChunk {
  kind: "user";
  chunkId: string;
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
  /**
   * 裁剪前 messages 总长度（subagent build_chunks 后的 chunk 数）。
   * OMIT 默认路径与 rollback 路径下都反映真实总数；前端 SubagentCard 用作
   * ongoing 主动重拉的版本指纹之一。spec 见 `ipc-data-api` "Expose subagent
   * messages total count" Requirement。老后端缺此字段时为 `undefined`，行为
   * 退化（版本指纹常量，不主动重拉，仅按既有 lazy 路径展开拉一次）。
   */
  messagesTotalCount?: number;
}

export interface SlashCommand {
  name: string;
  message: string | null;
  args: string | null;
  messageUuid: string;
  timestamp: string;
  instructions: string | null;
}

export interface TeammateMessage {
  uuid: string;
  teammateId: string;
  /** named color (blue/green/...) 或 hex；缺失时 UI 退化到 muted 视觉。 */
  color: string | null;
  /** 队友自填主题，UI header 截断到 80 字显示。 */
  summary: string | null;
  /** 队友消息正文，markdown 渲染走 lazyMarkdown 管线。 */
  body: string;
  timestamp: string;
  /**
   * 配对的 SendMessage tool_use_id；orphan 时为 null/undefined（serde
   * `skip_serializing_if = Option::is_none` 控制，UI 按 `?? null` 兼容）。
   */
  replyToToolUseId?: string | null;
  /** body 灌入主 session 的 token 估算。null 时 token 槽不渲染。 */
  tokenCount?: number | null;
  /** 运维噪声（idle/shutdown/terminated），UI 渲染极简单行不开卡。 */
  isNoise: boolean;
  /** 重发关键词命中，UI 加 RefreshCw + opacity 0.6。 */
  isResend: boolean;
}

export interface AIChunk {
  kind: "ai";
  chunkId: string;
  timestamp: string;
  durationMs: number | null;
  responses: AssistantResponse[];
  metrics: ChunkMetrics;
  semanticSteps: SemanticStep[];
  toolExecutions: ToolExecution[];
  subagents: SubagentProcess[];
  slashCommands: SlashCommand[];
  /**
   * 嵌入到该 turn 的队友回信。后端 `skip_serializing_if = Vec::is_empty`
   * 控制：无 teammate 时字段在 IPC payload 中省略，前端按 `?? []` 兼容。
   */
  teammateMessages?: TeammateMessage[];
}

export interface SystemChunk {
  kind: "system";
  chunkId: string;
  uuid: string;
  timestamp: string;
  durationMs: number | null;
  contentText: string;
  metrics: ChunkMetrics;
}

export interface CompactionTokenDelta {
  preCompactionTokens: number;
  postCompactionTokens: number;
  delta: number;
}

export interface ContextPhase {
  phaseNumber: number;
  firstAiGroupId: string;
  lastAiGroupId: string;
  compactGroupId?: string | null;
}

export interface ContextPhaseInfo {
  phases: ContextPhase[];
  compactionCount: number;
  aiGroupPhaseMap: Record<string, number>;
  compactionTokenDeltas: Record<string, CompactionTokenDelta>;
}

export interface CompactChunk {
  kind: "compact";
  chunkId: string;
  uuid: string;
  timestamp: string;
  durationMs: number | null;
  summaryText: string;
  metrics: ChunkMetrics;
  /**
   * 该 compact 边界对应的 token 数差值。`cdt-api` 组装层基于 chunks 邻接 AI 的
   * usage 派生填充；当 compact 之前/之后无 AI 或 AI usage 缺失时为 `null`/缺省。
   * spec: ipc-data-api "Expose CompactChunk derived metadata in SessionDetail"
   */
  tokenDelta?: CompactionTokenDelta | null;
  /**
   * 该 compact 在 chunks 序列中的 1-based ordinal + 1（chunks 中第 i 个 compact
   * → phase i+1，对齐原版 `groupTransformer.ts` `phaseCounter++`）。
   */
  phaseNumber?: number | null;
}

export type Chunk = UserChunk | AIChunk | SystemChunk | CompactChunk;

export interface SessionDetail {
  sessionId: string;
  projectId: string;
  chunks: Chunk[];
  metrics: Record<string, unknown>;
  metadata: Record<string, unknown>;
  /** Latest phase 的累计 injections（向后兼容；语义等价于 `injectionsByPhase[最大 phaseNumber]`）。 */
  contextInjections: unknown[];
  /**
   * 按 phase 切分的完整累计 injections，key = `phaseNumber.toString()`。
   * Phase Selector 切到旧 phase 时直接读这里的对应数组。
   * 老后端不返回此字段，前端 fallback 链：`injectionsByPhase[latest] ?? contextInjections ?? []`。
   */
  injectionsByPhase?: Record<string, unknown[]>;
  /** session 级 phase 元数据；前端按 `phases.length > 1` 决定 Phase Selector 显隐。 */
  phaseInfo?: ContextPhaseInfo;
  isOngoing: boolean;
}

// ---------------------------------------------------------------------------
// Repository groups / worktree（git 仓库聚合视图）
// ---------------------------------------------------------------------------

/**
 * git repo 唯一身份。无 git 元数据时为 null。
 */
export interface RepositoryIdentity {
  id: string;
  name: string;
}

/**
 * 一个 worktree —— 同 git 仓库的一个视图。`id` 与底层 `Project.id` 一致。
 */
export interface Worktree {
  id: string;
  path: string;
  name: string;
  gitBranch: string | null;
  isMainWorktree: boolean;
  /**
   * true 时表示该 worktree 的 cwd 即 git repo 根（且为主 working tree）。
   * 与 `isMainWorktree` 区分："主 .git common-dir" vs "working tree 根"；
   * 主仓子目录（如 `<repo>/crates`）`isMainWorktree=true` 但 `isRepoRoot=false`。
   * change `simplify-repository-as-project::D1`。
   */
  isRepoRoot?: boolean;
  /**
   * worktree cwd 相对 repo 根的子路径（如 `crates`、`.claude/worktrees/feat-x`）。
   * repo 根本身或解析失败时省略。grouper 内部纯字符串运算，0 syscall。
   * change `simplify-repository-as-project::D2`。
   */
  cwdRelativeToRepoRoot?: string;
  sessions: string[];
  createdAt: number | null;
  mostRecentSession: number | null;
}

/**
 * 一组共享 repo identity 的 worktree。无 git 时退化为单成员组。
 */
export interface RepositoryGroup {
  id: string;
  identity: RepositoryIdentity | null;
  name: string;
  worktrees: Worktree[];
  mostRecentSession: number | null;
  totalSessions: number;
}

export async function listProjects(): Promise<ProjectInfo[]> {
  return await invoke("list_projects");
}

export async function listRepositoryGroups(): Promise<RepositoryGroup[]> {
  return await invoke("list_repository_groups");
}

/**
 * k-way merge group-session 分页响应。Server 无状态——`nextCursor` 是
 * base64(JSON) 自描述每个 worktree 的指针位置。`null` 表示所有 worktree
 * 流已耗尽。change `simplify-repository-as-project::D3`。
 */
export interface GroupSessionPage {
  sessions: SessionSummary[];
  nextCursor: string | null;
}

/**
 * 取得 group 内 N 个 worktree 合并后的 session 列表，按 `(mtime desc, sid asc)`
 * 全序输出 `pageSize` 条，cursor 自描述续页位点。worktree filter 通过构造
 * 初始 cursor（让非选 worktree 标 `Exhausted`）在 server 端表达，前端无需
 * 任何客户端过滤。
 *
 * Spec: `openspec/specs/ipc-data-api/spec.md` §"Expose group session listing
 * via k-way merge pagination"。
 */
export async function listGroupSessions(
  groupId: string,
  pageSize: number = 50,
  cursor?: string | null,
): Promise<GroupSessionPage> {
  return await invoke("list_group_sessions", {
    groupId,
    pageSize,
    cursor: cursor ?? null,
  });
}

export interface WslDistroCandidate {
  distro: string;
  homePath: string;
  claudeRootPath: string;
  claudeRootExists: boolean;
}

export interface WslDistroScanReport {
  candidates: WslDistroCandidate[];
  distrosWithoutHome: string[];
}

/**
 * 枚举本机 WSL distro 并返回每个 distro 的 `~/.claude` UNC 候选路径。
 * 仅 Windows 平台返回非空数据；其他平台始终返回空报告。
 *
 * Spec：openspec/specs/wsl-distro-discovery/spec.md。
 */
export async function listWslDistros(): Promise<WslDistroScanReport> {
  return await invoke("list_wsl_distros");
}

/**
 * 取得一个 RepositoryGroup 内所有 worktree 合并后的 session 列表（按 mtime 倒序）。
 * 每条 SessionSummary 携带 `worktreeId` / `worktreeName` 表示归属 worktree。
 */
export async function getWorktreeSessions(
  groupId: string,
  pageSize: number = 50,
  cursor?: string,
): Promise<PaginatedResponse<SessionSummary>> {
  return await invoke("get_worktree_sessions", {
    groupId,
    pageSize,
    cursor: cursor ?? null,
  });
}

export async function listSessions(
  projectId: string,
  pageSize: number = 20,
  cursor?: string
): Promise<PaginatedResponse<SessionSummary>> {
  return await invoke("list_sessions", {
    projectId,
    pageSize,
    cursor: cursor ?? null,
  });
}

export async function listAllSessions(projectId: string): Promise<PaginatedResponse<SessionSummary>> {
  const pageSize = 50;
  let result = await listSessions(projectId, pageSize);
  const items = [...result.items];
  while (result.nextCursor) {
    result = await listSessions(projectId, pageSize, result.nextCursor);
    items.push(...result.items);
  }
  return { ...result, items };
}

export async function getSessionSummariesByIds(
  projectId: string,
  sessionIds: string[],
): Promise<SessionSummary[]> {
  if (sessionIds.length === 0) return [];
  return await invoke("get_session_summaries_by_ids", { projectId, sessionIds });
}

export async function searchSessions(
  projectId: string,
  query: string,
): Promise<SearchSessionsResult> {
  return await invoke("search_sessions", { projectId, query });
}

export async function getSessionDetail(
  projectId: string,
  sessionId: string
): Promise<SessionDetail> {
  return await invoke("get_session_detail", { projectId, sessionId });
}

export async function getProjectMemory(projectId: string): Promise<ProjectMemory> {
  return await invoke("get_project_memory", { projectId });
}

export async function readMemoryFile(
  projectId: string,
  file: string,
): Promise<MemoryFileContent> {
  return await invoke("read_memory_file", { projectId, file });
}

/**
 * 写入 / 覆盖项目 memory 目录内的 Markdown 文件——atomic 语义。
 * 返回写入后最新 ProjectMemory（前端无需再调 getProjectMemory）。
 *
 * change `ssh-project-memory-remote-rw`: SSH context 下走远端 SFTP write_atomic。
 */
export async function addMemory(
  projectId: string,
  file: string,
  content: string,
): Promise<ProjectMemory> {
  return await invoke("add_memory", { projectId, file, content });
}

/**
 * 删除项目 memory 目录内的 Markdown 文件。
 * 返回删除后最新 ProjectMemory。
 */
export async function deleteMemory(
  projectId: string,
  file: string,
): Promise<ProjectMemory> {
  return await invoke("delete_memory", { projectId, file });
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
  claudeRootPath: string | null;
  autoExpandAiGroups: boolean;
  /** "replace" | "new-tab"，默认 "replace" */
  sessionClickBehavior?: string;
}

/** 时间格式偏好；详见 `openspec/specs/configuration-management/spec.md`。 */
export type TimeFormat = "24h" | "12h";

export interface DisplayConfig {
  showTimestamps?: boolean;
  compactMode?: boolean;
  syntaxHighlighting?: boolean;
  fontSans?: string | null;
  fontMono?: string | null;
  /** "24h" | "12h"，默认 "24h"。旧后端缺字段时前端 fallback "24h"。 */
  timeFormat?: TimeFormat;
}

export type SshAuthMethod = "sshConfig" | "password";

export interface SshProfile {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authMethod: SshAuthMethod;
  privateKeyPath?: string | null;
}

export interface SshLastConnection {
  host: string;
  port?: number | null;
  username?: string | null;
  authMethod: SshAuthMethod;
  contextId?: string | null;
}

export interface SshConfig {
  profiles: SshProfile[];
  lastConnection?: SshLastConnection | null;
  autoReconnect: boolean;
}

export interface UpdaterConfig {
  autoUpdateCheckEnabled: boolean;
  skippedUpdateVersion?: string | null;
}

export interface HttpServerConfig {
  enabled: boolean;
  port: number;
}

export interface HttpServerStatus {
  running: boolean;
  port: number;
  lastError: string | null;
}

export interface AppConfig {
  notifications: NotificationConfig;
  general: GeneralConfig;
  display?: DisplayConfig;
  ssh?: SshConfig;
  updater?: UpdaterConfig;
  httpServer?: HttpServerConfig;
}

// =============================================================================
// 自动更新 IPC 类型
// =============================================================================

export type CheckUpdateResult =
  | { status: "up_to_date"; currentVersion: string }
  | {
      status: "available";
      currentVersion: string;
      newVersion: string;
      notes: string;
      signatureOk: boolean;
    }
  | { status: "error"; message: string };

export async function checkForUpdate(): Promise<CheckUpdateResult> {
  return await invoke("check_for_update");
}

// macOS 上探测当前 Tauri 进程是否被 Rosetta 翻译执行（Apple Silicon 装了 x86_64 包）。
// 其他平台始终返回 false。
export async function isRunningUnderRosetta(): Promise<boolean> {
  return await invoke("is_running_under_rosetta");
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

export async function sshConnect(
  request: SshConnectionRequest,
): Promise<SshConnectionResult> {
  return await invoke("ssh_connect", { request });
}

export async function sshDisconnect(contextId: string): Promise<void> {
  return await invoke("ssh_disconnect", { contextId });
}

export async function sshTestConnection(
  request: SshConnectionRequest,
): Promise<SshConnectionResult> {
  return await invoke("ssh_test_connection", { request });
}

export async function sshGetState(contextId?: string): Promise<SshState> {
  return await invoke("ssh_get_state", { contextId: contextId ?? null });
}

export async function sshGetConfigHosts(): Promise<string[]> {
  return await invoke("ssh_get_config_hosts");
}

export async function sshResolveHost(alias: string): Promise<ResolvedHost> {
  return await invoke("ssh_resolve_host", { alias });
}

export async function sshSaveLastConnection(
  payload: IpcSshLastConnection,
): Promise<IpcSshLastConnection | null> {
  return await invoke("ssh_save_last_connection", { request: payload });
}

export async function sshGetLastConnection(): Promise<IpcSshLastConnection | null> {
  return await invoke("ssh_get_last_connection");
}

export async function listContexts(): Promise<ContextSummary[]> {
  return await invoke("list_contexts");
}

export async function switchContext(contextId: string): Promise<void> {
  return await invoke("switch_context", { contextId });
}

export async function getActiveContext(): Promise<ContextSummary> {
  return await invoke("get_active_context");
}

export async function listenSshStatus(
  handler: (payload: SshStatusChange) => void,
): Promise<UnlistenFn> {
  return await subscribeEvent<SshStatusChange>("ssh_status", (event) => handler(event.payload));
}

export async function listenContextChanged(
  handler: (payload: ContextChanged) => void,
): Promise<UnlistenFn> {
  return await subscribeEvent<ContextChanged>("context_changed", (event) => handler(event.payload));
}

// ---------------------------------------------------------------------------
// server-mode：本机 HTTP server 启停 / 状态查询
//
// 详见 openspec/specs/server-mode/spec.md。仅 Tauri runtime 调用——浏览器
// runtime 已在 server 后面，没有控制 server 的入口（且会自杀失联）。
// ---------------------------------------------------------------------------

export async function startHttpServer(port: number): Promise<void> {
  await invoke("http_server_start", { port });
}

export async function stopHttpServer(): Promise<void> {
  await invoke("http_server_stop");
}

export async function getHttpServerStatus(): Promise<HttpServerStatus> {
  return await invoke("http_server_status");
}

// ---------------------------------------------------------------------------
// Telemetry：应用健康度 Signal Bus 快照（只读）+ correctness event 批量上报
//
// 详见 openspec/specs/application-telemetry/spec.md。pull-based 快照供 Settings
// → Diagnostics tab 渲染；correctness events 由前端 store 5s/50 累计窗口 flush。
// ---------------------------------------------------------------------------

export interface HistogramSnapshot {
  count: number;
  buckets: number[];
  p50Ns: number | null;
  p95Ns: number | null;
  p99Ns: number | null;
  maxBucket: number | null;
}

export interface TelemetryEvent {
  kind: string;
  tsUnixMs: number;
  fields: Record<string, string>;
}

export interface TelemetrySnapshot {
  schemaVersion: number;
  uptimeSecs: number;
  capturedAt: number;
  counters: Record<string, number>;
  histograms: Record<string, HistogramSnapshot>;
  recentEvents: TelemetryEvent[];
}

export interface CorrectnessEventItem {
  kind: string;
  count: number;
}

export async function getTelemetrySnapshot(): Promise<TelemetrySnapshot> {
  return await invoke("get_telemetry_snapshot");
}

export async function recordCorrectnessEvents(
  items: CorrectnessEventItem[],
): Promise<void> {
  await invoke("record_correctness_events", { items });
}
