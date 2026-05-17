/**
 * 数字/时间格式化工具，对齐 `../claude-devtools/src/renderer/utils/formatters.ts`。
 */

/** 毫秒 → 人类可读：ms / s / m s / h m。 */
export function formatDuration(ms: number | null | undefined): string | null {
  if (ms == null) return null;
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const min = Math.floor(ms / 60_000);
  const sec = Math.floor((ms % 60_000) / 1000);
  if (min < 60) return sec > 0 ? `${min}m ${sec}s` : `${min}m`;
  const hours = Math.floor(min / 60);
  const remMin = min % 60;
  return remMin > 0 ? `${hours}h ${remMin}m` : `${hours}h`;
}

/** Token 紧凑格式：1234 → 1.2k，1_200_000 → 1.2M。 */
export function formatTokensCompact(n: number | null | undefined): string {
  if (n == null) return "0";
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)}k`;
  return String(n);
}

/**
 * 绝对时间戳渲染入口。`hour12=true` 产 "上午 X 点 XX 分 XX 秒" 形式；
 * `hour12=false` 产 24 小时制 `HH:MM:SS`。`date` 接 `Date` / `number` (ms) /
 * 可被 `new Date(...)` 解析的 string。解析失败返回空串。
 *
 * 详见 `openspec/specs/configuration-management/spec.md` §"Display config
 * exposes time format preference"。
 */
export function formatClock(date: Date | number | string, hour12: boolean): string {
  try {
    const d = date instanceof Date ? date : new Date(date);
    if (Number.isNaN(d.getTime())) return "";
    return d.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12,
    });
  } catch {
    return "";
  }
}
