import type { SessionDetail } from "./api";
import {
  MAX_PANES,
  type Pane,
  type PaneLayout,
} from "./paneTypes";
import {
  createEmptyPane,
  findPane,
  findPaneByTabId,
  getAllTabs as helpersGetAllTabs,
  insertPane,
  rebalanceWidths,
  removePane,
  resizeAdjacent,
  updatePane,
} from "./paneHelpers";

// ---------------------------------------------------------------------------
// Tab 数据模型
// ---------------------------------------------------------------------------

export type TabType = "session" | "settings" | "notifications" | "memory" | "jobs";

export interface Tab {
  id: string;
  type: TabType;
  sessionId: string;
  /**
   * Detail / per-project state 链路用 worktree id（即底层 `Project.id`）。
   * `getSessionDetail(projectId, sessionId)` / `getToolOutput` /
   * `getImageAsset` / `getSubagentTrace` 都按此字段定位。change
   * `simplify-repository-as-project::D7`。
   */
  projectId: string;
  /**
   * Sidebar 顶层选中 `RepositoryGroup.id`，供 sidebar 高亮"该 tab 属于哪个
   * group"。`get_session_detail` 等链路**不**消费此字段。单 worktree group 时
   * 与 `projectId` 字符串相同（grouper standalone 场景）。change
   * `simplify-repository-as-project::D7`。
   */
  groupId: string;
  label: string;
  createdAt: number;
}

export interface TabUIState {
  expandedChunks: Set<string>;
  expandedItems: Set<string>;
  searchVisible: boolean;
  contextPanelVisible: boolean;
  // 滚动状态用「视觉位置」语义而非绝对 scrollTop——后者在 lazy markdown 占位
  // 高度 ≠ 真实渲染高度的场景下被浏览器 clamp，导致切回时位置漂移。详
  // change `tab-scroll-restore-anchor::design.md::Context`。
  /** 保存时点是否粘底（distanceFromBottom <= 16） */
  atBottom: boolean;
  /** 视口顶第一个 bottom > containerTop 的 chunk 的 chunkId；atBottom=true 时为 null */
  anchorChunkId: string | null;
  /** anchor 元素 rect.top - container rect.top；可正（视口内）可负（跨越视口顶） */
  anchorOffsetPx: number;
}

function createDefaultUIState(): TabUIState {
  return {
    expandedChunks: new Set(),
    expandedItems: new Set(),
    searchVisible: false,
    contextPanelVisible: false,
    atBottom: false,
    anchorChunkId: null,
    anchorOffsetPx: 0,
  };
}

// ---------------------------------------------------------------------------
// 响应式状态（模块级 $state）
// ---------------------------------------------------------------------------

const DEFAULT_PANE_ID = "pane-default";

let paneLayout: PaneLayout = $state({
  panes: [createEmptyPane(DEFAULT_PANE_ID)],
  focusedPaneId: DEFAULT_PANE_ID,
});
let notificationUnreadCount: number = $state(0);

/**
 * Sidebar 会话项点击的默认行为偏好。
 * - "replace"（默认）：替换当前 tab 内容，对齐 Chrome 普通点击 / 原版 SessionItem.handleClick
 * - "new-tab"：每次开新 tab
 * Cmd/Ctrl + 点击始终翻转该默认。
 *
 * 启动时由 App.svelte 从 backend config (`general.sessionClickBehavior`) 同步；
 * SettingsView 修改时立即 setter 同步，无需等 reload。
 */
export type SessionClickBehavior = "replace" | "new-tab";
let sessionClickBehavior: SessionClickBehavior = $state("replace");

// per-tab UI 状态 / session 缓存按 tabId 索引，跨 pane 迁移不丢
const tabUIStates = new Map<string, TabUIState>();
const tabSessionCache = new Map<string, SessionDetail>();
const tabSessionCacheOrder: string[] = [];
const TAB_SESSION_CACHE_CAPACITY = MAX_PANES + 1;

function getActiveTabIds(): Set<string> {
  const ids = new Set<string>();
  for (const pane of paneLayout.panes) {
    if (pane.activeTabId) ids.add(pane.activeTabId);
  }
  return ids;
}

function touchCacheOrder(tabId: string): void {
  const idx = tabSessionCacheOrder.indexOf(tabId);
  if (idx !== -1) tabSessionCacheOrder.splice(idx, 1);
  tabSessionCacheOrder.push(tabId);
}

function removeCacheOrder(tabId: string): void {
  const idx = tabSessionCacheOrder.indexOf(tabId);
  if (idx !== -1) tabSessionCacheOrder.splice(idx, 1);
}

function evictStaleCacheEntries(): void {
  const activeIds = getActiveTabIds();
  while (tabSessionCacheOrder.length > TAB_SESSION_CACHE_CAPACITY) {
    const candidate = tabSessionCacheOrder.find((id) => !activeIds.has(id));
    if (!candidate) break;
    tabSessionCacheOrder.splice(tabSessionCacheOrder.indexOf(candidate), 1);
    tabSessionCache.delete(candidate);
  }
}

// ---------------------------------------------------------------------------
// 内部工具
// ---------------------------------------------------------------------------

function focusedPane(): Pane {
  const p = findPane(paneLayout, paneLayout.focusedPaneId);
  // 防御：若 focusedPaneId 失效，fallback 到第一个
  return p ?? paneLayout.panes[0];
}

/**
 * Tab label 透传函数——SHALL NOT 做任何不可逆截断。
 *
 * 视觉截断由 TabBar.svelte 的 `.tab-label` CSS（`max-width` + `text-overflow:
 * ellipsis`）完成；hover tooltip 由 `<button title={tab.label}>` 提供完整字符串。
 *
 * 详 spec `openspec/specs/tab-management/spec.md` §`打开 session tab` 与
 * change `session-title-extraction-fix`（删除原 50 字 JS 截断造成的 hover 信息丢失）。
 */
function shortLabel(label: string): string {
  return label;
}

// ---------------------------------------------------------------------------
// Pane 查询
// ---------------------------------------------------------------------------

export function getPaneLayout(): PaneLayout {
  return paneLayout;
}

export function getFocusedPaneId(): string {
  return paneLayout.focusedPaneId;
}

export function getPaneById(paneId: string): Pane | undefined {
  return findPane(paneLayout, paneId);
}

// ---------------------------------------------------------------------------
// Tab 查询（focused pane 代理，保持旧 API 兼容）
// ---------------------------------------------------------------------------

export function getTabs(): Tab[] {
  return focusedPane().tabs;
}

export function getAllTabs(): Tab[] {
  return helpersGetAllTabs(paneLayout);
}

export function getActiveTabId(): string | null {
  return focusedPane().activeTabId;
}

export function getActiveTab(): Tab | null {
  const pane = focusedPane();
  if (!pane.activeTabId) return null;
  return pane.tabs.find((t) => t.id === pane.activeTabId) ?? null;
}

export function getUnreadCount(): number {
  return notificationUnreadCount;
}

export function setUnreadCount(count: number): void {
  notificationUnreadCount = count;
}

export function getSessionClickBehavior(): SessionClickBehavior {
  return sessionClickBehavior;
}

export function setSessionClickBehavior(value: SessionClickBehavior): void {
  sessionClickBehavior = value;
}

/**
 * 统一 sidebar / palette 等会话项的点击路由。
 * - opts.forceNewTab: 总走 openTab（修饰键 Cmd/Ctrl + 点击 / "Open in New Tab" 菜单）
 * - opts.forceReplace: 总走 openOrReplaceTab（"Open in Current Tab" 菜单）
 * - 默认按 sessionClickBehavior preference 决定
 */
export function openSessionTab(
  sessionId: string,
  projectId: string,
  label: string,
  opts?: { forceNewTab?: boolean; forceReplace?: boolean; groupId?: string },
): void {
  // 兼容：未传 groupId 时退化等于 projectId（单 worktree group 场景，二者
  // 字符串本就相同；多 worktree 老 caller 没传时 sidebar 高亮可能漂移到组上）。
  const groupId = opts?.groupId ?? projectId;
  if (opts?.forceNewTab) {
    openTab(sessionId, projectId, label, groupId);
    return;
  }
  if (opts?.forceReplace) {
    openOrReplaceTab(sessionId, projectId, label, groupId);
    return;
  }
  if (sessionClickBehavior === "replace") {
    openOrReplaceTab(sessionId, projectId, label, groupId);
  } else {
    openTab(sessionId, projectId, label, groupId);
  }
}

// ---------------------------------------------------------------------------
// Pane 生命周期
// ---------------------------------------------------------------------------

export function focusPane(paneId: string): void {
  if (!findPane(paneLayout, paneId)) return;
  if (paneLayout.focusedPaneId === paneId) return;
  paneLayout = { ...paneLayout, focusedPaneId: paneId };
}

/**
 * 把指定 tab 从 source pane 剥离，新建 pane 插入到 source 左/右。
 * source pane 的 activeTabId 若为该 tab，fallback 到相邻 tab；
 * source pane 若因此变空且非唯一 pane，自动移除。
 */
export function splitPane(
  sourcePaneId: string,
  tabId: string,
  direction: "left" | "right",
): void {
  if (paneLayout.panes.length >= MAX_PANES) return;

  const source = findPane(paneLayout, sourcePaneId);
  if (!source) return;
  const tab = source.tabs.find((t) => t.id === tabId);
  if (!tab) return;

  const oldIdx = source.tabs.findIndex((t) => t.id === tabId);
  const newSourceTabs = source.tabs.filter((t) => t.id !== tabId);
  const newSourceActive =
    source.activeTabId === tabId
      ? (newSourceTabs[oldIdx]?.id ?? newSourceTabs[oldIdx - 1]?.id ?? null)
      : source.activeTabId;

  const updatedSource: Pane = {
    ...source,
    tabs: newSourceTabs,
    activeTabId: newSourceActive,
  };

  const newPaneId = crypto.randomUUID();
  const newPane: Pane = {
    ...createEmptyPane(newPaneId),
    tabs: [tab],
    activeTabId: tab.id,
  };

  let next = updatePane(paneLayout, updatedSource);
  if (newSourceTabs.length === 0 && next.panes.length > 1) {
    // source 被拖空 → 新 pane 相对谁插入？相对第一个现存 pane
    next = removePane(next, sourcePaneId);
    const anchor = next.panes[0];
    next = insertPane(next, anchor.id, newPane, direction);
  } else {
    next = insertPane(next, sourcePaneId, newPane, direction);
  }
  paneLayout = { ...next, focusedPaneId: newPaneId };
}

export function closePane(paneId: string): void {
  if (paneLayout.panes.length <= 1) return;
  const pane = findPane(paneLayout, paneId);
  if (!pane) return;
  for (const t of pane.tabs) {
    tabUIStates.delete(t.id);
    tabSessionCache.delete(t.id);
    removeCacheOrder(t.id);
  }
  paneLayout = removePane(paneLayout, paneId);
}

export function resizePanes(paneId: string, newFraction: number): void {
  paneLayout = resizeAdjacent(paneLayout, paneId, newFraction);
}

// ---------------------------------------------------------------------------
// Tab 操作
// ---------------------------------------------------------------------------

export function openTab(
  sessionId: string,
  projectId: string,
  label: string,
  groupId?: string,
): void {
  // 若 session 已在任意 pane 打开 → focus 该 pane + 激活 tab
  for (const pane of paneLayout.panes) {
    const existing = pane.tabs.find(
      (t) => t.type === "session" && t.sessionId === sessionId,
    );
    if (existing) {
      paneLayout = updatePane(
        { ...paneLayout, focusedPaneId: pane.id },
        { ...pane, activeTabId: existing.id },
      );
      return;
    }
  }

  const tab: Tab = {
    id: crypto.randomUUID(),
    type: "session",
    sessionId,
    projectId,
    groupId: groupId ?? projectId,
    label: shortLabel(label),
    createdAt: Date.now(),
  };
  const pane = focusedPane();
  const updated: Pane = {
    ...pane,
    tabs: [...pane.tabs, tab],
    activeTabId: tab.id,
  };
  paneLayout = updatePane(paneLayout, updated);
}

/**
 * "在当前 tab 替换" 语义（对齐 Chrome 普通点击 + 原版 SessionItem `replaceActiveTab`）：
 * - 若 session 已在任意 pane 打开 → focus 已存在 tab（去重，与 openTab 一致）
 * - 否则 focused pane 当前 active 是 session tab → 原地替换其 sessionId/projectId/label，
 *   tabId 保留，per-tab UI state / session 缓存按 tabId 清掉（旧会话视为已离开可丢弃）
 * - 否则（active 是 settings/notifications/无 active）→ fallback 到 openTab 追加新 tab
 */
export function openOrReplaceTab(
  sessionId: string,
  projectId: string,
  label: string,
  groupId?: string,
): void {
  for (const pane of paneLayout.panes) {
    const existing = pane.tabs.find(
      (t) => t.type === "session" && t.sessionId === sessionId,
    );
    if (existing) {
      paneLayout = updatePane(
        { ...paneLayout, focusedPaneId: pane.id },
        { ...pane, activeTabId: existing.id },
      );
      return;
    }
  }

  const pane = focusedPane();
  const activeTab =
    pane.activeTabId !== null
      ? pane.tabs.find((t) => t.id === pane.activeTabId)
      : undefined;

  if (activeTab && activeTab.type === "session") {
    tabUIStates.delete(activeTab.id);
    tabSessionCache.delete(activeTab.id);
    removeCacheOrder(activeTab.id);
    const replaced: Tab = {
      ...activeTab,
      sessionId,
      projectId,
      groupId: groupId ?? projectId,
      label: shortLabel(label),
      createdAt: Date.now(),
    };
    const newTabs = pane.tabs.map((t) =>
      t.id === activeTab.id ? replaced : t,
    );
    paneLayout = updatePane(paneLayout, { ...pane, tabs: newTabs });
    return;
  }

  openTab(sessionId, projectId, label, groupId);
}

function openSingletonTab(type: "settings" | "notifications" | "jobs", label: string): void {
  // settings / notifications 单例：任意 pane 已有 → focus
  for (const pane of paneLayout.panes) {
    const existing = pane.tabs.find((t) => t.type === type);
    if (existing) {
      paneLayout = updatePane(
        { ...paneLayout, focusedPaneId: pane.id },
        { ...pane, activeTabId: existing.id },
      );
      return;
    }
  }
  const tab: Tab = {
    id: crypto.randomUUID(),
    type,
    sessionId: "",
    projectId: "",
    groupId: "",
    label,
    createdAt: Date.now(),
  };
  const pane = focusedPane();
  paneLayout = updatePane(paneLayout, {
    ...pane,
    tabs: [...pane.tabs, tab],
    activeTabId: tab.id,
  });
}

export function openSettingsTab(): void {
  openSingletonTab("settings", "Settings");
}

export function openNotificationsTab(): void {
  openSingletonTab("notifications", "Notifications");
}

export function openJobsTab(): void {
  openSingletonTab("jobs", "Background Jobs");
}

export function openMemoryTab(projectId: string, label = "Memory"): void {
  for (const pane of paneLayout.panes) {
    const existing = pane.tabs.find(
      (t) => t.type === "memory" && t.projectId === projectId,
    );
    if (existing) {
      paneLayout = updatePane(
        { ...paneLayout, focusedPaneId: pane.id },
        { ...pane, activeTabId: existing.id },
      );
      return;
    }
  }
  const tab: Tab = {
    id: crypto.randomUUID(),
    type: "memory",
    sessionId: "",
    projectId,
    groupId: projectId,
    label: shortLabel(label),
    createdAt: Date.now(),
  };
  const pane = focusedPane();
  paneLayout = updatePane(paneLayout, {
    ...pane,
    tabs: [...pane.tabs, tab],
    activeTabId: tab.id,
  });
}

/**
 * 在 focused pane 右侧创建新 pane 并在其中打开 session tab。
 * 不做"已存在则切换"去重——新 pane 持有独立 tab 副本，UI 状态和缓存
 * 按 tabId 隔离。达到 MAX_PANES 时 noop。
 */
export function openTabInNewPane(
  sessionId: string,
  projectId: string,
  label: string,
  groupId?: string,
): void {
  if (paneLayout.panes.length >= MAX_PANES) return;

  const tab: Tab = {
    id: crypto.randomUUID(),
    type: "session",
    sessionId,
    projectId,
    groupId: groupId ?? projectId,
    label: shortLabel(label),
    createdAt: Date.now(),
  };
  const newPaneId = crypto.randomUUID();
  const newPane: Pane = {
    ...createEmptyPane(newPaneId),
    tabs: [tab],
    activeTabId: tab.id,
  };
  const next = insertPane(paneLayout, paneLayout.focusedPaneId, newPane, "right");
  paneLayout = { ...next, focusedPaneId: newPaneId };
}

export function closeTab(tabId: string): void {
  const pane = findPaneByTabId(paneLayout, tabId);
  if (!pane) return;

  const idx = pane.tabs.findIndex((t) => t.id === tabId);
  if (idx === -1) return;

  tabUIStates.delete(tabId);
  tabSessionCache.delete(tabId);
  removeCacheOrder(tabId);

  const newTabs = pane.tabs.filter((t) => t.id !== tabId);
  let newActive: string | null = pane.activeTabId;
  if (pane.activeTabId === tabId) {
    if (newTabs.length === 0) {
      newActive = null;
    } else {
      const nextIdx = Math.min(idx, newTabs.length - 1);
      newActive = newTabs[nextIdx].id;
    }
  }

  const updated: Pane = { ...pane, tabs: newTabs, activeTabId: newActive };
  let next = updatePane(paneLayout, updated);

  if (newTabs.length === 0 && next.panes.length > 1) {
    next = removePane(next, pane.id);
  }
  paneLayout = next;
}

/**
 * 激活指定 tab（自动 focus 其所在 pane）。若 tabId 不存在静默返回。
 */
export function setActiveTab(tabId: string): void {
  const pane = findPaneByTabId(paneLayout, tabId);
  if (!pane) return;
  paneLayout = updatePane(
    { ...paneLayout, focusedPaneId: pane.id },
    { ...pane, activeTabId: tabId },
  );
}

// ---------------------------------------------------------------------------
// 跨 Pane tab 移动
// ---------------------------------------------------------------------------

export function reorderTabInPane(
  paneId: string,
  fromIndex: number,
  toIndex: number,
): void {
  if (fromIndex === toIndex) return;
  const pane = findPane(paneLayout, paneId);
  if (!pane) return;
  if (fromIndex < 0 || fromIndex >= pane.tabs.length) return;
  if (toIndex < 0 || toIndex >= pane.tabs.length) return;

  const next = pane.tabs.slice();
  const [moved] = next.splice(fromIndex, 1);
  next.splice(toIndex, 0, moved);
  paneLayout = updatePane(paneLayout, { ...pane, tabs: next });
}

/** 兼容旧 API：对 focused pane 内 reorder */
export function reorderTab(fromIndex: number, toIndex: number): void {
  reorderTabInPane(paneLayout.focusedPaneId, fromIndex, toIndex);
}

export function moveTabToPane(
  tabId: string,
  sourcePaneId: string,
  targetPaneId: string,
  insertIndex?: number,
): void {
  if (sourcePaneId === targetPaneId) return;
  const source = findPane(paneLayout, sourcePaneId);
  const target = findPane(paneLayout, targetPaneId);
  if (!source || !target) return;
  const tab = source.tabs.find((t) => t.id === tabId);
  if (!tab) return;

  const oldIdx = source.tabs.findIndex((t) => t.id === tabId);
  const newSourceTabs = source.tabs.filter((t) => t.id !== tabId);
  const newSourceActive =
    source.activeTabId === tabId
      ? (newSourceTabs[oldIdx]?.id ?? newSourceTabs[oldIdx - 1]?.id ?? null)
      : source.activeTabId;

  const newTargetTabs = [...target.tabs];
  if (insertIndex !== undefined && insertIndex >= 0 && insertIndex <= newTargetTabs.length) {
    newTargetTabs.splice(insertIndex, 0, tab);
  } else {
    newTargetTabs.push(tab);
  }

  let next = updatePane(paneLayout, {
    ...source,
    tabs: newSourceTabs,
    activeTabId: newSourceActive,
  });
  next = updatePane(next, {
    ...target,
    tabs: newTargetTabs,
    activeTabId: tab.id,
  });

  if (newSourceTabs.length === 0 && next.panes.length > 1) {
    next = removePane(next, sourcePaneId);
  }
  paneLayout = { ...next, focusedPaneId: targetPaneId };
}

export function moveTabToNewPane(
  tabId: string,
  sourcePaneId: string,
  adjacentPaneId: string,
  direction: "left" | "right",
): void {
  if (paneLayout.panes.length >= MAX_PANES) return;
  const source = findPane(paneLayout, sourcePaneId);
  if (!source) return;
  const tab = source.tabs.find((t) => t.id === tabId);
  if (!tab) return;

  const oldIdx = source.tabs.findIndex((t) => t.id === tabId);
  const newSourceTabs = source.tabs.filter((t) => t.id !== tabId);
  const newSourceActive =
    source.activeTabId === tabId
      ? (newSourceTabs[oldIdx]?.id ?? newSourceTabs[oldIdx - 1]?.id ?? null)
      : source.activeTabId;

  const newPaneId = crypto.randomUUID();
  const newPane: Pane = {
    ...createEmptyPane(newPaneId),
    tabs: [tab],
    activeTabId: tab.id,
  };

  let next = updatePane(paneLayout, {
    ...source,
    tabs: newSourceTabs,
    activeTabId: newSourceActive,
  });

  // adjacent 的实际存在 id（若 adjacent === source 且 source 被拖空要调整）
  let anchorId = adjacentPaneId;
  if (newSourceTabs.length === 0 && next.panes.length > 1) {
    next = removePane(next, sourcePaneId);
    if (anchorId === sourcePaneId) {
      anchorId = next.panes[0].id;
    }
  }
  next = insertPane(next, anchorId, newPane, direction);
  paneLayout = { ...next, focusedPaneId: newPaneId };
}

// ---------------------------------------------------------------------------
// Per-tab UI 状态（按 tabId 全局索引，跨 pane 迁移保留）
// ---------------------------------------------------------------------------

export function getTabUIState(tabId: string): TabUIState {
  let st = tabUIStates.get(tabId);
  if (!st) {
    st = createDefaultUIState();
    tabUIStates.set(tabId, st);
  }
  return st;
}

export function saveTabUIState(tabId: string, state: TabUIState): void {
  tabUIStates.set(tabId, state);
}


/**
 * 查找 tabId 当前指向的 sessionId（用于跨 pane 找 tab）。
 * 找不到（tab 已被关闭 / 不存在）返回 null。
 *
 * 给 SessionDetail.onDestroy 做 guard：openOrReplaceTab 保留 tabId 仅换 sessionId
 * 时会触发旧 SessionDetail destroy，若它 onDestroy 无条件 saveTabUIState(tabId, ...)
 * 会用旧 session 的 expanded / scroll 状态覆盖 openOrReplaceTab 刚 delete 的 slot，
 * 新 SessionDetail mount 时 getTabUIState(tabId) 拿到的就是旧 session 残留。
 */
export function getTabSessionId(tabId: string): string | null {
  for (const pane of paneLayout.panes) {
    const t = pane.tabs.find((x) => x.id === tabId);
    if (t) return t.sessionId;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Per-tab session 数据缓存
// ---------------------------------------------------------------------------

export function getCachedSession(tabId: string): SessionDetail | null {
  const detail = tabSessionCache.get(tabId) ?? null;
  if (detail) touchCacheOrder(tabId);
  return detail;
}

export function setCachedSession(tabId: string, detail: SessionDetail): void {
  touchCacheOrder(tabId);
  tabSessionCache.set(tabId, detail);
  evictStaleCacheEntries();
}

// ---------------------------------------------------------------------------
// 兼容层：旧调用点直接 import `tabs` 等的场景
// ---------------------------------------------------------------------------

// 用于需要响应式订阅整体 layout 的场景（例如 PaneContainer 的 {#each}）。
// 外部以函数调用方式读取 → getter，保持响应性。
export function _rawPaneLayout(): PaneLayout {
  return paneLayout;
}

// 兜底：若 rebalance 后 panes 为空（理论不该发生），强制重建默认 pane
export function _ensureDefaultPane(): void {
  if (paneLayout.panes.length === 0) {
    paneLayout = {
      panes: [createEmptyPane(DEFAULT_PANE_ID)],
      focusedPaneId: DEFAULT_PANE_ID,
    };
  } else {
    // 修正 focusedPaneId 悬空
    if (!findPane(paneLayout, paneLayout.focusedPaneId)) {
      paneLayout = { ...paneLayout, focusedPaneId: paneLayout.panes[0].id };
    }
  }
  // 静音 warning —— 仅用于诱导 TS 不抹掉该 helper 导出
  void rebalanceWidths;
}
