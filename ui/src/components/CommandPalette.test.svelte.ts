// CommandPalette 单测：覆盖 change cmdk-global-session-locate 的行为契约
// （spec ui-search::Command Palette 搜索模式）。
//
// 覆盖：
// - 全局 sessionId 跨项目定位（query ≥ 4）
// - 短查询（< 4）不启用全局 + 空态提示
// - 跨 worktree 同会话去重（worktree 级确定性）
// - 打开用结果行自身 projectId + groupId（不用当前选中项目）
// - A（全局 id）+ B（组内正文）合并：B 优先补 hits/归属
// - title 正向：命中已加载会话显示 title，且不为补 title 发额外 IPC
// - stale 快照修复：store 刷新后已打开面板同步
// - active-context 边界：快照外的 sessionId 不出现

import { describe, expect, test, beforeEach, afterEach, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";

vi.mock("../lib/api", () => ({
  listRepositoryGroups: vi.fn(),
  listProjects: vi.fn(),
  listGroupSessions: vi.fn(),
  searchGroupSessions: vi.fn(),
}));
vi.mock("../lib/tabStore.svelte", () => ({
  openTab: vi.fn(),
  openJobsTab: vi.fn(),
}));
vi.mock("../lib/jobsStore.svelte", () => ({
  getJobsDirExists: vi.fn(() => false),
}));

import CommandPalette from "./CommandPalette.svelte";
import {
  listRepositoryGroups,
  listProjects,
  listGroupSessions,
  searchGroupSessions,
} from "../lib/api";
import { loadProjectData } from "../lib/projectDataStore.svelte";
import { openTab } from "../lib/tabStore.svelte";

const SID_A1 = "aaaa1111-0000-0000-0000-000000000001"; // group G1 / project p1
const SID_B2 = "bbbb2222-0000-0000-0000-000000000002"; // group G1 / project p1
const SID_A3 = "aaaa3333-0000-0000-0000-000000000003"; // group G2 / project p2
const SID_DUP = "dddd5555-0000-0000-0000-000000000005"; // 跨 G3 两 worktree 重复

function worktree(id: string, name: string, sessions: string[], extra: Record<string, unknown> = {}) {
  return {
    id,
    path: `/repo/${name}`,
    name,
    gitBranch: "main",
    isMainWorktree: true,
    isRepoRoot: true,
    sessions,
    createdAt: 0,
    mostRecentSession: 0,
    ...extra,
  };
}

function group(id: string, name: string, worktrees: ReturnType<typeof worktree>[], mostRecent: number) {
  return { id, identity: null, name, worktrees, mostRecentSession: mostRecent, totalSessions: worktrees.reduce((n, w) => n + w.sessions.length, 0) };
}

function snapshot() {
  return [
    group("G1", "alpha", [worktree("p1", "alpha", [SID_A1, SID_B2], { mostRecentSession: 200 })], 200),
    group("G2", "beta", [worktree("p2", "beta", [SID_A3], { mostRecentSession: 100 })], 100),
    group("G3", "gamma", [
      worktree("p3main", "gamma", [SID_DUP], { mostRecentSession: 50, isMainWorktree: true, isRepoRoot: true }),
      worktree("p3wt", "gamma-feat", [SID_DUP], { mostRecentSession: 90, isMainWorktree: false, isRepoRoot: false }),
    ], 90),
  ];
}

async function setSnapshot(groups: ReturnType<typeof group>[]) {
  vi.mocked(listRepositoryGroups).mockResolvedValue(groups as never);
  await loadProjectData({ refresh: true });
}

async function renderPalette(props: Partial<{ selectedProjectId: string }> = {}) {
  const onSelectProject = vi.fn();
  const onClose = vi.fn();
  const r = render(CommandPalette, {
    props: { selectedProjectId: props.selectedProjectId ?? "", onSelectProject, onClose },
  });
  return { ...r, onSelectProject, onClose };
}

function sessionLabels(container: HTMLElement): string[] {
  // 会话区在「会话」section 之后；取所有 cp-item-label，过滤掉项目区
  return Array.from(container.querySelectorAll(".cp-item")).map(
    (el) => el.querySelector(".cp-item-label")?.textContent?.trim() ?? "",
  );
}

async function type(input: HTMLElement, value: string) {
  await fireEvent.input(input, { target: { value } });
}

beforeEach(() => {
  vi.mocked(listProjects).mockResolvedValue([] as never);
  vi.mocked(listGroupSessions).mockResolvedValue({ sessions: [], nextCursor: null } as never);
  vi.mocked(searchGroupSessions).mockResolvedValue({ results: [], totalMatches: 0, sessionsSearched: 0, query: "", isPartial: false } as never);
  vi.mocked(openTab).mockClear();
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("CommandPalette 全局 sessionId 定位", () => {
  test("query ≥ 4 跨所有项目定位（未选中项目也生效）", async () => {
    await setSnapshot(snapshot());
    const { container, getByLabelText } = await renderPalette();
    await type(getByLabelText("命令面板搜索"), "aaaa");
    await waitFor(() => {
      const labels = sessionLabels(container);
      expect(labels.some((l) => l.startsWith("aaaa1111"))).toBe(true);
      expect(labels.some((l) => l.startsWith("aaaa3333"))).toBe(true);
    });
  });

  test("query < 4 不启用全局匹配，未选项目时显示提示", async () => {
    await setSnapshot(snapshot());
    const { container, getByLabelText } = await renderPalette();
    await type(getByLabelText("命令面板搜索"), "aaa");
    await waitFor(() => {
      expect(container.textContent).toContain("个字符按 Session ID 全局定位");
    });
    const labels = sessionLabels(container);
    expect(labels.some((l) => l.startsWith("aaaa"))).toBe(false);
  });

  test("跨 worktree 同会话去重为一条（确定性保留 main worktree）", async () => {
    await setSnapshot(snapshot());
    const { container, getByLabelText } = await renderPalette();
    await type(getByLabelText("命令面板搜索"), "dddd5555");
    await waitFor(() => {
      const dups = sessionLabels(container).filter((l) => l.startsWith("dddd5555"));
      expect(dups.length).toBe(1);
    });
  });

  test("打开跨项目命中用其自身 projectId + groupId，不用当前选中项目", async () => {
    await setSnapshot(snapshot());
    const { container, getByLabelText } = await renderPalette({ selectedProjectId: "p1" });
    await type(getByLabelText("命令面板搜索"), "aaaa3333");
    let target: Element | undefined;
    await waitFor(() => {
      target = Array.from(container.querySelectorAll(".cp-item")).find((el) =>
        el.querySelector(".cp-item-label")?.textContent?.startsWith("aaaa3333"),
      );
      expect(target).toBeTruthy();
    });
    await fireEvent.click(target!);
    expect(openTab).toHaveBeenCalledWith(SID_A3, "p2", expect.any(String), "G2");
  });

  test("快照外的 sessionId 不出现（active-context 边界）", async () => {
    await setSnapshot(snapshot());
    const { container, getByLabelText } = await renderPalette();
    await type(getByLabelText("命令面板搜索"), "ffff9999");
    await waitFor(() => {
      expect(sessionLabels(container).some((l) => l.startsWith("ffff9999"))).toBe(false);
    });
  });
});

describe("CommandPalette A+B 合并与 title", () => {
  test("同会话双路命中：保留 B 的 hits，打开用合并 row 自身归属", async () => {
    await setSnapshot(snapshot());
    vi.mocked(searchGroupSessions).mockResolvedValue({
      results: [{ sessionId: SID_A1, projectId: "p1", sessionTitle: "正文命中会话", totalMatches: 5 }],
      totalMatches: 5, sessionsSearched: 1, query: "aaaa", isPartial: false,
    } as never);
    const { container, getByLabelText } = await renderPalette({ selectedProjectId: "p1" });
    await type(getByLabelText("命令面板搜索"), "aaaa");
    let row: Element | undefined;
    await waitFor(() => {
      row = Array.from(container.querySelectorAll(".cp-item")).find((el) =>
        el.querySelector(".cp-item-label")?.textContent?.includes("正文命中会话"),
      );
      expect(row).toBeTruthy();
      expect(row!.querySelector(".cp-item-badge")?.textContent?.trim()).toBe("5");
    });
    await fireEvent.click(row!);
    expect(openTab).toHaveBeenCalledWith(SID_A1, "p1", "正文命中会话", "G1");
  });

  test("title 已加载时显示且不调补 title 接口", async () => {
    await setSnapshot(snapshot());
    vi.mocked(listGroupSessions).mockResolvedValue({
      sessions: [{ sessionId: SID_A1, projectId: "p1", timestamp: 200, created: 0, messageCount: 3, title: "已加载标题", isOngoing: false, gitBranch: "main" }],
      nextCursor: null,
    } as never);
    const { container, getByLabelText } = await renderPalette({ selectedProjectId: "p1" });
    await waitFor(() => expect(vi.mocked(listGroupSessions)).toHaveBeenCalledTimes(1));
    await type(getByLabelText("命令面板搜索"), "aaaa");
    await waitFor(() => {
      expect(sessionLabels(container).some((l) => l === "已加载标题")).toBe(true);
    });
    // 命中多条也不应为补 title 再调 listGroupSessions（仅选组那一次）
    expect(vi.mocked(listGroupSessions)).toHaveBeenCalledTimes(1);
  });
});

describe("CommandPalette 排序 / 截断 / title 兜底", () => {
  test("确定性排序：按 worktreeMostRecent 倒序", async () => {
    await setSnapshot([
      group("Ghi", "hi", [worktree("phi", "hi", ["hi00match0001"], { mostRecentSession: 300 })], 300),
      group("Glo", "lo", [worktree("plo", "lo", ["lo00match0003"], { mostRecentSession: 100 })], 100),
      group("Gmid", "mid", [worktree("pmid", "mid", ["mid0match0002"], { mostRecentSession: 200 })], 200),
    ])
    const { container, getByLabelText } = await renderPalette();
    await type(getByLabelText("命令面板搜索"), "match");
    // query "match" 不匹配任何项目名 → 会话区即全部结果，直接比对顺序
    await waitFor(() => {
      expect(sessionLabels(container)).toEqual(["hi00matc", "mid0matc", "lo00matc"]);
    });
  });

  test("命中超过上限：截断到 20 条 + 显式提示", async () => {
    const ids = Array.from({ length: 25 }, (_, i) => `cap${String(i).padStart(2, "0")}match`);
    await setSnapshot([
      group("Gcap", "cap", [worktree("pcap", "cap", ids, { mostRecentSession: 10 })], 10),
    ]);
    const { container, getByLabelText } = await renderPalette();
    await type(getByLabelText("命令面板搜索"), "match");
    await waitFor(() => {
      const rows = sessionLabels(container).filter((l) => l.startsWith("cap"));
      expect(rows.length).toBe(20);
      expect(container.textContent).toContain("仅显示前 20 条");
    });
  });

  test("title 未加载：显示 id 前缀 + 项目名定位，且不发补 title IPC", async () => {
    await setSnapshot(snapshot());
    const { container, getByLabelText } = await renderPalette(); // 未选项目 → 不应调 listGroupSessions
    await type(getByLabelText("命令面板搜索"), "aaaa1111");
    let row: Element | undefined;
    await waitFor(() => {
      row = Array.from(container.querySelectorAll(".cp-item")).find((el) =>
        el.querySelector(".cp-item-label")?.textContent?.startsWith("aaaa1111"),
      );
      expect(row).toBeTruthy();
    });
    // 标签是 id 前缀（无 title），detail 含项目名定位
    expect(row!.querySelector(".cp-item-label")?.textContent?.trim()).toBe("aaaa1111");
    expect(row!.querySelector(".cp-item-detail")?.textContent).toContain("alpha");
    // 未选项目 → 全程不为补 title 调 listGroupSessions
    expect(vi.mocked(listGroupSessions)).not.toHaveBeenCalled();
  });

  test("短查询提示在有 actions 时仍显示（不被 totalResults 挤掉）", async () => {
    await setSnapshot(snapshot());
    const { container, getByLabelText } = await renderPalette();
    await type(getByLabelText("命令面板搜索"), "abc"); // <4，未选项目
    await waitFor(() => {
      expect(container.textContent).toContain("个字符按 Session ID 全局定位");
    });
  });
});

describe("CommandPalette 跨组 stale 守卫", () => {
  test("相同 query 切到另一组：旧组正文命中不残留（双维守卫 query+projectId）", async () => {
    await setSnapshot(snapshot());
    vi.mocked(searchGroupSessions).mockImplementation((groupId: string) => {
      if (groupId === "p1") {
        return Promise.resolve({
          results: [{ sessionId: SID_A1, projectId: "p1", sessionTitle: "p1正文命中", totalMatches: 3 }],
          totalMatches: 3, sessionsSearched: 1, query: "zzzz", isPartial: false,
        }) as never;
      }
      return new Promise(() => {}) as never; // p2 的搜索 pending，模拟切组后新结果未到
    });
    const { container, getByLabelText, rerender } = await renderPalette({ selectedProjectId: "p1" });
    await type(getByLabelText("命令面板搜索"), "zzzz"); // 仅 B 路命中（无全局 id 命中）
    await waitFor(() => expect(container.textContent).toContain("p1正文命中"));

    // 相同 query 切到 p2，p2 搜索 pending → 旧 p1 命中应立即消失（不 stale-as-fresh）
    await rerender({ selectedProjectId: "p2", onSelectProject: vi.fn(), onClose: vi.fn() });
    await waitFor(() => expect(container.textContent).not.toContain("p1正文命中"));
  });
});

describe("CommandPalette stale 快照修复", () => {
  test("store 刷新后已打开面板的全局命中同步（新增可见）", async () => {
    await setSnapshot(snapshot());
    const { container, getByLabelText } = await renderPalette();
    await type(getByLabelText("命令面板搜索"), "eeee7777");
    await waitFor(() => {
      expect(sessionLabels(container).some((l) => l.startsWith("eeee7777"))).toBe(false);
    });
    // 模拟 file-change 后 store 刷新带入新会话
    const next = snapshot();
    next[0].worktrees[0].sessions.push("eeee7777-0000-0000-0000-000000000007");
    await setSnapshot(next);
    await waitFor(() => {
      expect(sessionLabels(container).some((l) => l.startsWith("eeee7777"))).toBe(true);
    });
  });
});
