import type {
  AIChunk,
  Chunk,
  UserChunk,
  WorkflowItem,
} from '../api'
import type { Fixture } from './types'

const TS_BASE = 1_712_900_000_000

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

// ---------------------------------------------------------------------------
// 4 种 workflow 变体
// ---------------------------------------------------------------------------

const workflowCompleted: WorkflowItem = {
  runId: 'wf-run-001',
  name: 'deploy-pipeline',
  status: 'completed',
  phases: [
    { index: 0, title: 'Build' },
    { index: 1, title: 'Test' },
  ],
  agents: [
    { label: 'builder-1', phaseIndex: 0, state: 'completed', tokens: 12400, toolCalls: 8, durationMs: 45000 },
    { label: 'builder-2', phaseIndex: 0, state: 'completed', tokens: 9800, toolCalls: 5, durationMs: 38000 },
    { label: 'tester-1', phaseIndex: 1, state: 'completed', tokens: 6200, toolCalls: 12, durationMs: 62000 },
    { label: 'tester-2', phaseIndex: 1, state: 'completed', tokens: 7100, toolCalls: 9, durationMs: 55000 },
  ],
  totalTokens: 35500,
  durationMs: 162000,
  scriptPreview: '#!/bin/bash\nset -e\npnpm install && pnpm build\npnpm test --coverage',
}

const workflowPartialFailure: WorkflowItem = {
  runId: 'wf-run-002',
  name: 'integration-suite',
  status: 'partial_failure',
  phases: [
    { index: 0, title: 'Prepare' },
    { index: 1, title: 'Run Integration' },
    { index: 2, title: 'Cleanup' },
    { index: 3, title: 'Report' },
  ],
  agents: [
    { label: 'prep-agent', phaseIndex: 0, state: 'completed', tokens: 3200, toolCalls: 4, durationMs: 12000 },
    { label: 'integration-1', phaseIndex: 1, state: 'completed', tokens: 18000, toolCalls: 22, durationMs: 95000 },
    { label: 'integration-2', phaseIndex: 1, state: 'failed', tokens: 14500, toolCalls: 18, durationMs: 78000, resultPreview: 'AssertionError: expected 200 got 503' },
    { label: 'integration-3', phaseIndex: 1, state: 'completed', tokens: 16200, toolCalls: 20, durationMs: 88000 },
    { label: 'cleanup-agent', phaseIndex: 2, state: 'completed', tokens: 2100, toolCalls: 3, durationMs: 8000 },
    { label: 'report-agent', phaseIndex: 3, state: 'failed', tokens: 1800, toolCalls: 2, durationMs: 5000, resultPreview: 'Timeout: report generation exceeded 5s' },
  ],
  totalTokens: 55800,
  durationMs: 286000,
}

const workflowRunning: WorkflowItem = {
  runId: 'wf-run-003',
  name: 'analysis-pipeline',
  status: 'running',
  phases: [],
  agents: [],
}

const workflowEmpty: WorkflowItem = {
  runId: 'wf-run-004',
  name: 'empty-workflow',
  status: 'completed',
  phases: [{ index: 0, title: 'Init' }],
  agents: [],
  totalTokens: 0,
  durationMs: 200,
}

// ---------------------------------------------------------------------------
// Session chunks 构建
// ---------------------------------------------------------------------------

const userChunk1: UserChunk = {
  kind: 'user',
  chunkId: 'wf-u1:0',
  uuid: 'wf-u1',
  timestamp: ts(0),
  durationMs: null,
  content: 'Run the deploy pipeline',
  metrics: emptyMetrics(),
}

const aiChunkWithWorkflows: AIChunk = {
  kind: 'ai',
  chunkId: 'wf-a1:0',
  timestamp: ts(1),
  durationMs: 165000,
  responses: [
    {
      uuid: 'wf-a1-r1',
      timestamp: ts(1),
      content: 'Starting workflow execution with deploy-pipeline and integration-suite.',
      toolCalls: [],
      usage: {
        input_tokens: 2400,
        output_tokens: 680,
        cache_read_input_tokens: 1200,
        cache_creation_input_tokens: 100,
      },
      model: 'claude-sonnet-4-6',
    },
  ],
  metrics: {
    inputTokens: 2400,
    outputTokens: 680,
    cacheCreationTokens: 100,
    cacheReadTokens: 1200,
    toolCount: 0,
    costUsd: null,
  },
  semanticSteps: [
    { kind: 'tool_execution', toolUseId: 'wf-tool-1', toolName: 'Workflow', timestamp: ts(1) },
    { kind: 'tool_execution', toolUseId: 'wf-tool-2', toolName: 'Workflow', timestamp: ts(1.5) },
    { kind: 'text', text: 'Starting workflow execution with deploy-pipeline and integration-suite.', timestamp: ts(2) },
  ],
  toolExecutions: [
    {
      toolUseId: 'wf-tool-1',
      toolName: 'Workflow',
      input: { name: 'deploy-pipeline', run_id: 'wf-run-001' },
      output: { kind: 'text', text: 'Workflow completed successfully' },
      isError: false,
      startTs: ts(1),
      endTs: ts(2.7),
      sourceAssistantUuid: 'wf-a1-r1',
      workflowRunId: 'wf-run-001',
    },
    {
      toolUseId: 'wf-tool-2',
      toolName: 'Workflow',
      input: { name: 'integration-suite', run_id: 'wf-run-002' },
      output: { kind: 'text', text: '2 agents failed' },
      isError: false,
      startTs: ts(1.5),
      endTs: ts(4.8),
      sourceAssistantUuid: 'wf-a1-r1',
      workflowRunId: 'wf-run-002',
    },
  ],
  subagents: [],
  slashCommands: [],
}

const userChunk2: UserChunk = {
  kind: 'user',
  chunkId: 'wf-u2:0',
  uuid: 'wf-u2',
  timestamp: ts(5),
  durationMs: null,
  content: 'Now run the analysis pipeline',
  metrics: emptyMetrics(),
}

const aiChunkRunning: AIChunk = {
  kind: 'ai',
  chunkId: 'wf-a2:0',
  timestamp: ts(6),
  durationMs: null,
  responses: [
    {
      uuid: 'wf-a2-r1',
      timestamp: ts(6),
      content: 'Launching analysis pipeline...',
      toolCalls: [],
      usage: null,
      model: 'claude-sonnet-4-6',
    },
  ],
  metrics: emptyMetrics(),
  semanticSteps: [
    { kind: 'tool_execution', toolUseId: 'wf-tool-3', toolName: 'Workflow', timestamp: ts(6) },
    { kind: 'text', text: 'Launching analysis pipeline...', timestamp: ts(6.5) },
  ],
  toolExecutions: [
    {
      toolUseId: 'wf-tool-3',
      toolName: 'Workflow',
      input: { name: 'analysis-pipeline', run_id: 'wf-run-003' },
      output: { kind: 'missing' as const },
      isError: false,
      startTs: ts(6),
      endTs: null,
      sourceAssistantUuid: 'wf-a2-r1',
      workflowRunId: 'wf-run-003',
    },
  ],
  subagents: [],
  slashCommands: [],
}

const userChunk3: UserChunk = {
  kind: 'user',
  chunkId: 'wf-u3:0',
  uuid: 'wf-u3',
  timestamp: ts(10),
  durationMs: null,
  content: 'Also run the empty workflow test',
  metrics: emptyMetrics(),
}

const aiChunkEmpty: AIChunk = {
  kind: 'ai',
  chunkId: 'wf-a3:0',
  timestamp: ts(11),
  durationMs: 200,
  responses: [
    {
      uuid: 'wf-a3-r1',
      timestamp: ts(11),
      content: 'Empty workflow completed (no agents spawned).',
      toolCalls: [],
      usage: {
        input_tokens: 800,
        output_tokens: 120,
        cache_read_input_tokens: 400,
        cache_creation_input_tokens: 0,
      },
      model: 'claude-sonnet-4-6',
    },
  ],
  metrics: {
    inputTokens: 800,
    outputTokens: 120,
    cacheCreationTokens: 0,
    cacheReadTokens: 400,
    toolCount: 0,
    costUsd: null,
  },
  semanticSteps: [
    { kind: 'tool_execution', toolUseId: 'wf-tool-4', toolName: 'Workflow', timestamp: ts(11) },
    { kind: 'text', text: 'Empty workflow completed (no agents spawned).', timestamp: ts(11.1) },
  ],
  toolExecutions: [
    {
      toolUseId: 'wf-tool-4',
      toolName: 'Workflow',
      input: { name: 'empty-workflow', run_id: 'wf-run-004' },
      output: { kind: 'text', text: 'No agents spawned' },
      isError: false,
      startTs: ts(11),
      endTs: ts(11.01),
      sourceAssistantUuid: 'wf-a3-r1',
      workflowRunId: 'wf-run-004',
    },
  ],
  subagents: [],
  slashCommands: [],
}

// ---------------------------------------------------------------------------
// Launch error case: tool_result with is_error
// ---------------------------------------------------------------------------

const aiChunkLaunchError: AIChunk = {
  kind: 'ai',
  chunkId: 'wf-a4:0',
  timestamp: ts(15),
  durationMs: 1200,
  responses: [
    {
      uuid: 'wf-a4-r1',
      timestamp: ts(15),
      content: '',
      toolCalls: [
        {
          id: 'wf-tool-launch-1',
          name: 'Workflow',
          input: { name: 'broken-pipeline', run_id: 'wf-run-005' },
          isTask: false,
          taskDescription: null,
          taskSubagentType: null,
        },
      ],
      usage: null,
      model: 'claude-sonnet-4-6',
    },
  ],
  metrics: emptyMetrics(),
  semanticSteps: [
    { kind: 'tool_execution', toolUseId: 'wf-tool-launch-1', toolName: 'Workflow', timestamp: ts(15) },
  ],
  toolExecutions: [
    {
      toolUseId: 'wf-tool-launch-1',
      toolName: 'Workflow',
      input: { name: 'broken-pipeline', run_id: 'wf-run-005' },
      output: { kind: 'text', text: 'Error: workflow script not found at /scripts/broken.sh' },
      isError: true,
      errorMessage: 'workflow script not found at /scripts/broken.sh',
      startTs: ts(15),
      endTs: ts(15.02),
      sourceAssistantUuid: 'wf-a4-r1',
    },
  ],
  subagents: [],
  slashCommands: [],
}

const chunks: Chunk[] = [
  userChunk1,
  aiChunkWithWorkflows,
  userChunk2,
  aiChunkRunning,
  userChunk3,
  aiChunkEmpty,
  aiChunkLaunchError,
]

// ---------------------------------------------------------------------------
// Fixture 组装
// ---------------------------------------------------------------------------

export const workflowRichFixture: Fixture = {
  name: 'workflow-rich',
  projects: [
    {
      id: 'mock-wf-project',
      path: '/Users/test/workflow-demo',
      displayName: 'workflow-demo',
      sessionCount: 1,
    },
  ],
  sessions: {
    'mock-wf-project': [
      {
        sessionId: 'sess-wf-1',
        projectId: 'mock-wf-project',
        timestamp: TS_BASE,
        messageCount: 7,
        title: 'Workflow rendering test',
        isOngoing: true,
        gitBranch: 'feat/workflow-card-frontend',
      },
    ],
  },
  sessionDetails: {
    'mock-wf-project:sess-wf-1': {
      sessionId: 'sess-wf-1',
      projectId: 'mock-wf-project',
      chunks,
      metrics: { message_count: 7 },
      metadata: { last_modified: TS_BASE + 900_000, size: 42000, cwd: '/Users/test/workflow-demo' },
      contextInjections: [],
      isOngoing: true,
      title: 'Workflow rendering test',
      workflowItems: [workflowCompleted, workflowPartialFailure, workflowRunning, workflowEmpty],
    },
  },
  prefs: {
    'mock-wf-project': { pinned: [], hidden: [] },
  },
  config: {
    general: {
      launchAtLogin: false,
      showDockIcon: true,
      theme: 'dark',
      defaultTab: 'sessions',
      claudeRootPath: null,
      autoExpandAiGroups: true,
    },
    notifications: {
      enabled: true,
      soundEnabled: false,
      triggers: [],
    },
    keyboardShortcuts: {},
  },
  notifications: { notifications: [], total: 0, totalCount: 0, unreadCount: 0, hasMore: false },
  agentConfigs: [],
  searchResults: [],
}
