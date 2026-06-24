<script lang="ts">
  import TabBar from "../TabBar.svelte";
  import PaneSplitDropZone from "./PaneSplitDropZone.svelte";
  import SessionDetail from "../../routes/SessionDetail.svelte";
  import SettingsView from "../../routes/SettingsView.svelte";
  import NotificationsView from "../../routes/NotificationsView.svelte";
  import MemoryView from "../../routes/MemoryView.svelte";
  import JobsView from "../../routes/JobsView.svelte";
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
    /** 本 pane 是否是最左 pane；折叠 sidebar 时只在最左 pane 显示展开按钮 */
    isFirstPane: boolean;
  }

  let { pane, selectedProjectId, onSelectProject, isSolePane, isFirstPane }: Props = $props();

  const focused = $derived(getFocusedPaneId() === pane.id);
  const activeTab = $derived(
    pane.activeTabId ? pane.tabs.find((t) => t.id === pane.activeTabId) ?? null : null,
  );
  // sole pane + 无 tab（= Dashboard 工作台）时不渲染 TabBar：通知/设置已迁到
  // UnifiedTitleBar，留下一条 40px 空横条 + border-bottom 会在 chrome 与搜索框
  // 之间制造无意义空白带。多 pane 即便 tabs 为空也保留 TabBar——它承载 focus
  // accent indicator 与 tab drop 命中区域。
  const showTabBar = $derived(!(isSolePane && pane.tabs.length === 0));

  function onPointerDownCapture() {
    // 点到 pane 内部任意位置即 focus（不阻止默认，子组件正常处理）
    if (!focused) focusPane(pane.id);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  id={`pane-${pane.id}`}
  class="pane-view"
  class:focused
  class:sole={isSolePane}
  style:flex={pane.widthFraction}
  onpointerdowncapture={onPointerDownCapture}
>
  {#if showTabBar}
    <TabBar paneId={pane.id} {isFirstPane} />
  {/if}

  <div class="pane-body">
    {#if activeTab?.type === "settings"}
      <SettingsView />
    {:else if activeTab?.type === "notifications"}
      <NotificationsView />
    {:else if activeTab?.type === "jobs"}
      <JobsView />
    {:else if activeTab?.type === "memory"}
      <MemoryView projectId={activeTab.projectId} />
    {:else if activeTab?.type === "session"}
      <!-- key 复合 tabId + sessionId：切 tab（tabId 变）或同 tab 替换会话（openOrReplaceTab
           保留 tabId 仅换 sessionId/projectId）时都触发 destroy/recreate，确保 SessionDetail
           的 onMount 数据加载重跑。SessionDetail 内部仍假设 tabId 生命周期内不变（成立——
           重建后的新实例里 tabId 是新的固定值）。 -->
      {#key `${activeTab.id}@${activeTab.sessionId}`}
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
