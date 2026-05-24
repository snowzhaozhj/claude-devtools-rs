<script lang="ts">
  import { tick } from "svelte";
  import { contextMenu } from "../contextMenu.svelte";
  import { buildWorktreeChipItems, type MenuItemContext } from "../contextMenu/menu-items";
  import { getMenuSettings } from "../contextMenu/settings.svelte";
  import { getMenuItemDispatch } from "../contextMenu/dispatch";

  export interface ChipOption {
    value: string;
    label: string;
    /** 可选：worktree 文件系统路径——有值时该 chip 弹"在终端打开 / 在编辑器打开
     *  / 复制路径"右键菜单（Phase 2 spec sidebar-navigation::worktree chip 右键菜单）。
     *  "全部"等聚合 chip 不传 path → 无右键菜单。 */
    path?: string;
    /** 可选：worktree 名称（仅 chip 自身 label 已含 `⌗` 前缀，独立 name 给菜单
     *  用于"复制项目名"等纯文本场景） */
    name?: string;
  }

  interface Props {
    value: string;
    options: ChipOption[];
    onChange: (v: string) => void;
    ariaLabel?: string;
  }

  let { value, options, onChange, ariaLabel }: Props = $props();

  function buildCtx(): MenuItemContext {
    return {
      sessionId: "",
      projectId: "",
      settings: getMenuSettings(),
      selectionText: window.getSelection()?.toString() ?? "",
      dispatch: getMenuItemDispatch(),
    };
  }

  /** 仅含 path 的 chip 才弹菜单——"全部"等聚合 chip 跳过 */
  function chipMenuProvider(opt: ChipOption) {
    return () => {
      if (!opt.path) return [];
      return buildWorktreeChipItems(
        { path: opt.path, name: opt.name ?? opt.label },
        buildCtx(),
      );
    };
  }

  let chipEls: HTMLButtonElement[] = $state([]);

  function selectAt(i: number) {
    const opt = options[i];
    if (!opt) return;
    if (opt.value !== value) onChange(opt.value);
  }

  async function focusAt(i: number) {
    await tick();
    chipEls[i]?.focus();
  }

  function onKeydown(e: KeyboardEvent, i: number) {
    if (e.key === "ArrowRight") {
      if (i >= options.length - 1) return;
      e.preventDefault();
      selectAt(i + 1);
      void focusAt(i + 1);
    } else if (e.key === "ArrowLeft") {
      if (i <= 0) return;
      e.preventDefault();
      selectAt(i - 1);
      void focusAt(i - 1);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      selectAt(i);
    }
  }
</script>

<div class="worktree-chip-cluster" role="radiogroup" aria-label={ariaLabel}>
  {#each options as opt, i (opt.value)}
    <button
      type="button"
      class="worktree-chip"
      class:worktree-chip-active={opt.value === value}
      role="radio"
      aria-checked={opt.value === value}
      tabindex={opt.value === value ? 0 : -1}
      onclick={() => selectAt(i)}
      onkeydown={(e) => onKeydown(e, i)}
      bind:this={chipEls[i]}
      use:contextMenu={chipMenuProvider(opt)}
    >{opt.label}</button>
  {/each}
</div>

<style>
  .worktree-chip-cluster {
    display: flex;
    flex-wrap: nowrap;
    align-items: center;
    gap: 4px;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
    -webkit-mask-image: linear-gradient(to right, black calc(100% - 16px), transparent);
    mask-image: linear-gradient(to right, black calc(100% - 16px), transparent);
  }
  .worktree-chip-cluster::-webkit-scrollbar {
    display: none;
  }

  .worktree-chip {
    flex-shrink: 0;
    height: 24px;
    padding: 3px 10px;
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--color-text-secondary);
    border: 1px solid transparent;
    background: transparent;
    cursor: pointer;
    white-space: nowrap;
    user-select: none;
    outline: none;
    transition: background-color 0.12s, color 0.12s, border-color 0.12s;
  }
  .worktree-chip:hover {
    background: var(--tool-item-hover-bg, var(--color-surface-overlay));
  }
  .worktree-chip-active {
    background: var(--color-surface-overlay);
    color: var(--color-text);
    border-color: var(--color-border-emphasis, var(--color-border));
  }
  .worktree-chip:focus-visible {
    outline: 2px solid var(--color-accent-blue);
    outline-offset: 1px;
  }
</style>
