import { describe, expect, test } from "vitest";
import {
  parseInjection,
  parseInjections,
  selectActivePhaseInjections,
  groupClaudeMdByScope,
  sumTokens,
  formatTokens,
  type ContextInjection,
  type ClaudeMdInjection,
} from "./contextExtractor";
import type { SessionDetail } from "./api";

describe("parseInjection", () => {
  test("narrows known categories without losing fields", () => {
    const raw = {
      category: "tool-output",
      id: "t1",
      turnIndex: 1,
      aiGroupId: "a:0",
      estimatedTokens: 100,
      toolCount: 2,
      toolBreakdown: [{ toolName: "Bash", tokenCount: 100, isError: false, toolUseId: "tu1" }],
    };
    const inj = parseInjection(raw);
    expect(inj?.category).toBe("tool-output");
    expect(inj && inj.category === "tool-output" && inj.toolBreakdown[0].toolName).toBe("Bash");
  });

  test("returns null for unknown category", () => {
    expect(parseInjection({ category: "garbage" })).toBeNull();
    expect(parseInjection(null)).toBeNull();
    expect(parseInjection({})).toBeNull();
  });

  test("parses all 6 categories", () => {
    const cases: { category: string }[] = [
      { category: "claude-md" },
      { category: "mentioned-file" },
      { category: "tool-output" },
      { category: "thinking-text" },
      { category: "task-coordination" },
      { category: "user-message" },
    ];
    for (const c of cases) {
      expect(parseInjection(c)?.category).toBe(c.category);
    }
  });
});

describe("parseInjections", () => {
  test("filters out invalid items, keeps valid ones", () => {
    const out = parseInjections([
      { category: "user-message", id: "u1", turnIndex: 0, aiGroupId: "a:0", estimatedTokens: 1, textPreview: "hi" },
      { category: "unknown" },
      null,
      { category: "claude-md", id: "c1", path: "/p", displayName: "p", scope: "user", estimatedTokens: 10, firstSeenTurnIndex: 0 },
    ]);
    expect(out.length).toBe(2);
    expect(out[0].category).toBe("user-message");
    expect(out[1].category).toBe("claude-md");
  });

  test("returns empty for null/undefined/non-array", () => {
    expect(parseInjections(null)).toEqual([]);
    expect(parseInjections(undefined)).toEqual([]);
  });
});

describe("groupClaudeMdByScope", () => {
  test("merges enterprise + user into global; project / directory each own group", () => {
    const injections: ContextInjection[] = [
      { category: "claude-md", id: "1", path: "/e", displayName: "e", scope: "enterprise", estimatedTokens: 1, firstSeenTurnIndex: 0 },
      { category: "claude-md", id: "2", path: "/u", displayName: "u", scope: "user", estimatedTokens: 2, firstSeenTurnIndex: 0 },
      { category: "claude-md", id: "3", path: "/p", displayName: "p", scope: "project", estimatedTokens: 3, firstSeenTurnIndex: 0 },
      { category: "claude-md", id: "4", path: "/d", displayName: "d", scope: "directory", estimatedTokens: 4, firstSeenTurnIndex: 0 },
      // 非 claude-md 应被忽略
      { category: "user-message", id: "x", turnIndex: 0, aiGroupId: "a:0", estimatedTokens: 99, textPreview: "ignored" },
    ];
    const out = groupClaudeMdByScope(injections);
    expect(out.global.map((i) => i.id)).toEqual(["1", "2"]);
    expect(out.project.map((i) => i.id)).toEqual(["3"]);
    expect(out.directory.map((i) => i.id)).toEqual(["4"]);
  });

  test("empty injection list yields all empty groups", () => {
    const out = groupClaudeMdByScope([]);
    expect(out.global).toEqual([]);
    expect(out.project).toEqual([]);
    expect(out.directory).toEqual([]);
  });
});

function makeDetail(injectionsByPhase?: Record<string, unknown[]>): SessionDetail {
  return {
    sessionId: "s",
    projectId: "p",
    chunks: [],
    metrics: {},
    metadata: {},
    contextInjections: [
      { category: "user-message", id: "latest", turnIndex: 2, aiGroupId: "a:0", estimatedTokens: 5, textPreview: "latest" },
    ],
    injectionsByPhase,
    isOngoing: false,
  };
}

describe("selectActivePhaseInjections", () => {
  test("selectedPhase=null + has injectionsByPhase → latest phase", () => {
    const phase1Inj = [
      { category: "claude-md", id: "p1", path: "/p1", displayName: "p1", scope: "user", estimatedTokens: 10, firstSeenTurnIndex: 0 },
    ];
    const phase2Inj = [
      { category: "user-message", id: "p2", turnIndex: 1, aiGroupId: "a:1", estimatedTokens: 5, textPreview: "p2" },
    ];
    const detail = makeDetail({ "1": phase1Inj, "2": phase2Inj });
    const out = selectActivePhaseInjections(detail, null);
    expect(out.length).toBe(1);
    expect(out[0].id).toBe("p2");
  });

  test("selectedPhase=null + no injectionsByPhase → contextInjections fallback", () => {
    const detail = makeDetail();
    const out = selectActivePhaseInjections(detail, null);
    expect(out.length).toBe(1);
    expect((out[0] as ClaudeMdInjection | { id: string }).id).toBe("latest");
  });

  test("selectedPhase=N returns injectionsByPhase[N]", () => {
    const phase1Inj = [
      { category: "claude-md", id: "p1", path: "/p1", displayName: "p1", scope: "user", estimatedTokens: 10, firstSeenTurnIndex: 0 },
    ];
    const detail = makeDetail({ "1": phase1Inj, "2": [] });
    const out = selectActivePhaseInjections(detail, 1);
    expect(out.length).toBe(1);
    expect(out[0].id).toBe("p1");
  });

  test("selectedPhase=N when injectionsByPhase[N] missing → empty array", () => {
    const detail = makeDetail({ "1": [] });
    const out = selectActivePhaseInjections(detail, 99);
    expect(out).toEqual([]);
  });
});

describe("sumTokens + formatTokens", () => {
  test("sum is right", () => {
    expect(sumTokens([])).toBe(0);
    expect(
      sumTokens([
        { category: "user-message", id: "1", turnIndex: 0, aiGroupId: "a:0", estimatedTokens: 100, textPreview: "" },
        { category: "user-message", id: "2", turnIndex: 0, aiGroupId: "a:0", estimatedTokens: 50, textPreview: "" },
      ]),
    ).toBe(150);
  });

  test("formatTokens scales k / M", () => {
    expect(formatTokens(999)).toBe("999");
    expect(formatTokens(1000)).toBe("1.0k");
    expect(formatTokens(2500)).toBe("2.5k");
    expect(formatTokens(1_500_000)).toBe("1.5M");
  });
});
