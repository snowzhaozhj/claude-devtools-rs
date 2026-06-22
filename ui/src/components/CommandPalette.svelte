<script lang="ts">
  import { onMount } from "svelte";
  import {
    listGroupSessions,
    searchGroupSessions,
    type SessionSearchResult,
    type SessionSummary,
  } from "../lib/api";
  import { getProjectData, getProjectDataError, loadProjectData } from "../lib/projectDataStore.svelte";
  import { openTab, openJobsTab } from "../lib/tabStore.svelte";
  import { getJobsDirExists } from "../lib/jobsStore.svelte";
  import { shortenPath } from "../lib/toolHelpers";
  import { FOLDER_GIT2_SVG, MESSAGE_SQUARE, JOBS_SVG } from "../lib/icons";

  interface Props {
    selectedProjectId: string;
    onSelectProject: (id: string, name: string) => void;
    onClose: () => void;
  }

  let { selectedProjectId, onSelectProject, onClose }: Props = $props();

  // 归一化会话结果行：A（全局 id）/ B（组内正文）两路共用，统一身份与归属。
  // change cmdk-global-session-locate：前端无 per-session title/mtime，title 仅
  // 来自已加载会话，时间用 worktree 级近似（仅排序，不展示为会话时间）。
  interface SessionRow {
    sessionId: string;
    projectId: string;
    groupId: string;
    projectName: string;
    worktreeName: string;
    gitBranch: string | null;
    worktreeMostRecent: number;
    isPrimary: boolean;
    title?: string;
    hits?: number;
    /** 仅 empty-query 已加载会话有真实会话时间；全局命中无 per-session 时间故不展示 */
    timestamp?: number;
    messageCount?: number;
  }

  let query = $state("");
  let debouncedQuery = $state("");
  let sessions: SessionSummary[] = $state([]);
  let searchResults: SessionSearchResult[] = $state([]);
  // 这批 searchResults 对应的 query（trim 后）。debounce 间隙 / 失败时与当前 query
  // 不符 → contentMatchRows 丢弃，避免旧正文命中混入当前查询（codex F1 / SFH#1）。
  let searchResultsQuery = $state("");
  let sessionsSeq = 0;
  let searchSeq = 0;
  let selectedIndex = $state(0);
  let inputEl: HTMLInputElement | undefined = $state(undefined);

  const MAX_PROJECTS = 5;
  const MAX_SESSIONS = 20;
  /** 全局 sessionId 子串匹配的最小 query 长度（hex id 防爆量，D5） */
  const MIN_GLOBAL_ID_LEN = 4;
  const QUERY_DEBOUNCE_MS = 250;

  // 响应式读 store 快照（D3：不再 onMount 一次性复制；store 刷新后面板自动同步）
  const projectData = $derived(getProjectData());
  const projects = $derived(projectData?.projects ?? []);
  const repositoryGroups = $derived(projectData?.repositoryGroups ?? []);

  onMount(() => {
    inputEl?.focus();
    void loadProjectData().catch((e) =>
      console.error("CommandPalette: failed to load data", e),
    );
  });

  // 选中项目变化 → 拉该组已加载会话（带 title，作为 title 来源 + 空 query 列表）
  $effect(() => {
    const groupId = selectedProjectId;
    const seq = ++sessionsSeq;
    // 切组同步清空：避免旧组会话在新数据到达前被冠以新组 groupId/projectName 展示
    // 甚至以错归属打开（SFH#2）。
    sessions = [];
    if (!groupId) return;
    void listGroupSessions(groupId, 20)
      .then((r) => {
        if (seq === sessionsSeq) sessions = r.sessions;
      })
      .catch((e) => {
        console.error("CommandPalette: failed to load sessions", e);
        if (seq === sessionsSeq) sessions = [];
      });
  });

  // query → debouncedQuery（D8b：仅后端 searchGroupSessions IPC 防抖；前端 filter/sort
  // 走 raw query 即时更新——候选列表已 $derived 构建一次，每键只是轻量 filter，<1ms）
  $effect(() => {
    const q = query;
    const t = setTimeout(() => { debouncedQuery = q; }, QUERY_DEBOUNCE_MS);
    return () => clearTimeout(t);
  });

  // worktree id → 归属索引（B 路结果与全局命中共用，projectName 取 group 名以与项目区一致）
  const worktreeIndex = $derived.by(() => {
    const m = new Map<string, { groupId: string; projectName: string; worktreeName: string; gitBranch: string | null; mostRecent: number; isPrimary: boolean }>();
    for (const g of repositoryGroups) {
      for (const w of g.worktrees) {
        m.set(w.id, {
          groupId: g.id,
          projectName: g.name,
          worktreeName: w.name,
          gitBranch: w.gitBranch,
          mostRecent: w.mostRecentSession ?? 0,
          isPrimary: !!(w.isRepoRoot || w.isMainWorktree),
        });
      }
    }
    return m;
  });

  // 全部会话候选（normalized row），随快照变化构建一次（D1/D4）
  const allSessionRows = $derived.by((): SessionRow[] => {
    const rows: SessionRow[] = [];
    for (const g of repositoryGroups) {
      for (const w of g.worktrees) {
        const isPrimary = !!(w.isRepoRoot || w.isMainWorktree);
        for (const sid of w.sessions) {
          rows.push({
            sessionId: sid,
            projectId: w.id,
            groupId: g.id,
            projectName: g.name,
            worktreeName: w.name,
            gitBranch: w.gitBranch,
            worktreeMostRecent: w.mostRecentSession ?? 0,
            isPrimary,
          });
        }
      }
    }
    return rows;
  });

  // 已加载会话的 title 索引（D2：title 唯一来源，绝不为补 title 发 IPC）
  const loadedTitles = $derived.by(() => {
    const m = new Map<string, string>();
    for (const s of sessions) if (s.title) m.set(s.sessionId, s.title);
    return m;
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

  // 同 sessionId 去重，确定性 tie-break：优先 main/repo-root worktree，否则遍历首条（D4）
  function dedupBySessionId(rows: SessionRow[]): SessionRow[] {
    const map = new Map<string, SessionRow>();
    for (const r of rows) {
      const existing = map.get(r.sessionId);
      if (!existing || (r.isPrimary && !existing.isPrimary)) map.set(r.sessionId, r);
    }
    return [...map.values()];
  }

  // A 路：全局 sessionId 子串定位（仅 query ≥ 4，跨所有项目）
  const globalIdMatches = $derived.by((): SessionRow[] => {
    const q = query.trim().toLowerCase();
    if (q.length < MIN_GLOBAL_ID_LEN) return [];
    const matched = allSessionRows.filter(r => r.sessionId.toLowerCase().includes(q));
    return dedupBySessionId(matched);
  });

  // B 路：组内正文搜索结果 → normalized row（归属由 worktreeIndex 补 groupId/projectName）
  const contentMatchRows = $derived.by((): SessionRow[] => {
    // debounce 间隙 / 后端失败：searchResults 属于旧 query → 丢弃，不混入当前查询（codex F1 / SFH#1）
    if (searchResultsQuery !== query.trim()) return [];
    return searchResults.map((r): SessionRow => {
      const idx = worktreeIndex.get(r.projectId);
      return {
        sessionId: r.sessionId,
        projectId: r.projectId,
        groupId: idx?.groupId ?? selectedProjectId,
        projectName: idx?.projectName ?? "",
        worktreeName: idx?.worktreeName ?? "",
        gitBranch: idx?.gitBranch ?? null,
        worktreeMostRecent: idx?.mostRecent ?? 0,
        isPrimary: idx?.isPrimary ?? true,
        title: r.sessionTitle || loadedTitles.get(r.sessionId) || undefined,
        hits: r.totalMatches,
      };
    });
  });

  const filteredSessions = $derived.by((): SessionRow[] => {
    const q = query.trim();
    // 空 query：维持现状——展示当前选中组的已加载会话
    if (!q) {
      if (!selectedProjectId) return [];
      const idx = worktreeIndex.get(selectedProjectId);
      const groupName = projects.find(p => p.id === selectedProjectId)?.displayName ?? idx?.projectName ?? "";
      return sessions.slice(0, MAX_SESSIONS).map((s): SessionRow => ({
        sessionId: s.sessionId,
        projectId: s.projectId,
        groupId: selectedProjectId,
        projectName: groupName,
        worktreeName: idx?.worktreeName ?? "",
        gitBranch: s.gitBranch ?? null,
        worktreeMostRecent: s.timestamp ?? 0,
        isPrimary: true,
        title: s.title ?? undefined,
        timestamp: s.timestamp,
        messageCount: s.messageCount,
      }));
    }

    // identity = sessionId 合并 A + B：B 优先补 projectId/groupId/hits（归属权威）
    const map = new Map<string, SessionRow>();
    for (const a of globalIdMatches) {
      map.set(a.sessionId, { ...a, title: loadedTitles.get(a.sessionId) ?? a.title });
    }
    for (const b of contentMatchRows) {
      const a = map.get(b.sessionId);
      map.set(b.sessionId, { ...(a ?? {}), ...b, title: b.title ?? a?.title });
    }

    // 确定性排序：worktreeMostRecent 倒序 → projectName → sessionId（D6）
    const merged = [...map.values()].sort((x, y) =>
      (y.worktreeMostRecent - x.worktreeMostRecent) ||
      x.projectName.localeCompare(y.projectName) ||
      x.sessionId.localeCompare(y.sessionId));
    return merged.slice(0, MAX_SESSIONS);
  });

  // 截断提示（无静默 cap）：合并去重后总数 > 展示上限
  const sessionTotalBeforeCap = $derived.by(() => {
    const q = query.trim();
    if (!q) return selectedProjectId ? sessions.length : 0;
    const ids = new Set<string>();
    for (const a of globalIdMatches) ids.add(a.sessionId);
    for (const b of contentMatchRows) ids.add(b.sessionId);
    return ids.size;
  });
  const sessionsTruncated = $derived(sessionTotalBeforeCap > filteredSessions.length);

  // 短查询（1–3 字符）未选项目 → 给提示，不留无解释空白（D5）
  const showShortQueryHint = $derived.by(() => {
    const q = query.trim();
    return q.length > 0 && q.length < MIN_GLOBAL_ID_LEN && !selectedProjectId;
  });

  interface PaletteAction {
    id: string;
    label: string;
    detail: string;
    icon: string;
    handler: () => void;
  }

  const actions = $derived.by((): PaletteAction[] => {
    const q = query.toLowerCase();
    const items: PaletteAction[] = [];
    if (getJobsDirExists()) {
      const matches = !q || "background jobs".includes(q) || "open jobs".includes(q) || "bg".includes(q);
      if (matches) {
        items.push({
          id: "open-jobs",
          label: "Open Background Jobs",
          detail: "查看后台任务状态",
          icon: JOBS_SVG,
          handler: () => { openJobsTab(); onClose(); },
        });
      }
    }
    return items;
  });

  const totalResults = $derived(filteredProjects.length + filteredSessions.length + actions.length);

  // B 路后端正文搜索（debounced，scoped 当前选中组）
  $effect(() => {
    const q = debouncedQuery.trim();
    const projectId = selectedProjectId;
    const seq = ++searchSeq;
    if (!q || !projectId) {
      searchResults = [];
      searchResultsQuery = q;
      return;
    }
    void searchGroupSessions(projectId, q)
      .then((result) => {
        if (seq === searchSeq) { searchResults = result.results; searchResultsQuery = q; }
      })
      .catch((e) => {
        console.error("CommandPalette: failed to search sessions", e);
        // seq 守卫清空 + 标记当前 query：失败不残留旧命中（SFH#1）；全局 id 定位
        // 走纯前端内存，后端搜索失败时仍可用（graceful degradation）。
        if (seq === searchSeq) { searchResults = []; searchResultsQuery = q; }
      });
  });

  // 查询变化 → 重置选中（跟 raw query 即时，避免 type-then-Enter 选到旧列表项，codex F2）
  $effect(() => { query; selectedIndex = 0; });

  // ── 键盘导航 ──

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      onClose();
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (totalResults > 0) selectedIndex = Math.min(selectedIndex + 1, totalResults - 1);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      if (totalResults > 0) selectedIndex = Math.max(selectedIndex - 1, 0);
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      selectByIndex(selectedIndex);
    }
  }

  function rowTitle(row: SessionRow): string {
    return row.title || row.sessionId.slice(0, 8);
  }

  // 无 title 时的定位补充：项目名 + worktree/branch（D2 兜底）
  function rowLocation(row: SessionRow): string {
    if (row.title) return row.projectName;
    const parts = [row.projectName];
    if (row.worktreeName && row.worktreeName !== row.projectName) parts.push(row.worktreeName);
    else if (row.gitBranch) parts.push(row.gitBranch);
    return parts.filter(Boolean).join(" · ");
  }

  function openSession(row: SessionRow) {
    openTab(row.sessionId, row.projectId, rowTitle(row), row.groupId);
    onClose();
  }

  function selectByIndex(idx: number) {
    if (idx < 0 || idx >= totalResults) return;
    if (idx < filteredProjects.length) {
      const p = filteredProjects[idx];
      onSelectProject(p.id, p.displayName);
      onClose();
    } else if (idx < filteredProjects.length + filteredSessions.length) {
      const si = idx - filteredProjects.length;
      if (si < filteredSessions.length) openSession(filteredSessions[si]);
    } else {
      const ai = idx - filteredProjects.length - filteredSessions.length;
      if (ai < actions.length) actions[ai].handler();
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
    type="search"
    placeholder="搜索项目或会话..."
    bind:this={inputEl}
    bind:value={query}
    autocomplete="off"
    autocorrect="off"
    autocapitalize="off"
    spellcheck="false"
    enterkeyhint="search"
    aria-label="命令面板搜索"
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
          <svg
            class="cp-item-icon"
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            {@html FOLDER_GIT2_SVG}
          </svg>
          <span class="cp-item-label">{project.displayName}</span>
          <span class="cp-item-detail">{shortenPath(project.path)}</span>
          <span class="cp-item-badge">{project.sessionCount}</span>
        </button>
      {/each}
    {/if}

    {#if filteredSessions.length > 0}
      <div class="cp-section">会话</div>
      {#each filteredSessions as row, i (row.sessionId)}
        {@const flatIdx = filteredProjects.length + i}
        <button
          class="cp-item"
          class:cp-item-selected={flatIdx === selectedIndex}
          onclick={() => openSession(row)}
        >
          <svg
            class="cp-item-icon"
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d={MESSAGE_SQUARE} />
          </svg>
          <span class="cp-item-label">{rowTitle(row)}</span>
          <span class="cp-item-detail" title={rowLocation(row)}>{rowLocation(row)}</span>
          {#if row.hits}
            <span class="cp-item-badge" title="正文匹配数">{row.hits}</span>
          {:else if row.messageCount}
            <span class="cp-item-badge" title="消息数量">{row.messageCount}</span>
          {/if}
          {#if row.timestamp}
            <span class="cp-item-time">{formatTime(row.timestamp)}</span>
          {/if}
        </button>
      {/each}
      {#if sessionsTruncated}
        <div class="cp-truncation">仅显示前 {MAX_SESSIONS} 条，输入更多字符缩小范围</div>
      {/if}
    {/if}

    {#if actions.length > 0}
      <div class="cp-section">操作</div>
      {#each actions as action, i}
        {@const flatIdx = filteredProjects.length + filteredSessions.length + i}
        <button
          class="cp-item"
          class:cp-item-selected={flatIdx === selectedIndex}
          onclick={action.handler}
        >
          <svg
            class="cp-item-icon"
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            {@html action.icon}
          </svg>
          <span class="cp-item-label">{action.label}</span>
          <span class="cp-item-detail">{action.detail}</span>
        </button>
      {/each}
    {/if}

    <!-- 短查询提示独立渲染，不被 actions(jobs) 撑非零的 totalResults 挤掉（codex F3） -->
    {#if showShortQueryHint}
      <div class="cp-truncation">输入 ≥{MIN_GLOBAL_ID_LEN} 个字符按 Session ID 全局定位</div>
    {/if}

    {#if totalResults === 0 && !showShortQueryHint}
      {#if getProjectDataError()}
        <div class="cp-empty">加载失败，请重试</div>
      {:else}
        <div class="cp-empty">无匹配结果</div>
      {/if}
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

  .cp-input::-webkit-search-cancel-button,
  .cp-input::-webkit-search-decoration {
    appearance: none;
    -webkit-appearance: none;
  }

  .cp-results {
    flex: 1;
    overflow-y: auto;
    /* scrollbar-gutter-exempt: 浮层打开即定尺寸，滚动条首帧即在，无生命周期内宽度跳变 */
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
    width: 14px;
    height: 14px;
    color: var(--color-text-muted);
    flex-shrink: 0;
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

  .cp-truncation {
    text-align: center;
    padding: 6px 12px;
    color: var(--color-text-muted);
    font-size: 11px;
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
