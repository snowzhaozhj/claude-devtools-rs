<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { toolErrorText, toolOutputText } from "../../lib/toolHelpers";
  import OutputBlock from "../OutputBlock.svelte";

  interface Props {
    exec: ToolExecution;
    /** 完整输出懒加载中：以限高档稳定占位渲染，复制禁用。 */
    outputLoading?: boolean;
    /** 懒加载失败：显式失败态。 */
    outputLoadFailed?: boolean;
  }

  let { exec, outputLoading = false, outputLoadFailed = false }: Props = $props();

  const inputStr = $derived(JSON.stringify(exec.input, null, 2));
  const outputStr = $derived(exec.isError ? toolErrorText(exec) : toolOutputText(exec.output));

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

  {#if outputLoading || outputLoadFailed}
    <div class="viewer-section">
      <span class="viewer-label" class:viewer-label-err={exec.isError}>
        {exec.isError ? "ERROR" : "OUTPUT"}
      </span>
      <OutputBlock code="" isError={exec.isError} loading={outputLoading} loadFailed={outputLoadFailed} bytesHint={exec.outputBytes} />
    </div>
  {:else if outputStr}
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
