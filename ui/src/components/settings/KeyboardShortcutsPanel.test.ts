/**
 * KeyboardShortcutsPanel 单测——覆盖 spec keyboard-shortcuts::Settings Scenarios：
 * - pending overlay flow：录键 commit 后 hasPending=true，渲染 pending bar 含 "未保存改动"
 * - Save → updateConfig IPC + applyOverrides + 清 pending；finalOverrides 由 committed 套 pending 构造
 * - "__RESET__" sentinel：handleReset 已提交 override 时 pending 写 sentinel，Save 后 finalOverrides 删 key
 * - preSaveConflict 串行注入防御：Save 路径再走 findConflict，命中时显示 alert + 不持久化
 * - 丢弃：清空 pending overlay，registry / committed 不变
 * - configLoadError banner：subscribeConfigLoadError 推送时显示 retry 按钮；点击 retry 调 getConfig
 * - 重置全部：updateConfig({}) + 清 committed
 *
 * 测试基础设施：vitest globals: false（显式 import）+ @testing-library/svelte + mockIPC（参考
 * lib/keyboard/__tests__/customization.test.ts 的注入模式）。registry 模块级 state 通过
 * `_resetForTest()` 在每个 test 间清理。
 */
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, test } from "vitest";

import KeyboardShortcutsPanel from "./KeyboardShortcutsPanel.svelte";
import {
  _resetForTest,
  setConfigLoadError,
  registerShortcut,
} from "../../lib/keyboard/registry";
import { _resetPlatformCache } from "../../lib/platform";

beforeEach(() => {
  // 强制 mac 平台，让 mod+w 展开为 meta+w，advisoryHints 才能命中"macOS Cmd+W"分支
  Object.defineProperty(navigator, "userAgentData", {
    value: { platform: "macOS" },
    configurable: true,
    writable: true,
  });
  _resetPlatformCache();
  _resetForTest();
});

afterEach(() => {
  cleanup();
  clearMocks();
  _resetForTest();
  _resetPlatformCache();
});

/**
 * 找 Save / 丢弃 / 重置全部 按钮：SettingsButton 渲染成 <button>，按 textContent / aria-label 取。
 * 注意 SettingsButton 文案可能含中英文，匹配用 includes 而非 ===。
 */
function findButtonByText(container: HTMLElement, text: string): HTMLButtonElement | null {
  const buttons = container.querySelectorAll("button");
  for (const btn of buttons) {
    if (btn.textContent?.includes(text)) return btn as HTMLButtonElement;
  }
  return null;
}

function getRecorderForId(container: HTMLElement, id: string): HTMLElement {
  const safeId = id.replace(/[^a-zA-Z0-9]/g, "-");
  // KeyRecorderInput 把 `id` 传给 hintId 走 aria-describedby；recorder div 自己不带 id 属性
  const el = container.querySelector(
    `[role="button"][aria-describedby="kbd-recorder-${safeId}-hint"]`,
  );
  if (!el) throw new Error(`recorder for ${id} not found`);
  return el as HTMLElement;
}

describe("KeyboardShortcutsPanel", () => {
  test("初始 mount：无 pending bar / 无 saveError / 无 configLoadError", () => {
    const { container } = render(KeyboardShortcutsPanel, {
      props: { initialOverrides: {} },
    });
    expect(container.querySelector(".pending-bar")).toBeNull();
    expect(container.querySelector(".banner-error")).toBeNull();
    // 至少渲染一个 category section
    expect(container.querySelectorAll(".category").length).toBeGreaterThan(0);
  });

  test("录键 commit 后 pending bar 渲染 + 含 '未保存改动'", async () => {
    const { container } = render(KeyboardShortcutsPanel, {
      props: { initialOverrides: {} },
    });
    // 在 sidebar.toggle 行的录键器上模拟 focus + 完整组合键
    const recorder = getRecorderForId(container, "sidebar.toggle");
    await fireEvent.focus(recorder);
    await fireEvent.keyDown(recorder, {
      key: "x",
      code: "KeyX",
      ctrlKey: true,
      shiftKey: true,
    });
    // commit 后 pending overlay 写入 → pending bar 渲染
    const bar = container.querySelector(".pending-bar");
    expect(bar).not.toBeNull();
    expect(bar!.textContent).toContain("1 项未保存改动");
  });

  test("丢弃：清空 pending overlay + pending bar 消失", async () => {
    const { container } = render(KeyboardShortcutsPanel, {
      props: { initialOverrides: {} },
    });
    const recorder = getRecorderForId(container, "sidebar.toggle");
    await fireEvent.focus(recorder);
    await fireEvent.keyDown(recorder, {
      key: "x",
      code: "KeyX",
      ctrlKey: true,
      shiftKey: true,
    });
    expect(container.querySelector(".pending-bar")).not.toBeNull();

    const discard = findButtonByText(container, "丢弃");
    expect(discard).not.toBeNull();
    await fireEvent.click(discard!);
    expect(container.querySelector(".pending-bar")).toBeNull();
  });

  test("Save：commit 后点 Save 调 updateConfig 并清 pending bar", async () => {
    let captured: { section?: string; data?: Record<string, string> } = {};
    mockIPC((cmd, payload) => {
      if (cmd === "update_config") {
        const args = payload as { section: string; configData: Record<string, string> };
        captured = { section: args.section, data: args.configData };
        return Promise.resolve({ keyboardShortcuts: args.configData });
      }
      return Promise.resolve(null);
    });

    const { container } = render(KeyboardShortcutsPanel, {
      props: { initialOverrides: {} },
    });
    const recorder = getRecorderForId(container, "sidebar.toggle");
    await fireEvent.focus(recorder);
    await fireEvent.keyDown(recorder, {
      key: "x",
      code: "KeyX",
      ctrlKey: true,
      shiftKey: true,
    });

    const save = findButtonByText(container, "保存");
    expect(save).not.toBeNull();
    await fireEvent.click(save!);
    // microtask flush
    await new Promise((r) => setTimeout(r, 0));

    expect(captured.section).toBe("keyboardShortcuts");
    expect(captured.data).toBeDefined();
    expect(Object.keys(captured.data!)).toContain("sidebar.toggle");
    // pending 清空 → bar 消失
    expect(container.querySelector(".pending-bar")).toBeNull();
  });

  test("__RESET__ sentinel：initialOverrides 含 override + 录回 default 后 Save，IPC 收到的 finalOverrides 删 key", async () => {
    let captured: Record<string, string> | undefined;
    mockIPC((cmd, payload) => {
      if (cmd === "update_config") {
        const args = payload as { section: string; configData: Record<string, string> };
        captured = args.configData;
        return Promise.resolve({ keyboardShortcuts: args.configData });
      }
      return Promise.resolve(null);
    });

    const { container } = render(KeyboardShortcutsPanel, {
      props: {
        // 已持久化的 override：sidebar.toggle 改成 mod+shift+b
        initialOverrides: { "sidebar.toggle": "mod+shift+b" },
      },
    });
    // 该行 reset 按钮 enabled（currentBinding != defaultEffective）
    const resetBtn = container.querySelector(
      'button[aria-label="重置 切换侧栏折叠 / 展开 为默认"]',
    ) as HTMLButtonElement | null;
    expect(resetBtn).not.toBeNull();
    expect(resetBtn!.disabled).toBe(false);

    await fireEvent.click(resetBtn!);
    expect(container.querySelector(".pending-bar")).not.toBeNull();

    const save = findButtonByText(container, "保存");
    await fireEvent.click(save!);
    await new Promise((r) => setTimeout(r, 0));

    expect(captured).toBeDefined();
    // finalOverrides 删 sidebar.toggle key（committed 含但 pending 标 __RESET__）
    expect(captured!["sidebar.toggle"]).toBeUndefined();
  });

  test("configLoadError banner：setConfigLoadError 推送 → 渲染 alert + 重试按钮", async () => {
    const { container } = render(KeyboardShortcutsPanel, {
      props: { initialOverrides: {} },
    });
    // mount 后通过 subscribe 推送
    setConfigLoadError("read fail");
    // microtask flush
    await new Promise((r) => setTimeout(r, 0));

    const banner = container.querySelector('.banner-error[role="alert"]');
    expect(banner).not.toBeNull();
    expect(banner!.textContent).toContain("read fail");

    // 重试按钮存在
    const retryBtn = findButtonByText(container, "重试");
    expect(retryBtn).not.toBeNull();
  });

  test("重置全部：committed 有 override 时按钮 enabled，点击调 updateConfig({})", async () => {
    let captured: Record<string, string> | undefined;
    mockIPC((cmd, payload) => {
      if (cmd === "update_config") {
        const args = payload as { section: string; configData: Record<string, string> };
        captured = args.configData;
        return Promise.resolve({ keyboardShortcuts: args.configData });
      }
      return Promise.resolve(null);
    });

    const { container } = render(KeyboardShortcutsPanel, {
      props: { initialOverrides: { "sidebar.toggle": "mod+shift+b" } },
    });
    const resetAll = findButtonByText(container, "重置全部");
    expect(resetAll).not.toBeNull();
    expect(resetAll!.disabled).toBe(false);

    await fireEvent.click(resetAll!);
    await new Promise((r) => setTimeout(r, 0));

    expect(captured).toEqual({});
  });

  test("重置全部：committed 空 + 无 pending → 按钮 disabled", () => {
    const { container } = render(KeyboardShortcutsPanel, {
      props: { initialOverrides: {} },
    });
    const resetAll = findButtonByText(container, "重置全部");
    expect(resetAll).not.toBeNull();
    expect(resetAll!.disabled).toBe(true);
  });

  test("preSaveConflict：commit 引发与已注册 spec 冲突 → Save 显示 alert + 不调 updateConfig", async () => {
    let updateCalled = false;
    mockIPC((cmd) => {
      if (cmd === "update_config") {
        updateCalled = true;
        return Promise.resolve({ keyboardShortcuts: {} });
      }
      return Promise.resolve(null);
    });

    // 让 sidebar.toggle 已占位 keymap = mod+b（默认）
    registerShortcut({
      id: "sidebar.toggle",
      category: "sidebar",
      description: "切换侧栏折叠 / 展开",
      defaultBinding: "mod+b",
      handler: () => {},
    });

    const { container } = render(KeyboardShortcutsPanel, {
      props: { initialOverrides: {} },
    });
    // 在 command-palette.toggle 行录入 mod+b（与 sidebar.toggle 冲突）
    const recorder = getRecorderForId(container, "command-palette.toggle");
    await fireEvent.focus(recorder);
    await fireEvent.keyDown(recorder, {
      key: "b",
      code: "KeyB",
      metaKey: true,
    });
    expect(container.querySelector(".pending-bar")).not.toBeNull();

    const save = findButtonByText(container, "保存");
    await fireEvent.click(save!);
    await new Promise((r) => setTimeout(r, 0));

    // Save 路径 findConflict 命中 → preSaveConflict banner 渲染
    const banners = container.querySelectorAll('.banner-error[role="alert"]');
    let foundPreSave = false;
    for (const b of banners) {
      if (b.textContent?.includes("串行冲突")) foundPreSave = true;
    }
    expect(foundPreSave).toBe(true);
    // updateConfig 未被调（Save 早期 return）
    expect(updateCalled).toBe(false);
  });
});
