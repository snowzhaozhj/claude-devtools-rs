/**
 * Sessions 列表 store：by-`projectId` LRU cache + stale-while-revalidate +
 * generation token cancel + `loadMore` leading + trailing debounce。
 *
 * Spec：`openspec/specs/sidebar-navigation/spec.md` §"Sessions store
 * stale-while-revalidate 缓存" / §"In-flight 列表请求按 generation token
 * 取消" / §"`loadMoreSessions` leading + trailing debounce 100 ms"。
 *
 * 设计 D3/D4/D5 见 `openspec/changes/unify-session-list-loading-strategy/design.md`：
 * - LRU 16 个 `projectId`，内存级**不**持久化磁盘（用户原始拍板 B 范围约束）
 * - generation: 每次 fetch 启动 `++entry.generation`，resolve 时校验不等则丢弃
 * - 浏览器 runtime 额外用 AbortController abort 旧 fetch；Tauri runtime
 *   `invoke` 不可 abort 故仅靠 generation
 * - `loadMore` 100 ms leading + trailing debounce + 同 cursor inflight short-circuit
 */
import { listSessions, type PaginatedResponse, type SessionMetadataUpdate, type SessionSummary } from "./api";

const STORE_CAPACITY = 16;
const DEBOUNCE_MS = 100;
const DEFAULT_PAGE_SIZE = 20;

export interface SessionListEntry {
  sessions: SessionSummary[];
  nextCursor: string | null;
  total: number;
  lastFetchedAt: number;
  /** 每次 fetch 启动 +1；fetch resolve 时若与启动时记录的不等则丢弃响应。 */
  generation: number;
  /** 浏览器 runtime fetch 路径：abort 旧请求；Tauri runtime 为 null。 */
  inflightAbort: AbortController | null;
  /** 当前在飞 fetch 的 cursor（用于 loadMore inflight short-circuit）。 */
  inflightCursor: string | null;
  /** LRU 时间戳。 */
  lastAccessedAt: number;
  /** loadMore leading + trailing：上次实际 fire 的时间戳。 */
  lastFiredAt: number;
  /** loadMore trailing timer（cooldown 期间收到新调用时 schedule）。 */
  pendingMoreTimer: ReturnType<typeof setTimeout> | null;
}

const cache = new Map<string, SessionListEntry>();
let accessCounter = 0;
const subscribers = new Set<(projectId: string) => void>();

/** 订阅 store mutations。fn 收到的 projectId 是发生变化的 entry 的 key。
 * sidebar 通过订阅在 store 缓存被外部（含 file-change silent refresh /
 * `applyMetadata` push patch）更新时同步本地显示。 */
export function subscribe(fn: (projectId: string) => void): () => void {
  subscribers.add(fn);
  return () => {
    subscribers.delete(fn);
  };
}

function notifyChange(projectId: string): void {
  for (const fn of [...subscribers]) fn(projectId);
}

function createEntry(): SessionListEntry {
  return {
    sessions: [],
    nextCursor: null,
    total: 0,
    lastFetchedAt: 0,
    generation: 0,
    inflightAbort: null,
    inflightCursor: null,
    lastAccessedAt: 0,
    lastFiredAt: 0,
    pendingMoreTimer: null,
  };
}

function bumpAccess(entry: SessionListEntry): void {
  accessCounter += 1;
  entry.lastAccessedAt = accessCounter;
}

function evictIfNeeded(): void {
  while (cache.size > STORE_CAPACITY) {
    let oldestKey: string | null = null;
    let oldestAccessedAt = Number.POSITIVE_INFINITY;
    for (const [key, entry] of cache) {
      if (entry.lastAccessedAt < oldestAccessedAt) {
        oldestAccessedAt = entry.lastAccessedAt;
        oldestKey = key;
      }
    }
    if (oldestKey === null) break;
    const victim = cache.get(oldestKey);
    if (victim?.pendingMoreTimer) clearTimeout(victim.pendingMoreTimer);
    victim?.inflightAbort?.abort();
    cache.delete(oldestKey);
  }
}

function ensureEntry(projectId: string): SessionListEntry {
  let entry = cache.get(projectId);
  if (!entry) {
    entry = createEntry();
    cache.set(projectId, entry);
  }
  bumpAccess(entry);
  evictIfNeeded();
  return entry;
}

/** 同步读取缓存。无副作用——bump access 仅在写入路径触发，避免读副作用让
 * Sidebar 的"先 read 再决定走 SWR"语义可观察 / 可测。 */
export function read(projectId: string): SessionListEntry | undefined {
  const entry = cache.get(projectId);
  if (entry) bumpAccess(entry);
  return entry;
}

interface LoadFirstPageOptions {
  /** `replace` — 用 fetch 结果替换列表（首次访问、显式重载）；`merge` — SWR
   * 路径：保留尾部 + 首页 ghost reconcile（删除 server 已不返的 sessionId）。 */
  mode: "replace" | "merge";
  pageSize?: number;
}

/** 拉首页并写入 store。返回更新后的 entry。
 *
 * Generation token：启动时 `++entry.generation` 并记录 `my`；resolve 时若
 * `entry.generation !== my` 直接 return（丢弃响应）。AbortController 仅在
 * 浏览器 runtime 生效——Tauri `invoke` 不支持 abort，但 generation 是兜底。 */
export async function loadFirstPage(
  projectId: string,
  opts: LoadFirstPageOptions,
): Promise<SessionListEntry | null> {
  const entry = ensureEntry(projectId);

  // 启动新 fetch 前 abort 旧的 + 推进 generation
  entry.inflightAbort?.abort();
  entry.generation += 1;
  const my = entry.generation;
  const controller = typeof AbortController !== "undefined" ? new AbortController() : null;
  entry.inflightAbort = controller;
  entry.inflightCursor = null;

  let response: PaginatedResponse<SessionSummary>;
  try {
    response = await listSessions(projectId, opts.pageSize ?? DEFAULT_PAGE_SIZE);
  } catch (e) {
    if (entry.generation === my) {
      entry.inflightAbort = null;
      entry.inflightCursor = null;
    }
    if ((e as Error)?.name === "AbortError") return null;
    throw e;
  }

  // resolve 时 generation 校验
  if (entry.generation !== my) return null;

  entry.inflightAbort = null;
  entry.inflightCursor = null;
  entry.lastFetchedAt = Date.now();
  entry.total = response.total;
  entry.nextCursor = response.nextCursor;

  if (opts.mode === "replace") {
    entry.sessions = response.items;
  } else {
    // SWR merge：首页 ghost reconcile + 保留尾部
    entry.sessions = mergeFirstPage(entry.sessions, response.items);
  }
  return entry;
}

/** SWR 首页合并：
 * - 首页范围内 `firstPage.length` 条由 server 真相覆盖
 * - 旧 entry 中"前 `firstPage.length` 条但 server 已不返"的 sessionId → **移除**
 *   （session 文件被删除 / 重命名 / 移出首页范围）
 * - 超出首页范围的尾部条目保留（pinned/hidden 与翻页累加的尾部不受首页 refresh 影响）
 *
 * Spec sidebar-navigation §"Sessions store stale-while-revalidate 缓存"
 * Scenario `首页 SWR refresh 删除已不存在的 session`。 */
function mergeFirstPage(prev: SessionSummary[], firstPage: SessionSummary[]): SessionSummary[] {
  const newIds = new Set(firstPage.map((s) => s.sessionId));
  const pageSize = firstPage.length;
  const prevFirstPage = prev.slice(0, pageSize);
  const prevTail = prev.slice(pageSize);
  // 旧首页中 server 仍返回的 sessionId 直接由 server 数据覆盖（不复用旧实例）；
  // 旧首页中 server 未返回的整条 drop（ghost reconcile）。
  void prevFirstPage; // 仅作 reference 说明：当前实现直接用 firstPage 取代旧首页
  // 保留尾部，但同样按 sessionId 去重（避免 server 返回的尾部 sessionId 重复）
  const dedupedTail = prevTail.filter((s) => !newIds.has(s.sessionId));
  return [...firstPage, ...dedupedTail];
}

/** 翻页加载：leading + trailing 100 ms debounce + inflight short-circuit
 * （同 `cursor` 已在飞时丢弃）。
 *
 * Spec sidebar-navigation §"`loadMoreSessions` leading + trailing
 * debounce 100 ms" 全部三个 Scenario。 */
export function loadMore(projectId: string): void {
  const entry = ensureEntry(projectId);
  const cursor = entry.nextCursor;
  if (!cursor) return;

  // Inflight short-circuit（最先判定）：同 cursor 已在飞，直接丢弃
  if (entry.inflightCursor === cursor) return;

  const now = Date.now();
  const sinceLastFire = now - entry.lastFiredAt;

  if (sinceLastFire >= DEBOUNCE_MS) {
    // Leading：立即 fire
    fireLoadMore(projectId, cursor);
    return;
  }

  // Trailing：cooldown 内，schedule timer 在 `lastFiredAt + DEBOUNCE_MS` 触发
  if (entry.pendingMoreTimer !== null) return; // 已有 pending timer，不重复 schedule
  const delay = DEBOUNCE_MS - sinceLastFire;
  entry.pendingMoreTimer = setTimeout(() => {
    entry.pendingMoreTimer = null;
    const e = cache.get(projectId);
    if (!e) return;
    const nextCursor = e.nextCursor;
    if (!nextCursor) return;
    // trailing 触发时再次 inflight short-circuit 重判（cooldown 期间可能已经
    // 触发过一次 fetch 进入 inflight）
    if (e.inflightCursor === nextCursor) return;
    fireLoadMore(projectId, nextCursor);
  }, delay);
}

function fireLoadMore(projectId: string, cursor: string): void {
  const entry = cache.get(projectId);
  if (!entry) return;
  entry.lastFiredAt = Date.now();
  entry.generation += 1;
  const my = entry.generation;
  entry.inflightCursor = cursor;
  const controller = typeof AbortController !== "undefined" ? new AbortController() : null;
  entry.inflightAbort?.abort();
  entry.inflightAbort = controller;

  void (async () => {
    let response: PaginatedResponse<SessionSummary>;
    try {
      response = await listSessions(projectId, DEFAULT_PAGE_SIZE, cursor);
    } catch (e) {
      const e0 = cache.get(projectId);
      if (e0 && e0.generation === my) {
        e0.inflightAbort = null;
        e0.inflightCursor = null;
      }
      if ((e as Error)?.name !== "AbortError") console.error("loadMore failed", e);
      return;
    }
    const e1 = cache.get(projectId);
    if (!e1 || e1.generation !== my) return; // stale 响应丢弃
    e1.inflightAbort = null;
    e1.inflightCursor = null;
    // 翻页：追加去重，保留旧顺序；nextCursor 推进；total **不**变（spec
    // sidebar-navigation §"会话总数显示口径"：loadMore 路径不改 sessionsTotal）
    const existingIds = new Set(e1.sessions.map((s) => s.sessionId));
    const appended = response.items.filter((s) => !existingIds.has(s.sessionId));
    e1.sessions = [...e1.sessions, ...appended];
    e1.nextCursor = response.nextCursor;
    e1.lastFetchedAt = Date.now();
  })();
}

/** 收到 `session-metadata-update` 时更新 store 缓存中对应 sessionId 的元数据
 * 字段（`title` / `messageCount` / `isOngoing` / `gitBranch`），保证下次切回
 * 该 project 时缓存返回已 patch 的真值。
 *
 * 用 spread copy 而非 in-place mutate——sidebar 与 store 共享 SessionSummary
 * 实例时 in-place mutate 会跨边界传染；新对象让两侧的状态机各自独立。 */
export function applyMetadata(projectId: string, update: SessionMetadataUpdate): void {
  const entry = cache.get(projectId);
  if (!entry) return;
  const idx = entry.sessions.findIndex((s) => s.sessionId === update.sessionId);
  if (idx === -1) return;
  entry.sessions = entry.sessions.map((s, i) =>
    i === idx
      ? {
          ...s,
          title: update.title,
          messageCount: update.messageCount,
          isOngoing: update.isOngoing,
          gitBranch: update.gitBranch,
        }
      : s,
  );
  notifyChange(projectId);
}

/** Sidebar 自己 fetch 后把最新 sessions / cursor / total 写回 store 的入口。
 * Sidebar 仍持有 sessions 作为显示数据源（保留 reconcile pinned/hidden +
 * buffer race fix 等既有逻辑），store 作为 by-project 缓存镜像供下次切回
 * 该 project 时立即 hydrate。 */
export function setSessions(
  projectId: string,
  sessions: SessionSummary[],
  nextCursor: string | null,
  total: number,
): void {
  const entry = ensureEntry(projectId);
  entry.sessions = sessions;
  entry.nextCursor = nextCursor;
  entry.total = total;
  entry.lastFetchedAt = Date.now();
  notifyChange(projectId);
}

/** file-change 后用：保留 stale 数据但触发下次 read SWR refresh（清掉
 * `lastFetchedAt`）。当前 read 不依赖 `lastFetchedAt` 决策，预留接口供
 * 未来按 TTL 判 stale。 */
export function invalidate(projectId: string): void {
  const entry = cache.get(projectId);
  if (!entry) return;
  entry.lastFetchedAt = 0;
}

/** 测试专用：清空 store 全部状态。 */
export function __resetForTests(): void {
  for (const entry of cache.values()) {
    if (entry.pendingMoreTimer) clearTimeout(entry.pendingMoreTimer);
    entry.inflightAbort?.abort();
  }
  cache.clear();
  accessCounter = 0;
}

/** 测试专用：返回 cache 的 key 序列（按 LRU 顺序：最旧在前）。 */
export function __snapshotKeys(): string[] {
  return [...cache.entries()]
    .sort(([, a], [, b]) => a.lastAccessedAt - b.lastAccessedAt)
    .map(([k]) => k);
}
