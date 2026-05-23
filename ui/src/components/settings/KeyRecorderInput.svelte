<script lang="ts">
  /**
   * 录键 widget——三态：idle / recording / conflict。
   *
   * **行为契约**（spec keyboard-shortcuts::Settings 录键 widget）：
   * - **focus 进 recording**：调 `suspend()` 暂停 dispatcher，避免录入时触发已注册快捷键。
   * - **commit-on-fullkey**：捕到第一个含主键（非纯 modifier）的 keydown 就 `onCommit` 并 blur。
   * - **Escape cancel**：直接 blur 不 commit。
   * - **录键期间 preventDefault**：不依赖 dispatcher（已 suspend），组件自身吞键防浏览器原生行为。
   * - **blur resume**：调 `resume()` 还 dispatcher 控制权。
   *
   * **a11y 6 件套**（参考 ui/CLAUDE.md 搜索框规范的同等密度）：
   * - role="button" + tabindex=0：让 `<div>` 表现成可聚焦按钮（避免 `<button>` 嵌套）。
   * - aria-label：明确"按 X 键重绑 / 录键中"语义。
   * - aria-pressed：true=recording、false=idle，宣告"toggle 状态"。
   * - aria-describedby：指向 hint 区域（"按 Esc 取消 / 按组合键确认"），conflict 态时
   *   stateLabel 朗读"冲突：与「X」重叠"——`aria-invalid` 在 ARIA 1.1 spec 不被 button role
   *   支持，故走 describedby + ShortcutRow 的 `role="alert"` 视觉行替代。
   * - aria-live="polite"：状态变化时让 SR 朗读。
   * - tabindex 与点击：键盘 / 鼠标双入口 enter recording。
   *
   * 4 设计 token（占位实现，最终由 designer 定稿——`The Recorder Idle State Rule.`）：
   *   --surface-recording-bg / --border-recording / --surface-conflict-bg / --border-conflict
   */
  import { suspend, resume } from "../../lib/keyboard/registry";
  import { formatShortcut, normalize } from "../../lib/platform";

  interface Props {
    /** 当前 binding（normalized 字符串如 "meta+k"）。空串表示无绑定。 */
    currentBinding: string;
    /** commit 一个新的 normalized binding；调用方负责冲突校验。 */
    onCommit: (binding: string) => void;
    /** 父组件传入的冲突信息——非 null 时进 conflict 视觉态。 */
    conflict?: { conflictId: string; conflictLabel: string } | null;
    /** 控件不可交互（如 Save 进行中）。 */
    disabled?: boolean;
    /** for label association；id 也作为 aria-describedby 的 hint 区前缀。 */
    id?: string;
  }

  let {
    currentBinding,
    onCommit,
    conflict = null,
    disabled = false,
    id = "key-recorder",
  }: Props = $props();

  let recording = $state(false);
  let containerEl: HTMLDivElement | null = $state(null);

  function startRecording() {
    if (disabled || recording) return;
    recording = true;
    suspend();
  }

  function stopRecording() {
    if (!recording) return;
    recording = false;
    resume();
  }

  function handleFocus() {
    startRecording();
  }

  function handleBlur() {
    stopRecording();
  }

  function handleClick() {
    // 鼠标点击时手动聚焦让 onfocus 路径触发 recording
    containerEl?.focus();
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (disabled) return;
    if (!recording) {
      // idle 态：Enter / Space 进 recording（普通按钮交互）
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        startRecording();
      }
      return;
    }
    // recording 态：组件自吞键防浏览器 / 全局 listener（dispatcher 已 suspend）
    event.preventDefault();
    event.stopPropagation();

    // Escape 取消，不 commit
    if (event.key === "Escape") {
      stopRecording();
      containerEl?.blur();
      return;
    }

    // commit-on-fullkey：normalize 返回非空（即含主键）才 commit
    const normalized = normalize(event);
    if (!normalized) return; // 仅按下 modifier 时继续等
    onCommit(normalized);
    stopRecording();
    containerEl?.blur();
  }

  // 显示文本：recording 态写"录键中"；conflict 态保持 currentBinding 的 formatted；idle 态 formatted。
  // 注意 currentBinding 已经是 normalized 字符串，formatShortcut 接受 ShortcutBinding（含 string）。
  let displayText = $derived.by(() => {
    if (recording) return "按下组合键…";
    if (!currentBinding) return "未绑定";
    return formatShortcut(currentBinding);
  });

  let stateLabel = $derived.by(() => {
    if (recording) return "录键中，按目标组合键确认，按 Esc 取消";
    if (conflict) return `冲突：与「${conflict.conflictLabel}」重叠`;
    return "按 Enter 或点击进入录键";
  });

  let hintId = $derived(`${id}-hint`);
</script>

<div
  bind:this={containerEl}
  class="recorder"
  class:recording
  class:conflict={!!conflict && !recording}
  class:disabled
  role="button"
  tabindex={disabled ? -1 : 0}
  aria-label={`快捷键 ${displayText}`}
  aria-pressed={recording}
  aria-describedby={hintId}
  aria-live="polite"
  aria-disabled={disabled}
  onfocus={handleFocus}
  onblur={handleBlur}
  onclick={handleClick}
  onkeydown={handleKeyDown}
>
  <span class="recorder-text" class:recorder-text-empty={!currentBinding && !recording}>
    {displayText}
  </span>
  {#if recording}
    <span class="recorder-spinner" aria-hidden="true">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
        <path d="M21 12a9 9 0 1 1-6.219-8.56" />
      </svg>
    </span>
  {/if}
</div>
<span id={hintId} class="recorder-hint">{stateLabel}</span>

<style>
  .recorder {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 132px;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    background: var(--color-surface);
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--color-text);
    cursor: pointer;
    user-select: none;
    transition:
      background-color 0.12s,
      border-color 0.12s,
      color 0.12s;
  }
  .recorder:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: 1px;
  }
  .recorder:hover:not(.disabled):not(.recording) {
    border-color: var(--color-border-emphasis);
  }
  .recorder.recording {
    background: var(--surface-recording-bg);
    border-color: var(--border-recording);
    color: var(--color-text);
    cursor: default;
  }
  .recorder.conflict {
    background: var(--surface-conflict-bg);
    border-color: var(--border-conflict);
  }
  .recorder.disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .recorder-text {
    flex: 1;
    text-align: center;
    line-height: 1;
    letter-spacing: 0.02em;
  }
  .recorder-text-empty {
    color: var(--color-text-muted);
    font-style: italic;
  }

  .recorder-spinner {
    display: inline-flex;
    width: 12px;
    height: 12px;
    color: var(--border-recording);
    animation: recorder-spin 1.2s linear infinite;
  }
  .recorder-spinner :global(svg) {
    width: 12px;
    height: 12px;
  }
  @keyframes recorder-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .recorder-hint {
    /* sr-only：仅为 aria-describedby 服务，不渲染视觉文本（visual hint 由 ShortcutRow 渲染） */
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
