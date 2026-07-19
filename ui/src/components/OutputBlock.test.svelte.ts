// OutputBlock（自适应输出框架承载）组件单测。
//
// 对应 spec Scenario：
// - tool-viewer-routing::短工具输出完整内联 / 中长工具输出限高预览带信息气味 /
//   超大行导向输出首尾切片 / 省略量等于被省略的真实量
// - tool-viewer-routing::outputBytes 缺失时先加载再分档（loading 占位 + 复制禁用）
// - copy-to-clipboard::复制完整原文而非可见片段 / 完整原文未就绪时复制入口禁用

import { describe, expect, test, beforeEach, afterEach, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/svelte";

import OutputBlock from "./OutputBlock.svelte";

let writeTextMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  writeTextMock = vi.fn(() => Promise.resolve());
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: writeTextMock },
  });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const shortCode = "line1\nline2\n";
const boundedCode = Array.from({ length: 100 }, (_, i) => `line ${i}`).join("\n");
const oversizedCode = Array.from({ length: 2000 }, (_, i) => `line ${i}`).join("\n");

describe("OutputBlock 分级渲染", () => {
  test("短输出完整内联：无 header、无预览提示、复制常驻", () => {
    const { container } = render(OutputBlock, { props: { code: shortCode, lang: "text" } });
    expect(container.querySelector(".ao-header")).toBeNull();
    expect(container.textContent).not.toContain("预览");
    // 复制入口常驻（inline 右上角），非 hover-only
    expect(container.querySelector(".ao-inline-copy button")).not.toBeNull();
  });

  test("中长输出限高预览：header 信息气味含总行数、总字节数与预览状态", () => {
    const { container } = render(OutputBlock, { props: { code: boundedCode, lang: "text" } });
    const scent = container.querySelector(".ao-scent");
    expect(scent).not.toBeNull();
    expect(scent!.textContent).toContain("100 行");
    expect(scent!.textContent).toContain("预览");
    // 完整内容留 DOM（不切片）
    expect(container.querySelector(".output-seam")).toBeNull();
    expect(container.textContent).toContain("line 50");
  });

  test("超大输出首尾切片：省略接缝标注省略量，中段不在 DOM", () => {
    const { container } = render(OutputBlock, { props: { code: oversizedCode, lang: "text" } });
    const seam = container.querySelector(".output-seam");
    expect(seam).not.toBeNull();
    // 2000 行、每侧 400 行 → 省略 1200 行
    expect(seam!.textContent).toContain("已省略");
    expect(seam!.textContent).toContain("1200 行");
    // 中段（第 1000 行）不渲染
    expect(container.textContent).not.toContain("line 1000\n");
  });

  test("复制指向完整原文而非可见切片", async () => {
    const { container } = render(OutputBlock, { props: { code: oversizedCode, lang: "text" } });
    const btn = container.querySelector(".ao-header button") as HTMLButtonElement;
    expect(btn).not.toBeNull();
    await fireEvent.click(btn);
    expect(writeTextMock).toHaveBeenCalledWith(oversizedCode);
  });

  test("首尾空白行修剪：显示去空框，但复制仍为完整原文（图1 回归 + spec 复制原文）", async () => {
    // 终端输出常以空行开头 / 结尾（cargo / git / kbase fetch 等），
    // white-space:pre 忠实渲染会在框顶留空行 → 配常驻 copy icon 成"空框"。
    const raw = "\n\n0 errors\n\n";
    const { container } = render(OutputBlock, { props: { code: raw, lang: "text" } });
    // 显示修剪首尾空白行
    const pre = container.querySelector(".output-pre code");
    expect(pre!.textContent).toBe("0 errors");
    // 复制仍是完整原文（spec copy-to-clipboard::复制该输出面的完整原文）
    const btn = container.querySelector(".ao-inline-copy button") as HTMLButtonElement;
    await fireEvent.click(btn);
    expect(writeTextMock).toHaveBeenCalledWith(raw);
  });

  test("修剪只删整条空白行：保留 CRLF 首部去除 + 最后一行有意义的尾随空格", () => {
    // CRLF 空行需被首部修剪吃掉；最后一个非空行自身的尾随空格/tab 不得误删。
    const raw = "\r\n\r\nrecord \t\r\n\r\n";
    const { container } = render(OutputBlock, { props: { code: raw, lang: "text" } });
    const pre = container.querySelector(".output-pre code");
    // 首尾整条空白行（含 CRLF）删除，"record \t" 的尾随空格 tab 保留
    expect(pre!.textContent).toBe("record \t");
  });

  test("纯空白输出：不渲染带边框空 <pre>，仅极简占位 + 禁用复制入口（空框变体 + spec 空内容禁用）", () => {
    const { container } = render(OutputBlock, { props: { code: "\n\n  \n", lang: "text" } });
    expect(container.querySelector(".output-pre")).toBeNull();
    expect(container.querySelector(".ao")).toBeNull();
    expect(container.querySelector(".output-empty")).not.toBeNull();
    // spec copy-to-clipboard::空内容复制入口 SHALL 禁用并说明原因（不移除入口）
    const btn = container.querySelector(".output-empty button") as HTMLButtonElement;
    expect(btn).not.toBeNull();
    expect(btn.disabled).toBe(true);
    expect(btn.getAttribute("aria-label")).toContain("空");
  });

  test("无终止换行的纯空白也判空（EOF 无换行变体）", () => {
    // codex 报的边界：'   ' / '\n\n  ' 之前会漏判 → 残留空框
    for (const raw of ["   ", "\n\n  "]) {
      const { container, unmount } = render(OutputBlock, { props: { code: raw, lang: "text" } });
      expect(container.querySelector(".output-pre")).toBeNull();
      expect(container.querySelector(".output-empty")).not.toBeNull();
      unmount();
    }
  });

  test("loadFailed：显式失败态（无 aria-busy 假占位）+ 复制禁用 + 原因标签", () => {
    const { container } = render(OutputBlock, {
      props: { code: "", loadFailed: true, bytesHint: 32 * 1024 },
    });
    expect(container.querySelector('[aria-busy="true"]')).toBeNull();
    expect(container.textContent).toContain("加载失败");
    const btn = container.querySelector(".ao-header button") as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    expect(btn.getAttribute("aria-label")).toContain("加载失败");
  });

  test("loading：稳定占位 + 复制禁用 + 已知字节量气味", () => {
    const { container } = render(OutputBlock, {
      props: { code: "", loading: true, bytesHint: 32 * 1024 },
    });
    expect(container.querySelector('[aria-busy="true"]')).not.toBeNull();
    const btn = container.querySelector(".ao-header button") as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    expect(btn.getAttribute("aria-label")).toContain("加载中");
    const scent = container.querySelector(".ao-scent");
    expect(scent!.textContent).toContain("载入中");
    expect(scent!.textContent).not.toContain("0 行");
  });
});
