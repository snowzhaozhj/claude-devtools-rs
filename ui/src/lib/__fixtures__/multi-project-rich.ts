import type {
  AIChunk,
  Chunk,
  CompactChunk,
  SystemChunk,
  UserChunk,
} from '../api'
import type { Fixture } from './types'

const TS_BASE = 1_712_822_400_000

function ts(offsetMin: number): string {
  return new Date(TS_BASE + offsetMin * 60_000).toISOString()
}

function emptyMetrics() {
  return {
    inputTokens: 0,
    outputTokens: 0,
    cacheCreationTokens: 0,
    cacheReadTokens: 0,
    toolCount: 0,
    costUsd: null,
  }
}

function buildSession(
  projectId: string,
  sessionId: string,
  title: string,
  isOngoing: boolean,
  messageCount: number,
  offsetMin: number,
) {
  return {
    sessionId,
    projectId,
    timestamp: TS_BASE + offsetMin * 60_000,
    messageCount,
    title,
    isOngoing,
  }
}

const PROJECTS = [
  {
    id: 'mock-rich-rust',
    path: '/Users/test/rust-port',
    displayName: 'rust-port',
    sessionCount: 3,
  },
  {
    id: 'mock-rich-ts',
    path: '/Users/test/claude-devtools',
    displayName: 'claude-devtools',
    sessionCount: 2,
  },
  {
    id: 'mock-rich-docs',
    path: '/Users/test/docs',
    displayName: 'docs',
    sessionCount: 1,
  },
  {
    id: 'mock-rich-experiment',
    path: '/Users/test/experiment',
    displayName: 'experiment',
    sessionCount: 1,
  },
  {
    id: 'mock-rich-archive',
    path: '/Users/test/archive',
    displayName: 'archive',
    sessionCount: 1,
  },
]

// rust-port 项目：一个 ongoing session + 两个完成 session
const rustSessions = [
  buildSession('mock-rich-rust', 'sess-rust-active', 'IPC 字段重构', true, 42, 0),
  buildSession('mock-rich-rust', 'sess-rust-2', '修复 watcher flake', false, 18, -60),
  buildSession('mock-rich-rust', 'sess-rust-3', '加 contract test', false, 25, -180),
]

const tsSessions = [
  buildSession('mock-rich-ts', 'sess-ts-1', 'TS 原版回归', false, 12, -30),
  buildSession('mock-rich-ts', 'sess-ts-2', '比对 chunk-building', false, 8, -90),
]

const docsSessions = [
  buildSession('mock-rich-docs', 'sess-docs-1', '更新 README', false, 4, -300),
]

const experimentSessions = [
  buildSession('mock-rich-experiment', 'sess-exp-1', 'Playwright POC', false, 6, -480),
]

const archiveSessions = [
  buildSession('mock-rich-archive', 'sess-arch-1', '归档老 spec', false, 2, -600),
]

// 主 session（rust-port 的 active）含多种 chunk 类型
const userChunk: UserChunk = {
  kind: 'user',
  uuid: 'u-active-1',
  timestamp: ts(0),
  durationMs: null,
  content: '帮我查一下 IPC 字段',
  metrics: emptyMetrics(),
}

const systemChunk: SystemChunk = {
  kind: 'system',
  uuid: 's-active-1',
  timestamp: ts(0.1),
  durationMs: null,
  contentText: '会话已恢复',
  metrics: emptyMetrics(),
}

const aiChunk: AIChunk = {
  kind: 'ai',
  timestamp: ts(0.2),
  durationMs: 2200,
  responses: [
    {
      uuid: 'a-active-1',
      timestamp: ts(0.2),
      content: '我来帮你检查 LocalDataApi 的字段命名。',
      toolCalls: [],
      usage: null,
      model: 'claude-sonnet-4-6',
    },
  ],
  metrics: {
    inputTokens: 1500,
    outputTokens: 800,
    cacheCreationTokens: 200,
    cacheReadTokens: 300,
    toolCount: 1,
    costUsd: null,
  },
  semanticSteps: [
    { kind: 'text', text: '我来帮你检查 LocalDataApi 的字段命名。', timestamp: ts(0.2) },
    {
      kind: 'tool_execution',
      toolUseId: 'tu-active-1',
      toolName: 'Grep',
      timestamp: ts(0.3),
    },
  ],
  toolExecutions: [
    {
      toolUseId: 'tu-active-1',
      toolName: 'Grep',
      input: { pattern: 'pub async fn', path: 'crates/cdt-api' },
      output: { kind: 'text', text: '12 matches' },
      isError: false,
      startTs: ts(0.3),
      endTs: ts(0.32),
      sourceAssistantUuid: 'a-active-1',
      outputOmitted: false,
      outputBytes: 11,
    },
  ],
  subagents: [],
  slashCommands: [
    {
      name: '/commit',
      message: null,
      args: null,
      messageUuid: 'mu-active-1',
      timestamp: ts(0.05),
      instructions: null,
    },
  ],
  teammateMessages: [
    {
      uuid: 'tm-active-1',
      teammateId: 'reviewer',
      color: 'blue',
      summary: 'fields 检查通过',
      body: '我看了一遍，**所有 22 个 command** 字段都对。',
      timestamp: ts(0.4),
      replyToToolUseId: null,
      tokenCount: 32,
      isNoise: false,
      isResend: false,
    },
  ],
}

// 一个含 interruption 的 ai chunk
const aiChunkInterrupted: AIChunk = {
  kind: 'ai',
  timestamp: ts(0.5),
  durationMs: null,
  responses: [
    {
      uuid: 'a-active-2',
      timestamp: ts(0.5),
      content: '继续往下',
      toolCalls: [],
      usage: null,
      model: 'claude-sonnet-4-6',
    },
  ],
  metrics: emptyMetrics(),
  semanticSteps: [
    { kind: 'text', text: '继续往下', timestamp: ts(0.5) },
    { kind: 'interruption', text: '[Request interrupted by user]', timestamp: ts(0.55) },
  ],
  toolExecutions: [],
  subagents: [],
  slashCommands: [],
}

const compactChunk: CompactChunk = {
  kind: 'compact',
  uuid: 'c-active-1',
  timestamp: ts(0.6),
  durationMs: null,
  summaryText: '对话已 compact',
  metrics: emptyMetrics(),
}

const richChunks: Chunk[] = [userChunk, systemChunk, aiChunk, aiChunkInterrupted, compactChunk]

export const multiProjectRichFixture: Fixture = {
  name: 'multi-project-rich',
  projects: PROJECTS,
  sessions: {
    'mock-rich-rust': rustSessions,
    'mock-rich-ts': tsSessions,
    'mock-rich-docs': docsSessions,
    'mock-rich-experiment': experimentSessions,
    'mock-rich-archive': archiveSessions,
  },
  sessionDetails: {
    'mock-rich-rust:sess-rust-active': {
      sessionId: 'sess-rust-active',
      projectId: 'mock-rich-rust',
      chunks: richChunks,
      metrics: { totalTokens: 2800 },
      metadata: { gitBranch: 'feat/frontend-test-infrastructure' },
      contextInjections: [],
      isOngoing: true,
    },
    'mock-rich-rust:sess-rust-2': {
      sessionId: 'sess-rust-2',
      projectId: 'mock-rich-rust',
      chunks: [userChunk],
      metrics: {},
      metadata: {},
      contextInjections: [],
      isOngoing: false,
    },
  },
  prefs: {
    'mock-rich-rust': { pinned: ['sess-rust-active'], hidden: ['sess-rust-3'] },
  },
  config: {
    notifications: {
      enabled: true,
      soundEnabled: true,
      triggers: [
        {
          id: 'mock-trig-builtin',
          name: 'Compile Error',
          enabled: true,
          contentType: 'tool_result',
          mode: 'error_status',
          color: '#ef4444',
        },
      ],
    },
    general: {
      launchAtLogin: false,
      showDockIcon: true,
      theme: 'system',
      defaultTab: 'sessions',
      autoExpandAiGroups: false,
    },
    display: {
      fontSans: null,
      fontMono: null,
    },
  },
  notifications: {
    notifications: [
      {
        id: 'notif-1',
        timestamp: TS_BASE,
        sessionId: 'sess-rust-active',
        projectId: 'mock-rich-rust',
        filePath: '/path/to/sess.jsonl',
        source: 'tool_result',
        message: 'cargo build failed',
        triggerName: 'Compile Error',
        triggerColor: '#ef4444',
        isRead: false,
        createdAt: TS_BASE,
      },
      {
        id: 'notif-2',
        timestamp: TS_BASE - 60_000,
        sessionId: 'sess-ts-1',
        projectId: 'mock-rich-ts',
        filePath: '/path/to/ts.jsonl',
        source: 'tool_result',
        message: 'pnpm test failed',
        triggerName: 'Test Failure',
        triggerColor: '#f59e0b',
        isRead: true,
        createdAt: TS_BASE - 60_000,
      },
    ],
    total: 2,
    totalCount: 2,
    unreadCount: 1,
    hasMore: false,
  },
  agentConfigs: [
    {
      name: 'code-reviewer',
      color: 'purple',
      description: 'PR review',
      scope: { kind: 'global' },
      filePath: '/Users/test/.claude/agents/code-reviewer.md',
    },
  ],
  searchResults: [
    { sessionId: 'sess-rust-active', projectId: 'mock-rich-rust', matches: 3 },
  ],
}
