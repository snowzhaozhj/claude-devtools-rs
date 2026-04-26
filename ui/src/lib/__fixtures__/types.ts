// Fixture 数据契约。复用 ui/src/lib/api.ts 已有 interface，让 TS 编译期
// 兜底字段命名一致性。后端字段改时前端 interface 先同步，fixture TS 报错
// 立即捕获漂移（详见 openspec/changes/frontend-test-infrastructure/design.md D6）。

import type {
  AppConfig,
  GetNotificationsResult,
  PaginatedResponse,
  ProjectInfo,
  SessionDetail,
  SessionSummary,
} from '../api'
import type { AgentConfig } from '../agentConfigsStore.svelte'

export interface ProjectSessionPrefs {
  pinned: string[]
  hidden: string[]
}

/** 一个 fixture 场景的全部数据。tauriMock 按 command 路由读取相应字段。 */
export interface Fixture {
  /** fixture 名（与 selectFixture 入参一致）。 */
  name: string
  /** 项目列表。list_projects 直接返回。 */
  projects: ProjectInfo[]
  /** projectId → sessions[]。list_sessions 按 projectId 查找。 */
  sessions: Record<string, SessionSummary[]>
  /** "<projectId>:<sessionId>" → SessionDetail。get_session_detail 查找。 */
  sessionDetails: Record<string, SessionDetail>
  /** projectId → ProjectSessionPrefs。get_project_session_prefs 查找。 */
  prefs: Record<string, ProjectSessionPrefs>
  /** AppConfig。get_config / update_config 返回。 */
  config: AppConfig
  /** 通知列表。get_notifications 返回。 */
  notifications: GetNotificationsResult
  /** Agent configs。read_agent_configs 返回。 */
  agentConfigs: AgentConfig[]
  /** 搜索结果。search_sessions 返回（简单 mock：返回 sessions 子集）。 */
  searchResults: { sessionId: string; projectId: string; matches: number }[]
}

export type {
  AgentConfig,
  AppConfig,
  GetNotificationsResult,
  PaginatedResponse,
  ProjectInfo,
  SessionDetail,
  SessionSummary,
}

