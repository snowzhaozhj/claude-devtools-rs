<script lang="ts">
  import CollapsibleSection from "./CollapsibleSection.svelte";
  import DirectoryTree from "../DirectoryTree.svelte";
  import {
    sumTokens,
    groupClaudeMdByScope,
    type ClaudeMdInjection,
  } from "../../lib/contextExtractor";

  interface Props {
    injections: ClaudeMdInjection[];
    expanded: boolean;
    onToggle: () => void;
  }

  let { injections, expanded, onToggle }: Props = $props();

  const tokens = $derived(sumTokens(injections));
  const groups = $derived(groupClaudeMdByScope(injections));

  // 单文件场景下不显示 sub-header（避免视觉冗余）
  const totalNonEmptyGroups = $derived(
    (groups.global.length > 0 ? 1 : 0) +
      (groups.project.length > 0 ? 1 : 0) +
      (groups.directory.length > 0 ? 1 : 0),
  );
</script>

{#if injections.length > 0}
  <CollapsibleSection label="CLAUDE.md Files" count={injections.length} {tokens} {expanded} {onToggle}>
    {#if groups.global.length > 0}
      <div class="cm-group">
        {#if totalNonEmptyGroups > 1}<div class="cm-sub-label">Global</div>{/if}
        <DirectoryTree entries={groups.global} />
      </div>
    {/if}
    {#if groups.project.length > 0}
      <div class="cm-group" class:cm-group-spaced={groups.global.length > 0}>
        {#if totalNonEmptyGroups > 1}<div class="cm-sub-label">Project</div>{/if}
        <DirectoryTree entries={groups.project} />
      </div>
    {/if}
    {#if groups.directory.length > 0}
      <div
        class="cm-group"
        class:cm-group-spaced={groups.global.length > 0 || groups.project.length > 0}
      >
        {#if totalNonEmptyGroups > 1}<div class="cm-sub-label">Directory</div>{/if}
        <DirectoryTree entries={groups.directory} />
      </div>
    {/if}
  </CollapsibleSection>
{/if}

<style>
  .cm-group {
    min-width: 0;
  }

  .cm-group-spaced {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid var(--color-border-subtle, var(--color-border));
  }

  .cm-sub-label {
    margin-bottom: 5px;
    font-size: 10px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
</style>
