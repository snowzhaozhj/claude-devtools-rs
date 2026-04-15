<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listProjects, listSessions, type ProjectInfo, type SessionSummary } from "../lib/api";
  import { openTab } from "../lib/tabStore.svelte";
  import { shortenPath } from "../lib/toolHelpers";

  interface Props {
    selectedProjectId: string;
    onSelectProject: (id: string, name: string) => void;
    onClose: () => void;
  }

  let { selectedProjectId, onSelectProject, onClose }: Props = $props();

  let query = $state("");
  let projects: ProjectInfo[] = $state([]);
  let sessions: SessionSummary[] = $state([]);
  let selectedIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state(undefined);

  const MAX_PROJECTS = 5;
  const MAX_SESSIONS = 20;

  onMount(async () => {
    inputEl?.focus();
    try {
      projects = await listProjects();
      if (selectedProjectId) {
        const r = await listSessions(selectedProjectId);
        sessions = r.items;
      }
    } catch (e) {
      console.error("CommandPalette: failed to load data", e);
    }
  });

  // ── 过滤 ──

  const filteredProjects = $derived.by(() => {
    const q = query.toLowerCase();
    const list = q
      ? projects.filter(p =>
          p.displayName.toLowerCase().includes(q) ||
          p.path.toLowerCase().includes(q))
      : projects;
    return list.slice(0, MAX_PROJECTS);
  });

  const filteredSessions = $derived.by(() => {
    if (!selectedProjectId) return [];
    const q = query.toLowerCase();
    const list = q
      ? sessions.filter(s =>
          (s.title || s.sessionId).toLowerCase().includes(q))
      : sessions;
    return list.slice(0, MAX_SESSIONS);
  });

  const totalResults = $derived(filteredProjects.length + filteredSessions.length);

  // 查询变化 → 重置选中
  $effect(() => { query; selectedIndex = 0; });

  // ── 键盘导航 ──

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      onClose();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, totalResults - 1);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      selectByIndex(selectedIndex);
    }
  }

  function selectByIndex(idx: number) {
    if (idx < filteredProjects.length) {
      const p = filteredProjects[idx];
      onSelectProject(p.id, p.displayName);
      onClose();
    } else {
      const si = idx - filteredProjects.length;
      if (si < filteredSessions.length) {
        const s = filteredSessions[si];
        openTab(s.sessionId, selectedProjectId, s.title || s.sessionId.slice(0, 8));
        onClose();
      }
    }
  }

  function formatTime(ts: number): string {
    if (!ts) return "";
    const diff = Date.now() - ts;
    const mins = Math.floor(diff / 60000);
    if (mins < 1) return "刚刚";
    if (mins < 60) return `${mins}m`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours}h`;
    const days = Math.floor(hours / 24);
    if (days < 7) return `${days}d`;
    return new Date(ts).toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="cp-backdrop" onclick={onClose}></div>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="cp-dialog" onkeydown={handleKeyDown}>
  <input
    class="cp-input"
    type="text"
    placeholder="搜索项目或会话..."
    bind:this={inputEl}
    bind:value={query}
  />

  <div class="cp-results">
    {#if filteredProjects.length > 0}
      <div class="cp-section">项目</div>
      {#each filteredProjects as project, i}
        <button
          class="cp-item"
          class:cp-item-selected={i === selectedIndex}
          onclick={() => { onSelectProject(project.id, project.displayName); onClose(); }}
        >
          <span class="cp-item-icon">◆</span>
          <span class="cp-item-label">{project.displayName}</span>
          <span class="cp-item-detail">{shortenPath(project.path)}</span>
          <span class="cp-item-badge">{project.sessionCount}</span>
        </button>
      {/each}
    {/if}

    {#if filteredSessions.length > 0}
      <div class="cp-section">会话</div>
      {#each filteredSessions as session, i}
        {@const flatIdx = filteredProjects.length + i}
        <button
          class="cp-item"
          class:cp-item-selected={flatIdx === selectedIndex}
          onclick={() => { openTab(session.sessionId, selectedProjectId, session.title || session.sessionId.slice(0, 8)); onClose(); }}
        >
          <span class="cp-item-icon">◇</span>
          <span class="cp-item-label">{session.title || session.sessionId.slice(0, 8) + "…"}</span>
          <span class="cp-item-detail">C{session.messageCount || ""}</span>
          <span class="cp-item-time">{formatTime(session.timestamp)}</span>
        </button>
      {/each}
    {/if}

    {#if totalResults === 0}
      <div class="cp-empty">无匹配结果</div>
    {/if}
  </div>

  <div class="cp-footer">
    <span class="cp-hint">↑↓ 导航</span>
    <span class="cp-hint">↵ 选择</span>
    <span class="cp-hint">esc 关闭</span>
  </div>
</div>

<style>
  .cp-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 200;
  }

  .cp-dialog {
    position: fixed;
    top: 15%;
    left: 50%;
    transform: translateX(-50%);
    width: 520px;
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 12px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.2);
    z-index: 201;
    overflow: hidden;
  }

  .cp-input {
    width: 100%;
    font-size: 15px;
    font-family: inherit;
    color: var(--color-text);
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--color-border);
    padding: 14px 16px;
    outline: none;
  }

  .cp-input::placeholder {
    color: var(--color-text-muted);
  }

  .cp-results {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
  }

  .cp-section {
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-muted);
    padding: 8px 12px 4px;
    letter-spacing: 0.3px;
  }

  .cp-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 12px;
    background: none;
    border: none;
    border-radius: 6px;
    font: inherit;
    font-size: 13px;
    color: var(--color-text);
    text-align: left;
    cursor: pointer;
    transition: background 0.08s;
  }

  .cp-item:hover {
    background: var(--tool-item-hover-bg);
  }

  .cp-item-selected {
    background: var(--color-surface-raised);
  }

  .cp-item-icon {
    font-size: 12px;
    color: var(--color-text-muted);
    flex-shrink: 0;
    width: 16px;
    text-align: center;
  }

  .cp-item-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }

  .cp-item-detail {
    font-size: 12px;
    color: var(--color-text-muted);
    font-family: var(--font-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
    flex-shrink: 0;
  }

  .cp-item-badge {
    font-size: 11px;
    color: var(--color-text-muted);
    background: var(--badge-neutral-bg);
    padding: 1px 6px;
    border-radius: 4px;
    font-family: var(--font-mono);
    flex-shrink: 0;
  }

  .cp-item-time {
    font-size: 11px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .cp-empty {
    text-align: center;
    padding: 24px;
    color: var(--color-text-muted);
    font-size: 13px;
  }

  .cp-footer {
    display: flex;
    gap: 16px;
    padding: 8px 16px;
    border-top: 1px solid var(--color-border);
  }

  .cp-hint {
    font-size: 11px;
    color: var(--color-text-muted);
  }
</style>
