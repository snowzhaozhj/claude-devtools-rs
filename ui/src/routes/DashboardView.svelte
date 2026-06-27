<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { loadProjectData, getProjectData, type ProjectData } from "../lib/projectDataStore.svelte";
  import {
    deriveDashboardProjects,
    sortDashboardProjects,
    filterDashboardProjects,
    formatRelativeTime,
    type DashboardProject,
    type DashboardSortKey,
  } from "../lib/dashboardProjects";
  import { shortenPath } from "../lib/toolHelpers";
  import { errorMessage } from "../lib/errorMessage";
  import { FOLDER_GIT2_SVG, GIT_BRANCH_SVG } from "../lib/icons";
  import Skeleton from "../components/Skeleton.svelte";
  import Dropdown from "../lib/components/Dropdown.svelte";
  import {
    registerHandler,
    unregisterHandler,
    scheduleRefresh,
    cancelScheduledRefresh,
  } from "../lib/fileChangeStore.svelte";
  import { registerShortcut } from "../lib/keyboard/registry";
  import { getShortcutMeta } from "../lib/keyboard/defaults";
  import { contextMenu } from "../lib/contextMenu.svelte";
  import { buildProjectCardItems, type MenuItemContext } from "../lib/contextMenu/menu-items";
  import { getMenuSettings } from "../lib/contextMenu/settings.svelte";
  import { getMenuItemDispatch } from "../lib/contextMenu/dispatch";

  // ── Project card right-click menu（Task 9.6 / spec frontend-context-menu Phase 2）──
  function buildProjectCtx(): MenuItemContext {
    return {
      sessionId: "",
      projectId: "",
      settings: getMenuSettings(),
      selectionText: window.getSelection()?.toString() ?? "",
      dispatch: getMenuItemDispatch(),
    };
  }
  function projectMenuProvider(p: DashboardProject) {
    return () => buildProjectCardItems(
      { path: p.path, name: p.displayName },
      buildProjectCtx(),
    );
  }

  const SORT_OPTIONS: { value: DashboardSortKey; label: string }[] = [
    { value: "recent", label: "最近活动" },
    { value: "sessions", label: "会话数最多" },
    { value: "name", label: "字母序" },
  ];

  interface Props {
    selectedProjectId: string;
    onSelectProject: (id: string, name: string) => void;
  }

  let { selectedProjectId, onSelectProject }: Props = $props();

  type ViewMode = "list" | "grid";

  // 偏好持久化：list/grid 视图与排序方式都属于工作台级别偏好，应跨会话稳定。
  // 启动时读 localStorage 一次；后续在 effect 里同步写。
  const VIEW_STORAGE_KEY = "cdt:dashboard:view";
  const SORT_STORAGE_KEY = "cdt:dashboard:sort";

  function readView(): ViewMode {
    if (typeof localStorage === "undefined") return "list";
    const v = localStorage.getItem(VIEW_STORAGE_KEY);
    return v === "grid" ? "grid" : "list";
  }
  function readSort(): DashboardSortKey {
    if (typeof localStorage === "undefined") return "recent";
    const v = localStorage.getItem(SORT_STORAGE_KEY);
    return v === "sessions" || v === "name" ? v : "recent";
  }

  let viewMode: ViewMode = $state(readView());
  let sortKey: DashboardSortKey = $state(readSort());
  let filterQuery = $state("");
  // 模块级 store 已 cache 时直接复用，避免冷启首屏闪一帧 skeleton；
  // `untrack` 抑制 svelte-check `state_referenced_locally` 警告。
  let projectData: ProjectData | null = $state(untrack(() => getProjectData()));
  let loading = $state(untrack(() => projectData === null));
  // 首屏加载失败时的错误态：仅在无任何已有数据时升到 UI（silent 刷新失败
  // 不抹掉已显示的项目列表），驱动模板的错误分支 + 重试按钮。
  let loadError = $state<string | null>(null);
  let searchEl: HTMLInputElement | undefined = $state();

  // 点击当前已选项目时无 selectedProjectId 变化 → App 状态不更新；
  // 给当前行/卡片打短暂 pulse 触发 ring 动画，让点击被"听到"。
  let pulsingId = $state<string | null>(null);
  let pulseTimer: ReturnType<typeof setTimeout> | undefined;
  // `/` 聚焦搜索快捷键的 unregister 闭包；onMount 注册 / onDestroy 释放。
  let unregisterFocusShortcut: (() => void) | null = null;

  function handleSelect(p: DashboardProject) {
    if (p.id === selectedProjectId) {
      pulsingId = p.id;
      if (pulseTimer) clearTimeout(pulseTimer);
      pulseTimer = setTimeout(() => {
        pulsingId = null;
      }, 480);
    }
    onSelectProject(p.id, p.displayName);
  }

  async function loadData(silent = false) {
    if (!silent && projectData === null) loading = true;
    try {
      projectData = await loadProjectData({ refresh: silent });
      loadError = null;
    } catch (e) {
      console.error("Failed to load dashboard data:", e);
      // 已有数据时静默保留旧列表；首屏无数据时把错误升到 UI 让用户可感知 + 重试。
      // 用 errorMessage 而非 String(e)：桌面端 IPC 拒绝是 ApiError 对象，
      // String() 会显示 "[object Object]" 丢失原因。
      if (projectData === null) loadError = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  onMount(async () => {
    await loadData();
    registerHandler("dashboard-projects", (payload) => {
      // 三档触发条件（change `enrich-file-change-with-session-list-changed::D3`）：
      // dashboard 卡片显示 `sessionCount` per project，新 session 出现（unknown_session
      // 命中 → `sessionListChanged=true`）或删除 session（`deleted=true`）都会
      // 改变这个数字；纯 JSONL append（三个标志全 false）不动 → 不重拉。旧
      // 后端缺 `sessionListChanged` 字段时退化为只看 `projectListChanged ||
      // deleted`，与历史行为对齐。
      if (
        !payload.projectListChanged &&
        !payload.sessionListChanged &&
        !payload.deleted
      ) {
        return;
      }
      scheduleRefresh("dashboard:projects", () => untrack(() => loadData(true)));
    });
    // `/` 聚焦搜索：迁出 svelte:window onkeydown，统一进 keyboard registry。
    // dispatcher 内置 input 焦点守卫（meta.allowInInput=false 默认值）已等价覆盖
    // "input/textarea/contenteditable focus 时让浏览器原生处理"；handler 仅做
    // 实际的 focus + select。`searchEl` 缺失 → return false 让 dispatcher 不
    // preventDefault（首屏极短窗口尚未 bind 时 fallthrough）。
    const meta = getShortcutMeta("search.focus");
    if (meta) {
      unregisterFocusShortcut = registerShortcut({
        ...meta,
        handler: () => {
          if (!searchEl) return false;
          searchEl.focus();
          searchEl.select();
        },
      });
    }
  });

  onDestroy(() => {
    unregisterFocusShortcut?.();
    unregisterFocusShortcut = null;
    unregisterHandler("dashboard-projects");
    cancelScheduledRefresh("dashboard:projects");
    if (pulseTimer) clearTimeout(pulseTimer);
  });

  $effect(() => {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(VIEW_STORAGE_KEY, viewMode);
    }
  });
  $effect(() => {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(SORT_STORAGE_KEY, sortKey);
    }
  });

  // Sidebar / Settings 路径触发的 store 刷新（如 SettingsView 改 claudeRootPath
  // → cdt-refresh-projects → Sidebar.loadProjects → 写 store）需要在 Dashboard
  // 这边同步本地副本，否则 Dashboard 会卡在 mount 时的旧值。
  // 与 App.svelte 同款订阅模式（codex CR 反馈）。
  $effect(() => {
    const cached = getProjectData();
    if (cached) projectData = cached;
  });

  const derivedProjects = $derived(deriveDashboardProjects(projectData));
  const sorted = $derived(sortDashboardProjects(derivedProjects, sortKey));
  const visible = $derived(filterDashboardProjects(sorted, filterQuery));

  const sortLabel = $derived(
    sortKey === "recent" ? "最近活动" : sortKey === "sessions" ? "会话数最多" : "字母序",
  );
</script>

<div class="dashboard">
  <div class="dashboard-inner">
    <!-- 搜索框 —— 配齐 type=search + auto* + cancel-button 隐藏，避免 WKWebView
         弹「A ×」自动大写浮窗 / 浏览器历史下拉。详见 ui/CLAUDE.md「UI 组件规范」。 -->
    <div class="dash-search-wrap">
      <input
        bind:this={searchEl}
        class="dash-search"
        type="search"
        placeholder="搜索项目..."
        bind:value={filterQuery}
        autocomplete="off"
        autocorrect="off"
        autocapitalize="off"
        spellcheck="false"
        enterkeyhint="search"
        aria-label="搜索项目"
      />
      <kbd class="dash-kbd" title="按 / 聚焦搜索">/</kbd>
    </div>

    <!-- ⌘K 独立提示 -->
    <button
      class="dash-cmdk-hint"
      onclick={() => {
        window.dispatchEvent(new CustomEvent("cdt-open-command-palette"));
      }}
    >
      <kbd>⌘K</kbd>
      <span>跨项目搜索会话 / 工具 / 文件</span>
    </button>

    <!-- 工具栏：标题 + 排序 + 视图切换 -->
    <div class="dash-toolbar">
      <div class="dash-toolbar-title">
        {#if filterQuery}
          搜索结果 · {visible.length} / {sorted.length}
        {:else}
          {sorted.length} 个项目 · 按{sortLabel}排序
        {/if}
      </div>

      <div class="dash-toolbar-controls">
        <div class="dash-sort">
          <span class="dash-sort-label">排序</span>
          <Dropdown
            size="sm"
            minWidth={96}
            value={sortKey}
            options={SORT_OPTIONS}
            onChange={(v) => (sortKey = v as DashboardSortKey)}
            ariaLabel="排序方式"
          />
        </div>

        <div class="dash-view-toggle" role="group" aria-label="视图切换">
          <button
            class="dash-view-btn"
            class:dash-view-btn-active={viewMode === "list"}
            onclick={() => (viewMode = "list")}
            aria-pressed={viewMode === "list"}
            title="列表视图"
          >
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <line x1="8" y1="6" x2="21" y2="6" />
              <line x1="8" y1="12" x2="21" y2="12" />
              <line x1="8" y1="18" x2="21" y2="18" />
              <line x1="3" y1="6" x2="3.01" y2="6" />
              <line x1="3" y1="12" x2="3.01" y2="12" />
              <line x1="3" y1="18" x2="3.01" y2="18" />
            </svg>
          </button>
          <button
            class="dash-view-btn"
            class:dash-view-btn-active={viewMode === "grid"}
            onclick={() => (viewMode = "grid")}
            aria-pressed={viewMode === "grid"}
            title="网格视图"
          >
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <rect x="3" y="3" width="7" height="7" />
              <rect x="14" y="3" width="7" height="7" />
              <rect x="3" y="14" width="7" height="7" />
              <rect x="14" y="14" width="7" height="7" />
            </svg>
          </button>
        </div>
      </div>
    </div>

    <!-- 内容 -->
    {#if loading && derivedProjects.length === 0}
      {#if viewMode === "grid"}
        <div class="dash-grid" role="status" aria-busy="true" aria-label="正在加载项目">
          {#each Array.from({ length: 6 }) as _, i (i)}
            <Skeleton variant="card" height={108} />
          {/each}
        </div>
      {:else}
        <div class="dash-list-skeleton" role="status" aria-busy="true" aria-label="正在加载项目">
          {#each Array.from({ length: 8 }) as _, i (i)}
            <Skeleton variant="card" height={52} />
          {/each}
        </div>
      {/if}
    {:else if loadError && derivedProjects.length === 0}
      <div class="dash-status dash-error" role="alert">
        <div class="dash-error-title">加载项目失败</div>
        <div class="dash-error-detail">{loadError}</div>
        <button class="dash-retry" onclick={() => loadData()}>重试</button>
      </div>
    {:else if visible.length === 0}
      <div class="dash-status">
        {filterQuery ? "无匹配项目" : "未发现项目"}
      </div>
    {:else if viewMode === "list"}
      <ul class="dash-list" role="list">
        {#each visible as project (project.id)}
          {@const isActive = project.id === selectedProjectId}
          {@const isPulsing = project.id === pulsingId}
          <li>
            <button
              class="dash-row"
              class:dash-row-active={isActive}
              class:dash-row-pulse={isPulsing}
              onclick={() => handleSelect(project)}
              use:contextMenu={projectMenuProvider(project)}
            >
              <!-- 双 sub-flex：left(name + 当前 badge) | right(time + worktree + 💬N)
                   用 justify-content: space-between 把右侧 metadata 永久推到行尾，
                   不依赖 dash-row-time 的 margin-left:auto——避免 lastModified=null
                   fallback 时右侧紧贴左侧（codex 二审发现）。
                   active 切换时仅左 group 内 badge 显隐，右侧位置稳定。 -->
              <div class="dash-row-main">
                <div class="dash-row-main-left">
                  <span class="dash-row-name">{project.displayName}</span>
                  {#if isActive}
                    <span class="dash-row-current">当前</span>
                  {/if}
                </div>
                <div class="dash-row-main-right">
                  {#if project.lastModified !== null}
                    <span class="dash-row-time">{formatRelativeTime(project.lastModified)}</span>
                  {/if}
                  {#if project.worktreeCount > 1}
                    <span class="dash-row-chip" title="{project.worktreeCount} 个 worktree">
                      <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html GIT_BRANCH_SVG}</svg>
                      {project.worktreeCount}
                    </span>
                  {/if}
                  <span class="dash-row-sessions" title="{project.sessionCount} 个会话">
                    💬 {project.sessionCount}
                  </span>
                </div>
              </div>
              <div class="dash-row-path">{shortenPath(project.path)}</div>
            </button>
          </li>
        {/each}
      </ul>
    {:else}
      <div class="dash-grid">
        {#each visible as project (project.id)}
          {@const isActive = project.id === selectedProjectId}
          {@const isPulsing = project.id === pulsingId}
          <button
            class="dash-card"
            class:dash-card-active={isActive}
            class:dash-card-pulse={isPulsing}
            onclick={() => handleSelect(project)}
            use:contextMenu={projectMenuProvider(project)}
          >
            <div class="dash-card-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                {@html FOLDER_GIT2_SVG}
              </svg>
            </div>
            <div class="dash-card-name">{project.displayName}</div>
            <div class="dash-card-path">{shortenPath(project.path)}</div>
            <div class="dash-card-meta">
              {#if project.lastModified !== null}
                <span class="dash-card-time">{formatRelativeTime(project.lastModified)}</span>
              {/if}
              {#if project.worktreeCount > 1}
                <span class="dash-card-chip" title="{project.worktreeCount} 个 worktree">
                  <svg viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html GIT_BRANCH_SVG}</svg>
                  {project.worktreeCount}
                </span>
              {/if}
              <span class="dash-card-sessions">💬 {project.sessionCount}</span>
              {#if isActive}
                <span class="dash-card-current">当前</span>
              {/if}
            </div>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .dashboard {
    display: flex;
    justify-content: center;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    /* scrollbar-gutter-exempt: 全屏面板，滚动条首帧即确定，无动态跳变 */
    /* 顶部 padding 28px：原 48px 是为对抗 PaneView 空 TabBar (40px) 留出的呼吸；
       现 sole pane + 无 tab 已不再渲染 TabBar，搜索框直接贴 UnifiedTitleBar 下方
       成为视觉首焦——对齐 VS Code Welcome / Linear 的 "主输入贴顶" 工作台语言。 */
    padding: 28px 24px 48px;
  }

  .dashboard-inner {
    width: 100%;
    max-width: 1200px;
    min-width: 0;
  }

  /* ---------- 搜索 ---------- */

  .dash-search-wrap {
    position: relative;
    margin-bottom: 8px;
  }

  .dash-search {
    width: 100%;
    font-size: 15px;
    font-family: inherit;
    color: var(--color-text);
    background: var(--color-surface);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 10px;
    padding: 12px 16px;
    padding-right: 56px;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .dash-search:focus {
    border-color: var(--color-accent-blue);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--color-accent-blue) 15%, transparent);
  }

  .dash-search::placeholder {
    color: var(--color-text-muted);
  }

  /* 隐藏 type=search 在 WebKit 下的原生 clear 按钮，避免与 / kbd 视觉冲突 */
  .dash-search::-webkit-search-cancel-button,
  .dash-search::-webkit-search-decoration {
    -webkit-appearance: none;
    appearance: none;
    display: none;
  }

  .dash-kbd {
    position: absolute;
    right: 12px;
    top: 50%;
    transform: translateY(-50%);
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
    background: var(--badge-neutral-bg);
    padding: 2px 8px;
    border-radius: 4px;
    pointer-events: none;
  }

  /* ---------- ⌘K 提示 ---------- */

  .dash-cmdk-hint {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 24px;
    padding: 4px 8px;
    background: none;
    border: none;
    border-radius: 4px;
    font: inherit;
    font-size: 12px;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: color 0.12s, background 0.12s;
  }

  .dash-cmdk-hint:hover {
    color: var(--color-text-secondary);
    background: var(--tool-item-hover-bg);
  }

  .dash-cmdk-hint kbd {
    font-family: var(--font-mono);
    font-size: 11px;
    background: var(--badge-neutral-bg);
    padding: 1px 6px;
    border-radius: 4px;
    color: var(--color-text-secondary);
  }

  /* ---------- 工具栏 ---------- */

  .dash-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
    flex-wrap: wrap;
  }

  .dash-toolbar-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text-secondary);
  }

  .dash-toolbar-controls {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .dash-sort {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .dash-sort-label {
    line-height: 1;
  }

  .dash-view-toggle {
    display: inline-flex;
    border: 1px solid var(--color-border);
    border-radius: 6px;
    overflow: hidden;
  }

  .dash-view-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 4px 8px;
    background: var(--color-surface);
    border: none;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }

  .dash-view-btn + .dash-view-btn {
    border-left: 1px solid var(--color-border);
  }

  .dash-view-btn:hover {
    color: var(--color-text-secondary);
    background: var(--tool-item-hover-bg);
  }

  .dash-view-btn-active {
    background: var(--color-surface-raised);
    color: var(--color-text);
  }

  /* ---------- list 视图 ---------- */

  .dash-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    overflow: hidden;
    background: var(--color-surface);
  }

  .dash-list-skeleton {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .dash-list > li + li {
    border-top: 1px solid var(--color-border-subtle, var(--color-border));
  }

  .dash-row {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 14px;
    background: transparent;
    border: none;
    text-align: left;
    font: inherit;
    color: var(--color-text);
    cursor: pointer;
    transition: background 0.12s;
    min-width: 0;
    position: relative;
  }

  .dash-row:hover {
    background: var(--tool-item-hover-bg);
  }

  .dash-row-main {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
  }

  .dash-row-main-left {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    flex: 1 1 auto;
  }

  .dash-row-main-right {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .dash-row-name {
    font-size: 14px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex-shrink: 1;
    min-width: 0;
  }

  .dash-row-time {
    font-size: 12px;
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .dash-row-chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    color: var(--color-text-secondary);
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--color-surface-overlay);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .dash-row-chip svg {
    flex-shrink: 0;
  }

  .dash-row-sessions {
    font-size: 12px;
    color: var(--color-text-secondary);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .dash-row-current {
    font-size: 10px;
    font-weight: 500;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--badge-neutral-bg);
    color: var(--color-text-secondary);
    letter-spacing: 0.02em;
    flex-shrink: 0;
  }

  .dash-row-path {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* 选中态：raised bg + inset emphasis ring（无 side-stripe）。 */
  .dash-row-active {
    background: var(--color-surface-raised);
    box-shadow: inset 0 0 0 1.5px var(--color-border-emphasis);
  }

  .dash-row-active:hover {
    background: var(--color-surface-raised);
  }

  .dash-row-pulse {
    animation: dash-pulse 0.45s ease-out;
  }

  /* ---------- grid 视图 ---------- */

  .dash-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 12px;
    width: 100%;
  }

  .dash-card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 16px;
    background: var(--color-surface);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    cursor: pointer;
    text-align: left;
    font: inherit;
    color: var(--color-text);
    transition: border-color 0.15s, background 0.15s;
    min-width: 0;
  }

  .dash-card:hover {
    border-color: var(--color-border-emphasis);
    background: var(--color-surface-raised);
  }

  /* 选中态：raised + inset ring，去掉原来的 ::before 3px 装饰条。 */
  .dash-card-active {
    background: var(--color-surface-raised);
    box-shadow: inset 0 0 0 1.5px var(--color-border-emphasis);
    border-color: var(--color-border-emphasis);
  }

  .dash-card-pulse {
    animation: dash-pulse 0.45s ease-out;
  }

  .dash-card-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 6px;
    background: var(--color-surface-overlay);
    border: 1px solid var(--color-border);
    color: var(--color-text-secondary);
    margin-bottom: 8px;
  }

  .dash-card-icon svg {
    width: 16px;
    height: 16px;
  }

  .dash-card-name {
    font-size: 14px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dash-card-path {
    font-size: 11px;
    font-family: var(--font-mono);
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dash-card-meta {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    font-size: 12px;
    color: var(--color-text-secondary);
    margin-top: 6px;
  }

  .dash-card-time {
    font-variant-numeric: tabular-nums;
    color: var(--color-text-muted);
  }

  .dash-card-chip {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--color-surface-overlay);
    font-variant-numeric: tabular-nums;
  }

  .dash-card-sessions {
    font-variant-numeric: tabular-nums;
  }

  .dash-card-current {
    margin-left: auto;
    font-size: 10px;
    font-weight: 500;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--badge-neutral-bg);
    color: var(--color-text-secondary);
    letter-spacing: 0.02em;
  }

  /* ---------- 共享 ---------- */

  .dash-status {
    text-align: center;
    padding: 48px 0;
    color: var(--color-text-muted);
    font-size: 14px;
  }

  .dash-error {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }

  .dash-error-title {
    color: var(--color-danger);
    font-weight: 600;
  }

  .dash-error-detail {
    font-size: 12px;
    color: var(--color-text-muted);
    max-width: 480px;
    word-break: break-word;
  }

  .dash-retry {
    margin-top: 4px;
    padding: 6px 16px;
    font: inherit;
    font-size: 13px;
    color: var(--color-text);
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border-emphasis);
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.12s;
  }

  .dash-retry:hover {
    background: var(--tool-item-hover-bg);
  }

  @keyframes dash-pulse {
    0% {
      box-shadow: 0 0 0 0 color-mix(in oklch, var(--color-accent-blue) 45%, transparent);
    }
    100% {
      box-shadow: 0 0 0 10px color-mix(in oklch, var(--color-accent-blue) 0%, transparent);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .dash-row-pulse,
    .dash-card-pulse {
      animation: none;
    }
  }
</style>
