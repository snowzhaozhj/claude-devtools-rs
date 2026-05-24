<script lang="ts">
  /**
   * 录键 widget——四态：idle / recording / conflict / warning（Win 键守卫）。
   *
   * **行为契约**（spec keyboard-shortcuts::Settings 录键 widget）：
   * - **focus 进 recording**：调 `suspend()` 暂停 dispatcher，避免录入时触发已注册快捷键。
   * - **commit-on-fullkey**：捕到第一个含主键（非纯 modifier）的 keydown 就 `onCommit` 并 blur。
   *   commit binding 由 `recordBindingFromEvent` 产出 `mod+x` 字面量（跨平台 source-of-truth），
   *   不直接产出平台特化 `meta+x` / `ctrl+x`。
   * - **Win 键守卫**（non-mac 平台 + `event.metaKey`）：先于 `recordBindingFromEvent` 守卫，
   *   不 commit 不 blur，进 warning 子态显示提示文本，下次不含 metaKey 的 keydown 自动清除。
   * - **Escape cancel**：直接 blur 不 commit。
   * - **录键期间 preventDefault**：不依赖 dispatcher（已 suspend），组件自身吞键防浏览器原生行为。
   * - **blur resume**：调 `resume()` 还 dispatcher 控制权。
   *
   * **a11y**（参考 ui/CLAUDE.md 搜索框规范的同等密度）：
   * - role="button" + tabindex=0：让 `<div>` 表现成可聚焦按钮（避免 `<button>` 嵌套）。
   * - aria-label：明确"按 X 键重绑 / 录键中"语义。
   * - aria-pressed：true=recording、false=idle，宣告"toggle 状态"。
   * - aria-describedby：指向 hint span，stateLabel 含状态文本变化（recording / conflict / warning）。
   * - aria-live="polite"：声明在 hint span 而非 recorder 容器，避免 SR 在 focus / pressed
   *   等容器属性变化时双宣告 noise；仅 hint 文本切换才是用户关心的语义。
   * - tabindex 与点击：键盘 / 鼠标双入口 enter recording。
   *
   * 设计 token（`DESIGN.md::The Recorder Idle State Rule` + `The Conflict Is Warning Not Error Rule`）：
   *   --surface-recording-bg / --border-recording / --surface-conflict-bg / --border-conflict
   * warning 子态视觉复用 conflict token（按 `The Conflict Is Warning Not Error Rule` 鼓励的复用），
   * 靠 hint 文本与 conflict 区分。
   */
  import { suspend, resume } from "../../lib/keyboard/registry";
  import { formatShortcut, isMac, recordBindingFromEvent } from "../../lib/platform";

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
  /** Win 键守卫子态：non-mac 平台 + event.metaKey === true 时触发。下次不含 metaKey 的 keydown / Esc / blur 时清除。 */
  let winKeyWarning = $state(false);
  let containerEl: HTMLDivElement | null = $state(null);

  function startRecording() {
    if (disabled || recording) return;
    recording = true;
    suspend();
  }

  function stopRecording() {
    if (!recording) return;
    recording = false;
    winKeyWarning = false;
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

    // Escape 取消，不 commit；stopRecording 会清 winKeyWarning
    if (event.key === "Escape") {
      stopRecording();
      containerEl?.blur();
      return;
    }

    // Win 键守卫：non-mac 平台 + event.metaKey 时不 commit、不 blur，进 warning 子态等待用户重录
    if (!isMac() && event.metaKey) {
      winKeyWarning = true;
      return;
    }
    // 不含 metaKey 的下一次 keydown 自动清除 warning（无论是否触发 commit）
    winKeyWarning = false;

    // commit-on-fullkey：recordBindingFromEvent 产出 `mod+x` 字面量；返回非空才 commit
    const recorded = recordBindingFromEvent(event);
    if (!recorded) return; // 仅按下 modifier 时继续等
    onCommit(recorded);
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
    // 优先级：winKeyWarning > conflict > recording > idle
    if (winKeyWarning) return "Windows 不支持 Win 键作为修饰键，按目标组合键重新录入";
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
  class:warning={winKeyWarning}
  class:disabled
  role="button"
  tabindex={disabled ? -1 : 0}
  aria-label={`快捷键 ${displayText}`}
  aria-pressed={recording}
  aria-describedby={hintId}
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
<span id={hintId} class="recorder-hint" aria-live="polite">{stateLabel}</span>

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
  .recorder.warning {
    /* Win 键守卫态视觉与 conflict 等同（DESIGN.md::The Conflict Is Warning Not Error Rule
       鼓励的复用），靠 hint 文本与 conflict 区分。优先级：warning 覆盖 recording / conflict 视觉。 */
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
    animation: recorder-spin 0.9s linear infinite;
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
