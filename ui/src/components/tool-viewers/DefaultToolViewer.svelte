<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { toolOutputText } from "../../lib/toolHelpers";
  import OutputBlock from "../OutputBlock.svelte";

  interface Props {
    exec: ToolExecution;
  }

  let { exec }: Props = $props();

  const inputStr = $derived(JSON.stringify(exec.input, null, 2));
  const outputStr = $derived(toolOutputText(exec.output));
</script>

<div class="default-viewer">
  <div class="viewer-section">
    <span class="viewer-label">INPUT</span>
    <OutputBlock code={inputStr} lang="json" />
  </div>

  {#if outputStr}
    <div class="viewer-section">
      <span class="viewer-label" class:viewer-label-err={exec.isError}>
        {exec.isError ? "ERROR" : "OUTPUT"}
      </span>
      <OutputBlock code={outputStr} isError={exec.isError} />
    </div>
  {/if}
</div>

<style>
  .default-viewer {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .viewer-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .viewer-label {
    font-size: 9px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 1px;
    text-transform: uppercase;
  }

  .viewer-label-err {
    color: var(--tool-result-error-text);
  }
</style>
