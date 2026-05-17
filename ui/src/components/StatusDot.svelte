<script lang="ts">
  interface Props {
    status: "ok" | "error" | "pending" | "orphaned";
  }

  let { status }: Props = $props();
</script>

<span class="status-dot" data-status={status} aria-hidden="true"></span>

<style>
  /* 状态点：颜色统一走 CSS 变量，跟主题切换；pending / orphaned 视为同一
     语义（"不可用 / 等待中"），共用 muted 灰，避免引入第三种状态色。 */
  .status-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
    background-color: var(--color-text-muted);
  }
  .status-dot[data-status="ok"] {
    background-color: var(--color-success-bright);
  }
  .status-dot[data-status="error"] {
    background-color: var(--color-danger-bright);
  }
</style>
