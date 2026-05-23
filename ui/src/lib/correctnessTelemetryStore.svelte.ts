// Correctness 信号前端聚合 store
//
// 详见 OpenSpec change `add-telemetry-signal-bus` design D10：sidebar
// `session-metadata-update` listener 检测 stale-update 时调本 store 的
// `accumulate(kind)`，store 内部按 5 秒 setTimeout 或累计 ≥ 50 条触发批量
// flush 一次 IPC `record_correctness_events`，避免 file-change 风暴下每条事件
// 立刻调 IPC 把"低频 correctness 信号"事实上变成 IPC 热点。
//
// fire-and-forget 语义：flush 失败 silently 重置本地累计避免堆积。

import { recordCorrectnessEvents, type CorrectnessEventItem } from "./api";

const FLUSH_INTERVAL_MS = 5000;
const FLUSH_THRESHOLD = 50;

const counts = new Map<string, number>();
let flushTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleFlush() {
  if (flushTimer !== null) return;
  flushTimer = setTimeout(() => {
    flushTimer = null;
    void flush();
  }, FLUSH_INTERVAL_MS);
}

function totalAccumulated(): number {
  let total = 0;
  for (const v of counts.values()) total += v;
  return total;
}

/**
 * 累加一条 correctness event 计数。
 *
 * 仅在以下白名单 kind 上调用（与后端白名单对齐）：
 * - `stale_update.triggered`
 * - `cache.signature_skew_observed_in_ui`
 *
 * 调用频率允许高（每条 session-metadata-update event 都可能调），内部已节流。
 */
export function accumulate(kind: string, count = 1): void {
  if (count <= 0) return;
  counts.set(kind, (counts.get(kind) ?? 0) + count);
  if (totalAccumulated() >= FLUSH_THRESHOLD) {
    if (flushTimer !== null) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    void flush();
  } else {
    scheduleFlush();
  }
}

/**
 * 立即 flush 当前累计；调用后本地 counter 重置。
 *
 * 失败 silently 重置 —— `record_correctness_events` 是 fire-and-forget 语义，
 * 不重试避免堆积。下次 `accumulate` 重新累计。
 */
export async function flush(): Promise<void> {
  if (counts.size === 0) return;
  const items: CorrectnessEventItem[] = [];
  for (const [kind, count] of counts.entries()) {
    if (count > 0) items.push({ kind, count });
  }
  counts.clear();
  if (items.length === 0) return;
  try {
    await recordCorrectnessEvents(items);
  } catch (err) {
    // silent reset：下次 accumulate 重新计数
    console.warn("[telemetry] correctness flush failed", err);
  }
}

// 仅用于测试：访问内部 state
export function _getAccumulatedForTesting(): { kind: string; count: number }[] {
  return Array.from(counts.entries()).map(([kind, count]) => ({ kind, count }));
}

export function _resetForTesting(): void {
  counts.clear();
  if (flushTimer !== null) {
    clearTimeout(flushTimer);
    flushTimer = null;
  }
}
