import { MIN_FRACTION, type Pane, type PaneLayout } from "./paneTypes";
import type { Tab } from "./tabStore.svelte";

export function createEmptyPane(id: string, widthFraction = 1): Pane {
  return { id, tabs: [], activeTabId: null, widthFraction };
}

export function findPane(layout: PaneLayout, paneId: string): Pane | undefined {
  return layout.panes.find((p) => p.id === paneId);
}

export function findPaneByTabId(
  layout: PaneLayout,
  tabId: string,
): Pane | undefined {
  return layout.panes.find((p) => p.tabs.some((t) => t.id === tabId));
}

export function getAllTabs(layout: PaneLayout): Tab[] {
  return layout.panes.flatMap((p) => p.tabs);
}

export function updatePane(layout: PaneLayout, next: Pane): PaneLayout {
  return {
    ...layout,
    panes: layout.panes.map((p) => (p.id === next.id ? next : p)),
  };
}

/**
 * 把所有 pane 的 widthFraction 均分为 1 / n。insertPane / removePane
 * 之后调用以保持宽度权重合法。
 */
export function rebalanceWidths(panes: Pane[]): Pane[] {
  if (panes.length === 0) return panes;
  const fraction = 1 / panes.length;
  return panes.map((p) => ({ ...p, widthFraction: fraction }));
}

export function insertPane(
  layout: PaneLayout,
  anchorId: string,
  newPane: Pane,
  direction: "left" | "right",
): PaneLayout {
  const anchorIdx = layout.panes.findIndex((p) => p.id === anchorId);
  if (anchorIdx === -1) {
    // 找不到 anchor：追加到末尾（防御式；调用方应保证 anchor 有效）
    const panes = rebalanceWidths([...layout.panes, newPane]);
    return { ...layout, panes };
  }
  const insertIdx = direction === "right" ? anchorIdx + 1 : anchorIdx;
  const next = [...layout.panes];
  next.splice(insertIdx, 0, newPane);
  return { ...layout, panes: rebalanceWidths(next) };
}

/**
 * 移除指定 pane。若移除后 `focusedPaneId` 失效，fallback 到相邻 pane
 * （优先原索引位置，否则前一个）。不能移除唯一 pane；调用方需自行判断。
 */
export function removePane(layout: PaneLayout, paneId: string): PaneLayout {
  const idx = layout.panes.findIndex((p) => p.id === paneId);
  if (idx === -1) return layout;
  if (layout.panes.length <= 1) return layout;

  const next = layout.panes.filter((p) => p.id !== paneId);
  const rebalanced = rebalanceWidths(next);

  let focusedPaneId = layout.focusedPaneId;
  if (focusedPaneId === paneId) {
    const fallbackIdx = Math.min(idx, rebalanced.length - 1);
    focusedPaneId = rebalanced[fallbackIdx].id;
  }
  return { panes: rebalanced, focusedPaneId };
}

/**
 * 调整相邻两个 pane 的宽度权重，二者之和保持不变。被调 pane 是
 * paneId 指向的左侧；其右邻 pane 补差额。clamp 到 MIN_FRACTION。
 */
export function resizeAdjacent(
  layout: PaneLayout,
  paneId: string,
  newFraction: number,
): PaneLayout {
  const idx = layout.panes.findIndex((p) => p.id === paneId);
  if (idx === -1 || idx >= layout.panes.length - 1) return layout;

  const current = layout.panes[idx];
  const next = layout.panes[idx + 1];
  const combined = current.widthFraction + next.widthFraction;

  const clamped = Math.max(
    MIN_FRACTION,
    Math.min(combined - MIN_FRACTION, newFraction),
  );
  const nextWidth = combined - clamped;
  if (nextWidth < MIN_FRACTION) return layout;

  const panes = layout.panes.map((p, i) => {
    if (i === idx) return { ...p, widthFraction: clamped };
    if (i === idx + 1) return { ...p, widthFraction: nextWidth };
    return p;
  });
  return { ...layout, panes };
}

// ---------------------------------------------------------------------------
// DEV self-check asserts（UI 层暂无 vitest 基建，先做轻量自检，失败只写
// console.assert，不阻塞启动）
// ---------------------------------------------------------------------------

if (import.meta.env.DEV) {
  const mk = (id: string, fraction = 1): Pane => ({
    id,
    tabs: [],
    activeTabId: null,
    widthFraction: fraction,
  });

  // rebalanceWidths 均分
  const eq = rebalanceWidths([mk("a"), mk("b"), mk("c")]);
  console.assert(
    eq.every((p) => Math.abs(p.widthFraction - 1 / 3) < 1e-9),
    "rebalanceWidths: should evenly distribute",
  );

  // insertPane 右侧插入
  const base: PaneLayout = { panes: [mk("a"), mk("b")], focusedPaneId: "a" };
  const ins = insertPane(base, "a", mk("c"), "right");
  console.assert(
    ins.panes.map((p) => p.id).join(",") === "a,c,b",
    "insertPane right: should place after anchor",
  );

  // insertPane 左侧插入
  const insL = insertPane(base, "b", mk("c"), "left");
  console.assert(
    insL.panes.map((p) => p.id).join(",") === "a,c,b",
    "insertPane left: should place before anchor",
  );

  // removePane + focus fallback
  const rem = removePane(
    { panes: [mk("a"), mk("b"), mk("c")], focusedPaneId: "b" },
    "b",
  );
  console.assert(
    rem.panes.map((p) => p.id).join(",") === "a,c" &&
      rem.focusedPaneId === "c",
    "removePane: focus fallback should land on same-index or prev",
  );

  // resizeAdjacent 保持总和
  const rz = resizeAdjacent(
    { panes: [mk("a", 0.5), mk("b", 0.5)], focusedPaneId: "a" },
    "a",
    0.7,
  );
  const sumRz = rz.panes[0].widthFraction + rz.panes[1].widthFraction;
  console.assert(
    Math.abs(sumRz - 1) < 1e-9,
    "resizeAdjacent: neighbor widths should sum to original combined",
  );

  // resizeAdjacent clamp
  const rzMin = resizeAdjacent(
    { panes: [mk("a", 0.5), mk("b", 0.5)], focusedPaneId: "a" },
    "a",
    0.01,
  );
  console.assert(
    rzMin.panes[0].widthFraction === MIN_FRACTION,
    "resizeAdjacent: should clamp to MIN_FRACTION",
  );
}
