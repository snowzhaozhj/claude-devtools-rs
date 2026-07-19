import { describe, test, expect } from "vitest";
import {
  BOUNDED_LINE_THRESHOLD,
  BOUNDED_BYTE_THRESHOLD,
  OVERSIZED_LINE_THRESHOLD,
  utf8ByteLength,
  countLines,
  classifyBySize,
  classifyText,
  sizingForToolOutput,
  sliceHeadTail,
  sliceLineIndices,
  trimBlankEdgeLines,
  SLICE_MAX_LINES_PER_SIDE,
  SLICE_MAX_BYTES_PER_SIDE,
} from "./outputSizing";

describe("utf8ByteLength", () => {
  test("ascii 与字节等长", () => {
    expect(utf8ByteLength("abc")).toBe(3);
  });
  test("多字节中文按 UTF-8 计（每字 3 字节）", () => {
    expect(utf8ByteLength("中文")).toBe(6);
    // string.length 会是 2，必须不同
    expect(utf8ByteLength("中文")).not.toBe("中文".length);
  });
  test("emoji（代理对）按 UTF-8 4 字节", () => {
    expect(utf8ByteLength("😀")).toBe(4);
  });
});

describe("countLines", () => {
  test("空串 0 行", () => {
    expect(countLines("")).toBe(0);
  });
  test("单行无换行 1 行", () => {
    expect(countLines("abc")).toBe(1);
  });
  test("末尾单换行不额外计空行", () => {
    expect(countLines("a\nb\n")).toBe(2);
  });
  test("中间空行计入", () => {
    expect(countLines("a\n\nb")).toBe(3);
  });
});

describe("classifyBySize（>= 升档 + 任一维度达标）", () => {
  test("短内容 inline", () => {
    expect(classifyBySize(10, 100, true)).toBe("inline");
  });
  test("恰好 80 行升 bounded", () => {
    expect(classifyBySize(BOUNDED_LINE_THRESHOLD, 100, true)).toBe("bounded");
  });
  test("恰好 16 KiB 升 bounded", () => {
    expect(classifyBySize(1, BOUNDED_BYTE_THRESHOLD, true)).toBe("bounded");
  });
  test("恰好 1000 行升 oversized（允许时）", () => {
    expect(classifyBySize(OVERSIZED_LINE_THRESHOLD, 100, true)).toBe("oversized");
  });
  test("prose 路径 oversized 降级 bounded（不允许切片）", () => {
    expect(classifyBySize(OVERSIZED_LINE_THRESHOLD, 100, false)).toBe("bounded");
  });
  test("极长单行按字节升档，不因行数低判 inline", () => {
    expect(classifyBySize(1, BOUNDED_BYTE_THRESHOLD, true)).toBe("bounded");
    expect(classifyBySize(1, 512 * 1024, true)).toBe("oversized");
  });
});

describe("classifyText 多字节字节度量", () => {
  test("大量中文按 UTF-8 字节升档", () => {
    // 6000 个中文 = 18000 字节 >= 16 KiB，但 string.length=6000 < 阈值字节数
    const zh = "中".repeat(6000);
    expect(classifyText(zh, true)).toBe("bounded");
  });
});

describe("sizingForToolOutput（懒加载稳定分档）", () => {
  test("已加载真实内容按内容分档", () => {
    expect(
      sizingForToolOutput({ loadedText: "short", outputBytes: undefined, omitted: false, allowOversized: true }),
    ).toEqual({ ready: true, tier: "inline" });
  });
  test("omitted + outputBytes 缺失 → fetch-first bounded 占位", () => {
    expect(
      sizingForToolOutput({ loadedText: null, outputBytes: undefined, omitted: true, allowOversized: true }),
    ).toEqual({ ready: false, tier: "bounded" });
  });
  test("omitted + 字节短 → 仍 bounded 占位（行数未知不判 inline）", () => {
    expect(
      sizingForToolOutput({ loadedText: null, outputBytes: 100, omitted: true, allowOversized: true }),
    ).toEqual({ ready: false, tier: "bounded" });
  });
  test("omitted + 字节达 oversized → oversized 占位（工具路径）", () => {
    expect(
      sizingForToolOutput({ loadedText: null, outputBytes: 512 * 1024, omitted: true, allowOversized: true }),
    ).toEqual({ ready: false, tier: "oversized" });
  });
  test("prose 路径字节达 oversized 仍 bounded（不切片）", () => {
    expect(
      sizingForToolOutput({ loadedText: null, outputBytes: 512 * 1024, omitted: true, allowOversized: false }),
    ).toEqual({ ready: false, tier: "bounded" });
  });
  test("预估短档字节但真实 2001 行 → 加载后校正 oversized", () => {
    // 未加载：outputBytes=2000 短，但锁定 bounded 占位（不 inline）
    const loading = sizingForToolOutput({ loadedText: null, outputBytes: 2000, omitted: true, allowOversized: true });
    expect(loading).toEqual({ ready: false, tier: "bounded" });
    // 加载后：2001 行 → oversized
    const text = "x\n".repeat(2001);
    const loaded = sizingForToolOutput({ loadedText: text, outputBytes: 2000, omitted: true, allowOversized: true });
    expect(loaded.ready).toBe(true);
    expect(loaded.tier).toBe("oversized");
  });
});

describe("sliceHeadTail（首尾切片预算与安全）", () => {
  test("行数不足两侧上限之和 → 不切片返回 null", () => {
    const text = Array.from({ length: 100 }, (_, i) => `line ${i}`).join("\n");
    expect(sliceHeadTail(text)).toBeNull();
  });
  test("超大多行切首尾 + 省略量精确", () => {
    const total = 2000;
    const text = Array.from({ length: total }, (_, i) => `line ${i}`).join("\n");
    const sliced = sliceHeadTail(text);
    expect(sliced).not.toBeNull();
    expect(sliced!.omittedLines).toBe(total - countLines(sliced!.head) - countLines(sliced!.tail));
    expect(sliced!.omittedLines).toBeGreaterThan(0);
  });
  test("无换行超大单行按字节切码点安全、不整行重复", () => {
    const text = "a".repeat(500 * 1024);
    const sliced = sliceHeadTail(text);
    expect(sliced).not.toBeNull();
    // 首尾各不超过每侧字节上限，且不覆盖整行
    expect(utf8ByteLength(sliced!.head)).toBeLessThanOrEqual(128 * 1024);
    expect(utf8ByteLength(sliced!.tail)).toBeLessThanOrEqual(128 * 1024);
    expect(sliced!.omittedBytes).toBeGreaterThan(0);
  });
  test("多字节单行切分不拆码点", () => {
    const text = "中".repeat(200 * 1024); // 600 KiB
    const sliced = sliceHeadTail(text);
    expect(sliced).not.toBeNull();
    // 切出的片段仍是合法中文（不含替换字符），逐字符校验字节数为 3
    for (const ch of sliced!.head) expect(utf8ByteLength(ch)).toBe(3);
    for (const ch of sliced!.tail) expect(utf8ByteLength(ch)).toBe(3);
  });
});

// 结构化行数组切片索引（Read 行号数组 / Diff 行数组接入，spec
// tool-viewer-routing::首尾切片的渲染上限与切分安全）。
describe("sliceLineIndices（结构化行数组切片索引）", () => {
  test("行数不足两侧上限之和 → 不切片返回 null", () => {
    expect(sliceLineIndices(Array.from({ length: 800 }, () => 10))).toBeNull();
  });
  test("超大多行给出首尾索引 + 省略量精确", () => {
    const total = 2000;
    const idx = sliceLineIndices(Array.from({ length: total }, () => 10));
    expect(idx).not.toBeNull();
    expect(idx!.headCount).toBe(SLICE_MAX_LINES_PER_SIDE);
    expect(idx!.tailCount).toBe(SLICE_MAX_LINES_PER_SIDE);
    expect(idx!.omittedLines).toBe(total - idx!.headCount - idx!.tailCount);
    // 省略字节 = 总量 − 首尾实渲量（每行 +1 近似换行）
    expect(idx!.omittedBytes).toBe(idx!.omittedLines * 11);
  });
  test("每侧字节上限先达即停（长行少量即触顶）", () => {
    // 每行 64 KiB：字节预算 128 KiB/侧 → 每侧只能取 ~2 行
    const idx = sliceLineIndices(Array.from({ length: 2000 }, () => 64 * 1024));
    expect(idx).not.toBeNull();
    expect(idx!.headCount).toBeLessThanOrEqual(
      Math.ceil(SLICE_MAX_BYTES_PER_SIDE / (64 * 1024)),
    );
    expect(idx!.headCount).toBeGreaterThan(0);
  });
  test("首行超字节预算仍必取一行（不空切）", () => {
    const idx = sliceLineIndices([
      512 * 1024,
      ...Array.from({ length: 1500 }, () => 10),
      512 * 1024,
    ]);
    expect(idx).not.toBeNull();
    expect(idx!.headCount).toBeGreaterThanOrEqual(1);
    expect(idx!.tailCount).toBeGreaterThanOrEqual(1);
  });
});

describe("trimBlankEdgeLines（首尾整条空白行修剪）", () => {
  test("删首尾整条空白行（LF）", () => {
    expect(trimBlankEdgeLines("\n\n0 errors\n\n")).toBe("0 errors");
  });

  test("兼容 CRLF 首部 + 保留末行自身尾随空格/tab", () => {
    expect(trimBlankEdgeLines("\r\n\r\nrecord \t\r\n\r\n")).toBe("record \t");
  });

  test("保留中间空白行 + 末行有意义尾随空格", () => {
    // 中间空行不动；最后一个非空行 "  b   " 的行首缩进 + 尾随空格全留。
    expect(trimBlankEdgeLines("a\n\n  b   ")).toBe("a\n\n  b   ");
  });

  test("纯空白全变空串：无换行纯空格 / tab", () => {
    expect(trimBlankEdgeLines("   ")).toBe("");
    expect(trimBlankEdgeLines("\t \t")).toBe("");
  });

  test("纯空白全变空串：换行 + 尾随空格无终止换行", () => {
    // codex 报的 EOF 无换行空白变体：\n\n  / \n\n  \n 都应判空。
    expect(trimBlankEdgeLines("\n\n  ")).toBe("");
    expect(trimBlankEdgeLines("\n\n  \n")).toBe("");
  });

  test("首行自身缩进保留（首个非空行不是整条空白行）", () => {
    expect(trimBlankEdgeLines("\n  indented\nnext")).toBe("  indented\nnext");
  });

  test("无首尾空白行：原样返回", () => {
    expect(trimBlankEdgeLines("line1\nline2")).toBe("line1\nline2");
  });

  test("性能：首尾有内容 + 大量中间空白行 O(n) 无回溯（守卫二次退化）", () => {
    // 正则 /(?:\r?\n[ \t]*)+$/ 在此形态近二次回溯（8000 行数百 ms）；
    // 单遍扫描应在毫秒级。中间空白行须完整保留。
    const mid = "\n ".repeat(20000);
    const input = `head${mid}tail`;
    const t0 = performance.now();
    const out = trimBlankEdgeLines(input);
    const dt = performance.now() - t0;
    expect(out).toBe(input); // 首尾都是非空白行，整体不变
    expect(dt).toBeLessThan(50);
  });
});
