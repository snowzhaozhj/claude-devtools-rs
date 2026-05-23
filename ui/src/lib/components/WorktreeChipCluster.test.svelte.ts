// WorktreeChipCluster.svelte 单测。
//
// 覆盖：
// - 按传入 options 顺序渲染（不在组件内排序，排序责任归 Sidebar 调用方）
// - 单选切换调 onChange，重复点已选 chip 不触发
// - ARIA：role="radiogroup" + 每个 chip role="radio" + aria-checked 反映选中
// - tabindex roving：选中 chip tabindex=0，其他 tabindex=-1
// - 「全部」chip 无 ⌗ 前缀
// - 键盘 ArrowRight / ArrowLeft 切换并即触发 onChange
// - 边界不绕回：最末按 ArrowRight 停尾、最首按 ArrowLeft 停首
// - Enter / Space 等价点击

import { afterEach, describe, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/svelte";

import WorktreeChipCluster from "./WorktreeChipCluster.svelte";

afterEach(() => cleanup());

const allOpts = [
  { value: "__all__", label: "全部" },
  { value: "wt-a", label: "⌗rust-port" },
  { value: "wt-b", label: "⌗feat-x" },
];

describe("WorktreeChipCluster 渲染", () => {
  test("按传入 options 顺序渲染 chip（不在组件内排序）", () => {
    const reordered = [
      { value: "wt-b", label: "⌗feat-x" },
      { value: "__all__", label: "全部" },
      { value: "wt-a", label: "⌗rust-port" },
    ];
    const { container } = render(WorktreeChipCluster, {
      value: "__all__",
      options: reordered,
      onChange: () => {},
    });
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    expect(chips).toHaveLength(3);
    expect(chips[0].textContent?.trim()).toBe("⌗feat-x");
    expect(chips[1].textContent?.trim()).toBe("全部");
    expect(chips[2].textContent?.trim()).toBe("⌗rust-port");
  });

  test("「全部」chip 无 ⌗ 前缀（与 worktree chip 视觉前缀拉开权重）", () => {
    const { container } = render(WorktreeChipCluster, {
      value: "__all__",
      options: allOpts,
      onChange: () => {},
    });
    const allChip = container.querySelector<HTMLButtonElement>(
      '[role="radio"]',
    );
    expect(allChip?.textContent?.trim()).toBe("全部");
    expect(allChip?.textContent?.includes("⌗")).toBe(false);
  });

  test("ARIA：cluster 用 role=radiogroup，每个 chip role=radio + aria-checked", () => {
    const { container } = render(WorktreeChipCluster, {
      value: "wt-a",
      options: allOpts,
      onChange: () => {},
      ariaLabel: "按 worktree 过滤会话",
    });
    const cluster = container.querySelector('[role="radiogroup"]');
    expect(cluster).not.toBeNull();
    expect(cluster!.getAttribute("aria-label")).toBe("按 worktree 过滤会话");
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    expect(chips[0].getAttribute("aria-checked")).toBe("false");
    expect(chips[1].getAttribute("aria-checked")).toBe("true");
    expect(chips[2].getAttribute("aria-checked")).toBe("false");
  });

  test("tabindex roving：选中 chip tabindex=0，其他 tabindex=-1", () => {
    const { container } = render(WorktreeChipCluster, {
      value: "wt-b",
      options: allOpts,
      onChange: () => {},
    });
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    expect(chips[0].getAttribute("tabindex")).toBe("-1");
    expect(chips[1].getAttribute("tabindex")).toBe("-1");
    expect(chips[2].getAttribute("tabindex")).toBe("0");
  });
});

describe("WorktreeChipCluster 单选切换", () => {
  test("点击未选中 chip 触发 onChange 传新 value", async () => {
    const onChange = vi.fn();
    const { container } = render(WorktreeChipCluster, {
      value: "__all__",
      options: allOpts,
      onChange,
    });
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    await fireEvent.click(chips[1]);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("wt-a");
  });

  test("点击已选中 chip 不触发 onChange（避免无意义重拉）", async () => {
    const onChange = vi.fn();
    const { container } = render(WorktreeChipCluster, {
      value: "wt-a",
      options: allOpts,
      onChange,
    });
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    await fireEvent.click(chips[1]);
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("WorktreeChipCluster 键盘导航", () => {
  test("ArrowRight 切到下一个 chip 并触发 onChange", async () => {
    const onChange = vi.fn();
    const { container } = render(WorktreeChipCluster, {
      value: "__all__",
      options: allOpts,
      onChange,
    });
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    await fireEvent.keyDown(chips[0], { key: "ArrowRight" });
    expect(onChange).toHaveBeenCalledWith("wt-a");
  });

  test("ArrowLeft 切到上一个 chip 并触发 onChange", async () => {
    const onChange = vi.fn();
    const { container } = render(WorktreeChipCluster, {
      value: "wt-b",
      options: allOpts,
      onChange,
    });
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    await fireEvent.keyDown(chips[2], { key: "ArrowLeft" });
    expect(onChange).toHaveBeenCalledWith("wt-a");
  });

  test("最末 chip 按 ArrowRight 停尾不绕回", async () => {
    const onChange = vi.fn();
    const { container } = render(WorktreeChipCluster, {
      value: "wt-b",
      options: allOpts,
      onChange,
    });
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    await fireEvent.keyDown(chips[2], { key: "ArrowRight" });
    expect(onChange).not.toHaveBeenCalled();
  });

  test("最首 chip 按 ArrowLeft 停首不绕回", async () => {
    const onChange = vi.fn();
    const { container } = render(WorktreeChipCluster, {
      value: "__all__",
      options: allOpts,
      onChange,
    });
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    await fireEvent.keyDown(chips[0], { key: "ArrowLeft" });
    expect(onChange).not.toHaveBeenCalled();
  });

  test("Enter 等价点击：聚焦未选中 chip 按 Enter 触发选中", async () => {
    const onChange = vi.fn();
    const { container } = render(WorktreeChipCluster, {
      value: "__all__",
      options: allOpts,
      onChange,
    });
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    await fireEvent.keyDown(chips[2], { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("wt-b");
  });

  test("Space 等价点击：聚焦未选中 chip 按 Space 触发选中", async () => {
    const onChange = vi.fn();
    const { container } = render(WorktreeChipCluster, {
      value: "__all__",
      options: allOpts,
      onChange,
    });
    const chips = container.querySelectorAll<HTMLButtonElement>(
      '[role="radio"]',
    );
    await fireEvent.keyDown(chips[1], { key: " " });
    expect(onChange).toHaveBeenCalledWith("wt-a");
  });
});
