/**
 * ShortcutRow 单测——覆盖 spec keyboard-shortcuts::Settings 录键 widget Scenarios：
 * - reset 按钮 disabled 状态：currentBinding === defaultEffective 时
 * - conflict prop 非 null 时渲染 row-hint-warning + role="alert" 文案
 * - advisoryHints 多条独立 role="note" 渲染（macOS Cmd+W + 浏览器拦截两条同时显示）
 * - default kbd 文本：formatShortcut 平台映射展示
 * - disabled prop 透传到 KeyRecorderInput + reset button（row 加 row-disabled class）
 *
 * 测试基础设施：vitest globals: false（显式 import）+ @testing-library/svelte。
 */
import { cleanup, render } from "@testing-library/svelte";
import { afterEach, describe, expect, test, vi } from "vitest";

import ShortcutRow from "./ShortcutRow.svelte";
import type { ShortcutMeta } from "../../lib/keyboard/defaults";

afterEach(() => {
  cleanup();
});

const meta: ShortcutMeta = {
  id: "tab.close",
  category: "tabs",
  description: "关闭当前 tab",
  defaultBinding: "mod+w",
};

describe("ShortcutRow", () => {
  test("currentBinding === defaultEffective 时 reset 按钮 disabled", () => {
    const { container } = render(ShortcutRow, {
      props: {
        meta,
        currentBinding: "meta+w", // mac 平台 mod 展开
        defaultEffective: "meta+w",
        onCommit: vi.fn(),
        onReset: vi.fn(),
      },
    });
    // ariaLabel 包含 "重置 ... 为默认"
    const resetBtn = container.querySelector(
      'button[aria-label="重置 关闭当前 tab 为默认"]',
    ) as HTMLButtonElement | null;
    expect(resetBtn).not.toBeNull();
    expect(resetBtn!.disabled).toBe(true);
    expect(resetBtn!.getAttribute("title")).toBe("已是默认值");
  });

  test("currentBinding !== defaultEffective 时 reset 按钮 enabled", () => {
    const { container } = render(ShortcutRow, {
      props: {
        meta,
        currentBinding: "meta+shift+w",
        defaultEffective: "meta+w",
        onCommit: vi.fn(),
        onReset: vi.fn(),
      },
    });
    const resetBtn = container.querySelector(
      'button[aria-label="重置 关闭当前 tab 为默认"]',
    ) as HTMLButtonElement;
    expect(resetBtn.disabled).toBe(false);
    expect(resetBtn.getAttribute("title")).toBe("重置为默认");
  });

  test("conflict 非 null 时渲染 row-hint-warning + alert 文案", () => {
    const { container } = render(ShortcutRow, {
      props: {
        meta,
        currentBinding: "meta+k",
        defaultEffective: "meta+w",
        onCommit: vi.fn(),
        onReset: vi.fn(),
        conflict: { conflictId: "command-palette.toggle", conflictLabel: "打开 / 关闭命令面板" },
      },
    });
    const warning = container.querySelector(".row-hint-warning") as HTMLElement | null;
    expect(warning).not.toBeNull();
    expect(warning!.getAttribute("role")).toBe("alert");
    expect(warning!.textContent).toContain("打开 / 关闭命令面板");
    expect(warning!.textContent).toContain("冲突");
  });

  test("conflict null 时不渲染 warning hint", () => {
    const { container } = render(ShortcutRow, {
      props: {
        meta,
        currentBinding: "meta+w",
        defaultEffective: "meta+w",
        onCommit: vi.fn(),
        onReset: vi.fn(),
      },
    });
    expect(container.querySelector(".row-hint-warning")).toBeNull();
  });

  test("advisoryHints 多条独立 role='note' 渲染", () => {
    const { container } = render(ShortcutRow, {
      props: {
        meta,
        currentBinding: "meta+w",
        defaultEffective: "meta+w",
        onCommit: vi.fn(),
        onReset: vi.fn(),
        advisoryHints: [
          "macOS 上 ⌘W 由系统优先处理",
          "浏览器中 Ctrl+W 会被原生拦截",
        ],
      },
    });
    const notes = container.querySelectorAll('.row-hint-info[role="note"]');
    expect(notes.length).toBe(2);
    expect(notes[0].textContent).toContain("⌘W 由系统优先处理");
    expect(notes[1].textContent).toContain("Ctrl+W 会被原生拦截");
  });

  test("disabled 时 row 加 row-disabled class + reset 按钮 disabled", () => {
    const { container } = render(ShortcutRow, {
      props: {
        meta,
        currentBinding: "meta+shift+w",
        defaultEffective: "meta+w",
        onCommit: vi.fn(),
        onReset: vi.fn(),
        disabled: true,
      },
    });
    const row = container.querySelector(".row") as HTMLElement;
    expect(row.classList.contains("row-disabled")).toBe(true);
    // reset 按钮即使 currentBinding != default 也 disabled（disabled 短路 isAtDefault）
    const resetBtn = container.querySelector(
      'button[aria-label="重置 关闭当前 tab 为默认"]',
    ) as HTMLButtonElement;
    expect(resetBtn.disabled).toBe(true);
  });

  test("默认 kbd 渲染 formatShortcut 后的字符（mac 上 ⌘）", () => {
    const { container } = render(ShortcutRow, {
      props: {
        meta,
        currentBinding: "meta+w",
        defaultEffective: "meta+w",
        onCommit: vi.fn(),
        onReset: vi.fn(),
      },
    });
    const kbd = container.querySelector(".row-default kbd") as HTMLElement;
    // 不强测平台映射；仅断言出现 W 字符（formatShortcut 始终 uppercase 主键）
    expect(kbd.textContent).toMatch(/W/);
  });
});
