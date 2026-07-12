// ReadToolViewer 分档单测。
//
// 对应 spec Scenario（tool-viewer-routing）：
// - 三档分级（内容面 = strip 后输出）+ 超大切片保留真实文件行号
// - markdown 富文本不切片（preview 模式 oversized 降级 bounded），
//   切 code 模式后才允许 top/tail 切片
// - 懒加载 loading / failed 态复制禁用（copy-to-clipboard 未就绪禁用）

import { describe, expect, test, afterEach } from "vitest";
import { cleanup, render, fireEvent } from "@testing-library/svelte";

import ReadToolViewer from "./ReadToolViewer.svelte";
import type { ToolExecution } from "../../lib/api";

afterEach(() => {
  cleanup();
});

function readExec(filePath: string, catNText: string): ToolExecution {
  return {
    toolUseId: "tr-1",
    toolName: "Read",
    input: { file_path: filePath },
    output: { kind: "text", text: catNText },
    isError: false,
    startTs: "2026-01-01T00:00:00Z",
    endTs: "2026-01-01T00:00:01Z",
    sourceAssistantUuid: "a-1",
  } as ToolExecution;
}

/** cat -n 风格 `<num>\t<text>` 内容，行号从 startLine 起。 */
function catN(lines: number, startLine = 1, text = (i: number) => `line body ${i}`): string {
  return Array.from({ length: lines }, (_, i) => `${startLine + i}\t${text(startLine + i)}`).join("\n");
}

describe("ReadToolViewer 分档", () => {
  test("短文件完整内联：无信息气味、无限高 class", () => {
    const { container } = render(ReadToolViewer, {
      props: { exec: readExec("/src/a.rs", catN(10)) },
    });
    expect(container.querySelector(".file-scent")).toBeNull();
    expect(container.querySelector(".code-container.bounded")).toBeNull();
  });

  test("中长文件限高预览：信息气味 + bounded class，完整内容留 DOM", () => {
    const { container } = render(ReadToolViewer, {
      props: { exec: readExec("/src/a.rs", catN(120)) },
    });
    const scent = container.querySelector(".file-scent");
    expect(scent!.textContent).toContain("120 行");
    expect(container.querySelector(".code-container.bounded")).not.toBeNull();
    expect(container.textContent).toContain("line body 60");
  });

  test("超大文件首尾切片：接缝 + 中段不在 DOM + tail 保留真实文件行号", () => {
    const { container } = render(ReadToolViewer, {
      props: { exec: readExec("/src/a.rs", catN(2000)) },
    });
    const seam = container.querySelector(".read-seam");
    expect(seam).not.toBeNull();
    expect(seam!.textContent).toContain("已省略");
    expect(container.textContent).not.toContain("line body 1000");
    // tail 首行 = 第 1601 行（2000 - 400），data-line 必须是真实文件行号
    const tailFirst = container.querySelector('.line[data-line="1601"]');
    expect(tailFirst).not.toBeNull();
  });

  test("markdown preview 不切片（无接缝），切 code 模式后才切片", async () => {
    const { container } = render(ReadToolViewer, {
      props: { exec: readExec("/docs/a.md", catN(2000, 1, (i) => `- item ${i}`)) },
    });
    // 默认 preview：富文本不切片 → 无接缝
    expect(container.querySelector(".read-seam")).toBeNull();
    expect(container.querySelector(".md-preview.bounded")).not.toBeNull();
    // 切 code 模式：行导向 → 允许切片
    const toggle = container.querySelector<HTMLButtonElement>(".view-toggle");
    await fireEvent.click(toggle!);
    expect(container.querySelector(".read-seam")).not.toBeNull();
  });

  test("loading / failed：复制禁用 + 原因标签", () => {
    const { container } = render(ReadToolViewer, {
      props: { exec: readExec("/src/a.rs", ""), outputLoading: true },
    });
    const btn = container.querySelector<HTMLButtonElement>(".file-header button:last-of-type");
    expect(btn?.disabled).toBe(true);
    expect(btn?.getAttribute("aria-label")).toContain("加载中");

    cleanup();
    const { container: c2 } = render(ReadToolViewer, {
      props: { exec: readExec("/src/a.rs", ""), outputLoadFailed: true },
    });
    expect(c2.textContent).toContain("加载失败");
    const btn2 = c2.querySelector<HTMLButtonElement>(".file-header button:last-of-type");
    expect(btn2?.disabled).toBe(true);
  });
});
