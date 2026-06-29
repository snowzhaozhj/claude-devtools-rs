import { describe, test, expect } from "vitest";
import { exportSession } from "./index";
import { exportAsMarkdown } from "./markdownExporter";
import { projectSessionDetail } from "./projection";
import type { ExportOptions } from "./types";
import type { SessionDetail, AIChunk, UserChunk, SystemChunk, CompactChunk, SemanticStep, ToolExecution, SubagentProcess } from "../api";

function makeMinimalDetail(overrides?: Partial<SessionDetail>): SessionDetail {
  return {
    sessionId: "abc12345-def6-7890-abcd-ef1234567890",
    projectId: "project-001",
    chunks: [],
    metrics: { message_count: 0 },
    metadata: { last_modified: 1700000000000, size: 1024, cwd: "/home/user/project" },
    contextInjections: [],
    isOngoing: false,
    title: "Test Session",
    ...overrides,
  };
}

function makeUserChunk(text: string): UserChunk {
  return {
    kind: "user",
    chunkId: "u1",
    uuid: "u-uuid-1",
    timestamp: "2024-01-01T00:00:00Z",
    durationMs: null,
    content: text,
    metrics: { inputTokens: 10, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 0, costUsd: null },
  };
}

function makeAIChunk(textSteps: string[], toolExecs?: ToolExecution[]): AIChunk {
  const semanticSteps: SemanticStep[] = textSteps.map((t) => ({ kind: "text" as const, text: t, timestamp: "2024-01-01T00:00:01Z" }));
  for (const exec of toolExecs ?? []) {
    semanticSteps.push({ kind: "tool_execution", toolUseId: exec.toolUseId, toolName: exec.toolName, timestamp: exec.startTs });
  }
  return {
    kind: "ai",
    chunkId: "a1",
    timestamp: "2024-01-01T00:00:01Z",
    durationMs: 5000,
    responses: [],
    metrics: { inputTokens: 0, outputTokens: 100, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 0, costUsd: null },
    semanticSteps,
    toolExecutions: toolExecs ?? [],
    subagents: [],
    slashCommands: [],
  };
}

describe("Markdown exporter", () => {
  test("includes metadata table with session info", () => {
    const detail = makeMinimalDetail({ chunks: [makeUserChunk("hello")] });
    const md = exportSession(detail, "markdown");
    expect(md).toContain("# Test Session");
    expect(md).toContain("| Session ID |");
    expect(md).toContain("abc12345-def6-7890-abcd-ef1234567890");
    expect(md).toContain("/home/user/project");
  });

  test("renders user and assistant turns with headers", () => {
    const detail = makeMinimalDetail({
      chunks: [makeUserChunk("What is 2+2?"), makeAIChunk(["The answer is 4."])],
    });
    const md = exportSession(detail, "markdown");
    expect(md).toContain("## Turn 1 — User");
    expect(md).toContain("What is 2+2?");
    expect(md).toContain("## Turn 2 — Assistant");
    expect(md).toContain("The answer is 4.");
  });

  test("includes tool executions", () => {
    const toolExec: ToolExecution = {
      toolUseId: "t1",
      toolName: "Bash",
      input: { command: "ls -la" },
      output: { kind: "text", text: "total 0\ndrwxr-xr-x" },
      isError: false,
      startTs: "2024-01-01T00:00:01Z",
      endTs: "2024-01-01T00:00:02Z",
      sourceAssistantUuid: "a-uuid",
      outputOmitted: false,
    };
    const detail = makeMinimalDetail({
      chunks: [makeAIChunk(["Running command"], [toolExec])],
    });
    const md = exportSession(detail, "markdown");
    expect(md).toContain("### Tool: Bash");
    expect(md).toContain("$ ls -la");
  });
});

describe("JSON exporter", () => {
  test("outputs valid JSON with expected fields", () => {
    const detail = makeMinimalDetail({ chunks: [makeUserChunk("test")] });
    const json = exportSession(detail, "json");
    const parsed = JSON.parse(json);
    expect(parsed.sessionId).toBe("abc12345-def6-7890-abcd-ef1234567890");
    expect(parsed.chunks).toHaveLength(1);
    expect(parsed.chunks[0].kind).toBe("user");
  });

  test("projection filters out thinking when disabled", () => {
    const ai: AIChunk = {
      ...makeAIChunk(["visible text"]),
      semanticSteps: [
        { kind: "thinking", text: "secret thought", timestamp: "2024-01-01T00:00:01Z" },
        { kind: "text", text: "visible text", timestamp: "2024-01-01T00:00:01Z" },
      ],
    };
    const detail = makeMinimalDetail({ chunks: [ai] });
    const json = exportSession(detail, "json");
    const parsed = JSON.parse(json);
    expect(parsed.chunks[0].semanticSteps).toHaveLength(2);
  });
});

describe("HTML exporter", () => {
  test("generates valid self-contained HTML", () => {
    const detail = makeMinimalDetail({ chunks: [makeUserChunk("hello world")] });
    const html = exportSession(detail, "html");
    expect(html).toContain("<!DOCTYPE html>");
    expect(html).toContain("<html");
    expect(html).toContain("</html>");
    expect(html).toContain("<style>");
    expect(html).toContain("<script>");
  });

  test("includes CSP meta tag", () => {
    const detail = makeMinimalDetail();
    const html = exportSession(detail, "html");
    expect(html).toContain("Content-Security-Policy");
    expect(html).toContain("default-src 'none'");
  });

  test("includes theme toggle and TOC", () => {
    const detail = makeMinimalDetail({
      chunks: [makeUserChunk("q1"), makeAIChunk(["a1"])],
    });
    const html = exportSession(detail, "html");
    expect(html).toContain("theme-toggle");
    expect(html).toContain("class=\"toc\"");
    expect(html).toContain("1. User");
    expect(html).toContain("2. Assistant");
  });

  test("renders chunk content in HTML", () => {
    const detail = makeMinimalDetail({
      chunks: [makeUserChunk("Tell me about Rust")],
    });
    const html = exportSession(detail, "html");
    expect(html).toContain("Tell me about Rust");
  });
});

describe("HTML XSS protection", () => {
  test("removes or escapes script tags in user messages", () => {
    const malicious = '<script>alert("xss")</script>';
    const detail = makeMinimalDetail({ chunks: [makeUserChunk(malicious)] });
    const html = exportSession(detail, "html");
    expect(html).not.toContain('<script>alert("xss")</script>');
  });

  test("escapes event handlers in content", () => {
    const malicious = '<img src=x onerror="alert(1)">';
    const detail = makeMinimalDetail({ chunks: [makeUserChunk(malicious)] });
    const html = exportSession(detail, "html");
    expect(html).not.toContain('onerror="alert');
  });

  test("escapes HTML in session title", () => {
    const detail = makeMinimalDetail({ title: '<script>evil</script>' });
    const html = exportSession(detail, "html");
    expect(html).toContain("&lt;script&gt;evil&lt;/script&gt;");
    expect(html).not.toContain("<script>evil</script>");
  });

  test("escapes HTML in tool names", () => {
    const toolExec: ToolExecution = {
      toolUseId: "t1",
      toolName: '<img src=x onerror="alert(1)">',
      input: {},
      output: { kind: "text", text: "output" },
      isError: false,
      startTs: "2024-01-01T00:00:01Z",
      endTs: "2024-01-01T00:00:02Z",
      sourceAssistantUuid: "a-uuid",
      outputOmitted: false,
    };
    const detail = makeMinimalDetail({
      chunks: [makeAIChunk(["test"], [toolExec])],
    });
    const html = exportSession(detail, "html");
    expect(html).not.toContain('onerror="alert');
    expect(html).toContain("&lt;img");
  });

  test("escapes HTML in metadata cwd", () => {
    const detail = makeMinimalDetail();
    detail.metadata.cwd = '"><script>alert(1)</script>';
    const html = exportSession(detail, "html");
    expect(html).not.toContain("<script>alert(1)</script>");
  });
});

describe("Bug 1: chronological ordering", () => {
  function makeChronologicalAIChunk(): AIChunk {
    const toolExec: ToolExecution = {
      toolUseId: "t1",
      toolName: "Bash",
      input: { command: "echo hello" },
      output: { kind: "text", text: "hello" },
      isError: false,
      startTs: "2024-01-01T00:00:02Z",
      endTs: "2024-01-01T00:00:03Z",
      sourceAssistantUuid: "a-uuid",
      outputOmitted: false,
    };
    const steps: SemanticStep[] = [
      { kind: "text", text: "Text before tool", timestamp: "2024-01-01T00:00:01Z" },
      { kind: "tool_execution", toolUseId: "t1", toolName: "Bash", timestamp: "2024-01-01T00:00:02Z" },
      { kind: "text", text: "Text after tool (final)", timestamp: "2024-01-01T00:00:04Z" },
    ];
    return {
      kind: "ai",
      chunkId: "a1",
      timestamp: "2024-01-01T00:00:01Z",
      durationMs: 5000,
      responses: [],
      metrics: { inputTokens: 0, outputTokens: 100, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 1, costUsd: null },
      semanticSteps: steps,
      toolExecutions: [toolExec],
      subagents: [],
      slashCommands: [],
    };
  }

  test("markdown: tool call appears between text A and text B, not after B", () => {
    const detail = makeMinimalDetail({ chunks: [makeChronologicalAIChunk()] });
    const md = exportSession(detail, "markdown");
    const textBeforeIdx = md.indexOf("Text before tool");
    const toolIdx = md.indexOf("### Tool: Bash");
    const textAfterIdx = md.indexOf("Text after tool (final)");
    expect(textBeforeIdx).toBeGreaterThan(-1);
    expect(toolIdx).toBeGreaterThan(-1);
    expect(textAfterIdx).toBeGreaterThan(-1);
    expect(textBeforeIdx).toBeLessThan(toolIdx);
    expect(toolIdx).toBeLessThan(textAfterIdx);
  });

  test("html: tool call appears between text A and text B, not after B", () => {
    const detail = makeMinimalDetail({ chunks: [makeChronologicalAIChunk()] });
    const html = exportSession(detail, "html");
    const textBeforeIdx = html.indexOf("Text before tool");
    const toolIdx = html.indexOf("Bash");
    const textAfterIdx = html.indexOf("Text after tool (final)");
    expect(textBeforeIdx).toBeGreaterThan(-1);
    expect(toolIdx).toBeGreaterThan(-1);
    expect(textAfterIdx).toBeGreaterThan(-1);
    expect(textBeforeIdx).toBeLessThan(toolIdx);
    expect(toolIdx).toBeLessThan(textAfterIdx);
  });

  test("subagent card appears at spawn position, not at end", () => {
    const sub: SubagentProcess = {
      sessionId: "sub-1",
      rootTaskDescription: null,
      spawnTs: "2024-01-01T00:00:02Z",
      endTs: "2024-01-01T00:00:05Z",
      metrics: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 0, costUsd: null },
      team: null,
      subagentType: "code-reviewer",
      messages: [],
      mainSessionImpact: null,
      isOngoing: false,
      durationMs: 3000,
      parentTaskId: null,
      description: "Review code",
    };
    const steps: SemanticStep[] = [
      { kind: "text", text: "Before subagent", timestamp: "2024-01-01T00:00:01Z" },
      { kind: "subagent_spawn", placeholderId: "sub-1", timestamp: "2024-01-01T00:00:02Z" },
      { kind: "text", text: "After subagent final", timestamp: "2024-01-01T00:00:06Z" },
    ];
    const ai: AIChunk = {
      kind: "ai",
      chunkId: "a1",
      timestamp: "2024-01-01T00:00:01Z",
      durationMs: 5000,
      responses: [],
      metrics: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 0, costUsd: null },
      semanticSteps: steps,
      toolExecutions: [],
      subagents: [sub],
      slashCommands: [],
    };
    const detail = makeMinimalDetail({ chunks: [ai] });
    const md = exportSession(detail, "markdown");
    const beforeIdx = md.indexOf("Before subagent");
    const subIdx = md.indexOf("Review code");
    const afterIdx = md.indexOf("After subagent final");
    expect(beforeIdx).toBeLessThan(subIdx);
    expect(subIdx).toBeLessThan(afterIdx);
  });
});

describe("Bug 2: tool output completeness", () => {
  test("markdown: full mode renders tool output content", () => {
    const toolExec: ToolExecution = {
      toolUseId: "t1",
      toolName: "Read",
      input: { file_path: "/tmp/test.txt" },
      output: { kind: "text", text: "file contents here" },
      isError: false,
      startTs: "2024-01-01T00:00:01Z",
      endTs: "2024-01-01T00:00:02Z",
      sourceAssistantUuid: "a-uuid",
      outputOmitted: false,
    };
    const ai: AIChunk = {
      kind: "ai",
      chunkId: "a1",
      timestamp: "2024-01-01T00:00:01Z",
      durationMs: 1000,
      responses: [],
      metrics: { inputTokens: 0, outputTokens: 50, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 1, costUsd: null },
      semanticSteps: [
        { kind: "tool_execution", toolUseId: "t1", toolName: "Read", timestamp: "2024-01-01T00:00:01Z" },
      ],
      toolExecutions: [toolExec],
      subagents: [],
      slashCommands: [],
    };
    const detail = makeMinimalDetail({ chunks: [ai] });
    const md = exportSession(detail, "markdown");
    expect(md).toContain("file contents here");
  });
});

describe("D7: includeSubagents=false with Task tool", () => {
  test("Task tool still renders when subagents disabled", () => {
    const taskExec: ToolExecution = {
      toolUseId: "task-1",
      toolName: "Task",
      input: { description: "Do something" },
      output: { kind: "text", text: "task result" },
      isError: false,
      startTs: "2024-01-01T00:00:02Z",
      endTs: "2024-01-01T00:00:10Z",
      sourceAssistantUuid: "a-uuid",
      outputOmitted: false,
    };
    const sub: SubagentProcess = {
      sessionId: "sub-1",
      rootTaskDescription: null,
      spawnTs: "2024-01-01T00:00:02Z",
      endTs: "2024-01-01T00:00:10Z",
      metrics: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 0, costUsd: null },
      team: null,
      subagentType: null,
      messages: [],
      mainSessionImpact: null,
      isOngoing: false,
      durationMs: 8000,
      parentTaskId: "task-1",
      description: "Do something",
    };
    const steps: SemanticStep[] = [
      { kind: "tool_execution", toolUseId: "task-1", toolName: "Task", timestamp: "2024-01-01T00:00:02Z" },
      { kind: "subagent_spawn", placeholderId: "sub-1", timestamp: "2024-01-01T00:00:02Z" },
    ];
    const ai: AIChunk = {
      kind: "ai",
      chunkId: "a1",
      timestamp: "2024-01-01T00:00:01Z",
      durationMs: 10000,
      responses: [],
      metrics: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 1, costUsd: null },
      semanticSteps: steps,
      toolExecutions: [taskExec],
      subagents: [sub],
      slashCommands: [],
    };
    const detail = makeMinimalDetail({ chunks: [ai] });
    const opts: ExportOptions = { format: "markdown", includeThinking: true, toolOutputMode: "full", toolOutputMaxLength: 2000, includeSubagents: false };
    const md = exportAsMarkdown(detail, opts);
    expect(md).toContain("### Tool: Task");
    expect(md).not.toContain("Subagent:");
  });
});

describe("issue #534: missing display items", () => {
  function aiWith(overrides: Partial<AIChunk>): AIChunk {
    return {
      kind: "ai",
      chunkId: "a1",
      timestamp: "2024-01-01T00:00:01Z",
      durationMs: 1000,
      responses: [],
      metrics: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 0, costUsd: null },
      semanticSteps: [],
      toolExecutions: [],
      subagents: [],
      slashCommands: [],
      teammateMessages: [],
      ...overrides,
    };
  }

  test("markdown renders slash command with args + instructions", () => {
    const ai = aiWith({
      slashCommands: [{ name: "review", message: null, args: "PR 123", messageUuid: "su-1", timestamp: "2024-01-01T00:00:01Z", instructions: "审查指令文本" }],
    });
    const md = exportSession(makeMinimalDetail({ chunks: [ai] }), "markdown");
    expect(md).toContain("### Slash: /review");
    expect(md).toContain("PR 123");
    expect(md).toContain("审查指令文本");
  });

  test("markdown renders teammate message", () => {
    const ai = aiWith({
      teammateMessages: [{ uuid: "tm-1", teammateId: "member-1", color: null, summary: null, body: "队友消息内容", timestamp: "2024-01-01T00:00:01Z", isNoise: false, isResend: false }],
    });
    const md = exportSession(makeMinimalDetail({ chunks: [ai] }), "markdown");
    expect(md).toContain("### Teammate: member-1");
    expect(md).toContain("队友消息内容");
  });

  test("markdown renders teammate spawn, not as plain tool", () => {
    const spawnExec: ToolExecution = {
      toolUseId: "tu-spawn", toolName: "Agent", input: {}, output: { kind: "missing" },
      isError: false, startTs: "2024-01-01T00:00:01Z", endTs: null, sourceAssistantUuid: "a-uuid",
      outputOmitted: false, teammateSpawn: { name: "member-2", color: null },
    };
    const ai = aiWith({
      semanticSteps: [{ kind: "tool_execution", toolUseId: "tu-spawn", toolName: "Agent", timestamp: "2024-01-01T00:00:01Z" }],
      toolExecutions: [spawnExec],
    });
    const md = exportSession(makeMinimalDetail({ chunks: [ai] }), "markdown");
    expect(md).toContain("teammate spawned");
    expect(md).toContain("member-2");
    expect(md).not.toContain("### Tool: Agent");
  });

  test("markdown renders workflow summary once, deduped, not as plain tool", () => {
    const wfExec = (id: string): ToolExecution => ({
      toolUseId: id, toolName: "Workflow", input: {}, output: { kind: "missing" },
      isError: false, startTs: "2024-01-01T00:00:01Z", endTs: null, sourceAssistantUuid: "a-uuid",
      outputOmitted: false, workflowRunId: "wf_1",
    });
    const ai = aiWith({
      semanticSteps: [
        { kind: "tool_execution", toolUseId: "w1", toolName: "Workflow", timestamp: "2024-01-01T00:00:01Z" },
        { kind: "tool_execution", toolUseId: "w2", toolName: "Workflow", timestamp: "2024-01-01T00:00:02Z" },
      ],
      toolExecutions: [wfExec("w1"), wfExec("w2")],
    });
    const detail = makeMinimalDetail({
      chunks: [ai],
      workflowItems: [{ runId: "wf_1", name: "review-pr", status: "completed", phases: [], agents: [] }],
    });
    const md = exportSession(detail, "markdown");
    expect(md).toContain("### Workflow: review-pr");
    expect(md.match(/### Workflow:/g)?.length).toBe(1);
    expect(md).not.toContain("### Tool: Workflow");
  });

  test("markdown renders subagent inner conversation", () => {
    const inner: AIChunk = aiWith({
      semanticSteps: [{ kind: "text", text: "inner subagent reply", timestamp: "2024-01-01T00:00:03Z" }],
    });
    const sub: SubagentProcess = {
      sessionId: "sub-1", rootTaskDescription: null, spawnTs: "2024-01-01T00:00:02Z", endTs: "2024-01-01T00:00:05Z",
      metrics: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 0, costUsd: null },
      team: null, subagentType: "code-reviewer", messages: [inner], mainSessionImpact: null, isOngoing: false,
      durationMs: 3000, parentTaskId: null, description: "Review code",
    };
    const ai = aiWith({
      semanticSteps: [{ kind: "subagent_spawn", placeholderId: "sub-1", timestamp: "2024-01-01T00:00:02Z" }],
      subagents: [sub],
    });
    const md = exportSession(makeMinimalDetail({ chunks: [ai] }), "markdown");
    expect(md).toContain("### Subagent: Review code");
    expect(md).toContain("inner subagent reply");
  });

  test("markdown marks omitted subagent messages", () => {
    const sub: SubagentProcess = {
      sessionId: "sub-1", rootTaskDescription: null, spawnTs: "2024-01-01T00:00:02Z", endTs: null,
      metrics: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, toolCount: 0, costUsd: null },
      team: null, subagentType: null, messages: [], mainSessionImpact: null, isOngoing: false,
      durationMs: null, parentTaskId: null, description: "Big agent", messagesOmitted: true,
    };
    const ai = aiWith({
      semanticSteps: [{ kind: "subagent_spawn", placeholderId: "sub-1", timestamp: "2024-01-01T00:00:02Z" }],
      subagents: [sub],
    });
    const md = exportSession(makeMinimalDetail({ chunks: [ai] }), "markdown");
    expect(md).toContain("内部对话已省略");
  });

  test("html renders slash / teammate / workflow", () => {
    const wfExec: ToolExecution = {
      toolUseId: "w1", toolName: "Workflow", input: {}, output: { kind: "missing" },
      isError: false, startTs: "2024-01-01T00:00:01Z", endTs: null, sourceAssistantUuid: "a-uuid",
      outputOmitted: false, workflowRunId: "wf_1",
    };
    const ai = aiWith({
      slashCommands: [{ name: "review", message: null, args: "PR 9", messageUuid: "su-1", timestamp: "2024-01-01T00:00:01Z", instructions: null }],
      teammateMessages: [{ uuid: "tm-1", teammateId: "member-1", color: null, summary: null, body: "hello team", timestamp: "2024-01-01T00:00:01Z", isNoise: false, isResend: false }],
      semanticSteps: [{ kind: "tool_execution", toolUseId: "w1", toolName: "Workflow", timestamp: "2024-01-01T00:00:01Z" }],
      toolExecutions: [wfExec],
    });
    const detail = makeMinimalDetail({
      chunks: [ai],
      workflowItems: [{ runId: "wf_1", name: "ship-it", status: "running", phases: [], agents: [] }],
    });
    const html = exportSession(detail, "html");
    expect(html).toContain("/review");
    expect(html).toContain("member-1");
    expect(html).toContain("ship-it");
    expect(html).not.toContain("tool-header\">Workflow");
  });
});
