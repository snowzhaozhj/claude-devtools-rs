import { describe, test, expect } from "vitest";
import { collectVirtualMatches, type AIChunkLike } from "./searchHighlight";

describe("collectVirtualMatches", () => {
  const chunks: AIChunkLike[] = [
    {
      chunkId: "chunk-1",
      toolExecutions: [
        { toolUseId: "tu-1", toolName: "Read", summary: "/src/lib/api.ts" },
        { toolUseId: "tu-2", toolName: "Write", summary: "/src/lib/utils.ts" },
      ],
    },
    {
      chunkId: "chunk-2",
      toolExecutions: [
        { toolUseId: "tu-3", toolName: "Bash", summary: "pnpm test" },
        { toolUseId: "tu-4", toolName: "Edit", summary: "/src/config.ts" },
      ],
    },
    {
      chunkId: "chunk-3",
      toolExecutions: [
        { toolUseId: "tu-5", toolName: "Read", summary: "/README.md" },
      ],
    },
  ];

  test("returns empty for empty query", () => {
    expect(collectVirtualMatches("", chunks, new Set())).toEqual([]);
  });

  test("matches tool name case-insensitive", () => {
    const result = collectVirtualMatches("read", chunks, new Set());
    expect(result).toHaveLength(2);
    expect(result[0].toolUseId).toBe("tu-1");
    expect(result[1].toolUseId).toBe("tu-5");
  });

  test("matches summary text", () => {
    const result = collectVirtualMatches("config.ts", chunks, new Set());
    expect(result).toHaveLength(1);
    expect(result[0].toolUseId).toBe("tu-4");
    expect(result[0].chunkId).toBe("chunk-2");
  });

  test("excludes expanded chunks", () => {
    const expanded = new Set(["chunk-1"]);
    const result = collectVirtualMatches("Read", chunks, expanded);
    expect(result).toHaveLength(1);
    expect(result[0].chunkId).toBe("chunk-3");
  });

  test("excludes all expanded chunks", () => {
    const expanded = new Set(["chunk-1", "chunk-2", "chunk-3"]);
    const result = collectVirtualMatches("Read", chunks, expanded);
    expect(result).toHaveLength(0);
  });

  test("matches both name and summary in same execution", () => {
    const result = collectVirtualMatches("Bash", chunks, new Set());
    expect(result).toHaveLength(1);
    expect(result[0].toolUseId).toBe("tu-3");
  });

  test("partial match on tool name", () => {
    const result = collectVirtualMatches("rea", chunks, new Set());
    expect(result).toHaveLength(2);
  });

  test("text field contains toolName and summary", () => {
    const result = collectVirtualMatches("Read", chunks, new Set());
    expect(result[0].text).toBe("Read /src/lib/api.ts");
  });
});
