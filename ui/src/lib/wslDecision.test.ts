// wslDecision 决策函数单测。
//
// 覆盖 spec settings-ui::"General section Use WSL 按钮" 的 5 类 scenario：
// 1. 单 candidate → auto-apply
// 2. 多 candidate → select
// 3. 0 candidate + 0 distrosWithoutHome → no-distro
// 4. 0 candidate + N distrosWithoutHome → all-failed
// 5. （IPC 调用失败由 SettingsView 在 catch 块兜底，不进 decideWslAction）

import { describe, expect, test } from "vitest";

import type { WslDistroCandidate } from "./api";
import { decideWslAction } from "./wslDecision";

const ubuntu: WslDistroCandidate = {
  distro: "Ubuntu",
  homePath: "/home/alice",
  claudeRootPath: "\\\\wsl.localhost\\Ubuntu\\home\\alice\\.claude",
  claudeRootExists: true,
};
const debian: WslDistroCandidate = {
  distro: "Debian-12",
  homePath: "/root",
  claudeRootPath: "\\\\wsl.localhost\\Debian-12\\root\\.claude",
  claudeRootExists: false,
};

describe("decideWslAction", () => {
  test("单 candidate 走 auto-apply", () => {
    const decision = decideWslAction({
      candidates: [ubuntu],
      distrosWithoutHome: [],
    });
    expect(decision.kind).toBe("auto-apply");
    if (decision.kind === "auto-apply") {
      expect(decision.candidate.distro).toBe("Ubuntu");
    }
  });

  test("多 candidate 走 select", () => {
    const decision = decideWslAction({
      candidates: [ubuntu, debian],
      distrosWithoutHome: [],
    });
    expect(decision.kind).toBe("select");
    if (decision.kind === "select") {
      expect(decision.candidates).toHaveLength(2);
      expect(decision.candidates.map((c) => c.distro)).toEqual([
        "Ubuntu",
        "Debian-12",
      ]);
    }
  });

  test("空 candidates + 空 distrosWithoutHome → no-distro 提示", () => {
    const decision = decideWslAction({
      candidates: [],
      distrosWithoutHome: [],
    });
    expect(decision.kind).toBe("no-distro");
    if (decision.kind === "no-distro") {
      expect(decision.message).toBe("未检测到 WSL distro");
    }
  });

  test("空 candidates 但 distrosWithoutHome 非空 → all-failed", () => {
    const decision = decideWslAction({
      candidates: [],
      distrosWithoutHome: ["Ubuntu", "Debian-12"],
    });
    expect(decision.kind).toBe("all-failed");
    if (decision.kind === "all-failed") {
      expect(decision.distros).toEqual(["Ubuntu", "Debian-12"]);
      expect(decision.message).toContain("Ubuntu");
      expect(decision.message).toContain("Debian-12");
      expect(decision.message).toContain("无法解析 home");
    }
  });

  test("all-failed 文案保留 distro 名顺序", () => {
    const decision = decideWslAction({
      candidates: [],
      distrosWithoutHome: ["Z-distro", "A-distro"],
    });
    expect(decision.kind).toBe("all-failed");
    if (decision.kind === "all-failed") {
      // 不重新排序——保持 backend 原始返回顺序
      expect(decision.distros).toEqual(["Z-distro", "A-distro"]);
      expect(decision.message.indexOf("Z-distro")).toBeLessThan(
        decision.message.indexOf("A-distro"),
      );
    }
  });
});
