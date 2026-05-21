// Worktree filter via cursor `Exhausted` helper（change
// `simplify-repository-as-project::D6`）。前端切 filter 为某 worktree 时，
// 构造初始 cursor 让所有非选 worktree `WorktreeOffset = Exhausted`，
// server 的 k-way merge 自然只产出选中 worktree 的 sessions。
//
// 后端 GroupCursor wire 形态见 `crates/cdt-api/src/ipc/types.rs::GroupCursor`：
//
//   { "perWorktree": { "<wt-id>": { "kind": "not_started" }, ... } }
//
// 然后 `base64(JSON.stringify(...))` 传入 `list_group_sessions` 的 cursor 参数。
// tag 值用 snake_case 与仓内 IPC enum 一致；外层结构 / 字段 camelCase。
//
// Spec: openspec/specs/sidebar-navigation/spec.md §"Worktree filter dropdown
// for multi-worktree group" Scenario "切 filter 构造 server-side filter cursor"。

import type { Worktree } from "./api";

type WorktreeOffsetWire =
  | { kind: "not_started" }
  | { kind: "after_mtime"; mtimeMs: number; sid: string }
  | { kind: "exhausted" };

interface GroupCursorWire {
  perWorktree: Record<string, WorktreeOffsetWire>;
}

/** base64(utf8(json)) — Tauri/HTTP 双 runtime 都用 ASCII-safe base64 */
function encodeBase64Json(obj: unknown): string {
  const json = JSON.stringify(obj);
  if (typeof btoa === "function") {
    // utf8 → 字节 → btoa；emoji 等多字节字符避免 btoa 直接 throw
    const bytes = new TextEncoder().encode(json);
    let bin = "";
    for (const b of bytes) bin += String.fromCharCode(b);
    return btoa(bin);
  }
  // Node (vitest) fallback — Buffer 在浏览器/Tauri 不可用
  return Buffer.from(json, "utf8").toString("base64");
}

/**
 * 构造 worktree filter 初始 cursor：选中 worktree `NotStarted`，其余 `Exhausted`。
 *
 * 续页 cursor 由 server 在 `list_group_sessions` 响应中自然返回（保持
 * `Exhausted` 标记），前端 loadMore 直接续传该 cursor 即可。
 */
export function buildFilterCursor(
  groupWorktrees: Worktree[],
  selectedWorktreeId: string,
): string {
  const perWorktree: Record<string, WorktreeOffsetWire> = {};
  for (const w of groupWorktrees) {
    perWorktree[w.id] =
      w.id === selectedWorktreeId ? { kind: "not_started" } : { kind: "exhausted" };
  }
  return encodeBase64Json({ perWorktree } satisfies GroupCursorWire);
}

/**
 * 组合 sessionListStore 的 cache key —— 同 group 不同 worktree filter SHALL
 * 独立缓存，避免切 filter 串台。`null` 表示 "全部" filter。
 */
export function sessionListCacheKey(
  groupId: string,
  filterWorktreeId: string | null,
): string {
  return `${groupId}::${filterWorktreeId ?? ""}`;
}
