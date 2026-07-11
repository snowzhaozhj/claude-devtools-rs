// WriteToolViewer 分档单测。
//
// 对应 spec Scenario（tool-viewer-routing）：
// - 写入型工具按输入内容规模分档：分档依据待写入内容（input.content），
//   SHALL NOT 因输出回执很小而判入完整内联档。
// - 超大行导向输出首尾切片（code 模式）。

import { describe, expect, test, afterEach } from "vitest";
import { cleanup, render } from "@testing-library/svelte";

import WriteToolViewer from "./WriteToolViewer.svelte";
import type { ToolExecution } from "../../lib/api";

afterEach(() => {
  cleanup();
});

function writeExec(content: string): ToolExecution {
  return {
    toolUseId: "tw-1",
    toolName: "Write",
    input: { file_path: "/src/big.rs", content },
    // 回执极小：分档不得依据它
    output: { kind: "text", text: "File written successfully." },
    isError: false,
    startTs: "2026-01-01T00:00:00Z",
    endTs: "2026-01-01T00:00:01Z",
    sourceAssistantUuid: "a-1",
  } as ToolExecution;
}

describe("WriteToolViewer 按输入内容分档", () => {
  test("短内容完整内联：无信息气味", () => {
    const { container } = render(WriteToolViewer, {
      props: { exec: writeExec("fn main() {}\n") },
    });
    expect(container.querySelector(".write-scent")).toBeNull();
  });

  test("中长待写入内容 + 极小回执 → 按输入升限高档（信息气味出现）", () => {
    const content = Array.from({ length: 120 }, (_, i) => `line ${i}`).join("\n");
    const { container } = render(WriteToolViewer, {
      props: { exec: writeExec(content) },
    });
    const scent = container.querySelector(".write-scent");
    expect(scent).not.toBeNull();
    expect(scent!.textContent).toContain("120 行");
    expect(scent!.textContent).toContain("预览");
    expect(container.querySelector(".write-code-container.bounded")).not.toBeNull();
  });

  test("超大待写入内容 → 首尾切片 + 省略接缝，中段不在 DOM", () => {
    const content = Array.from({ length: 2000 }, (_, i) => `line ${i}`).join("\n");
    const { container } = render(WriteToolViewer, {
      props: { exec: writeExec(content) },
    });
    const seam = container.querySelector(".write-seam");
    expect(seam).not.toBeNull();
    expect(seam!.textContent).toContain("已省略");
    expect(container.textContent).not.toContain("line 1000\n");
  });
});
