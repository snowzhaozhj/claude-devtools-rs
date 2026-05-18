<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { toolErrorText, toolOutputText } from "../../lib/toolHelpers";
  import DiffViewer from "../DiffViewer.svelte";
  import OutputBlock from "../OutputBlock.svelte";

  interface Props {
    exec: ToolExecution;
  }

  let { exec }: Props = $props();

  const input = $derived(exec.input as Record<string, unknown>);
  const filePath = $derived(String(input?.file_path ?? input?.filePath ?? ""));
  const oldString = $derived(String(input?.old_string ?? input?.oldString ?? ""));
  const newString = $derived(String(input?.new_string ?? input?.newString ?? ""));
  // 对齐原版 EditToolViewer.tsx L42-70：diff 下方显示工具回执。失败时展示
  // 错误详情（红色），成功时展示后端 toolUseResult.content（如有）。否则该区段
  // 直接折叠不渲染。
  const resultText = $derived(exec.isError ? toolErrorText(exec) : toolOutputText(exec.output));
</script>

{#if oldString && newString}
  <DiffViewer fileName={filePath} {oldString} {newString} />
{:else if newString}
  <DiffViewer fileName={filePath} oldString="" {newString} />
{:else}
  <DiffViewer fileName={filePath} {oldString} newString="" />
{/if}

{#if resultText}
  <div class="edit-result">
    <span class="edit-result-label" class:edit-result-label-err={exec.isError}>
      {exec.isError ? "ERROR" : "RESULT"}
    </span>
    <OutputBlock code={resultText} isError={exec.isError} />
  </div>
{/if}

<style>
  .edit-result {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 8px;
    min-width: 0;
  }

  .edit-result-label {
    font-size: 9px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 1px;
    text-transform: uppercase;
  }

  .edit-result-label-err {
    color: var(--tool-result-error-text);
  }
</style>
