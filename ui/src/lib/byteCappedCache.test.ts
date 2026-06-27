import { describe, test, expect } from "vitest";
import { ByteCappedCache } from "./byteCappedCache";

// sizeOf：每条按 key.length + value.length 计字节，便于精确断言。
function makeCache(maxEntries: number, maxBytes: number) {
  return new ByteCappedCache<string>({
    maxEntries,
    maxBytes,
    sizeOf: (key, value) => key.length + value.length,
  });
}

describe("ByteCappedCache", () => {
  test("命中返回值，未命中返回 undefined", () => {
    const c = makeCache(10, 1000);
    c.set("a", "x");
    expect(c.get("a")).toBe("x");
    expect(c.get("missing")).toBeUndefined();
  });

  test("count cap：超过条目数从最旧端淘汰", () => {
    const c = makeCache(2, 10_000);
    c.set("a", "1");
    c.set("b", "2");
    c.set("c", "3"); // 触发淘汰最旧的 a
    expect(c.size).toBe(2);
    expect(c.get("a")).toBeUndefined();
    expect(c.get("b")).toBe("2");
    expect(c.get("c")).toBe("3");
  });

  test("byte cap：累计字节超限即淘汰，byteSize 精确回收", () => {
    // 每条 key(1) + value(4) = 5 字节；maxBytes=10 只容得下 2 条。
    const c = makeCache(100, 10);
    c.set("a", "wxyz"); // 5
    c.set("b", "wxyz"); // 10
    expect(c.byteSize).toBe(10);
    c.set("c", "wxyz"); // 15 > 10 → 淘汰最旧 a，回到 10
    expect(c.byteSize).toBe(10);
    expect(c.size).toBe(2);
    expect(c.get("a")).toBeUndefined();
  });

  test("LRU touch：get 后最近使用的不被优先淘汰", () => {
    const c = makeCache(2, 10_000);
    c.set("a", "1");
    c.set("b", "2");
    c.get("a"); // a 移到最新端
    c.set("c", "3"); // 淘汰最旧的 b（不是 a）
    expect(c.get("a")).toBe("1");
    expect(c.get("b")).toBeUndefined();
    expect(c.get("c")).toBe("3");
  });

  test("覆写同 key：字节计数不重复累加", () => {
    const c = makeCache(10, 10_000);
    c.set("a", "xx"); // 1 + 2 = 3
    c.set("a", "yyyy"); // 1 + 4 = 5（先减旧 3 再加新 5）
    expect(c.size).toBe(1);
    expect(c.byteSize).toBe(5);
    expect(c.get("a")).toBe("yyyy");
  });

  test("单条超 maxBytes：清空其余后仍存入该条", () => {
    const c = makeCache(100, 5);
    c.set("a", "1"); // 2 字节
    c.set("big", "0123456789"); // 13 字节 > 5：清空 a 后仍存入
    expect(c.get("a")).toBeUndefined();
    expect(c.get("big")).toBe("0123456789");
    expect(c.size).toBe(1);
    expect(c.byteSize).toBe(13);
  });

  test("非纯 sizeOf：覆写/淘汰扣的是入账值而非重算，byteSize 不漂移", () => {
    // sizeOf 每次调用返回递增值——若覆写/淘汰时重算就会漂移甚至变负。
    let next = 10;
    const c = new ByteCappedCache<string>({
      maxEntries: 1, // 强制每次 set 触发淘汰
      maxBytes: 1_000_000,
      sizeOf: () => next++,
    });
    c.set("a", "x"); // 入账 10，byteSize=10
    expect(c.byteSize).toBe(10);
    c.set("b", "y"); // 淘汰 a（扣回入账的 10，不重算）→ 入账 11
    expect(c.size).toBe(1);
    expect(c.byteSize).toBe(11);
    c.set("b", "z"); // 覆写 b（扣回入账的 11，不重算）→ 入账 12
    expect(c.size).toBe(1);
    expect(c.byteSize).toBe(12);
    expect(c.byteSize).toBeGreaterThanOrEqual(0);
  });
});
