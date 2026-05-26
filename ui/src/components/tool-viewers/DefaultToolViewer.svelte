<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { stripAnsi, toolErrorText, toolOutputText } from "../../lib/toolHelpers";
  import OutputBlock from "../OutputBlock.svelte";

  interface Props {
    exec: ToolExecution;
  }

  let { exec }: Props = $props();

  const inputStr = $derived(JSON.stringify(exec.input, null, 2));
  // 默认 viewer 接 Grep / Glob / WebFetch / MCP 等 stdout-style fallback——走 stripAnsi
  // 与 BashToolViewer 一致（codex CR PR #328：决策权在 viewer 层；ReadToolViewer
  // 走文件 raw 契约不剥真实 ESC 字节）。
  const outputStr = $derived(exec.isError ? toolErrorText(exec) : stripAnsi(toolOutputText(exec.output)));

  // codex PR 二审 MEDIUM #3：阻止右键事件冒泡到 AI 消息层 surface 菜单。
  // DefaultToolViewer 用于尚未实现专化菜单的工具类型——保留浏览器原生 contextmenu
  // 行为（用户可复制 INPUT/OUTPUT JSON 文本），但 stopPropagation 阻止冒泡到
  // SessionDetail::buildAssistantMessageItems。
  function stopContextMenuBubble(e: MouseEvent) {
    e.stopPropagation();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="default-viewer" oncontextmenu={stopContextMenuBubble}>
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
    min-width: 0;
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
