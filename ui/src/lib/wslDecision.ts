// WSL distro 扫描结果 → UI 行为的纯函数决策。
//
// SettingsView 调 listWslDistros 拿 report 后用本函数决定下一步 UI 行为；
// spec 的 5 类 scenario（settings-ui::"General section Use WSL 按钮"）在
// 这一层守护，避免在 SettingsView 内直接散开 if/else 分支难以单测。
//
// 见 openspec/specs/settings-ui/spec.md::"General section Use WSL 按钮"。

import type { WslDistroCandidate, WslDistroScanReport } from "./api";

export type WslDecision =
  | { kind: "auto-apply"; candidate: WslDistroCandidate }
  | { kind: "select"; candidates: WslDistroCandidate[] }
  | { kind: "no-distro"; message: string }
  | { kind: "all-failed"; distros: string[]; message: string };

export function decideWslAction(report: WslDistroScanReport): WslDecision {
  const { candidates, distrosWithoutHome } = report;

  if (candidates.length === 1) {
    return { kind: "auto-apply", candidate: candidates[0] };
  }

  if (candidates.length >= 2) {
    return { kind: "select", candidates };
  }

  if (distrosWithoutHome.length > 0) {
    return {
      kind: "all-failed",
      distros: distrosWithoutHome,
      message: `检测到 WSL distro 但无法解析 home：${distrosWithoutHome.join(", ")}`,
    };
  }

  return { kind: "no-distro", message: "未检测到 WSL distro" };
}
