<script lang="ts">
  import TabBar from "../TabBar.svelte";
  import PaneSplitDropZone from "./PaneSplitDropZone.svelte";
  import SessionDetail from "../../routes/SessionDetail.svelte";
  import SettingsView from "../../routes/SettingsView.svelte";
  import NotificationsView from "../../routes/NotificationsView.svelte";
  import DashboardView from "../../routes/DashboardView.svelte";
  import {
    focusPane,
    getFocusedPaneId,
  } from "../../lib/tabStore.svelte";
  import type { Pane } from "../../lib/paneTypes";

  interface Props {
    pane: Pane;
    /** 用户选中的项目 id；仅唯一 pane + 无 tab 时给 DashboardView 用 */
    selectedProjectId: string;
    onSelectProject: (id: string, name: string) => void;
    /** 本 pane 是否是唯一 pane；决定空状态要不要显示 Dashboard */
    isSolePane: boolean;
  }

  let { pane, selectedProjectId, onSelectProject, isSolePane }: Props = $props();

  const focused = $derived(getFocusedPaneId() === pane.id);
  const activeTab = $derived(
    pane.activeTabId ? pane.tabs.find((t) => t.id === pane.activeTabId) ?? null : null,
  );

  function onPointerDownCapture() {
    // 点到 pane 内部任意位置即 focus（不阻止默认，子组件正常处理）
    if (!focused) focusPane(pane.id);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="pane-view"
  class:focused
  class:sole={isSolePane}
  style:flex={pane.widthFraction}
  onpointerdowncapture={onPointerDownCapture}
>
  <!-- TabBar 始终渲染：即使无 tab 也要显示右侧"通知/设置"工具栏入口 -->
  <TabBar paneId={pane.id} />

  <div class="pane-body">
    {#if activeTab?.type === "settings"}
      <SettingsView />
    {:else if activeTab?.type === "notifications"}
      <NotificationsView />
    {:else if activeTab?.type === "session"}
      {#key activeTab.id}
        <SessionDetail
          tabId={activeTab.id}
          projectId={activeTab.projectId}
          sessionId={activeTab.sessionId}
        />
      {/key}
    {:else if isSolePane}
      <DashboardView {selectedProjectId} {onSelectProject} />
    {:else}
      <div class="pane-empty">
        <span>此 Pane 暂无 Tab</span>
      </div>
    {/if}
  </div>

  <PaneSplitDropZone paneId={pane.id} side="left" />
  <PaneSplitDropZone paneId={pane.id} side="right" />
</div>

<style>
  .pane-view {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
  }
  /* 仅多 pane 场景下，给 focused pane 的 TabBar 顶端一条 2px 蓝色 accent；
     非 focused 用透明占位防止切换抖动。单 pane 时不渲染任何 focus 视觉。 */
  .pane-view:not(.sole) :global(.tab-bar) {
    box-shadow: inset 0 2px 0 0 transparent;
    transition: box-shadow 0.12s;
  }
  .pane-view:not(.sole).focused :global(.tab-bar) {
    box-shadow: inset 0 2px 0 0 var(--color-border-emphasis);
  }
  .pane-body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .pane-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    font-size: 13px;
  }
</style>
