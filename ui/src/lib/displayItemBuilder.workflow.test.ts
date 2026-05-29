import { describe, expect, test } from "vitest";
import { buildDisplayItems, buildSummary } from "./displayItemBuilder";
import type { AIChunk, WorkflowItem } from "./api";

function makeWorkflow(overrides: Partial<WorkflowItem> = {}): WorkflowItem {
  return {
    runId: "wf-test-001",
    name: "test-pipeline",
    status: "completed",
    phases: [{ index: 0, title: "Build" }],
    agents: [
      { label: "agent-1", phaseIndex: 0, status: "done", tokens: 1000 },
    ],
    totalTokens: 1000,
    durationMs: 5000,
    ...overrides,
  };
}

function makeChunkWithWorkflows(workflows: WorkflowItem[]): AIChunk {
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
      toolCount: 0,
      costUsd: null,
    },
    semanticSteps: [
      {
        kind: "text",
        text: "Running workflow...",
        timestamp: "2024-01-01T00:00:00.000Z",
      },
    ],
    toolExecutions: [],
    subagents: [],
    slashCommands: [],
    workflows,
  };
}

describe("buildDisplayItems — workflow", () => {
  test("includes WorkflowDisplayItem for each workflow in chunk", () => {
    const wf1 = makeWorkflow({ runId: "wf-1", name: "pipeline-a" });
    const wf2 = makeWorkflow({ runId: "wf-2", name: "pipeline-b" });
    const chunk = makeChunkWithWorkflows([wf1, wf2]);

    const { items } = buildDisplayItems(chunk);
    const wfItems = items.filter((i) => i.type === "workflow");

    expect(wfItems).toHaveLength(2);
    expect(wfItems[0].type === "workflow" && wfItems[0].workflow.runId).toBe(
      "wf-1",
    );
    expect(wfItems[1].type === "workflow" && wfItems[1].workflow.runId).toBe(
      "wf-2",
    );
  });

  test("workflow items carry chunk timestamp", () => {
    const chunk = makeChunkWithWorkflows([makeWorkflow()]);
    const { items } = buildDisplayItems(chunk);
    const wfItem = items.find((i) => i.type === "workflow");

    expect(wfItem).toBeDefined();
    if (wfItem && wfItem.type === "workflow") {
      expect(wfItem.timestamp).toBe("2024-01-01T00:00:00.000Z");
    }
  });

  test("chunk without workflows produces no workflow items", () => {
    const chunk = makeChunkWithWorkflows([]);
    const { items } = buildDisplayItems(chunk);
    const wfItems = items.filter((i) => i.type === "workflow");

    expect(wfItems).toHaveLength(0);
  });

  test("chunk with undefined workflows (old backend) produces no workflow items", () => {
    const chunk = makeChunkWithWorkflows([]);
    delete (chunk as unknown as Record<string, unknown>).workflows;
    const { items } = buildDisplayItems(chunk);
    const wfItems = items.filter((i) => i.type === "workflow");

    expect(wfItems).toHaveLength(0);
  });
});

describe("buildSummary — workflow", () => {
  test("counts workflows in summary string", () => {
    const chunk = makeChunkWithWorkflows([
      makeWorkflow({ runId: "wf-1" }),
      makeWorkflow({ runId: "wf-2" }),
    ]);
    const { items } = buildDisplayItems(chunk);
    const summary = buildSummary(items);

    expect(summary).toContain("2 workflows");
  });

  test("singular workflow in summary", () => {
    const chunk = makeChunkWithWorkflows([makeWorkflow()]);
    const { items } = buildDisplayItems(chunk);
    const summary = buildSummary(items);

    expect(summary).toContain("1 workflow");
    expect(summary).not.toContain("workflows");
  });

  test("no workflow mention when none present", () => {
    const chunk = makeChunkWithWorkflows([]);
    const { items } = buildDisplayItems(chunk);
    const summary = buildSummary(items);

    expect(summary).not.toContain("workflow");
  });
});
