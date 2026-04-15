<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import DiffViewer from "../DiffViewer.svelte";

  interface Props {
    exec: ToolExecution;
  }

  let { exec }: Props = $props();

  const input = $derived(exec.input as Record<string, unknown>);
  const filePath = $derived(String(input?.file_path ?? input?.filePath ?? ""));
  const oldString = $derived(String(input?.old_string ?? input?.oldString ?? ""));
  const newString = $derived(String(input?.new_string ?? input?.newString ?? ""));
</script>

{#if oldString && newString}
  <DiffViewer fileName={filePath} {oldString} {newString} />
{:else if newString}
  <DiffViewer fileName={filePath} oldString="" {newString} />
{:else}
  <DiffViewer fileName={filePath} {oldString} newString="" />
{/if}
