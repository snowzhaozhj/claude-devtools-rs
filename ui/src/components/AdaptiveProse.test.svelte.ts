// AdaptiveProse（prose 两档轻量框）组件单测。
//
// 对应 spec Scenario（session-display）：
// - 短文本输出完整内联
// - 长文本输出限高预览且完整内容留在 DOM
// - markdown prose 不做 top/tail 切片（oversized 降级 bounded）

import { describe, expect, test, afterEach } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";

import AdaptiveProse from "./AdaptiveProse.svelte";

afterEach(() => {
  cleanup();
});

function bodySnippet(html: string) {
  return createRawSnippet(() => ({ render: () => `<div class="prose-body">${html}</div>` }));
}

describe("AdaptiveProse 两档", () => {
  test("短 prose 完整内联：无框架 header、无预览提示", () => {
    const { container } = render(AdaptiveProse, {
      props: { text: "short text", body: bodySnippet("<p>short text</p>") },
    });
    expect(container.querySelector(".ao")).toBeNull();
    expect(container.textContent).not.toContain("预览");
    expect(container.querySelector(".prose-body")).not.toBeNull();
  });

  test("长 prose 限高预览：轻量 variant + 信息气味 + 完整内容留 DOM", () => {
    const text = Array.from({ length: 100 }, (_, i) => `para ${i}`).join("\n");
    const { container } = render(AdaptiveProse, {
      props: { text, body: bodySnippet("<p>rendered</p>") },
    });
    const frame = container.querySelector(".ao");
    expect(frame).not.toBeNull();
    expect(frame!.classList.contains("ao-prose")).toBe(true);
    const scent = container.querySelector(".ao-scent");
    expect(scent!.textContent).toContain("100 行");
    expect(scent!.textContent).toContain("预览");
    // body 完整渲染在限高 viewport 内（不切片）
    expect(container.querySelector(".ao-viewport .prose-body")).not.toBeNull();
  });

  test("超大 prose 不切片：仍是限高预览（无省略接缝语义）", () => {
    const text = Array.from({ length: 2000 }, (_, i) => `para ${i}`).join("\n");
    const { container } = render(AdaptiveProse, {
      props: { text, body: bodySnippet("<p>rendered</p>") },
    });
    // 单一 viewport 完整渲染（AdaptiveProse 走 allowOversized=false）
    expect(container.querySelectorAll(".ao-viewport").length).toBe(1);
    expect(container.querySelector(".prose-body")).not.toBeNull();
  });
});
