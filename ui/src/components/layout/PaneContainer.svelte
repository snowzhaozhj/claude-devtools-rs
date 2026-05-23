<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import PaneView from "./PaneView.svelte";
  import PaneResizeHandle from "./PaneResizeHandle.svelte";
  import { getPaneLayout, getActiveTabId } from "../../lib/tabStore.svelte";
  import { registerShortcut } from "../../lib/keyboard/registry";
  import { getShortcutMeta } from "../../lib/keyboard/defaults";
  import {
    triggerJumpToLatest,
    triggerOpenSearch,
  } from "../../lib/keyboard/session-detail-handlers";

  interface Props {
    selectedProjectId: string;
    onSelectProject: (id: string, name: string) => void;
  }

  let { selectedProjectId, onSelectProject }: Props = $props();

  const layout = $derived(getPaneLayout());
  let containerEl: HTMLElement | null = $state(null);

  // 共用 dispatcher——`session.jump-to-latest` / `search.in-session` 两条快捷键
  // 在 PaneContainer（应用唯一 instance）注册一次，按 active tabId 派发到对应
  // SessionDetail 实例的回调（详 session-detail-handlers.ts 与 design.md::D8）。
  // active tab 非 SessionDetail（Dashboard / Settings / Notifications / Memory）
  // 时 trigger 返回 `false`，dispatcher 不 preventDefault → 浏览器原生行为放行。
  let unregisterSharedShortcuts: (() => void) | null = null;

  onMount(() => {
    const unregisters: Array<() => void> = [];
    const jumpMeta = getShortcutMeta("session.jump-to-latest");
    if (jumpMeta) {
      unregisters.push(
        registerShortcut({
          ...jumpMeta,
          handler: () => triggerJumpToLatest(getActiveTabId()),
        }),
      );
    }
    const searchMeta = getShortcutMeta("search.in-session");
    if (searchMeta) {
      unregisters.push(
        registerShortcut({
          ...searchMeta,
          handler: () => triggerOpenSearch(getActiveTabId()),
        }),
      );
    }
    unregisterSharedShortcuts = () => {
      for (const u of unregisters) u();
    };
  });

  onDestroy(() => {
    unregisterSharedShortcuts?.();
    unregisterSharedShortcuts = null;
  });
</script>

<div class="pane-container" bind:this={containerEl}>
  {#each layout.panes as pane, idx (pane.id)}
    <PaneView
      {pane}
      {selectedProjectId}
      {onSelectProject}
      isSolePane={layout.panes.length === 1}
      isFirstPane={idx === 0}
    />
    {#if idx < layout.panes.length - 1}
      <PaneResizeHandle leftPaneId={pane.id} {containerEl} />
    {/if}
  {/each}
</div>

<style>
  .pane-container {
    flex: 1;
    display: flex;
    flex-direction: row;
    overflow: hidden;
    min-height: 0;
  }
</style>
