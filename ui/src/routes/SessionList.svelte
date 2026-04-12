<script lang="ts">
  import { onMount } from "svelte";
  import { listSessions, type SessionSummary, type PaginatedResponse } from "../lib/api";

  interface Props {
    projectId: string;
  }

  let { projectId }: Props = $props();

  let sessions: SessionSummary[] = $state([]);
  let total = $state(0);
  let loading = $state(true);
  let error: string | null = $state(null);

  onMount(async () => {
    try {
      const result: PaginatedResponse<SessionSummary> = await listSessions(projectId);
      sessions = result.items;
      total = result.total;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  function formatTime(timestamp: number): string {
    if (timestamp === 0) return "未知";
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);

    if (diffMins < 1) return "刚刚";
    if (diffMins < 60) return `${diffMins} 分钟前`;

    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours} 小时前`;

    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 7) return `${diffDays} 天前`;

    return date.toLocaleDateString("zh-CN");
  }

  function formatSessionId(id: string): string {
    if (id.length > 16) {
      return id.slice(0, 8) + "..." + id.slice(-4);
    }
    return id;
  }
</script>

<div class="session-list">
  <div class="list-header">
    <span class="count">{total} 个会话</span>
  </div>

  {#if loading}
    <div class="loading">加载中...</div>
  {:else if error}
    <div class="error">{error}</div>
  {:else if sessions.length === 0}
    <div class="empty">暂无会话</div>
  {:else}
    {#each sessions as session}
      <div class="session-card">
        <div class="session-id">{formatSessionId(session.sessionId)}</div>
        <div class="session-time">{formatTime(session.timestamp)}</div>
      </div>
    {/each}
  {/if}
</div>

<style>
  .session-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .list-header {
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .count {
    font-size: 13px;
    color: #565f89;
  }

  .loading, .error, .empty {
    text-align: center;
    padding: 40px;
    color: #565f89;
  }

  .error {
    color: #f7768e;
  }

  .session-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: #24283b;
    border: 1px solid #3b4261;
    border-radius: 6px;
    padding: 10px 14px;
    cursor: pointer;
    transition: border-color 0.15s;
  }

  .session-card:hover {
    border-color: #7aa2f7;
  }

  .session-id {
    font-family: "SF Mono", "Fira Code", monospace;
    font-size: 13px;
    color: #c0caf5;
  }

  .session-time {
    font-size: 12px;
    color: #565f89;
  }
</style>
