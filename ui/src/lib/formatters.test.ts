import { describe, expect, test } from "vitest";
import { formatClock, formatDuration, formatTokensCompact } from "./formatters";

describe("formatClock", () => {
  // 固定时刻：2026-05-17 T 06:23:05 UTC = 14:23:05 Asia/Shanghai (CI runners 默认 UTC，
  // 本地多在 Asia/Shanghai)。断言不锁具体小时数，而是断言 12h 模式含"上午/下午"前缀、
  // 24h 模式严格匹配 `HH:MM:SS` 且不含"上午/下午"——规避 TZ 漂移。
  const ts = new Date("2026-05-17T06:23:05Z");

  test("24h 模式产 HH:MM:SS 且不含上午/下午", () => {
    const out = formatClock(ts, false);
    expect(out).toMatch(/^\d{2}:\d{2}:\d{2}$/);
    expect(out).not.toMatch(/上午|下午/);
  });

  test("12h 模式产含上午或下午前缀", () => {
    const out = formatClock(ts, true);
    expect(out).toMatch(/上午|下午/);
  });

  test("接受 number (ms) 输入", () => {
    expect(formatClock(ts.getTime(), false)).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });

  test("接受 ISO string 输入", () => {
    expect(formatClock("2026-05-17T06:23:05Z", false)).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });

  test("非法输入返回空串而非抛错", () => {
    expect(formatClock("not-a-date", false)).toBe("");
    expect(formatClock(Number.NaN, true)).toBe("");
  });
});

describe("formatDuration", () => {
  test("null/undefined → null", () => {
    expect(formatDuration(null)).toBeNull();
    expect(formatDuration(undefined)).toBeNull();
  });

  test("ms / s / m s / h m 分段", () => {
    expect(formatDuration(500)).toBe("500ms");
    expect(formatDuration(1500)).toBe("1.5s");
    expect(formatDuration(65_000)).toBe("1m 5s");
    expect(formatDuration(60_000)).toBe("1m");
    expect(formatDuration(3_660_000)).toBe("1h 1m");
    expect(formatDuration(3_600_000)).toBe("1h");
  });
});

describe("formatTokensCompact", () => {
  test("k / M 阈值", () => {
    expect(formatTokensCompact(null)).toBe("0");
    expect(formatTokensCompact(999)).toBe("999");
    expect(formatTokensCompact(1_234)).toBe("1.2k");
    expect(formatTokensCompact(1_200_000)).toBe("1.2M");
  });
});
