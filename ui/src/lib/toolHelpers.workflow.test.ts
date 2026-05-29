import { describe, expect, test } from "vitest";
import { getToolSummary } from "./toolHelpers";

describe("getToolSummary — Workflow", () => {
  test("returns name when provided", () => {
    const result = getToolSummary("Workflow", { name: "deploy-pipeline", run_id: "wf-001" });
    expect(result).toBe("deploy-pipeline");
  });

  test("truncates long name", () => {
    const longName = "a".repeat(60);
    const result = getToolSummary("Workflow", { name: longName });
    expect(result.length).toBeLessThanOrEqual(51);
    expect(result).toContain("…");
  });

  test("falls back to runId when name absent", () => {
    const result = getToolSummary("Workflow", { run_id: "wf-abcdef-123" });
    expect(result).toBe("run wf-abcdef-123");
  });

  test("supports camelCase runId", () => {
    const result = getToolSummary("Workflow", { runId: "wf-xyz" });
    expect(result).toBe("run wf-xyz");
  });

  test("returns 'Workflow' when no name or runId", () => {
    const result = getToolSummary("Workflow", {});
    expect(result).toBe("Workflow");
  });

  test("returns empty string when input is null", () => {
    const result = getToolSummary("Workflow", null);
    expect(result).toBe("");
  });
});
