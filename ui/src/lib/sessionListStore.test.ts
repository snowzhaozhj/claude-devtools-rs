import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

vi.mock("./api", async () => {
  const actual = await vi.importActual<typeof import("./api")>("./api");
  return {
    ...actual,
    listSessions: vi.fn(),
  };
});

import { listSessions, type PaginatedResponse, type SessionSummary } from "./api";
import {
  __resetForTests,
  __snapshotKeys,
  applyMetadata,
  loadFirstPage,
  loadMore,
  read,
} from "./sessionListStore.svelte";

const mockedListSessions = vi.mocked(listSessions);

function sessionSummary(sessionId: string, projectId = "p1"): SessionSummary {
  return {
    sessionId,
    projectId,
    timestamp: 1_700_000_000_000,
    messageCount: 0,
    title: null,
    isOngoing: false,
    gitBranch: null,
  };
}

function paginated(items: SessionSummary[], total = items.length, nextCursor: string | null = null): PaginatedResponse<SessionSummary> {
  return { items, total, nextCursor };
}

beforeEach(() => {
  mockedListSessions.mockReset();
  __resetForTests();
});

afterEach(() => {
  __resetForTests();
});

describe("sessionListStore", () => {
  test("read 在未加载时返回 undefined", () => {
    expect(read("p1")).toBeUndefined();
  });

  test("loadFirstPage replace 写入 store 后 read 命中", async () => {
    mockedListSessions.mockResolvedValueOnce(paginated([sessionSummary("s1"), sessionSummary("s2")], 2));

    const entry = await loadFirstPage("p1", { mode: "replace" });
    expect(entry?.sessions.map((s) => s.sessionId)).toEqual(["s1", "s2"]);
    expect(read("p1")?.total).toBe(2);
  });

  test("loadFirstPage merge 首页 ghost reconcile 删除 server 已不返的 sessionId", async () => {
    // 初始：s1 s2 s3 s4 s5（全在首页范围）
    mockedListSessions.mockResolvedValueOnce(
      paginated([
        sessionSummary("s1"),
        sessionSummary("s2"),
        sessionSummary("s3"),
        sessionSummary("s4"),
        sessionSummary("s5"),
      ], 5),
    );
    await loadFirstPage("p1", { mode: "replace" });

    // SWR refresh：server 返 s1 s2 s4 s5 s6（删 s3，加 s6）
    mockedListSessions.mockResolvedValueOnce(
      paginated([
        sessionSummary("s1"),
        sessionSummary("s2"),
        sessionSummary("s4"),
        sessionSummary("s5"),
        sessionSummary("s6"),
      ], 5),
    );
    await loadFirstPage("p1", { mode: "merge" });

    expect(read("p1")?.sessions.map((s) => s.sessionId)).toEqual(["s1", "s2", "s4", "s5", "s6"]);
  });

  test("loadFirstPage merge 保留尾部（超出首页 pageSize 范围的条目）", async () => {
    // 初始：模拟已翻页加载到 7 条
    const initial = [
      sessionSummary("s1"),
      sessionSummary("s2"),
      sessionSummary("s3"),
      sessionSummary("s4"),
      sessionSummary("s5"),
      sessionSummary("s6"),
      sessionSummary("s7"),
    ];
    mockedListSessions.mockResolvedValueOnce(paginated(initial, 7));
    await loadFirstPage("p1", { mode: "replace" });

    // SWR refresh 返首页 5 条：s1 s2 s4 s5 s8（s3 删，s8 新增）
    mockedListSessions.mockResolvedValueOnce(
      paginated(
        [sessionSummary("s1"), sessionSummary("s2"), sessionSummary("s4"), sessionSummary("s5"), sessionSummary("s8")],
        8,
      ),
    );
    await loadFirstPage("p1", { mode: "merge" });

    const ids = read("p1")?.sessions.map((s) => s.sessionId);
    // 首页 5 条按 server；尾部 s6 s7 保留（超出首页 5 范围）
    expect(ids).toEqual(["s1", "s2", "s4", "s5", "s8", "s6", "s7"]);
  });

  test("generation cancel：快速两次 loadFirstPage 仅最后一次写入 store", async () => {
    let resolve1: (v: PaginatedResponse<SessionSummary>) => void = () => {};
    const slow = new Promise<PaginatedResponse<SessionSummary>>((r) => {
      resolve1 = r;
    });
    mockedListSessions.mockImplementationOnce(() => slow);
    mockedListSessions.mockResolvedValueOnce(paginated([sessionSummary("s-late")], 1));

    const p1 = loadFirstPage("p1", { mode: "replace" });
    const p2 = loadFirstPage("p1", { mode: "replace" });
    // p2 启动时已 ++generation；p1 resolve 时 generation 不等，被丢弃
    resolve1(paginated([sessionSummary("s-early")], 1));

    await Promise.all([p1, p2]);
    // 最终 store 应为第二次的内容
    expect(read("p1")?.sessions.map((s) => s.sessionId)).toEqual(["s-late"]);
  });

  test("applyMetadata in-place 更新 + read-after-write 命中已 patch 字段", async () => {
    mockedListSessions.mockResolvedValueOnce(paginated([sessionSummary("s1")], 1));
    await loadFirstPage("p1", { mode: "replace" });

    applyMetadata("p1", {
      projectId: "p1",
      sessionId: "s1",
      title: "patched",
      messageCount: 42,
      isOngoing: true,
      gitBranch: "main",
    });

    const entry = read("p1");
    expect(entry?.sessions[0].title).toBe("patched");
    expect(entry?.sessions[0].messageCount).toBe(42);
    expect(entry?.sessions[0].isOngoing).toBe(true);
    expect(entry?.sessions[0].gitBranch).toBe("main");
  });

  test("applyMetadata 找不到 sessionId 时静默丢弃", async () => {
    mockedListSessions.mockResolvedValueOnce(paginated([sessionSummary("s1")], 1));
    await loadFirstPage("p1", { mode: "replace" });
    expect(() =>
      applyMetadata("p1", {
        projectId: "p1",
        sessionId: "s-ghost",
        title: "x",
        messageCount: 1,
        isOngoing: false,
        gitBranch: null,
      }),
    ).not.toThrow();
  });

  test("LRU evict：超出 16 个 project 后驱逐最久未访问条目", async () => {
    // 灌 17 个 project 各一条 session
    for (let i = 0; i < 17; i++) {
      mockedListSessions.mockResolvedValueOnce(paginated([sessionSummary(`s-${i}`, `p${i}`)], 1));
      await loadFirstPage(`p${i}`, { mode: "replace" });
    }

    const keys = __snapshotKeys();
    expect(keys.length).toBe(16);
    // 最早访问的 p0 SHALL 被 evict
    expect(keys).not.toContain("p0");
    // 最新访问的 p16 SHALL 保留
    expect(keys).toContain("p16");
  });

  test("loadMore leading 立即触发 + inflight 期间同 cursor 短路", async () => {
    vi.useFakeTimers();
    // 首次 loadFirstPage 设 nextCursor=c1
    mockedListSessions.mockResolvedValueOnce(paginated([sessionSummary("s1")], 10, "c1"));
    await loadFirstPage("p1", { mode: "replace" });

    let resolveMore: (v: PaginatedResponse<SessionSummary>) => void = () => {};
    const slowMore = new Promise<PaginatedResponse<SessionSummary>>((r) => {
      resolveMore = r;
    });
    mockedListSessions.mockImplementation(() => slowMore);

    loadMore("p1"); // leading fire（立即）
    loadMore("p1"); // inflight 同 cursor short-circuit
    loadMore("p1");
    loadMore("p1");

    // 仍是首次 mock 调用计 1 + leading fire 计 1 = 2 次（首页 + 一次 loadMore）
    expect(mockedListSessions).toHaveBeenCalledTimes(2);

    resolveMore(paginated([sessionSummary("s2")], 10, "c2"));
    await vi.runAllTimersAsync();
    // 应只产生 1 次 loadMore fetch（leading），后续被 inflight short-circuit
    expect(mockedListSessions).toHaveBeenCalledTimes(2);
    expect(read("p1")?.sessions.map((s) => s.sessionId)).toEqual(["s1", "s2"]);
    expect(read("p1")?.nextCursor).toBe("c2");
    vi.useRealTimers();
  });

  test("loadMore trailing：cooldown 内重复调，cooldown 结束后才 trailing fire", async () => {
    vi.useFakeTimers();
    mockedListSessions.mockResolvedValueOnce(paginated([sessionSummary("s1")], 10, "c1"));
    await loadFirstPage("p1", { mode: "replace" });

    // 第一次 loadMore：leading fire（立即）
    mockedListSessions.mockResolvedValueOnce(paginated([sessionSummary("s2")], 10, "c2"));
    loadMore("p1");
    await vi.advanceTimersByTimeAsync(0); // 让 leading fire 的 Promise 解析
    expect(mockedListSessions).toHaveBeenCalledTimes(2);
    expect(read("p1")?.nextCursor).toBe("c2");

    // cooldown 内再调几次：合并为 1 个 trailing timer
    loadMore("p1");
    loadMore("p1");
    loadMore("p1");

    // cooldown 内 fetch 不增加
    expect(mockedListSessions).toHaveBeenCalledTimes(2);

    mockedListSessions.mockResolvedValueOnce(paginated([sessionSummary("s3")], 10, null));
    // 推进到 cooldown 结束 + 一些缓冲（debounce 100ms）
    await vi.advanceTimersByTimeAsync(150);
    // trailing fire 后总 fetch 数 +1
    expect(mockedListSessions).toHaveBeenCalledTimes(3);
    expect(read("p1")?.sessions.map((s) => s.sessionId)).toEqual(["s1", "s2", "s3"]);
    vi.useRealTimers();
  });
});
