import { describe, expect, test } from "vitest";
import { buildDisplayItems, buildSummary } from "./displayItemBuilder";
import type { AIChunk } from "./api";

function makeChunkWithWorkflowTool(): AIChunk {
  return {
    kind: "ai",
    chunkId: "wf-chunk-1:0",
    timestamp: "2024-01-01T00:00:00.000Z",
    durationMs: 1000,
    responses: [
      {
        uuid: "wf-resp-1",
        timestamp: "2024-01-01T00:00:00.000Z",
        content: "Running workflow...",
        toolCalls: [],
        usage: null,
        model: "claude-sonnet-4-6",
      },
    ],
    metrics: {
      inputTokens: 100,
      outputTokens: 50,
      cacheCreationTokens: 0,
      cacheReadTokens: 0,
      toolCount: 1,
      costUsd: null,
    },
    semanticSteps: [
      {
        kind: "tool_execution",
        toolUseId: "wf-tool-1",
        toolName: "Workflow",
        timestamp: "2024-01-01T00:00:00.000Z",
      },
      {
        kind: "text",
        text: "Running workflow...",
        timestamp: "2024-01-01T00:00:01.000Z",
      },
    ],
    toolExecutions: [
      {
        toolUseId: "wf-tool-1",
        toolName: "Workflow",
        input: { name: "deploy-pipeline", run_id: "wf-run-001" },
        output: { kind: "text", text: "completed" },
        isError: false,
        startTs: "2024-01-01T00:00:00.000Z",
        endTs: "2024-01-01T00:00:02.000Z",
        sourceAssistantUuid: "wf-resp-1",
        workflowRunId: "wf-run-001",
      },
    ],
    subagents: [],
    slashCommands: [],
  };
}

describe("buildDisplayItems — workflow tool execution", () => {
  test("Workflow tool execution appears as tool DisplayItem (rendering matches at component level)", () => {
    const chunk = makeChunkWithWorkflowTool();
    const { items } = buildDisplayItems(chunk);

    const toolItems = items.filter((i) => i.type === "tool");
    expect(toolItems).toHaveLength(1);
    expect(toolItems[0].type === "tool" && toolItems[0].execution.workflowRunId).toBe("wf-run-001");
  });

  test("tool item with workflowRunId preserves toolName as Workflow", () => {
    const chunk = makeChunkWithWorkflowTool();
    const { items } = buildDisplayItems(chunk);

    const toolItem = items.find((i) => i.type === "tool");
    expect(toolItem).toBeDefined();
    if (toolItem && toolItem.type === "tool") {
      expect(toolItem.execution.toolName).toBe("Workflow");
    }
  });
});

describe("buildSummary — workflow tool counted as tool call", () => {
  test("Workflow tool counted in tool calls summary when no workflowRunIds provided", () => {
    const chunk = makeChunkWithWorkflowTool();
    const { items } = buildDisplayItems(chunk);
    const summary = buildSummary(items);

    expect(summary).toContain("1 tool call");
  });

  test("Workflow tool counted as workflow when workflowRunIds provided", () => {
    const chunk = makeChunkWithWorkflowTool();
    const { items } = buildDisplayItems(chunk);
    const workflowRunIds = new Set(["wf-run-001"]);
    const summary = buildSummary(items, workflowRunIds);

    expect(summary).toContain("1 workflow");
    expect(summary).not.toContain("tool call");
  });
});
