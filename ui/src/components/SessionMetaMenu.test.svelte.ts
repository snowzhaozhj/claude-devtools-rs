// SessionMetaMenu 单测：覆盖 spec session-display::SessionDetail 顶 bar
// meta-action menu 入口的全部 scenarios。
//
// 覆盖：
// - trigger 渲染 + a11y 属性（haspopup / expanded / controls）
// - 点击 trigger 展开 menu，role=menu + orientation=vertical + 项序
// - 平台分支：Tauri runtime 渲染 3 项 + 分隔线；HTTP server mode 隐藏 Finder 项
// - cwd 缺失降级：前两项 disabled，第三项可用
// - 点击「在文件管理器中打开」调 openPath
// - 点击「复制工作目录路径」/「复制 Session ID」调 clipboard.writeText
// - 反馈状态：成功显「已复制」、失败显「打开失败」/「复制失败」、1500ms 自动恢复
// - ESC / 外部 click 关闭 menu
// - 焦点回到 trigger

import { describe, expect, test, beforeEach, afterEach, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/svelte";
import { tick } from "svelte";

import SessionMetaMenu from "./SessionMetaMenu.svelte";

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn(() => Promise.resolve()),
}));
vi.mock("../lib/runtime", () => ({
  isTauriRuntime: vi.fn(() => true),
}));

import { openPath } from "@tauri-apps/plugin-opener";
import { isTauriRuntime } from "../lib/runtime";

const SAMPLE_CWD = "/Users/test/project/feat-foo";
const SAMPLE_SID = "01234567-89ab-cdef-0123-456789abcdef";

let writeTextMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  writeTextMock = vi.fn(() => Promise.resolve());
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: writeTextMock },
  });
  vi.mocked(isTauriRuntime).mockReturnValue(true);
  vi.mocked(openPath).mockReset().mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function getTrigger(container: HTMLElement): HTMLButtonElement {
  const btn = container.querySelector('button[aria-haspopup="menu"]');
  if (!btn) throw new Error("trigger not found");
  return btn as HTMLButtonElement;
}

describe("SessionMetaMenu trigger 渲染", () => {
  test("trigger 含 ARIA 属性 + 默认关闭", () => {
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    const trigger = getTrigger(container);
    expect(trigger.getAttribute("aria-haspopup")).toBe("menu");
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(trigger.getAttribute("aria-label")).toBe("会话操作");
    expect(trigger.getAttribute("aria-controls")).toMatch(/^session-meta-menu-/);
    expect(container.querySelector('[role="menu"]')).toBeNull();
  });

  test("trigger 含 SVG icon 且无 text label", () => {
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    const trigger = getTrigger(container);
    expect(trigger.querySelector("svg")).not.toBeNull();
    expect(trigger.textContent?.trim()).toBe("");
  });
});

describe("SessionMetaMenu menu 展开", () => {
  test("点击 trigger 展开 menu，aria-expanded=true，role=menu 出现", async () => {
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    const trigger = getTrigger(container);
    await fireEvent.click(trigger);
    await tick();
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
    const menu = container.querySelector('[role="menu"]');
    expect(menu).not.toBeNull();
    expect(menu!.getAttribute("aria-orientation")).toBe("vertical");
  });

  test("Tauri mode 渲染 6 项（含 3 导出项）+ 2 个 separator", async () => {
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    const items = container.querySelectorAll('[role="menuitem"]');
    expect(items.length).toBe(6);
    expect(items[0].textContent).toMatch(/(Finder|文件管理器)/);
    expect(items[1].textContent).toContain("复制工作目录路径");
    expect(items[2].textContent).toContain("复制 Session ID");
    expect(items[3].textContent).toContain("Markdown");
    expect(items[4].textContent).toContain("JSON");
    expect(items[5].textContent).toContain("HTML");
    const seps = container.querySelectorAll('[role="separator"]');
    expect(seps.length).toBe(2);
  });

  test("HTTP server mode 隐藏 Finder 项 + 1 个 separator（导出组前）", async () => {
    vi.mocked(isTauriRuntime).mockReturnValue(false);
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    const items = container.querySelectorAll('[role="menuitem"]');
    expect(items.length).toBe(5);
    expect(items[0].textContent).toContain("复制工作目录路径");
    expect(items[1].textContent).toContain("复制 Session ID");
    expect(items[2].textContent).toContain("Markdown");
    expect(container.querySelectorAll('[role="separator"]').length).toBe(1);
  });

  test("cwd 缺失：前两项 disabled，复制 Session ID 可用", async () => {
    const { container } = render(SessionMetaMenu, {
      cwd: undefined,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    const items = container.querySelectorAll('[role="menuitem"]');
    expect(items[0].getAttribute("aria-disabled")).toBe("true");
    expect(items[1].getAttribute("aria-disabled")).toBe("true");
    expect(items[2].getAttribute("aria-disabled")).toBeNull();
  });
});

describe("SessionMetaMenu 操作 + 反馈", () => {
  test("点击「在 Finder 中打开」调 openPath(cwd)，menu 关闭", async () => {
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    const items = container.querySelectorAll('[role="menuitem"]');
    await fireEvent.click(items[0] as HTMLElement);
    await tick();
    expect(openPath).toHaveBeenCalledWith(SAMPLE_CWD);
    expect(container.querySelector('[role="menu"]')).toBeNull();
  });

  test("点击「复制工作目录」调 clipboard.writeText(cwd) + 显「已复制」", async () => {
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    const items = container.querySelectorAll('[role="menuitem"]');
    await fireEvent.click(items[1] as HTMLElement);
    await tick();
    await tick();
    expect(writeTextMock).toHaveBeenCalledWith(SAMPLE_CWD);
    expect(container.querySelector('[role="status"]')?.textContent).toContain("已复制");
  });

  test("点击「复制 Session ID」调 clipboard.writeText(sessionId)", async () => {
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    const items = container.querySelectorAll('[role="menuitem"]');
    await fireEvent.click(items[2] as HTMLElement);
    await tick();
    await tick();
    expect(writeTextMock).toHaveBeenCalledWith(SAMPLE_SID);
  });

  test("openPath reject 显「打开失败」红字", async () => {
    vi.mocked(openPath).mockRejectedValueOnce(new Error("permission"));
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    const items = container.querySelectorAll('[role="menuitem"]');
    await fireEvent.click(items[0] as HTMLElement);
    await tick();
    await tick();
    const toast = container.querySelector('[role="status"]');
    expect(toast?.textContent).toContain("打开失败");
    expect(toast?.classList.contains("meta-toast-error")).toBe(true);
  });

  test("clipboard reject 显「复制失败」", async () => {
    writeTextMock.mockRejectedValueOnce(new Error("not allowed"));
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    const items = container.querySelectorAll('[role="menuitem"]');
    await fireEvent.click(items[1] as HTMLElement);
    await tick();
    await tick();
    expect(container.querySelector('[role="status"]')?.textContent).toContain("复制失败");
  });

  test("反馈 toast 在 1500ms 后消失", async () => {
    vi.useFakeTimers();
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    const items = container.querySelectorAll('[role="menuitem"]');
    await fireEvent.click(items[2] as HTMLElement);
    await Promise.resolve();
    await Promise.resolve();
    await tick();
    expect(container.querySelector('[role="status"]')).not.toBeNull();
    vi.advanceTimersByTime(1600);
    await tick();
    expect(container.querySelector('[role="status"]')).toBeNull();
  });
});

describe("SessionMetaMenu 关闭行为", () => {
  test("ESC 关闭 menu 且焦点回 trigger", async () => {
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    const trigger = getTrigger(container);
    await fireEvent.click(trigger);
    await tick();
    await fireEvent.keyDown(window, { key: "Escape" });
    await tick();
    expect(container.querySelector('[role="menu"]')).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  test("外部 click 关闭 menu", async () => {
    const { container } = render(SessionMetaMenu, {
      cwd: SAMPLE_CWD,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    expect(container.querySelector('[role="menu"]')).not.toBeNull();
    // 在 window 上 mousedown 模拟外部点击（target 不在 trigger / menu 内）
    const outsideEl = document.createElement("div");
    document.body.appendChild(outsideEl);
    await fireEvent.mouseDown(outsideEl);
    await tick();
    expect(container.querySelector('[role="menu"]')).toBeNull();
    outsideEl.remove();
  });

  test("点击 disabled item 不触发动作 / 不关闭 menu（aria-disabled 路径）", async () => {
    const { container } = render(SessionMetaMenu, {
      cwd: undefined,
      sessionId: SAMPLE_SID,
      projectId: "test-project-id",
    });
    await fireEvent.click(getTrigger(container));
    await tick();
    const items = container.querySelectorAll('[role="menuitem"]');
    await fireEvent.click(items[1] as HTMLElement);
    await tick();
    expect(writeTextMock).not.toHaveBeenCalled();
  });
});
