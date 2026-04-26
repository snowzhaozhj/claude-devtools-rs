import type { AIChunk, UserChunk, Chunk } from '../api'
import type { Fixture } from './types'

const PROJECT_ID = 'mock-single-proj'
const SESSION_ID = 'mock-single-sess'
const TS = '2026-04-11T10:00:00Z'

const userChunk: UserChunk = {
  kind: 'user',
  uuid: 'u1',
  timestamp: TS,
  durationMs: null,
  content: '帮我看一下这个 bug',
  metrics: {
    inputTokens: 0,
    outputTokens: 0,
    cacheCreationTokens: 0,
    cacheReadTokens: 0,
    toolCount: 0,
    costUsd: null,
  },
}

const aiChunk: AIChunk = {
  kind: 'ai',
  timestamp: '2026-04-11T10:00:05Z',
  durationMs: 1500,
  responses: [
    {
      uuid: 'a1',
      timestamp: '2026-04-11T10:00:05Z',
      content: '好的，我来分析一下',
      toolCalls: [],
      usage: null,
      model: 'claude-sonnet-4-6',
    },
  ],
  metrics: {
    inputTokens: 1000,
    outputTokens: 500,
    cacheCreationTokens: 0,
    cacheReadTokens: 0,
    toolCount: 1,
    costUsd: null,
  },
  semanticSteps: [
    { kind: 'text', text: '好的，我来分析一下', timestamp: '2026-04-11T10:00:05Z' },
    {
      kind: 'tool_execution',
      toolUseId: 'tu1',
      toolName: 'Read',
      timestamp: '2026-04-11T10:00:06Z',
    },
  ],
  toolExecutions: [
    {
      toolUseId: 'tu1',
      toolName: 'Read',
      input: { file_path: '/src/main.rs' },
      output: { kind: 'text', text: 'fn main() {}' },
      isError: false,
      startTs: '2026-04-11T10:00:06Z',
      endTs: '2026-04-11T10:00:07Z',
      sourceAssistantUuid: 'a1',
      outputOmitted: false,
      outputBytes: 12,
    },
  ],
  subagents: [],
  slashCommands: [],
}

const chunks: Chunk[] = [userChunk, aiChunk]

export const singleProjectFixture: Fixture = {
  name: 'single-project',
  projects: [
    {
      id: PROJECT_ID,
      path: '/Users/test/single-proj',
      displayName: 'single-proj',
      sessionCount: 1,
    },
  ],
  sessions: {
    [PROJECT_ID]: [
      {
        sessionId: SESSION_ID,
        projectId: PROJECT_ID,
        timestamp: 1_712_822_400_000,
        messageCount: 2,
        title: '帮我看一下这个 bug',
        isOngoing: false,
      },
    ],
  },
  sessionDetails: {
    [`${PROJECT_ID}:${SESSION_ID}`]: {
      sessionId: SESSION_ID,
      projectId: PROJECT_ID,
      chunks,
      metrics: {},
      metadata: {},
      contextInjections: [],
      isOngoing: false,
    },
  },
  prefs: {},
  config: {
    notifications: { enabled: true, soundEnabled: true, triggers: [] },
    general: {
      launchAtLogin: false,
      showDockIcon: true,
      theme: 'system',
      defaultTab: 'sessions',
      autoExpandAiGroups: false,
    },
  },
  notifications: {
    notifications: [],
    total: 0,
    totalCount: 0,
    unreadCount: 0,
    hasMore: false,
  },
  agentConfigs: [],
  searchResults: [],
}
