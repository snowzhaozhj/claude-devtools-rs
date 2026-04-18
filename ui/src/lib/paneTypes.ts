import type { Tab } from "./tabStore.svelte";

export const MAX_PANES = 4;
export const MIN_FRACTION = 0.1;

export interface Pane {
  id: string;
  tabs: Tab[];
  activeTabId: string | null;
  widthFraction: number;
}

export interface PaneLayout {
  panes: Pane[];
  focusedPaneId: string;
}
