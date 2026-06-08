import { describe, test, expect } from "vitest";
import { exportSession } from "./index";
import type { SessionDetail, AIChunk, UserChunk, SystemChunk, CompactChunk, SemanticStep, ToolExecution } from "../api";

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
