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
