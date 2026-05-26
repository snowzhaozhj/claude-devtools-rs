<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { stripAnsi, toolErrorText, toolOutputText } from "../../lib/toolHelpers";
  import DiffViewer from "../DiffViewer.svelte";
  import OutputBlock from "../OutputBlock.svelte";
  import { contextMenu } from "../../lib/contextMenu.svelte";
  import { buildFileToolItems, type MenuItemContext } from "../../lib/contextMenu/menu-items";
  import { getMenuSettings } from "../../lib/contextMenu/settings.svelte";
  import { getMenuItemDispatch } from "../../lib/contextMenu/dispatch";

  interface Props {
    exec: ToolExecution;
    sessionId?: string;
    projectId?: string;
  }

  let { exec, sessionId = "", projectId = "" }: Props = $props();

  const input = $derived(exec.input as Record<string, unknown>);
  const filePath = $derived(String(input?.file_path ?? input?.filePath ?? ""));
  const oldString = $derived(String(input?.old_string ?? input?.oldString ?? ""));
  const newString = $derived(String(input?.new_string ?? input?.newString ?? ""));
  // 对齐原版 EditToolViewer.tsx L42-70：diff 下方显示工具回执。失败时展示
  // 错误详情（红色），成功时展示后端 toolUseResult.content（如有）。否则该区段
  // 直接折叠不渲染。Edit 失败回执是 stderr-style，与 Bash / Default 一致走 stripAnsi
  // （codex CR PR #328：决策权在 viewer 层，toolOutputText 自身不剥）。
  const resultText = $derived(exec.isError ? toolErrorText(exec) : stripAnsi(toolOutputText(exec.output)));

  function buildCtx(): MenuItemContext {
    return {
      sessionId,
      projectId,
      settings: getMenuSettings(),
      selectionText: window.getSelection()?.toString() ?? "",
      dispatch: getMenuItemDispatch(),
    };
  }
</script>

<!-- contextMenu 覆盖整个工具块（diff + result）；display: contents 让 wrapping
     div 不影响外层 grid/flex 布局——视觉上等价旧的"无 wrapper"形态 -->
<div class="edit-tool-wrap" use:contextMenu={() => buildFileToolItems(exec, buildCtx())}>
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
</div>

<style>
  /* display: contents 让 wrapping div 不影响 layout——子节点视觉上仍然是
     EditToolViewer 的直接孩子，仅作为 use:contextMenu 的事件挂载点 */
  .edit-tool-wrap {
    display: contents;
  }

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
