import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface FileChangePayload {
  projectId: string;
  sessionId: string;
  deleted: boolean;
}

type Handler = (payload: FileChangePayload) => void;

const handlers = new Map<string, Handler>();
const inFlight = new Map<string, Promise<void>>();

let unlisten: UnlistenFn | null = null;
let initPromise: Promise<void> | null = null;

export function initFileChangeStore(): Promise<void> {
  if (initPromise) return initPromise;
  initPromise = (async () => {
    unlisten = await listen<FileChangePayload>("file-change", (event) => {
      const payload = event.payload;
      for (const handler of handlers.values()) {
        try {
          handler(payload);
        } catch (e) {
          console.warn("file-change handler threw:", e);
        }
      }
    });
  })();
  return initPromise;
}

export function disposeFileChangeStore(): void {
  unlisten?.();
  unlisten = null;
  initPromise = null;
  handlers.clear();
  inFlight.clear();
  for (const t of trailingTimers.values()) clearTimeout(t);
  trailingTimers.clear();
  trailingPending.clear();
  lastRunAt.clear();
}

export function registerHandler(key: string, fn: Handler): void {
  handlers.set(key, fn);
}

export function unregisterHandler(key: string): void {
  handlers.delete(key);
}

/**
 * 同 key 的并发刷新合并为一次。第一次调用启动 fn()，后续在 Promise resolve
 * 之前到达的调用复用同一个 Promise；resolve 后 key 从 map 移除。
 */
export function dedupeRefresh(
  key: string,
  fn: () => Promise<void>,
): Promise<void> {
  const existing = inFlight.get(key);
  if (existing) return existing;
  const p = (async () => {
    try {
      await fn();
    } finally {
      inFlight.delete(key);
    }
  })();
  inFlight.set(key, p);
  return p;
}

// ---------------------------------------------------------------------------
// scheduleRefresh — leading + trailing 节流（默认 250ms）
//
// 活跃 Claude 会话每秒可触发多次 file-change（JSONL 实时追加）。`dedupeRefresh`
// 只合并 in-flight 并发，IPC 一旦 resolve 就允许下一次立即开跑——高频写入下
// 仍会每几百 ms 触发一次完整 IPC + DOM reconcile（CLAUDE.md "file-change 节流链"
// 段已点名）。
//
// scheduleRefresh：
//   1. 距上次执行 ≥ 窗口：立即触发（leading），保留 UX 即时感
//   2. 窗口内：保存为 pending，到窗口末尾跑一次最新 fn（trailing）
//   3. 内部走 dedupeRefresh，仍享有 in-flight 合并
// ---------------------------------------------------------------------------

const TRAILING_DEBOUNCE_MS = 250;

const trailingTimers = new Map<string, ReturnType<typeof setTimeout>>();
const trailingPending = new Map<string, () => Promise<void>>();
const lastRunAt = new Map<string, number>();

/**
 * 节流刷新：高频 file-change 下，同 key 在 250ms 窗口内合并为一次"末尾刷新"。
 * 窗口外的首次调用立即触发，保留首屏即时感。
 */
export function scheduleRefresh(key: string, fn: () => Promise<void>): void {
  const now = Date.now();
  const last = lastRunAt.get(key) ?? 0;

  if (now - last >= TRAILING_DEBOUNCE_MS) {
    lastRunAt.set(key, now);
    void dedupeRefresh(key, fn);
    return;
  }

  trailingPending.set(key, fn);
  if (trailingTimers.has(key)) return;

  const delay = TRAILING_DEBOUNCE_MS - (now - last);
  const timer = setTimeout(() => {
    trailingTimers.delete(key);
    const pending = trailingPending.get(key);
    trailingPending.delete(key);
    if (pending) {
      lastRunAt.set(key, Date.now());
      void dedupeRefresh(key, pending);
    }
  }, delay);
  trailingTimers.set(key, timer);
}

/** 仅供测试：清理 throttle 状态。 */
export function _resetScheduleRefreshForTest(): void {
  for (const t of trailingTimers.values()) clearTimeout(t);
  trailingTimers.clear();
  trailingPending.clear();
  lastRunAt.clear();
}
