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

export type TabType = "session" | "settings" | "notifications";

export interface Tab {
  id: string;
  type: TabType;
  sessionId: string;
  projectId: string;
  label: string;
  createdAt: number;
}

export interface TabUIState {
  expandedChunks: Set<number>;
  expandedItems: Set<string>;
  searchVisible: boolean;
  contextPanelVisible: boolean;
  scrollTop: number;
}

function createDefaultUIState(): TabUIState {
  return {
    expandedChunks: new Set(),
    expandedItems: new Set(),
    searchVisible: false,
    contextPanelVisible: false,
    scrollTop: 0,
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

// per-tab UI 状态 / session 缓存按 tabId 索引，跨 pane 迁移不丢
const tabUIStates = new Map<string, TabUIState>();
const tabSessionCache = new Map<string, SessionDetail>();

// ---------------------------------------------------------------------------
// 内部工具
// ---------------------------------------------------------------------------

function focusedPane(): Pane {
  const p = findPane(paneLayout, paneLayout.focusedPaneId);
  // 防御：若 focusedPaneId 失效，fallback 到第一个
  return p ?? paneLayout.panes[0];
}

function shortLabel(label: string): string {
  return label.length > 50 ? label.slice(0, 50) + "…" : label;
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

function openSingletonTab(type: "settings" | "notifications", label: string): void {
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

/**
 * 在 focused pane 右侧创建新 pane 并在其中打开 session tab。
 * 不做"已存在则切换"去重——新 pane 持有独立 tab 副本，UI 状态和缓存
 * 按 tabId 隔离。达到 MAX_PANES 时 noop。
 */
export function openTabInNewPane(
  sessionId: string,
  projectId: string,
  label: string,
): void {
  if (paneLayout.panes.length >= MAX_PANES) return;

  const tab: Tab = {
    id: crypto.randomUUID(),
    type: "session",
    sessionId,
    projectId,
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

// ---------------------------------------------------------------------------
// Per-tab session 数据缓存
// ---------------------------------------------------------------------------

export function getCachedSession(tabId: string): SessionDetail | null {
  return tabSessionCache.get(tabId) ?? null;
}

export function setCachedSession(tabId: string, detail: SessionDetail): void {
  tabSessionCache.set(tabId, detail);
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
