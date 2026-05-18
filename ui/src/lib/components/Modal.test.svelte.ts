// Modal.svelte 通用 dialog 行为单测。
//
// 覆盖：
// - open=true 时渲染 dialog；open=false 时不渲染
// - ESC 关闭 → onClose 触发
// - 点击 overlay 关闭 → onClose 触发；点击 dialog 内部不触发
// - 主按钮触发 onPrimary；disabled 时不触发
// - a11y：role=dialog + aria-modal=true + aria-labelledby（有 title 时）

import { afterEach, describe, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/svelte";
import { tick } from "svelte";

import Modal from "./Modal.svelte";

afterEach(() => cleanup());

describe("Modal 渲染", () => {
  test("open=false 时不渲染任何 dialog", () => {
    const { container } = render(Modal, {
      open: false,
      onClose: () => {},
    });
    expect(container.querySelector('[role="dialog"]')).toBeNull();
  });

  test("open=true 时渲染 dialog 含 a11y 属性", () => {
    const { container } = render(Modal, {
      open: true,
      title: "选择 WSL distro",
      onClose: () => {},
    });
    const dialog = container.querySelector('[role="dialog"]');
    expect(dialog).not.toBeNull();
    expect(dialog!.getAttribute("aria-modal")).toBe("true");
    expect(dialog!.getAttribute("aria-labelledby")).toBe("cdt-modal-title");
    expect(container.querySelector("#cdt-modal-title")?.textContent).toBe(
      "选择 WSL distro",
    );
  });
});

describe("Modal 交互", () => {
  test("ESC 关闭触发 onClose", async () => {
    const onClose = vi.fn();
    const { container } = render(Modal, { open: true, onClose });
    const overlay = container.querySelector(".modal-overlay") as HTMLElement;
    await fireEvent.keyDown(overlay, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test("点击 overlay 关闭触发 onClose", async () => {
    const onClose = vi.fn();
    const { container } = render(Modal, { open: true, onClose });
    const overlay = container.querySelector(".modal-overlay") as HTMLElement;
    await fireEvent.click(overlay);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test("点击 dialog 内部 不触发 onClose", async () => {
    const onClose = vi.fn();
    const { container } = render(Modal, {
      open: true,
      title: "title",
      onClose,
    });
    const titleEl = container.querySelector("#cdt-modal-title") as HTMLElement;
    await fireEvent.click(titleEl);
    expect(onClose).not.toHaveBeenCalled();
  });

  test("主按钮点击触发 onPrimary", async () => {
    const onPrimary = vi.fn();
    const { container } = render(Modal, {
      open: true,
      onPrimary,
      onClose: () => {},
      primaryLabel: "应用",
    });
    await tick();
    const primaryBtn = Array.from(
      container.querySelectorAll<HTMLButtonElement>("button"),
    ).find((b) => b.textContent?.trim() === "应用");
    expect(primaryBtn).toBeTruthy();
    await fireEvent.click(primaryBtn!);
    expect(onPrimary).toHaveBeenCalledTimes(1);
  });

  test("primaryDisabled=true 时主按钮不可点击", async () => {
    const onPrimary = vi.fn();
    const { container } = render(Modal, {
      open: true,
      onPrimary,
      primaryDisabled: true,
      onClose: () => {},
      primaryLabel: "应用",
    });
    await tick();
    const primaryBtn = Array.from(
      container.querySelectorAll<HTMLButtonElement>("button"),
    ).find((b) => b.textContent?.trim() === "应用");
    expect(primaryBtn?.disabled).toBe(true);
  });

  test("不传 onPrimary 时不渲染主按钮", () => {
    const { container } = render(Modal, {
      open: true,
      onClose: () => {},
    });
    const buttons = container.querySelectorAll<HTMLButtonElement>("button");
    // 仅渲染取消按钮一个
    expect(buttons.length).toBe(1);
    expect(buttons[0].textContent?.trim()).toBe("取消");
  });
});
