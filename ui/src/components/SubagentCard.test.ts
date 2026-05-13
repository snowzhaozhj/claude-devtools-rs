/**
 * SubagentCard module-level `loadSubagentTrace` inflight 去重测试。
 *
 * 覆盖 spec `session-display` "SubagentCard 在 ongoing 期间主动重拉 trace"
 * Requirement 的两条 Scenario：
 * - "同 sessionId 同版本并发触发 inflight 复用"
 * - "同 sessionId 跨版本不复用旧 Promise"
 *
 * 注：组件级行为（已展开 ongoing 重拉、未展开不拉、老后端 undefined 退化）
 * 依赖 Svelte 5 component lifecycle 测试基础设施，本仓 vitest 配置以
 * `globals: false` + module 级测试为主，组件级 case 留给端到端手测覆盖
 * （`/opsx:apply` tasks 7.1 / 7.2）。
 */

import { afterEach, describe, expect, test, vi } from "vitest";

import {
  __resetSubagentTraceInflightForTest,
  loadSubagentTrace,
} from "./SubagentCard.svelte";
import * as api from "../lib/api";

afterEach(() => {
  __resetSubagentTraceInflightForTest();
  vi.restoreAllMocks();
});

describe("loadSubagentTrace inflight 去重", () => {
  test("同 sessionId 同 version 并发触发 SHALL 复用同一 Promise", async () => {
    const spy = vi
      .spyOn(api, "getSubagentTrace")
      .mockResolvedValue([]);

    const p1 = loadSubagentTrace("root", "sub", "1|_|5");
    const p2 = loadSubagentTrace("root", "sub", "1|_|5");
    // 复用：两次拿到同一 Promise 引用
    expect(p1).toBe(p2);
    await Promise.all([p1, p2]);
    // IPC 只调用一次
    expect(spy).toHaveBeenCalledTimes(1);
  });

  test("同 sessionId 跨 version 触发 SHALL 各自独立 Promise", async () => {
    const spy = vi
      .spyOn(api, "getSubagentTrace")
      .mockResolvedValue([]);

    const p1 = loadSubagentTrace("root", "sub", "1|_|5");
    const p2 = loadSubagentTrace("root", "sub", "1|_|8");
    expect(p1).not.toBe(p2);
    await Promise.all([p1, p2]);
    expect(spy).toHaveBeenCalledTimes(2);
  });

  test("Promise settle 后从 Map 清出，下次同 key 重新走 IPC", async () => {
    const spy = vi
      .spyOn(api, "getSubagentTrace")
      .mockResolvedValue([]);

    await loadSubagentTrace("root", "sub", "1|_|5");
    await loadSubagentTrace("root", "sub", "1|_|5");
    // 因为第一次 settle 后 inflight Map 已删 key，第二次重新调
    expect(spy).toHaveBeenCalledTimes(2);
  });

  test("不同 sessionId 各自独立 inflight", async () => {
    const spy = vi
      .spyOn(api, "getSubagentTrace")
      .mockResolvedValue([]);

    const p1 = loadSubagentTrace("root", "sub-A", "1|_|5");
    const p2 = loadSubagentTrace("root", "sub-B", "1|_|5");
    expect(p1).not.toBe(p2);
    await Promise.all([p1, p2]);
    expect(spy).toHaveBeenCalledTimes(2);
  });

  test("IPC 抛错时 inflight key 也 SHALL 被清出（finally）", async () => {
    const spy = vi
      .spyOn(api, "getSubagentTrace")
      .mockRejectedValueOnce(new Error("boom"))
      .mockResolvedValueOnce([]);

    await expect(loadSubagentTrace("root", "sub", "1|_|5")).rejects.toThrow(
      "boom",
    );
    // 第二次同 key 不复用 rejected Promise，重新调 IPC
    await loadSubagentTrace("root", "sub", "1|_|5");
    expect(spy).toHaveBeenCalledTimes(2);
  });
});
