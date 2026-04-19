<script lang="ts">
  /**
   * SettingsToggle — Linear 风格滑块 Switch，对齐原版
   * `SettingsToggle.tsx`。9×5 圆角 track + 16px 白色 thumb + `#6366f1` 启用紫色。
   */
  interface Props {
    enabled: boolean;
    onChange: (value: boolean) => void;
    disabled?: boolean;
    ariaLabel?: string;
  }

  let { enabled, onChange, disabled = false, ariaLabel }: Props = $props();

  function handleClick() {
    if (disabled) return;
    onChange(!enabled);
  }
</script>

<button
  type="button"
  role="switch"
  aria-checked={enabled}
  aria-label={ariaLabel}
  {disabled}
  class="toggle-switch"
  class:toggle-switch-on={enabled}
  class:toggle-switch-disabled={disabled}
  onclick={handleClick}
>
  <span class="toggle-thumb" class:toggle-thumb-on={enabled}></span>
</button>

<style>
  .toggle-switch {
    position: relative;
    display: inline-flex;
    flex-shrink: 0;
    width: 36px;
    height: 20px;
    padding: 0;
    border: 2px solid transparent;
    border-radius: 9999px;
    background: var(--color-switch-off-track);
    cursor: pointer;
    transition:
      background-color 0.2s ease-in-out,
      opacity 0.15s ease-in-out;
  }
  .toggle-switch:focus-visible {
    outline: 2px solid var(--color-switch-on);
    outline-offset: 2px;
  }
  .toggle-switch-on {
    background: var(--color-switch-on);
  }
  .toggle-switch-disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .toggle-thumb {
    pointer-events: none;
    display: inline-block;
    width: 16px;
    height: 16px;
    border-radius: 9999px;
    background: var(--color-switch-thumb);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
    transform: translateX(0);
    transition: transform 0.2s ease-in-out;
  }
  .toggle-thumb-on {
    transform: translateX(16px);
  }
</style>
