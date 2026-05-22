import { describe, expect, test } from "vitest";

import { readAndValidateApiBase } from "./runtime";

describe("readAndValidateApiBase", () => {
  test("returns null when query has no apiBase", () => {
    expect(readAndValidateApiBase("")).toBeNull();
    expect(readAndValidateApiBase("?http=1")).toBeNull();
    expect(readAndValidateApiBase("?foo=bar&baz=qux")).toBeNull();
  });

  test("accepts http://localhost with port", () => {
    expect(
      readAndValidateApiBase("?apiBase=http%3A%2F%2Flocalhost%3A3456"),
    ).toBe("http://localhost:3456");
    expect(
      readAndValidateApiBase("?apiBase=http%3A%2F%2Flocalhost%3A4000"),
    ).toBe("http://localhost:4000");
  });

  test("accepts http://127.0.0.1", () => {
    expect(
      readAndValidateApiBase("?apiBase=http%3A%2F%2F127.0.0.1%3A3456"),
    ).toBe("http://127.0.0.1:3456");
  });

  test("accepts https variants", () => {
    expect(
      readAndValidateApiBase("?apiBase=https%3A%2F%2Flocalhost%3A8443"),
    ).toBe("https://localhost:8443");
  });

  test("normalizes to origin (strips path / query / fragment)", () => {
    // `?apiBase=http://localhost:3456/evil?x=1` 注入：URL.origin 只取
    // `<protocol>//<host>:<port>` 不含 path/query，让后续 ${base}/api/... 拼接安全
    expect(
      readAndValidateApiBase(
        "?apiBase=http%3A%2F%2Flocalhost%3A3456%2Fevil%3Fx%3D1",
      ),
    ).toBe("http://localhost:3456");
  });

  test("rejects non-localhost hosts (防 origin 劫持)", () => {
    expect(
      readAndValidateApiBase("?apiBase=http%3A%2F%2Fevil.com"),
    ).toBeNull();
    expect(
      readAndValidateApiBase("?apiBase=http%3A%2F%2Flocalhost.evil.com"),
    ).toBeNull();
    expect(
      readAndValidateApiBase("?apiBase=http%3A%2F%2F8.8.8.8"),
    ).toBeNull();
  });

  test("rejects non-http(s) protocols (防 javascript: / file: 注入)", () => {
    expect(
      readAndValidateApiBase("?apiBase=javascript%3Aalert%281%29"),
    ).toBeNull();
    expect(
      readAndValidateApiBase("?apiBase=file%3A%2F%2F%2Fetc%2Fpasswd"),
    ).toBeNull();
    expect(
      readAndValidateApiBase("?apiBase=ftp%3A%2F%2Flocalhost%3A21"),
    ).toBeNull();
  });

  test("rejects malformed URL", () => {
    expect(readAndValidateApiBase("?apiBase=not-a-url")).toBeNull();
    expect(readAndValidateApiBase("?apiBase=%2F%2Frelative%2Fpath")).toBeNull();
  });
});
