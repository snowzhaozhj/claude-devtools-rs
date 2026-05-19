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
  gitBranch: string | null = null,
) {
  return {
    sessionId,
    projectId,
    timestamp: TS_BASE + offsetMin * 60_000,
    messageCount,
    title,
    isOngoing,
    gitBranch,
  }
}

// 注：mock-rich-rust-wt-feat 仅出现在 repositoryGroups 字段（作为
// rust-port group 的 worktree 子项），**不**进 PROJECTS 数组——保留
// listProjects() fallback 路径下"5 个 ProjectInfo"的语义稳定，避免破
// 坏既有 e2e（startup-and-dashboard 等期望"5 个项目"）。
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
  buildSession('mock-rich-rust', 'sess-rust-active', 'IPC 字段重构', true, 42, 0, 'feat/frontend-test-infrastructure'),
  buildSession('mock-rich-rust', 'sess-rust-2', '修复 watcher flake', false, 18, -60, 'main'),
  buildSession('mock-rich-rust', 'sess-rust-3', '加 contract test', false, 25, -180, 'main'),
]

const rustWtFeatSessions = [
  buildSession(
    'mock-rich-rust-wt-feat',
    'sess-rust-wt-1',
    'worktree feat-x：实现按钮',
    false,
    14,
    -45,
    'feat/x',
  ),
]

const tsSessions = [
  buildSession('mock-rich-ts', 'sess-ts-1', 'TS 原版回归', false, 12, -30, 'main'),
  buildSession('mock-rich-ts', 'sess-ts-2', '比对 chunk-building', false, 8, -90, 'main'),
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
  chunkId: 'u-active-1:0',
  uuid: 'u-active-1',
  timestamp: ts(0),
  durationMs: null,
  content: [
    { type: 'text', text: '帮我查一下 IPC 字段' },
    {
      type: 'image',
      source: {
        type: 'base64',
        media_type: 'image/png',
        data: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAFgwJ/lw1rYQAAAABJRU5ErkJggg==',
        dataOmitted: false,
      },
    },
  ],
  metrics: emptyMetrics(),
}

const systemChunk: SystemChunk = {
  kind: 'system',
  chunkId: 's-active-1:0',
  uuid: 's-active-1',
  timestamp: ts(0.1),
  durationMs: null,
  contentText: '会话已恢复',
  metrics: emptyMetrics(),
}

const subagentTraceChunk: AIChunk = {
  kind: 'ai',
  chunkId: 'sub-rich-1:a1:0',
  timestamp: ts(0.36),
  durationMs: 1400,
  responses: [
    {
      uuid: 'sub-rich-1-a1',
      timestamp: ts(0.36),
      content: '我检查了 fixture、类型定义和 subagent 渲染路径。',
      toolCalls: [],
      usage: {
        input_tokens: 1800,
        output_tokens: 520,
        cache_read_input_tokens: 2600,
        cache_creation_input_tokens: 120,
      },
      model: 'claude-sonnet-4-6',
    },
  ],
  metrics: {
    inputTokens: 1800,
    outputTokens: 520,
    cacheCreationTokens: 120,
    cacheReadTokens: 2600,
    toolCount: 1,
    costUsd: null,
  },
  semanticSteps: [
    { kind: 'thinking', text: 'Need compare SubagentProcess fields with mock fixture rendering requirements.', timestamp: ts(0.36) },
    { kind: 'tool_execution', toolUseId: 'sub-rich-grep-1', toolName: 'Grep', timestamp: ts(0.37) },
    { kind: 'text', text: 'Fixture needs a subagent_spawn semantic step plus a matching SubagentProcess.', timestamp: ts(0.38) },
  ],
  toolExecutions: [
    {
      toolUseId: 'sub-rich-grep-1',
      toolName: 'Grep',
      input: { pattern: 'interface SubagentProcess', path: 'ui/src/lib/api.ts' },
      output: { kind: 'text', text: 'ui/src/lib/api.ts:217:export interface SubagentProcess' },
      isError: false,
      startTs: ts(0.37),
      endTs: ts(0.38),
      sourceAssistantUuid: 'sub-rich-1-a1',
      outputOmitted: false,
      outputBytes: 55,
    },
  ],
  subagents: [],
  slashCommands: [],
}

const aiChunk: AIChunk = {
  kind: 'ai',
  chunkId: 'a-active-1:0',
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
    toolCount: 2,
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
    {
      kind: 'tool_execution',
      toolUseId: 'task-sub-rich-1',
      toolName: 'Task',
      timestamp: ts(0.34),
    },
    {
      kind: 'subagent_spawn',
      placeholderId: 'sub-rich-1',
      timestamp: ts(0.35),
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
    {
      toolUseId: 'task-sub-rich-1',
      toolName: 'Task',
      input: {
        description: 'Audit IPC field mappings for fixture coverage',
        prompt: 'Check the mock fixture fields and report what is missing.',
        subagent_type: 'general-purpose',
      },
      output: { kind: 'structured', value: { session_id: 'sub-rich-1' } },
      isError: false,
      startTs: ts(0.34),
      endTs: ts(0.35),
      sourceAssistantUuid: 'a-active-1',
      outputOmitted: false,
    },
  ],
  subagents: [
    {
      sessionId: 'sub-rich-1',
      rootTaskDescription: 'Audit IPC field mappings for fixture coverage',
      spawnTs: ts(0.35),
      endTs: ts(0.48),
      metrics: {
        inputTokens: 1800,
        outputTokens: 520,
        cacheCreationTokens: 120,
        cacheReadTokens: 2600,
        toolCount: 1,
        costUsd: null,
      },
      team: null,
      subagentType: 'general-purpose',
      messages: [subagentTraceChunk],
      mainSessionImpact: { totalTokens: 96 },
      isOngoing: false,
      durationMs: 7800,
      parentTaskId: 'task-sub-rich-1',
      description: 'Audit IPC field mappings for fixture coverage',
      headerModel: 'sonnet4.6',
      lastIsolatedTokens: 5040,
      messagesOmitted: false,
      messagesTotalCount: 1,
    },
  ],
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
  chunkId: 'a-active-2:0',
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
  metrics: { ...emptyMetrics(), toolCount: 1 },
  semanticSteps: [
    { kind: 'text', text: '继续往下', timestamp: ts(0.5) },
    { kind: 'tool_execution', toolUseId: 'tu-active-2', toolName: 'Bash', timestamp: ts(0.52) },
    { kind: 'interruption', text: '[Request interrupted by user]', timestamp: ts(0.55) },
  ],
  toolExecutions: [
    {
      toolUseId: 'tu-active-2',
      toolName: 'Bash',
      input: { command: 'pnpm --dir ui run check' },
      output: { kind: 'text', text: '0 errors' },
      isError: false,
      startTs: ts(0.52),
      endTs: ts(0.54),
      sourceAssistantUuid: 'a-active-2',
      outputOmitted: false,
      outputBytes: 8,
    },
  ],
  subagents: [],
  slashCommands: [],
}

const compactChunk: CompactChunk = {
  kind: 'compact',
  chunkId: 'c-active-1:0',
  uuid: 'c-active-1',
  timestamp: ts(0.6),
  durationMs: null,
  summaryText: '对话已 compact。包含历史摘要 + 新一阶段的开篇上下文。',
  metrics: emptyMetrics(),
  tokenDelta: {
    preCompactionTokens: 30000,
    postCompactionTokens: 5000,
    delta: -25000,
  },
  phaseNumber: 2,
}

const richChunks: Chunk[] = [userChunk, systemChunk, aiChunk, aiChunkInterrupted, compactChunk]

export const multiProjectRichFixture: Fixture = {
  name: 'multi-project-rich',
  projects: PROJECTS,
  sessions: {
    'mock-rich-rust': rustSessions,
    'mock-rich-rust-wt-feat': rustWtFeatSessions,
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
      // Latest phase（= phase 2）的累计 injections——保留以兼容旧前端
      contextInjections: [
        {
          category: 'claude-md',
          id: 'claude-user-p2',
          path: '/Users/mock/.claude/CLAUDE.md',
          displayName: 'CLAUDE.md',
          scope: 'user',
          estimatedTokens: 1800,
          firstSeenTurnIndex: 0,
        },
        {
          category: 'claude-md',
          id: 'claude-project-p2',
          path: '/Users/mock/rust-port/CLAUDE.md',
          displayName: 'CLAUDE.md',
          scope: 'project',
          estimatedTokens: 3200,
          firstSeenTurnIndex: 0,
        },
        {
          category: 'mentioned-file',
          id: 'mentioned-context-panel',
          path: '/Users/mock/rust-port/ui/src/components/ContextPanel.svelte',
          displayName: 'ContextPanel.svelte',
          estimatedTokens: 740,
          firstSeenTurnIndex: 0,
          firstSeenInGroup: 'a-active-2:0',
          exists: true,
        },
        {
          category: 'tool-output',
          id: 'to-active-2',
          turnIndex: 2,
          aiGroupId: 'a-active-2:0',
          estimatedTokens: 480,
          toolCount: 1,
          toolBreakdown: [
            { toolName: 'Bash', tokenCount: 320, isError: false, toolUseId: 'tu-active-2' },
          ],
        },
        {
          category: 'thinking-text',
          id: 'tt-active-2',
          turnIndex: 2,
          aiGroupId: 'a-active-2:0',
          estimatedTokens: 220,
          breakdown: [
            { type: 'thinking', tokenCount: 150 },
            { type: 'text', tokenCount: 70 },
          ],
        },
        {
          category: 'task-coordination',
          id: 'tc-active-2',
          turnIndex: 2,
          aiGroupId: 'a-active-2:0',
          estimatedTokens: 95,
          breakdown: [
            { type: 'task-tool', toolName: 'Task', tokenCount: 60, label: 'Task #1: rename audit' },
            { type: 'send-message', toolName: 'SendMessage', tokenCount: 35, label: 'SendMessage #1' },
          ],
        },
        {
          category: 'user-message',
          id: 'um-active-2',
          turnIndex: 2,
          aiGroupId: 'a-active-2:0',
          estimatedTokens: 18,
          textPreview: '继续往下',
        },
      ],
      injectionsByPhase: {
        '1': [
          {
            category: 'claude-md',
            id: 'claude-user-p1',
            path: '/Users/mock/.claude/CLAUDE.md',
            displayName: 'CLAUDE.md',
            scope: 'user',
            estimatedTokens: 1800,
            firstSeenTurnIndex: 0,
          },
          {
            category: 'claude-md',
            id: 'claude-project-p1',
            path: '/Users/mock/rust-port/CLAUDE.md',
            displayName: 'CLAUDE.md',
            scope: 'project',
            estimatedTokens: 3200,
            firstSeenTurnIndex: 0,
          },
          {
            category: 'claude-md',
            id: 'claude-ui-p1',
            path: '/Users/mock/rust-port/ui/CLAUDE.md',
            displayName: 'CLAUDE.md',
            scope: 'directory',
            estimatedTokens: 960,
            firstSeenTurnIndex: 0,
          },
          {
            category: 'user-message',
            id: 'um-active-1',
            turnIndex: 0,
            aiGroupId: 'a-active-1:0',
            estimatedTokens: 24,
            textPreview: 'LocalDataApi 的 list_sessions 用 camelCase 还是 snake_case？',
          },
          {
            category: 'tool-output',
            id: 'to-active-1',
            turnIndex: 0,
            aiGroupId: 'a-active-1:0',
            estimatedTokens: 280,
            toolCount: 1,
            toolBreakdown: [
              { toolName: 'Grep', tokenCount: 280, isError: false, toolUseId: 'tu-active-1' },
            ],
          },
        ],
        '2': [
          {
            category: 'claude-md',
            id: 'claude-user-p2',
            path: '/Users/mock/.claude/CLAUDE.md',
            displayName: 'CLAUDE.md',
            scope: 'user',
            estimatedTokens: 1800,
            firstSeenTurnIndex: 0,
          },
          {
            category: 'claude-md',
            id: 'claude-project-p2',
            path: '/Users/mock/rust-port/CLAUDE.md',
            displayName: 'CLAUDE.md',
            scope: 'project',
            estimatedTokens: 3200,
            firstSeenTurnIndex: 0,
          },
          {
            category: 'mentioned-file',
            id: 'mentioned-context-panel',
            path: '/Users/mock/rust-port/ui/src/components/ContextPanel.svelte',
            displayName: 'ContextPanel.svelte',
            estimatedTokens: 740,
            firstSeenTurnIndex: 0,
            firstSeenInGroup: 'a-active-2:0',
            exists: true,
          },
          {
            category: 'tool-output',
            id: 'to-active-2',
            turnIndex: 2,
            aiGroupId: 'a-active-2:0',
            estimatedTokens: 480,
            toolCount: 1,
            toolBreakdown: [
              { toolName: 'Bash', tokenCount: 320, isError: false, toolUseId: 'tu-active-2' },
            ],
          },
          {
            category: 'thinking-text',
            id: 'tt-active-2',
            turnIndex: 2,
            aiGroupId: 'a-active-2:0',
            estimatedTokens: 220,
            breakdown: [
              { type: 'thinking', tokenCount: 150 },
              { type: 'text', tokenCount: 70 },
            ],
          },
          {
            category: 'task-coordination',
            id: 'tc-active-2',
            turnIndex: 2,
            aiGroupId: 'a-active-2:0',
            estimatedTokens: 95,
            breakdown: [
              { type: 'task-tool', toolName: 'Task', tokenCount: 60, label: 'Task #1: rename audit' },
              { type: 'send-message', toolName: 'SendMessage', tokenCount: 35, label: 'SendMessage #1' },
            ],
          },
          {
            category: 'user-message',
            id: 'um-active-2',
            turnIndex: 2,
            aiGroupId: 'a-active-2:0',
            estimatedTokens: 18,
            textPreview: '继续往下',
          },
        ],
      },
      phaseInfo: {
        phases: [
          { phaseNumber: 1, firstAiGroupId: 'a-active-1:0', lastAiGroupId: 'a-active-1:0' },
          {
            phaseNumber: 2,
            firstAiGroupId: 'a-active-2:0',
            lastAiGroupId: 'a-active-2:0',
            compactGroupId: 'c-active-1:0',
          },
        ],
        compactionCount: 1,
        aiGroupPhaseMap: { 'a-active-1:0': 1, 'a-active-2:0': 2 },
        compactionTokenDeltas: {
          'c-active-1:0': { preCompactionTokens: 6304, postCompactionTokens: 5553, delta: -751 },
        },
      },
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
  memories: {
    'mock-rich-rust': {
      projectId: 'mock-rich-rust',
      hasMemory: true,
      count: 3,
      defaultFile: 'MEMORY.md',
      layers: [
        { file: 'MEMORY.md', title: 'Index', hook: 'MEMORY.md', kind: 'index' },
        {
          file: 'feedback_chinese_language.md',
          title: '始终使用中文',
          hook: '对话/注释/文档全部简体中文',
          kind: 'entry',
        },
        {
          file: 'project_ui_todo.md',
          title: 'UI 功能路线图',
          hook: '剩余待办等痛点再做',
          kind: 'orphan',
        },
      ],
    },
    'mock-rich-rust-wt-feat': {
      projectId: 'mock-rich-rust-wt-feat',
      hasMemory: false,
      count: 0,
      defaultFile: null,
      layers: [],
    },
  },
  memoryFiles: {
    'mock-rich-rust:MEMORY.md': '- [始终使用中文](feedback_chinese_language.md) — 对话/注释/文档全部简体中文\n- [UI 功能路线图](project_ui_todo.md) — 剩余待办等痛点再做\n',
    'mock-rich-rust:feedback_chinese_language.md': '# 始终使用中文\n\n对话、注释、文档和 OpenSpec 产物全部使用简体中文。\n',
    'mock-rich-rust:project_ui_todo.md': '# UI 功能路线图\n\n- 数据层完成\n- Memory 查看功能对齐原版\n',
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
      claudeRootPath: null,
      autoExpandAiGroups: false,
      sessionClickBehavior: 'replace',
    },
    display: {
      fontSans: null,
      fontMono: null,
      timeFormat: '24h',
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
  repositoryGroups: [
    {
      id: 'mock-rich-repo-rust',
      identity: { id: 'mock-rich-repo-rust', name: 'rust-port' },
      name: 'rust-port',
      worktrees: [
        {
          id: 'mock-rich-rust',
          path: '/Users/test/rust-port',
          name: 'rust-port',
          gitBranch: 'main',
          isMainWorktree: true,
          sessions: rustSessions.map((s) => s.sessionId),
          createdAt: null,
          mostRecentSession: rustSessions[0].timestamp,
        },
        {
          id: 'mock-rich-rust-wt-feat',
          path: '/Users/test/rust-port/.claude/worktrees/feat-x',
          name: 'feat-x',
          gitBranch: 'feat/x',
          isMainWorktree: false,
          sessions: rustWtFeatSessions.map((s) => s.sessionId),
          createdAt: null,
          mostRecentSession: rustWtFeatSessions[0].timestamp,
        },
      ],
      mostRecentSession: rustSessions[0].timestamp,
      totalSessions: rustSessions.length + rustWtFeatSessions.length,
    },
    {
      id: 'mock-rich-ts',
      identity: null,
      name: 'claude-devtools',
      worktrees: [
        {
          id: 'mock-rich-ts',
          path: '/Users/test/claude-devtools',
          name: 'claude-devtools',
          gitBranch: null,
          isMainWorktree: true,
          sessions: tsSessions.map((s) => s.sessionId),
          createdAt: null,
          mostRecentSession: tsSessions[0].timestamp,
        },
      ],
      mostRecentSession: tsSessions[0].timestamp,
      totalSessions: tsSessions.length,
    },
    {
      id: 'mock-rich-docs',
      identity: null,
      name: 'docs',
      worktrees: [
        {
          id: 'mock-rich-docs',
          path: '/Users/test/docs',
          name: 'docs',
          gitBranch: null,
          isMainWorktree: true,
          sessions: docsSessions.map((s) => s.sessionId),
          createdAt: null,
          mostRecentSession: docsSessions[0].timestamp,
        },
      ],
      mostRecentSession: docsSessions[0].timestamp,
      totalSessions: docsSessions.length,
    },
    {
      id: 'mock-rich-experiment',
      identity: null,
      name: 'experiment',
      worktrees: [
        {
          id: 'mock-rich-experiment',
          path: '/Users/test/experiment',
          name: 'experiment',
          gitBranch: null,
          isMainWorktree: true,
          sessions: experimentSessions.map((s) => s.sessionId),
          createdAt: null,
          mostRecentSession: experimentSessions[0].timestamp,
        },
      ],
      mostRecentSession: experimentSessions[0].timestamp,
      totalSessions: experimentSessions.length,
    },
    {
      id: 'mock-rich-archive',
      identity: null,
      name: 'archive',
      worktrees: [
        {
          id: 'mock-rich-archive',
          path: '/Users/test/archive',
          name: 'archive',
          gitBranch: null,
          isMainWorktree: true,
          sessions: archiveSessions.map((s) => s.sessionId),
          createdAt: null,
          mostRecentSession: archiveSessions[0].timestamp,
        },
      ],
      mostRecentSession: archiveSessions[0].timestamp,
      totalSessions: archiveSessions.length,
    },
  ],
}
