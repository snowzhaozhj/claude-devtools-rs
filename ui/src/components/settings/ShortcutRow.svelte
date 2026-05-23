<script lang="ts">
  /**
   * 单条 shortcut 行：左 description / 中 KeyRecorderInput / 右"重置默认" + 可选 hint。
   *
   * **行为契约**（spec keyboard-shortcuts::Settings 录键 widget）：
   * - reset 按钮在 `currentBinding === defaultEffective` 时 disabled（`spec scenario "重置全部"`）。
   * - hint 区出现在 conflict / OS-level Cmd+W / HTTP 浏览器拦截 等 advisory 文案——这是用户可见
   *   的视觉 hint（与 KeyRecorderInput 内 sr-only `.recorder-hint` 互补，不重叠职责）。
   * - i18n 暂直接中文（spec design ##Open Question 4：仓库未启用 i18n）。
   *
   * 父组件（KeyboardShortcutsPanel）负责：
   * - 维护 `pendingOverrides` overlay 并把 currentBinding 传下来
   * - findConflict 三参数计算 + 把 conflict 信息透传
   * - reset 时 emit `onReset` 让 panel 把该 id 从 pendingOverrides 删除
   * - commit 时 emit `onCommit(binding)` 让 panel 写 pendingOverrides
   */
  import KeyRecorderInput from "./KeyRecorderInput.svelte";
  import SettingsButton from "../../lib/components/SettingsButton.svelte";
  import { formatShortcut, resolveBinding } from "../../lib/platform";
  import { ROTATE_CCW_SVG } from "../../lib/icons";
  import type { ShortcutMeta } from "../../lib/keyboard/defaults";

  interface Props {
    meta: ShortcutMeta;
    /** 当前 effective binding（normalized 字符串），从 panel 的 pendingOverrides ?? defaults 派生。 */
    currentBinding: string;
    /** 当前平台展开后的 default normalized binding（用来判定 reset 按钮是否 disabled）。 */
    defaultEffective: string;
    onCommit: (binding: string) => void;
    onReset: () => void;
    /** 父组件 findConflict 计算结果——非 null 时显示 conflict 视觉态 + hint。 */
    conflict?: { conflictId: string; conflictLabel: string } | null;
    /** OS / 运行时级提示（如 Cmd+W 在 macOS / HTTP 浏览器 下被拦截）。多条用 \n 分隔。 */
    advisoryHints?: string[];
    disabled?: boolean;
  }

  let {
    meta,
    currentBinding,
    defaultEffective,
    onCommit,
    onReset,
    conflict = null,
    advisoryHints = [],
    disabled = false,
  }: Props = $props();

  let recorderId = $derived(`kbd-recorder-${meta.id.replace(/[^a-zA-Z0-9]/g, "-")}`);
  let isAtDefault = $derived(currentBinding === defaultEffective);
  let defaultDisplay = $derived(formatShortcut(meta.defaultBinding));
  // 防御：如果 panel 传下来的 currentBinding 与传下来的 defaultEffective 不一致但
  // resolveBinding(meta.defaultBinding) 与 currentBinding 一致（panel 计算口径偏差），
  // 仍然把"已是 default"判定走 panel 口径——保留 panel 单一真相源
</script>

<div class="row" class:row-disabled={disabled}>
  <div class="row-info">
    <span class="row-desc">{meta.description}</span>
    <span class="row-default">默认 <kbd>{defaultDisplay}</kbd></span>
  </div>
  <div class="row-controls">
    <KeyRecorderInput
      id={recorderId}
      {currentBinding}
      {onCommit}
      {conflict}
      {disabled}
    />
    <SettingsButton
      variant="ghost"
      size="sm"
      disabled={disabled || isAtDefault}
      onClick={onReset}
      ariaLabel={`重置 ${meta.description} 为默认`}
      title={isAtDefault ? "已是默认值" : "重置为默认"}
    >
      {#snippet icon()}
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html ROTATE_CCW_SVG}</svg>
      {/snippet}
      重置
    </SettingsButton>
  </div>
  {#if conflict}
    <div class="row-hint row-hint-warning" role="alert">
      <span aria-hidden="true">⚠</span>
      与「{conflict.conflictLabel}」冲突，请选择不同的组合键
    </div>
  {/if}
  {#each advisoryHints as hint}
    <div class="row-hint row-hint-info" role="note">
      <span aria-hidden="true">ⓘ</span>
      {hint}
    </div>
  {/each}
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-rows: auto;
    gap: 12px 16px;
    padding: 14px 16px;
    background: var(--color-surface);
    transition: background-color 0.1s;
  }
  .row-disabled {
    opacity: 0.6;
  }
  .row-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .row-desc {
    font-size: 14px;
    font-weight: 500;
    color: var(--color-text);
    line-height: 1.35;
  }
  .row-default {
    font-size: 12px;
    color: var(--color-text-secondary);
    line-height: 1.5;
  }
  .row-default kbd {
    display: inline-block;
    padding: 1px 6px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-surface-overlay);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text);
    line-height: 1.2;
  }
  .row-controls {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  .row-hint {
    grid-column: 1 / -1;
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 4px;
    font-size: 12px;
    line-height: 1.5;
  }
  .row-hint-warning {
    background: var(--surface-conflict-bg);
    border: 1px solid var(--border-conflict);
    color: var(--color-warning-text);
  }
  .row-hint-info {
    background: var(--color-info-bg);
    border: 1px solid var(--color-info-border);
    color: var(--color-info-text);
  }
</style>
