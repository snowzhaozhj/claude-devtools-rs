<script lang="ts">
  interface Props {
    status: "ok" | "error" | "pending" | "orphaned";
  }

  let { status }: Props = $props();

  // 走主题 token（非硬编码十六进制）——浅/深主题自动切换，避免亮色主题下
  // 用暗色调表达状态色。
  const colors: Record<string, string> = {
    ok: "var(--color-success)",
    error: "var(--color-danger)",
    pending: "var(--color-text-muted)",
    orphaned: "var(--color-text-muted)",
  };
</script>

<span class="status-dot" style="background-color: {colors[status] ?? colors.pending}"></span>

<style>
  .status-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }
</style>
