/**
 * KeyRecorderInput 单测——覆盖 spec keyboard-shortcuts::Settings 录键 widget Scenarios：
 * - 进 recording 调 suspend()，离开 recording 调 resume()（dispatcher 引用计数）
 * - commit-on-fullkey：normalize 返回非空（含主键）时 onCommit + blur
 * - 仅 modifier 按下不 commit（normalize 返回空串）
 * - Escape 取消，不 commit，blur 后 resume
 * - currentBinding 空串时 displayText = "未绑定"，placeholder 视觉态
 * - conflict prop 非 null + 非 recording 时附 conflict class
 * - disabled 时 tabindex=-1 + click/keydown 不进 recording
 *
 * 测试基础设施：vitest globals: false（显式 import）+ @testing-library/svelte。
 */
import { cleanup, fireEvent, render } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import KeyRecorderInput from "./KeyRecorderInput.svelte";
import * as registry from "../../lib/keyboard/registry";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("KeyRecorderInput", () => {
  let suspendSpy: ReturnType<typeof vi.spyOn>;
  let resumeSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    // suspend / resume 是 module 级引用计数，每个 test 独立 spy
    suspendSpy = vi.spyOn(registry, "suspend").mockImplementation(() => {});
    resumeSpy = vi.spyOn(registry, "resume").mockImplementation(() => {});
  });

  function getRecorder(container: HTMLElement): HTMLElement {
    const el = container.querySelector('[role="button"]');
    if (!el) throw new Error("recorder root not found");
    return el as HTMLElement;
  }

  test("focus 进 recording 调 suspend()，blur 离开调 resume()", async () => {
    const { container } = render(KeyRecorderInput, {
      props: { currentBinding: "meta+k", onCommit: vi.fn() },
    });
    const recorder = getRecorder(container);

    await fireEvent.focus(recorder);
    expect(suspendSpy).toHaveBeenCalledTimes(1);
    expect(resumeSpy).not.toHaveBeenCalled();

    await fireEvent.blur(recorder);
    expect(resumeSpy).toHaveBeenCalledTimes(1);
  });

  test("commit-on-fullkey：含主键时 onCommit + 调 resume()", async () => {
    const onCommit = vi.fn();
    const { container } = render(KeyRecorderInput, {
      props: { currentBinding: "", onCommit },
    });
    const recorder = getRecorder(container);
    await fireEvent.focus(recorder);

    // 仅 modifier：normalize 返空串，不 commit
    await fireEvent.keyDown(recorder, { key: "Shift", code: "ShiftLeft", shiftKey: true });
    expect(onCommit).not.toHaveBeenCalled();
    expect(resumeSpy).not.toHaveBeenCalled();

    // 完整组合键：normalize 返 "ctrl+shift+k"（mac）/"ctrl+shift+k"（其他），均触发 commit
    await fireEvent.keyDown(recorder, {
      key: "k",
      code: "KeyK",
      ctrlKey: true,
      shiftKey: true,
    });
    expect(onCommit).toHaveBeenCalledTimes(1);
    const committed = onCommit.mock.calls[0][0] as string;
    // 不强测平台映射，只确认非空且包含 "k"（normalize 主键归一小写）
    expect(committed).toBeTruthy();
    expect(committed.toLowerCase()).toContain("k");
    // commit 后 blur → resume
    expect(resumeSpy).toHaveBeenCalledTimes(1);
  });

  test("Escape 取消：不 commit，但 blur + resume", async () => {
    const onCommit = vi.fn();
    const { container } = render(KeyRecorderInput, {
      props: { currentBinding: "meta+k", onCommit },
    });
    const recorder = getRecorder(container);
    await fireEvent.focus(recorder);
    expect(suspendSpy).toHaveBeenCalled();

    await fireEvent.keyDown(recorder, { key: "Escape", code: "Escape" });
    expect(onCommit).not.toHaveBeenCalled();
    expect(resumeSpy).toHaveBeenCalledTimes(1);
  });

  test("currentBinding 空串：displayText = '未绑定'", () => {
    const { container } = render(KeyRecorderInput, {
      props: { currentBinding: "", onCommit: vi.fn() },
    });
    const recorder = getRecorder(container);
    expect(recorder.textContent).toContain("未绑定");
  });

  test("conflict prop 非 null + 非 recording 态附 conflict class", async () => {
    const { container } = render(KeyRecorderInput, {
      props: {
        currentBinding: "meta+k",
        onCommit: vi.fn(),
        conflict: { conflictId: "tab.close", conflictLabel: "关闭标签页" },
      },
    });
    const recorder = getRecorder(container);
    expect(recorder.classList.contains("conflict")).toBe(true);

    // recording 期间 conflict class 让位（避免 conflict 边色覆盖 recording 边色）
    await fireEvent.focus(recorder);
    expect(recorder.classList.contains("recording")).toBe(true);
    expect(recorder.classList.contains("conflict")).toBe(false);
  });

  test("disabled 时 tabindex=-1 + 点击不进 recording", async () => {
    const onCommit = vi.fn();
    const { container } = render(KeyRecorderInput, {
      props: { currentBinding: "meta+k", onCommit, disabled: true },
    });
    const recorder = getRecorder(container);
    expect(recorder.getAttribute("tabindex")).toBe("-1");

    // click → handleClick 调 focus，但 startRecording guard `if (disabled) return`
    await fireEvent.click(recorder);
    expect(suspendSpy).not.toHaveBeenCalled();
    expect(recorder.classList.contains("recording")).toBe(false);
  });

  test("录键期间 dispatcher 不触发已注册快捷键（spec scenario 9.4）", async () => {
    // 真 registry：注册一条 sidebar.toggle 走 mod+b（mac 平台 → meta+b）
    // suspend / resume 不能 spy（需要真生效），先 restore 一次再独立跑
    suspendSpy.mockRestore();
    resumeSpy.mockRestore();
    // 强制 mac 平台让 mod 展开为 meta（jsdom 默认 UA 不含 mac，会展开成 ctrl）
    Object.defineProperty(navigator, "userAgentData", {
      value: { platform: "macOS" },
      configurable: true,
      writable: true,
    });
    const platform = await import("../../lib/platform");
    platform._resetPlatformCache();
    const realRegistry = await import("../../lib/keyboard/registry");
    realRegistry._resetForTest();

    const handler = vi.fn();
    realRegistry.registerShortcut({
      id: "sidebar.toggle",
      category: "sidebar",
      description: "切换侧栏",
      defaultBinding: "mod+b",
      handler,
    });

    const { container } = render(KeyRecorderInput, {
      props: { currentBinding: "", onCommit: vi.fn() },
    });
    const recorder = getRecorder(container);

    // 先 focus 进 recording → suspend()
    await fireEvent.focus(recorder);
    expect(realRegistry.isSuspended()).toBe(true);

    // 直接在 document 上派一发"sidebar.toggle 应触发"的 keydown（绕过录键器自身 stopPropagation）
    const event = new KeyboardEvent("keydown", {
      key: "b",
      code: "KeyB",
      metaKey: true,
      bubbles: true,
      cancelable: true,
    });
    document.dispatchEvent(event);
    // suspendCount > 0 → dispatcher 早期 return，handler 不应被调
    expect(handler).not.toHaveBeenCalled();

    // blur 回 resume → 再派一发应该触发
    await fireEvent.blur(recorder);
    expect(realRegistry.isSuspended()).toBe(false);
    document.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "b",
        code: "KeyB",
        metaKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );
    expect(handler).toHaveBeenCalledTimes(1);

    realRegistry._resetForTest();
  });

  test("idle 态按 Enter 进 recording", async () => {
    const { container } = render(KeyRecorderInput, {
      props: { currentBinding: "meta+k", onCommit: vi.fn() },
    });
    const recorder = getRecorder(container);
    await fireEvent.focus(recorder);
    // 第一次 focus 即 startRecording
    expect(suspendSpy).toHaveBeenCalledTimes(1);

    // 模拟 idle 态进 recording 的额外路径：手工 stopRecording 后 Enter 触发
    await fireEvent.blur(recorder);
    expect(resumeSpy).toHaveBeenCalledTimes(1);
    // blur 之后再触发 Enter（不 focus）→ handleKeyDown idle 分支
    await fireEvent.keyDown(recorder, { key: "Enter", code: "Enter" });
    // suspend 被调第二次
    expect(suspendSpy).toHaveBeenCalledTimes(2);
  });
});
