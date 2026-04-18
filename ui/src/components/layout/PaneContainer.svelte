<script lang="ts">
  import PaneView from "./PaneView.svelte";
  import PaneResizeHandle from "./PaneResizeHandle.svelte";
  import { getPaneLayout } from "../../lib/tabStore.svelte";

  interface Props {
    selectedProjectId: string;
    onSelectProject: (id: string, name: string) => void;
  }

  let { selectedProjectId, onSelectProject }: Props = $props();

  const layout = $derived(getPaneLayout());
  let containerEl: HTMLElement | null = $state(null);
</script>

<div class="pane-container" bind:this={containerEl}>
  {#each layout.panes as pane, idx (pane.id)}
    <PaneView
      {pane}
      {selectedProjectId}
      {onSelectProject}
      isSolePane={layout.panes.length === 1}
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
