<script lang="ts">
  import type { ToolExecution } from "../../lib/api";
  import { stripAnsi, toolErrorText, toolOutputText } from "../../lib/toolHelpers";
  import OutputBlock from "../OutputBlock.svelte";
  import { contextMenu } from "../../lib/contextMenu.svelte";
  import { buildBashToolItems, type MenuItemContext } from "../../lib/contextMenu/menu-items";
  import { getMenuSettings } from "../../lib/contextMenu/settings.svelte";
  import { getMenuItemDispatch } from "../../lib/contextMenu/dispatch";
  import CopyButton from "../../lib/components/CopyButton.svelte";

  interface Props {
    exec: ToolExecution;
    /** Phase 2 contextMenu surface ctx——SessionDetail / SubagentCard 路径下传，
     *  老 caller 缺省时 fallback 到空 sessionId/projectId（ctx.dispatch 仍可用） */
    sessionId?: string;
    projectId?: string;
  }

  let { exec, sessionId = "", projectId = "" }: Props = $props();

  const input = $derived(exec.input as Record<string, unknown>);
  const command = $derived(String(input?.command ?? ""));
  // Bash 输出是 terminal-style——成功 stdout 与失败 stderr/errorMessage 都可能含
  // ANSI 字节（cargo test / nextest / git 失败常带颜色），统一走 stripAnsi 渲染
  // 成纯文本。`toolErrorText` 内部对 `output.text` 路径走 cleanDisplayText 已有
  // ANSI 清洗，但 `exec.errorMessage` 顶层字段路径漏；外包 stripAnsi 兜底（幂等）
  // 同时覆盖两条分支（codex CR PR #328 第五轮）。
  const outputStr = $derived(stripAnsi(exec.isError ? toolErrorText(exec) : toolOutputText(exec.output)));

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

<div class="bash-viewer" use:contextMenu={() => buildBashToolItems(exec, buildCtx())}>
  <!-- Command -->
  <div class="bash-command">
    <span class="bash-prompt">$</span>
    <code class="bash-cmd">{command}</code>
    <CopyButton text={outputStr || command} mode="inline" />
  </div>

  <!-- Output -->
  {#if outputStr}
    <div class="bash-output-section">
      <span class="output-label" class:output-label-err={exec.isError}>
        {exec.isError ? "ERROR" : "OUTPUT"}
      </span>
      <OutputBlock code={outputStr} lang="bash" isError={exec.isError} />
    </div>
  {/if}
</div>

<style>
  .bash-viewer {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .bash-command {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 12px;
    background: var(--code-bg);
    border: 1px solid var(--code-border);
    border-radius: 6px;
  }

  .bash-prompt {
    color: var(--syntax-string);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
    flex-shrink: 0;
    user-select: none;
  }

  .bash-cmd {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--color-text);
    background: none;
    padding: 0;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .bash-output-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .output-label {
    font-size: 9px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 1px;
    text-transform: uppercase;
  }

  .output-label-err {
    color: var(--tool-result-error-text);
  }
</style>
