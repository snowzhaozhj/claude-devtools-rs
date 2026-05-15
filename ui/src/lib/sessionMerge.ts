import type { SessionSummary, SessionMetadataUpdate } from "./api";

/**
 * 把 `pending` buffer 中匹配 sessionId 的 update 应用到 `arr` —— 兜底 listener
 * 收到 `session-metadata-update` 时 `sessions` 数组还没扩展到那条 sessionId 的
 * race（典型场景：page 2 的后台扫描 broadcast emit 早于 page 2 IPC return，前
 * 端 `sessions.map` 找不到目标，update 静默丢失）。listener 把所有 update 写入
 * pending buffer，sessions 写入后立即调本函数把 buffer 中已经在新 sessions 里
 * 的 sessionId 一次性 patch 上去。
 *
 * 详见 spec `sidebar-navigation/spec.md::会话元数据增量 patch` Scenario "更新到
 * 达时 sessions 还未包含 sessionId 时缓冲到 pending buffer"。
 */
export function applyPendingMetadata(
  arr: SessionSummary[],
  pending: Map<string, SessionMetadataUpdate>,
): SessionSummary[] {
  if (pending.size === 0) return arr;
  return arr.map((s) => {
    const upd = pending.get(s.sessionId);
    if (!upd) return s;
    return {
      ...s,
      title: upd.title,
      messageCount: upd.messageCount,
      isOngoing: upd.isOngoing,
      gitBranch: upd.gitBranch,
    };
  });
}

/**
 * silent 刷新合并语义：把旧 sessions 中**已 patch 过**的元数据字段
 * （title 非 null / messageCount > 0 / isOngoing true / gitBranch 非 null 任一）
 * merge 进新骨架，让 silent 刷新过程中已展示的元数据不被瞬间重置为占位。
 * 时间戳走新骨架（最新值）。
 */
export function mergeSilentMetadata(
  prev: SessionSummary[],
  next: SessionSummary[],
): SessionSummary[] {
  const prevMap = new Map(prev.map((s) => [s.sessionId, s]));
  return next.map((skel) => {
    const old = prevMap.get(skel.sessionId);
    if (!old) return skel;
    const hasMeta =
      old.title !== null ||
      old.messageCount > 0 ||
      old.isOngoing ||
      old.gitBranch !== null;
    if (!hasMeta) return skel;
    return {
      ...skel,
      title: old.title,
      messageCount: old.messageCount,
      isOngoing: old.isOngoing,
      gitBranch: old.gitBranch,
    };
  });
}

/**
 * 把 `next` 合并进 `prev`：prev 中存在的 sessionId 用 mergeSilentMetadata 保留
 * 已 patch 元数据，prev 中不存在的追加；prev 中不在 next 里的尾部条目保留。
 * `sort=true` 按 timestamp 倒序排序（silent 刷新与 reconcile 路径）；
 * `sort=false` 保留 prev 顺序（loadMoreSessions 翻页追加路径，对齐 spec
 * `sidebar-navigation::Scenario "加载更多时保持已加载顺序"`）。
 */
export function mergeSessions(
  prev: SessionSummary[],
  next: SessionSummary[],
  sort = true,
): SessionSummary[] {
  const byId = new Map(prev.map((s) => [s.sessionId, s]));
  const merged = [...prev];
  for (const item of next) {
    const old = byId.get(item.sessionId);
    if (old) {
      const updated = mergeSilentMetadata([old], [item])[0];
      byId.set(item.sessionId, updated);
      const idx = merged.findIndex((s) => s.sessionId === item.sessionId);
      if (idx >= 0) merged[idx] = updated;
    } else {
      byId.set(item.sessionId, item);
      merged.push(item);
    }
  }
  return sort ? merged.sort((a, b) => b.timestamp - a.timestamp) : merged;
}

export interface SilentRefreshResult {
  sessions: SessionSummary[];
  nextCursor: string | null;
}

/**
 * silent 刷新策略（spec `sidebar-navigation::Requirement "会话元数据增量 patch"`）：
 * 把 file-change 或"有更新"按钮触发的第一页结果合并进现有 `sessions`，
 * 保留 prev 中超出第一页的尾部条目；保留 prev 的 `sessionsNextCursor`，
 * 不让用户已翻到的分页位置被重置。
 */
export function applySilentRefresh(
  prev: SessionSummary[],
  prevCursor: string | null,
  firstPageItems: SessionSummary[],
): SilentRefreshResult {
  return {
    sessions: mergeSessions(prev, firstPageItems, true),
    nextCursor: prevCursor,
  };
}
