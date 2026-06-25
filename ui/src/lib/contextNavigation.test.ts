import { describe, expect, test } from "vitest";

import { resolveUserGroupNavTarget } from "./contextNavigation";

describe("resolveUserGroupNavTarget", () => {
  test("完整 turn：aiGroupId 是 AIChunk → 向前找紧邻 UserChunk", () => {
    const chunks = [
      { chunkId: "u0:0", kind: "user" },
      { chunkId: "a0:0", kind: "ai" },
    ];
    expect(resolveUserGroupNavTarget(chunks, "a0:0")).toBe("u0:0");
  });

  test("被打断 turn：aiGroupId 本身是 UserChunk → 直接返回它，不回溯上一条", () => {
    // [U1(被打断), U2, A2]：点 U1 的 injection（aiGroupId=u1:0）应定位 u1:0 本身。
    const chunks = [
      { chunkId: "u1:0", kind: "user" },
      { chunkId: "u2:0", kind: "user" },
      { chunkId: "a2:0", kind: "ai" },
    ];
    expect(resolveUserGroupNavTarget(chunks, "u1:0")).toBe("u1:0");
  });

  test("完整 turn 无前置 UserChunk → 退化为 AIChunk 本身", () => {
    const chunks = [{ chunkId: "a0:0", kind: "ai" }];
    expect(resolveUserGroupNavTarget(chunks, "a0:0")).toBe("a0:0");
  });

  test("aiGroupId 命中不到任何 chunk → 返回 null（不导航）", () => {
    const chunks = [{ chunkId: "a0:0", kind: "ai" }];
    expect(resolveUserGroupNavTarget(chunks, "ghost:0")).toBeNull();
  });
});
