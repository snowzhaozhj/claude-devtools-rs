/*
 * pathLabel 中段截断算法。
 *
 * spec: openspec/specs/frontend-context-menu/spec.md
 *   ::Requirement ContextMenuItem 类型扩展 / AppContextMenu 视觉规格扩展
 * design: D-V6 + D7 pathLabel 字段
 *
 * CSS 原生不支持中段 ellipsis（`text-overflow: ellipsis` 仅末段），路径类
 * label 需要保留首段（home prefix `~/` 识别上下文）+ 尾段（文件名识别目标），
 * 中间用 `…` 省略。
 *
 * 算法：
 * 1. home 缩写：以 `process.env.HOME` 或常见 `/Users/...` / `/home/...` /
 *    `/root` / `C:\\Users\\<name>\\...` 起首时替换为 `~/`
 * 2. 总长 ≤ 50 直接返回 short = full（无截断）
 * 3. 总长 > 50 时取前缀（`~/` 或 root segment 共 8 字符内）+ `…/` +
 *    尾段（最多 30 字符，含 `…/` 占位）
 *
 * 设计取舍：
 * - 不依赖 Tauri / Node API 检测 home（前端模块要在 vitest jsdom + 浏览器
 *   `?http=1` 浏览器 mock 模式都能跑），用纯 string-prefix 模式匹配
 * - "首段 + 尾段" 模式对齐 macOS Finder / VS Code 路径显示惯例
 * - 总长上限 50 是经验值（D-V6 max-width 320px / 13px label font 实测约 50 字符宽）
 */

const MAX_TOTAL = 50;
const ELLIPSIS = "…";

/**
 * 把绝对路径转成显示用的中段截断形态。
 *
 * @param fullPath 绝对路径（macOS / Linux POSIX 或 Windows 反斜杠）
 * @returns `{ short, full }`：short 用于显示，full 用作 hover title
 */
export function truncatePath(fullPath: string): { short: string; full: string } {
  if (!fullPath) return { short: "", full: "" };

  const homeAbbreviated = abbreviateHome(fullPath);

  // 总长 ≤ MAX_TOTAL 不截断
  if (homeAbbreviated.length <= MAX_TOTAL) {
    return { short: homeAbbreviated, full: fullPath };
  }

  // 中段截断：保留首段 + ELLIPSIS + 尾段
  const sep = homeAbbreviated.includes("\\") && !homeAbbreviated.includes("/")
    ? "\\"
    : "/";
  const segments = homeAbbreviated.split(sep);

  // 单段路径（罕见）— 走简单首尾保留
  if (segments.length <= 2) {
    const headLen = 8;
    const tailLen = MAX_TOTAL - headLen - ELLIPSIS.length;
    const head = homeAbbreviated.slice(0, headLen);
    const tail = homeAbbreviated.slice(homeAbbreviated.length - tailLen);
    return { short: `${head}${ELLIPSIS}${tail}`, full: fullPath };
  }

  // 多段：保留首段 + 尾文件名（含倒数第二个目录名让 context 更清晰）
  const head = segments[0]; // "~" 或 "" 或 root segment
  const tail = segments[segments.length - 1]; // 文件名

  // 优先尝试 head/.../parentDir/tail
  if (segments.length >= 3) {
    const parent = segments[segments.length - 2];
    const candidate = `${head}${sep}${ELLIPSIS}${sep}${parent}${sep}${tail}`;
    if (candidate.length <= MAX_TOTAL) {
      return { short: candidate, full: fullPath };
    }
  }

  // fallback: head/.../tail（必要时尾段也截断）
  const headPart = `${head}${sep}${ELLIPSIS}${sep}`;
  const tailBudget = MAX_TOTAL - headPart.length;
  const tailDisplay = tail.length <= tailBudget
    ? tail
    : `${ELLIPSIS}${tail.slice(tail.length - tailBudget + ELLIPSIS.length)}`;
  return { short: `${headPart}${tailDisplay}`, full: fullPath };
}

/**
 * 把 home 目录前缀替换为 `~/`，覆盖：
 * - macOS：`/Users/<name>/...`
 * - Linux：`/home/<name>/...` / `/root`
 * - Windows：`C:\\Users\\<name>\\...`（反斜杠）
 *
 * 不传 HOME 环境变量——前端模块需在 vitest / 浏览器 / Tauri 三场景一致行为。
 */
function abbreviateHome(path: string): string {
  // macOS / Linux
  const posixHome = /^\/(Users|home)\/[^/]+(\/|$)/;
  if (posixHome.test(path)) {
    return path.replace(posixHome, "~/").replace(/^~\/$/, "~");
  }
  if (path === "/root" || path.startsWith("/root/")) {
    return path.replace(/^\/root/, "~");
  }

  // Windows: C:\Users\name\... → ~\...
  const winHome = /^[A-Za-z]:\\Users\\[^\\]+(\\|$)/;
  if (winHome.test(path)) {
    return path.replace(winHome, "~\\");
  }

  return path;
}
