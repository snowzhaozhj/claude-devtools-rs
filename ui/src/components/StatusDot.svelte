<script lang="ts">
  interface Props {
    status: "ok" | "error" | "pending" | "orphaned";
  }

  let { status }: Props = $props();

  const colors: Record<string, string> = {
    ok: "#4ade80",
    error: "#f87171",
    pending: "#71717a",
    orphaned: "#71717a",
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
